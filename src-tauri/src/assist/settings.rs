//! Word Assist runtime state and settings resolution (1b).
//!
//! This module owns the pieces the Tauri command layer and the loopback
//! server share: the resolved engine settings, the pairing/token state,
//! the rate limiter, the sentence cache and the assist audit log.
//!
//! ASSIST AUDIT LOG: a separate in-memory log in the same shape as the
//! app's `audit::AuditLog`. The listener writes here, the Word Assist tab
//! reads here; the main Privacy Audit page keeps showing the app's own
//! log. Keeping them separate avoids touching the existing audit surface
//! while the feature is gated; a Phase 4 decision can merge them.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::cache::SentenceCache;
use super::mask::MaskMode;
use super::validate::StrictnessProfile;
use crate::audit::AuditLog;
use crate::language_check::RuleMap;

/// Resolved, validated engine settings for one session. Built from the
/// persisted `Settings` at every enable; never mutated in place (a
/// settings change goes through disable/enable so the listener restarts
/// with coherent values).
#[derive(Debug, Clone)]
pub struct AssistSettings {
    pub port: u16,
    pub dialect: String,
    pub strictness: StrictnessProfile,
    /// S1 (Filter) or S2 (Mask) protected-span strategy. Shipped: S1.
    pub mask_mode: MaskMode,
    pub tier1_enabled: bool,
    /// Empty string means "the app's last-used chat model".
    pub tier1_model: String,
    pub experimental_languages: HashMap<String, bool>,
    /// The user's per-rule overrides (language_check_rules), applied on
    /// top of the strictness profile in every check (D-4: single source
    /// of truth).
    pub rules: RuleMap,
}

impl AssistSettings {
    pub fn from_persisted(s: &crate::persistence::Settings) -> Result<Self, String> {
        let strictness = StrictnessProfile::parse(&s.word_assist_strictness).ok_or_else(|| {
            format!(
                "unknown strictness profile '{}'. Supported: dev, conservative, balanced, thorough",
                s.word_assist_strictness
            )
        })?;
        if s.word_assist_port == 0 {
            return Err("port 0 is not allowed for the Word Assist listener".into());
        }
        Ok(Self {
            port: s.word_assist_port,
            dialect: s.language_check_dialect.clone(),
            strictness,
            mask_mode: MaskMode::Filter, // D-4: S1 shipped, S2 kept for the Phase 4 arbitration
            tier1_enabled: s.word_assist_tier1_enabled,
            tier1_model: s.word_assist_tier1_model.clone(),
            experimental_languages: s
                .word_assist_experimental_languages
                .iter()
                .filter(|(_, on)| **on)
                .map(|(k, _)| (k.clone(), true))
                .collect(),
            rules: s.language_check_rules.clone(),
        })
    }

    pub fn experimental(&self, script: &str) -> bool {
        self.experimental_languages
            .get(script)
            .copied()
            .unwrap_or(false)
    }
}

/// Everything the server task and the command layer share. Lives in
/// Tauri-managed state; the server task holds an `Arc` to the same value.
pub struct AssistState {
    pub settings: Mutex<Option<AssistSettings>>,
    pub running: AtomicBool,
    /// Port the listener actually bound (reported to the UI even when the
    /// settings port was taken — no silent move: bind failure IS failure,
    /// this only reports the bound value).
    pub bound_port: Mutex<Option<u16>>,
    /// One-time bearer token the add-in obtains through /v1/pair.
    pub token: Mutex<Option<String>>,
    /// Current 6-character pairing code + expiry (5 minutes).
    pub pairing: Mutex<Option<PairingCode>>,
    /// The sentence cache (Tier 0 + Tier 1 verdicts, hash-keyed).
    pub cache: SentenceCache,
    /// Single-flight Tier 1 scheduler.
    pub scheduler: super::scheduler::Scheduler,
    /// Sliding-window request budget for the listener.
    pub rate: RateLimiter,
    /// Assist-surface audit log (see module docs).
    pub audit: AuditLog,
    /// Hash-chained ledger (Phase 3; E-4: no MAC, no local key), attached
    /// when the feature is enabled (the data dir is only known at the
    /// command layer).
    pub ledger: Mutex<Option<super::ledger::AssistLedger>>,
    /// Shared Ollama client for Tier 1.
    pub ollama: std::sync::Arc<crate::ollama::OllamaState>,
}

#[derive(Debug, Clone)]
pub struct PairingCode {
    pub code: String,
    pub expires_at: Instant,
}

impl PairingCode {
    pub fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

/// Sliding-window request budget for the listener (shared with server.rs).
pub const RATE_MAX: usize = 120;
pub const RATE_WINDOW: Duration = Duration::from_secs(60);

impl AssistState {
    pub fn new(ollama: std::sync::Arc<crate::ollama::OllamaState>) -> Self {
        Self {
            settings: Mutex::new(None),
            running: AtomicBool::new(false),
            bound_port: Mutex::new(None),
            token: Mutex::new(None),
            pairing: Mutex::new(None),
            cache: SentenceCache::new(super::cache::DEFAULT_SENTENCE_CACHE_CAPACITY),
            scheduler: super::scheduler::Scheduler::new(),
            rate: RateLimiter::new(RATE_MAX, RATE_WINDOW),
            audit: AuditLog::default(),
            ledger: Mutex::new(None),
            ollama,
        }
    }

    /// Attach the Phase 3 ledger (enable path). A ledger that fails to
    /// open (broken chain) disables the feature: the
    /// record is a commitment, and half-keeping it is worse than not
    /// keeping it.
    pub fn attach_ledger(&self, dir: &std::path::Path) -> Result<(), String> {
        let ledger = super::ledger::AssistLedger::open(dir)?;
        *self.ledger.lock().map_err(|e| e.to_string())? = Some(ledger);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // mirrors AssistLedger::append
    pub fn ledger_record(
        &self,
        kind: &str,
        paragraph_id: &str,
        issue_id: &str,
        rule_id: &str,
        category: &str,
        applied_start: usize,
        applied_end: usize,
        replacement_len: usize,
        paragraph_text_hash: &str,
    ) {
        if let Ok(mut guard) = self.ledger.lock() {
            if let Some(ledger) = guard.as_mut() {
                let result = ledger.append(
                    kind,
                    paragraph_id,
                    issue_id,
                    rule_id,
                    category,
                    applied_start,
                    applied_end,
                    replacement_len,
                    paragraph_text_hash,
                );
                if let Err(e) = result {
                    log::warn!("assist ledger append failed: {e}");
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Generate a fresh pairing code (6 characters from an unambiguous
    /// alphabet — no 0/O/1/I/L). Returns the code for the UI to show.
    pub fn new_pairing_code(&self) -> Result<String, String> {
        let code = super::scheduler::rand_compat::random_pairing_code()?;
        *self
            .pairing
            .lock()
            .map_err(|e| format!("assist state lock: {e}"))? = Some(PairingCode {
            code: code.clone(),
            expires_at: Instant::now() + PAIRING_CODE_TTL,
        });
        Ok(code)
    }

    /// Constant-time-ish pairing code check (equal length + fold). The
    /// code is a 6-character human-mediated secret; this keeps timing
    /// leaks out of the picture without pulling a crypto dependency.
    pub fn take_pairing_code(&self, presented: &str) -> Result<bool, String> {
        let mut guard = self
            .pairing
            .lock()
            .map_err(|e| format!("assist state lock: {e}"))?;
        let ok = match guard.as_ref() {
            Some(p) if p.is_valid() => constant_time_eq(p.code.as_bytes(), presented.as_bytes()),
            _ => false,
        };
        if ok {
            // One-time: burn the code so a replayed request cannot pair.
            *guard = None;
        }
        Ok(ok)
    }

    pub fn set_token(&self, token: String) -> Result<(), String> {
        *self
            .token
            .lock()
            .map_err(|e| format!("assist state lock: {e}"))? = Some(token);
        Ok(())
    }

    pub fn token_matches(&self, presented: &str) -> bool {
        match self.token.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(expected) => constant_time_eq(expected.as_bytes(), presented.as_bytes()),
                None => false,
            },
            Err(_) => false,
        }
    }

    pub fn clear_session(&self) -> Result<(), String> {
        *self.token.lock().map_err(|e| e.to_string())? = None;
        *self.pairing.lock().map_err(|e| e.to_string())? = None;
        Ok(())
    }
}

pub const PAIRING_CODE_TTL: Duration = Duration::from_secs(300);

/// Fixed-time equality over byte slices (no early return on mismatch).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false; // length is not a secret (6 chars, fixed alphabet)
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Sliding-window rate limiter: `max` requests per `window` per key.
/// Keys are loopback IPs in practice; the map is bounded by eviction.
pub struct RateLimiter {
    window: Duration,
    max: usize,
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new(max: usize, window: Duration) -> Self {
        Self {
            window,
            max,
            hits: Mutex::new(HashMap::new()),
        }
    }

    /// True = allowed (and recorded), false = over budget.
    pub fn allow(&self, key: &str) -> bool {
        let mut hits = self.hits.lock().expect("rate limiter lock");
        let now = Instant::now();
        let entry = hits.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < self.window);
        if entry.len() >= self.max {
            return false;
        }
        entry.push(now);
        // Bound the map: a loopback listener has at most a handful of keys.
        if hits.len() > 64 {
            hits.clear();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{constant_time_eq, AssistState, RateLimiter};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn constant_time_eq_basics() {
        assert!(constant_time_eq(b"abcdef", b"abcdef"));
        assert!(!constant_time_eq(b"abcdef", b"abcdeg"));
        assert!(!constant_time_eq(b"abc", b"abcdef"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn rate_limiter_windows() {
        let rl = RateLimiter::new(3, Duration::from_millis(50));
        assert!(rl.allow("k"));
        assert!(rl.allow("k"));
        assert!(rl.allow("k"));
        assert!(!rl.allow("k"));
        assert!(rl.allow("other"));
        std::thread::sleep(Duration::from_millis(60));
        assert!(rl.allow("k"));
    }

    #[test]
    fn pairing_code_lifecycle() {
        let state = AssistState::new(Arc::new(crate::ollama::OllamaState::default()));
        let code = state.new_pairing_code().expect("code");
        assert_eq!(code.len(), 6);
        // Wrong code fails.
        assert!(!state.take_pairing_code("zzzzzz").expect("check"));
        // Right code works once.
        assert!(state.take_pairing_code(&code).expect("check"));
        // And is burned after use.
        assert!(!state.take_pairing_code(&code).expect("check"));
    }

    #[test]
    fn settings_from_persisted_validates() {
        let mut s = crate::persistence::Settings::default();
        let resolved = super::AssistSettings::from_persisted(&s).expect("default settings");
        assert_eq!(resolved.port, 38080);
        assert_eq!(resolved.strictness.name(), "dev"); // E-1 provisional default
        s.word_assist_strictness = "nonsense".into();
        assert!(super::AssistSettings::from_persisted(&s).is_err());
        s.word_assist_strictness = "balanced".into();
        s.word_assist_port = 0;
        assert!(super::AssistSettings::from_persisted(&s).is_err());
    }
}
