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
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
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
    /// Paired-client token, HASH ONLY in memory (H2). The plaintext token
    /// exists at issue time and in the add-in's localStorage; the engine
    /// persists only SHA-256(token) + metadata (H2, owner ruling).
    pub token: Mutex<Option<StoredToken>>,
    /// Where the token record is persisted. Attached at enable time
    /// (the data dir is only known at the command layer).
    pub token_store: Mutex<Option<TokenStore>>,
    /// One-time pairing code state with failure budget, burn and
    /// cooldown (H1, D-3 as ruled).
    pub pairing: PairingState,
    /// Route-specific rate limiter for /v1/pair (H1: 120/60 s is far too
    /// loose for a pairing endpoint).
    pub pair_rate: RateLimiter,
    /// Injectable clock (H1: the pairing expiry path must be testable).
    /// Production uses RealClock; tests swap in a FakeClock or shorten
    /// the pairing TTL via `PairingState::with_ttl`.
    pub clock: Arc<dyn Clock>,
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
    /// TEST SEAM: when set (tests only), Tier 1 calls go to this base URL
    /// instead of the real Ollama daemon, so the full check→call→cache
    /// path is exercised end-to-end against a mock daemon. Production
    /// leaves this None and `crate::ollama::base_url()` is used.
    pub tier1_base_url_override: Mutex<Option<String>>,
}

/// Injectable clock for anything that must be testable (H1).
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

/// The production clock.
pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Deterministic clock for tests: starts at the real construction instant
/// and only moves when `advance` is called.
pub struct FakeClock {
    base: Instant,
    millis: AtomicI64,
}

impl FakeClock {
    pub fn new() -> Self {
        Self {
            base: Instant::now(),
            millis: AtomicI64::new(0),
        }
    }

    pub fn advance(&self, d: Duration) {
        self.millis
            .fetch_add(d.as_millis() as i64, Ordering::SeqCst);
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        let offset = self.millis.load(Ordering::SeqCst).max(0) as u64;
        self.base + Duration::from_millis(offset)
    }
}

// --------------------------------------------------------------- pairing

/// Pairing code TTL (D-3: five minutes).
pub const PAIRING_CODE_TTL: Duration = Duration::from_secs(300);
/// H1: the pairing code dies after 5 wrong attempts GLOBALLY (everything
/// arrives from loopback, so per-IP budgets are meaningless). The 5th
/// failure burns the code.
pub const PAIRING_MAX_FAILURES: u32 = 5;
/// H1: issuing a NEW code after a burn (lockout) is gated by a cooldown
/// that starts at 30 s and doubles, capped here.
pub const PAIRING_COOLDOWN_INITIAL: Duration = Duration::from_secs(30);
pub const PAIRING_COOLDOWN_CAP: Duration = Duration::from_secs(1800);
/// H1: route-specific /v1/pair budget. The pairing endpoint accepts ONE
/// code with a 5-attempt life; 10 requests per minute is already
/// generous for a human typing a code, and it blunts automated floods
/// even when no code is live.
pub const PAIR_ROUTE_MAX: usize = 10;
pub const PAIR_ROUTE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairOutcome {
    /// Code matched: it is now burned (one-time use).
    Accepted,
    /// Wrong code; the budget is not yet exhausted.
    Failure { attempts_used: u32 },
    /// The code existed but its TTL elapsed. It is cleared on check.
    Expired,
    /// A code was burned by the 5th failure and no new one was issued.
    LockedOut,
    /// No pairing code was ever issued (or it was consumed by a success).
    NoCode,
}

#[derive(Debug, Clone)]
struct CodeEntry {
    code: String,
    expires_at: Instant,
}

/// Pairing state machine (H1): one live code, a global 5-failure budget,
/// burn-on-success / burn-on-5th-failure, and a doubling cooldown that
/// gates the NEXT code after a burn. The clock is injectable so the
/// expiry and cooldown paths are unit-testable.
pub struct PairingState {
    inner: Mutex<PairingInner>,
    clock: Arc<dyn Clock>,
    ttl: Duration,
}

#[derive(Debug, Default)]
struct PairingInner {
    code: Option<CodeEntry>,
    /// Wrong attempts against the CURRENT code. Reset when a new code is
    /// issued.
    failures: u32,
    /// Set when a code is burned by the 5th failure; gates new codes.
    cooldown_until: Option<Instant>,
    /// How many burns have happened this session (drives the doubling).
    cooldown_step: u32,
}

impl PairingState {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self::with_ttl(clock, PAIRING_CODE_TTL)
    }

    pub fn with_ttl(clock: Arc<dyn Clock>, ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(PairingInner::default()),
            clock,
            ttl,
        }
    }

    /// Issue a fresh code. This is the "explicit user action" path (the
    /// Word Assist tab button, via the Tauri command); after a burn it is
    /// gated by the doubling cooldown.
    pub fn issue_new(&self) -> Result<String, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("pairing lock: {e}"))?;
        if let Some(until) = guard.cooldown_until {
            let now = self.clock.now();
            if now < until {
                let wait = until.saturating_duration_since(now).as_secs() + 1;
                return Err(format!(
                    "pairing is locked for {wait} s after a failed-code burn; wait and try again"
                ));
            }
        }
        let code = super::scheduler::rand_compat::random_pairing_code()?;
        guard.code = Some(CodeEntry {
            code: code.clone(),
            expires_at: self.clock.now() + self.ttl,
        });
        guard.failures = 0;
        Ok(code)
    }

    /// Check a presented code. Constant-time on the fixed-length code
    /// value; success burns; the 5th failure burns and starts a cooldown.
    pub fn take(&self, presented: &str) -> Result<PairOutcome, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("pairing lock: {e}"))?;
        let Some(entry) = guard.code.as_ref() else {
            return Ok(if guard.cooldown_until.is_some() {
                PairOutcome::LockedOut
            } else {
                PairOutcome::NoCode
            });
        };
        if self.clock.now() >= entry.expires_at {
            guard.code = None;
            return Ok(PairOutcome::Expired);
        }
        let ok = constant_time_eq(entry.code.as_bytes(), presented.as_bytes());
        if ok {
            // One-time: burn the code so a replayed request cannot pair.
            guard.code = None;
            return Ok(PairOutcome::Accepted);
        }
        guard.failures = guard.failures.saturating_add(1);
        if guard.failures >= PAIRING_MAX_FAILURES {
            guard.code = None;
            // Cooldown starts at 30 s and doubles per burn, capped.
            guard.cooldown_step = guard.cooldown_step.saturating_add(1);
            let shift = guard.cooldown_step.saturating_sub(1).min(6);
            let secs = PAIRING_COOLDOWN_INITIAL
                .as_secs()
                .saturating_mul(1u64 << shift);
            let cooldown = Duration::from_secs(secs).min(PAIRING_COOLDOWN_CAP);
            guard.cooldown_until = Some(self.clock.now() + cooldown);
            return Ok(PairOutcome::LockedOut);
        }
        Ok(PairOutcome::Failure {
            attempts_used: guard.failures,
        })
    }

    /// A live (unexpired) code exists. Used by the status/pairing lights.
    pub fn has_live_code(&self) -> bool {
        match self.inner.lock() {
            Ok(guard) => match guard.code.as_ref() {
                Some(entry) => self.clock.now() < entry.expires_at,
                None => false,
            },
            Err(_) => false,
        }
    }

    /// Seconds left on the burn cooldown, if any (UI hint).
    pub fn cooldown_remaining_secs(&self) -> Option<u64> {
        match self.inner.lock() {
            Ok(guard) => guard
                .cooldown_until
                .map(|until| until.saturating_duration_since(self.clock.now()).as_secs()),
            Err(_) => None,
        }
    }

    /// Drop any live code (session wipe). Cooldown history survives: a
    /// burned code stays burned.
    pub fn clear_code(&self) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("pairing lock: {e}"))?;
        guard.code = None;
        guard.failures = 0;
        Ok(())
    }
}

// ------------------------------------------------------------------ token

/// Sliding expiry: a paired token dies 30 days after its last use (H2).
pub const TOKEN_SLIDING_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
/// last_used rewrites are throttled: a per-request disk write would turn
/// every check into an I/O event; one write a minute keeps the sliding
/// window honest to within a minute without wearing the store.
pub const TOKEN_LAST_USED_WRITE_THROTTLE: Duration = Duration::from_secs(60);
/// The label recorded for every issued token (the only client kind).
pub const TOKEN_LABEL: &str = "word-add-in";

/// What the engine persists for a paired client: the SHA-256 of the
/// token, never the token itself (H2). Plaintext lives only (a) at issue
/// time in the /v1/pair response and (b) in the add-in's localStorage,
/// which the owner ACCEPTED as a same-user trust boundary (documented in
/// docs/LEDGER_THREAT_MODEL.md §"Token and pairing (H1/H2)").
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredToken {
    /// hex(SHA-256(raw token)). The raw token is never stored.
    pub token_hash: String,
    /// Unix seconds.
    pub created_at: u64,
    /// Unix seconds; refreshed on use (sliding 30-day expiry).
    pub last_used: u64,
    /// Free-form label; "word-add-in" for the only client kind that can
    /// pair today.
    pub label: String,
}

impl StoredToken {
    /// Pure sliding-expiry check (unit-tested): true when the token is
    /// dead because `now - last_used` exceeds the TTL.
    pub fn is_expired(last_used_unix: u64, now_unix: u64, ttl: Duration) -> bool {
        let ttl_secs = ttl.as_secs();
        match now_unix.checked_sub(last_used_unix) {
            Some(age) => age > ttl_secs,
            // Clock moved backwards: treat as fresh rather than expiring.
            None => false,
        }
    }

    pub fn hash_of(raw_token: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(raw_token.as_bytes());
        let d = h.finalize();
        d.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Where a token record is persisted. Keychain preferred (OS credential
/// manager); file fallback in the app data dir with 0600 on unix, mirroring
/// the TLS key policy. On Windows there is NO file fallback (see
/// `server::ensure_certificate_chain` for the same ruling on the TLS key):
/// Windows Credential Manager is always present, and the code refuses
/// rather than writing an unhardened file.
pub struct TokenStore {
    file_path: Option<std::path::PathBuf>,
}

const KEYCHAIN_SERVICE: &str = "scholarscribe";
const KEYCHAIN_TOKEN_ENTRY: &str = "word-assist-client-token-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreLocation {
    Keychain,
    File,
}

impl TokenStore {
    pub fn for_dir(dir: &std::path::Path) -> Self {
        Self {
            file_path: Some(dir.join("word-assist-token.json")),
        }
    }

    /// Read the persisted record. Keychain first, then file. Returns
    /// (record, where it came from).
    pub fn load(&self) -> Result<Option<(StoredToken, StoreLocation)>, String> {
        if let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_TOKEN_ENTRY) {
            match entry.get_password() {
                Ok(json) => {
                    return serde_json::from_str::<StoredToken>(&json)
                        .map(|t| Some((t, StoreLocation::Keychain)))
                        .map_err(|e| format!("token record parse (keychain): {e}"));
                }
                Err(keyring::Error::NoEntry) => {}
                Err(e) => {
                    // Keychain unavailable: fall through to the file.
                    log::warn!("token keychain read failed ({e}); trying file fallback");
                }
            }
        }
        let Some(path) = &self.file_path else {
            return Ok(None);
        };
        let Ok(json) = std::fs::read_to_string(path) else {
            return Ok(None);
        };
        serde_json::from_str::<StoredToken>(&json)
            .map(|t| Some((t, StoreLocation::File)))
            .map_err(|e| format!("token record parse (file): {e}"))
    }

    /// Write the record. Keychain first; file fallback (0600 on unix).
    /// Returns where it landed so the caller can audit a fallback.
    pub fn save(&self, token: &StoredToken) -> Result<StoreLocation, String> {
        let json = serde_json::to_string(token).map_err(|e| e.to_string())?;
        if let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_TOKEN_ENTRY) {
            match entry.set_password(&json) {
                Ok(()) => return Ok(StoreLocation::Keychain),
                Err(e) => {
                    log::warn!("token keychain write failed ({e}); using file fallback");
                }
            }
        }
        let Some(path) = &self.file_path else {
            return Err(
                "no token persistence backend available (keychain failed, no data dir)".into(),
            );
        };
        write_private(path, json.as_bytes())?;
        Ok(StoreLocation::File)
    }

    /// Remove the record from every backend (revoke). Idempotent.
    pub fn delete(&self) -> Result<(), String> {
        if let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_TOKEN_ENTRY) {
            // NoEntry is fine — revoking an absent token is a no-op.
            let _ = entry.delete_credential();
        }
        if let Some(path) = &self.file_path {
            if path.exists() {
                std::fs::remove_file(path).map_err(|e| format!("delete token file: {e}"))?;
            }
        }
        Ok(())
    }
}

/// 0600 file write shared by the TLS key fallback and the token store
/// fallback. On non-unix there is no mode bit: the caller decides whether
/// to refuse (the TLS key path refuses on Windows; the token store has no
/// file fallback on Windows at all).
pub(crate) fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut f = std::fs::File::create(path).map_err(|e| format!("create key file: {e}"))?;
        f.write_all(bytes)
            .map_err(|e| format!("write key file: {e}"))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("restrict key file: {e}"))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        let _ = bytes;
        Err("private-file fallback is refused on this platform (keychain required)".into())
    }
}

/// Sliding-window request budget for the listener (shared with server.rs).
pub const RATE_MAX: usize = 120;
pub const RATE_WINDOW: Duration = Duration::from_secs(60);

impl AssistState {
    pub fn new(ollama: std::sync::Arc<crate::ollama::OllamaState>) -> Self {
        let clock: Arc<dyn Clock> = Arc::new(RealClock);
        Self {
            settings: Mutex::new(None),
            running: AtomicBool::new(false),
            bound_port: Mutex::new(None),
            token: Mutex::new(None),
            token_store: Mutex::new(None),
            pairing: PairingState::new(Arc::clone(&clock)),
            pair_rate: RateLimiter::new(PAIR_ROUTE_MAX, PAIR_ROUTE_WINDOW),
            clock,
            cache: SentenceCache::new(super::cache::DEFAULT_SENTENCE_CACHE_CAPACITY),
            scheduler: super::scheduler::Scheduler::new(),
            rate: RateLimiter::new(RATE_MAX, RATE_WINDOW),
            audit: AuditLog::default(),
            ledger: Mutex::new(None),
            ollama,
            tier1_base_url_override: Mutex::new(None),
        }
    }

    /// Attach the token persistence layer (enable path). Immediately
    /// restores a persisted record so a paired add-in keeps working after
    /// an app restart (H2). A fallback-file store is audited.
    pub fn attach_token_store(&self, dir: &std::path::Path) -> Result<(), String> {
        let store = TokenStore::for_dir(dir);
        *self.token_store.lock().map_err(|e| e.to_string())? = Some(store);
        self.restore_token()
    }

    /// Read the persisted token record back into memory (restart path).
    /// An expired record is revoked instead of restored.
    fn restore_token(&self) -> Result<(), String> {
        let guard = self.token_store.lock().map_err(|e| e.to_string())?;
        let Some(store) = guard.as_ref() else {
            return Ok(());
        };
        let loaded = store.load()?;
        let Some((record, location)) = loaded else {
            return Ok(());
        };
        if location == StoreLocation::File {
            self.audit.record(
                "assist_token_store_fallback",
                "localhost",
                "token record loaded from the 0600 file fallback (keychain unavailable)",
                0,
                0,
            );
        }
        let now = unix_now();
        if StoredToken::is_expired(record.last_used, now, TOKEN_SLIDING_TTL) {
            // Sliding expiry elapsed while the app was closed: revoke.
            let _ = store.delete();
            self.audit.record(
                "assist_token_expired",
                "localhost",
                "stored token expired (30 days unused); revoked on restore",
                0,
                0,
            );
            return Ok(());
        }
        *self.token.lock().map_err(|e| e.to_string())? = Some(record);
        Ok(())
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
    /// Explicit user action only (the tab button); gated by the burn
    /// cooldown inside PairingState (H1).
    pub fn new_pairing_code(&self) -> Result<String, String> {
        self.pairing.issue_new()
    }

    /// Issue a new token after a successful pairing: 32 CSPRNG bytes
    /// (hex-encoded on the wire), persisted as SHA-256 + metadata only
    /// (H2). Returns the plaintext token for the /v1/pair response.
    pub fn issue_token(&self) -> Result<String, String> {
        let raw = super::scheduler::rand_compat::random_token()?;
        let now = unix_now();
        let record = StoredToken {
            token_hash: StoredToken::hash_of(&raw),
            created_at: now,
            last_used: now,
            label: TOKEN_LABEL.to_string(),
        };
        // Persist first: a token we cannot persist would be forgotten on
        // restart, silently re-pairing the user on every launch (the H2
        // conflict). Persistence failure = pair failure.
        {
            let guard = self.token_store.lock().map_err(|e| e.to_string())?;
            match guard.as_ref() {
                Some(store) => {
                    let location = store.save(&record)?;
                    if location == StoreLocation::File {
                        self.audit.record(
                            "assist_token_store_fallback",
                            "localhost",
                            "token record written to the 0600 file fallback (keychain unavailable)",
                            0,
                            0,
                        );
                    }
                }
                None => return Err("token store not attached; enable Word Assist first".into()),
            }
        }
        *self.token.lock().map_err(|e| e.to_string())? = Some(record);
        Ok(raw)
    }

    /// Constant-time bearer check against the stored HASH, with the
    /// sliding 30-day expiry and a throttled last_used refresh (H2).
    pub fn token_matches(&self, presented: &str) -> bool {
        let hash = StoredToken::hash_of(presented);
        let mut guard = match self.token.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        let Some(record) = guard.as_ref() else {
            return false;
        };
        // Both sides are fixed-length 32-byte hex digests, so the
        // constant-time compare is safe to run on the hashes.
        if !constant_time_eq(record.token_hash.as_bytes(), hash.as_bytes()) {
            return false;
        }
        let now = unix_now();
        if StoredToken::is_expired(record.last_used, now, TOKEN_SLIDING_TTL) {
            // Sliding expiry elapsed: revoke on the spot.
            if let Ok(store_guard) = self.token_store.lock() {
                if let Some(store) = store_guard.as_ref() {
                    let _ = store.delete();
                }
            }
            *guard = None;
            self.audit.record(
                "assist_token_expired",
                "localhost",
                "paired token expired (30 days unused); revoked",
                0,
                0,
            );
            return false;
        }
        // Sliding window: refresh last_used. Disk writes are throttled;
        // in-memory is always current.
        let previous_last_used = record.last_used;
        let mut persisted = record.clone();
        persisted.last_used = now;
        *guard = Some(persisted.clone());
        if now.saturating_sub(previous_last_used) >= TOKEN_LAST_USED_WRITE_THROTTLE.as_secs() {
            if let Ok(store_guard) = self.token_store.lock() {
                if let Some(store) = store_guard.as_ref() {
                    if let Err(e) = store.save(&persisted) {
                        log::warn!("token last_used refresh failed: {e}");
                    }
                }
            }
        }
        true
    }

    /// Revoke the paired token everywhere: memory, keychain, file (H2
    /// revoke). Idempotent; used by unpair, disable (rotate-on-disable)
    /// and the explicit rotate action.
    pub fn revoke_token(&self) -> Result<(), String> {
        *self.token.lock().map_err(|e| e.to_string())? = None;
        let had = {
            let guard = self.token_store.lock().map_err(|e| e.to_string())?;
            match guard.as_ref() {
                Some(store) => {
                    store.delete()?;
                    true
                }
                None => false,
            }
        };
        self.audit.record(
            "assist_token_revoked",
            "localhost",
            if had {
                "paired token revoked (memory + persisted record)"
            } else {
                "token revoke requested; nothing persisted"
            },
            0,
            0,
        );
        Ok(())
    }

    /// H2 rotate: revoke the current token. The replacement reaches the
    /// add-in only through pairing (the token is never pushed), so
    /// rotation = revoke + a fresh pairing code issued by the user.
    /// The command layer pairs this with new_pairing_code().
    pub fn rotate_token(&self) -> Result<(), String> {
        self.revoke_token()?;
        self.audit.record(
            "assist_token_rotated",
            "localhost",
            "token rotated: old token revoked; a fresh pairing code is required",
            0,
            0,
        );
        Ok(())
    }

    /// Session wipe used by enable/disable. NOTE (H2): this NO LONGER
    /// deletes the persisted token — enable calls it and the restored
    /// record must survive; disable calls revoke_token() explicitly
    /// (rotate-on-disable ruling).
    pub fn clear_session(&self) -> Result<(), String> {
        *self.token.lock().map_err(|e| e.to_string())? = None;
        self.pairing.clear_code()?;
        Ok(())
    }
}

/// Unix seconds now (token timestamps are persisted as unix seconds).
pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

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
    use super::{
        constant_time_eq, AssistState, FakeClock, PairOutcome, StoredToken, TokenStore,
        PAIRING_COOLDOWN_CAP, PAIRING_COOLDOWN_INITIAL, PAIRING_MAX_FAILURES, TOKEN_SLIDING_TTL,
    };
    use std::sync::Arc;
    use std::time::Duration;

    fn test_state() -> AssistState {
        AssistState::new(Arc::new(crate::ollama::OllamaState::default()))
    }

    /// Serializes the tests that touch the shared OS keychain entry, so
    /// parallel test threads cannot delete each other's record on
    /// machines where a keychain is actually available (the file
    /// fallback path is per-test-dir and immune).
    static TOKEN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn constant_time_eq_basics() {
        assert!(constant_time_eq(b"abcdef", b"abcdef"));
        assert!(!constant_time_eq(b"abcdef", b"abcdeg"));
        assert!(!constant_time_eq(b"abc", b"abcdef"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn rate_limiter_windows() {
        let rl = super::RateLimiter::new(3, Duration::from_millis(50));
        assert!(rl.allow("k"));
        assert!(rl.allow("k"));
        assert!(rl.allow("k"));
        assert!(!rl.allow("k"));
        assert!(rl.allow("other"));
        std::thread::sleep(Duration::from_millis(60));
        assert!(rl.allow("k"));
    }

    #[test]
    fn pairing_code_lifecycle_burn_on_success() {
        let state = test_state();
        let code = state.new_pairing_code().expect("code");
        assert_eq!(code.len(), 6);
        // Wrong code: a counted failure, not a burn.
        assert_eq!(
            state.pairing.take("zzzzzz").expect("check"),
            PairOutcome::Failure { attempts_used: 1 }
        );
        // Right code works once.
        assert_eq!(
            state.pairing.take(&code).expect("check"),
            PairOutcome::Accepted
        );
        // And is burned after use: the same code now reports NoCode.
        assert_eq!(
            state.pairing.take(&code).expect("check"),
            PairOutcome::NoCode
        );
    }

    #[test]
    fn pairing_code_expires_after_ttl() {
        let clock = Arc::new(FakeClock::new());
        let mut state = AssistState::new(Arc::new(crate::ollama::OllamaState::default()));
        state.pairing = super::PairingState::with_ttl(
            Arc::clone(&clock) as Arc<dyn super::Clock>,
            Duration::from_secs(60),
        );
        let code = state.new_pairing_code().expect("code");
        clock.advance(Duration::from_secs(61));
        assert_eq!(
            state.pairing.take(&code).expect("check"),
            PairOutcome::Expired
        );
        // Expired code is gone: the same code no longer matches anything.
        assert_eq!(
            state.pairing.take(&code).expect("check"),
            PairOutcome::NoCode
        );
    }

    #[test]
    fn pairing_burns_on_fifth_failure_and_starts_cooldown() {
        let clock = Arc::new(FakeClock::new());
        let mut state = AssistState::new(Arc::new(crate::ollama::OllamaState::default()));
        state.pairing = super::PairingState::new(Arc::clone(&clock) as Arc<dyn super::Clock>);
        let code = state.new_pairing_code().expect("code");
        for i in 1..PAIRING_MAX_FAILURES {
            assert_eq!(
                state.pairing.take("zzzzzz").expect("check"),
                PairOutcome::Failure { attempts_used: i },
                "failure {i} must not lock yet"
            );
        }
        // The 5th failure burns the code and locks.
        assert_eq!(
            state.pairing.take("zzzzzz").expect("check"),
            PairOutcome::LockedOut
        );
        // Even the CORRECT code is dead now (burned).
        assert_eq!(
            state.pairing.take(&code).expect("check"),
            PairOutcome::LockedOut
        );
        // A new code inside the cooldown is refused.
        assert!(state.new_pairing_code().is_err());
        // After the initial cooldown (30 s) passes, a new code is allowed.
        clock.advance(PAIRING_COOLDOWN_INITIAL + Duration::from_secs(1));
        let code2 = state.new_pairing_code().expect("code after cooldown");
        assert_eq!(code2.len(), 6);
        // The budget is per code: 5 fresh failures lock again, next
        // cooldown doubles to 60 s.
        for _ in 0..PAIRING_MAX_FAILURES {
            let _ = state.pairing.take("zzzzzz").expect("check");
        }
        assert!(state.pairing.cooldown_remaining_secs().is_some());
        assert!(state.pairing.cooldown_remaining_secs().unwrap() >= 60);
        assert!(state.pairing.cooldown_remaining_secs().unwrap() <= 60);
        // Long-session worst case never exceeds the cap.
        assert!(PAIRING_COOLDOWN_CAP >= PAIRING_COOLDOWN_INITIAL);
    }

    #[test]
    fn pairing_no_code_reports_nocode_not_lockout() {
        let state = test_state();
        assert_eq!(
            state.pairing.take("zzzzzz").expect("check"),
            PairOutcome::NoCode
        );
        assert!(!state.pairing.has_live_code());
    }

    #[test]
    fn token_hash_is_sha256_and_persistence_round_trips() {
        let _guard = TOKEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("wa-test-token-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let store = TokenStore::for_dir(&dir);
        let record = StoredToken {
            token_hash: StoredToken::hash_of("raw-token"),
            created_at: 1_000,
            last_used: 2_000,
            label: super::TOKEN_LABEL.into(),
        };
        let _ = store.delete();
        let location = store.save(&record).expect("save");
        let (loaded, _) = store.load().expect("load").expect("record");
        assert_eq!(loaded.token_hash, record.token_hash);
        assert_eq!(loaded.last_used, 2_000);
        let _ = location;
        store.delete().expect("delete");
        assert!(store.load().expect("load").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn token_sliding_expiry_pure_math() {
        let ttl = TOKEN_SLIDING_TTL;
        // 29 days unused: alive.
        assert!(!StoredToken::is_expired(1_000, 1_000 + 29 * 86_400, ttl));
        // 30 days + 1 s: dead.
        assert!(StoredToken::is_expired(1_000, 1_000 + 30 * 86_400 + 1, ttl));
        // Exactly 30 days: still inside (expiry is strict >).
        assert!(!StoredToken::is_expired(1_000, 1_000 + 30 * 86_400, ttl));
        // Clock moved backwards: alive.
        assert!(!StoredToken::is_expired(5_000, 1_000, ttl));
    }

    #[test]
    fn issue_and_match_token_hash_only() {
        let _guard = TOKEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("wa-test-issue-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let state = test_state();
        state.attach_token_store(&dir).expect("attach");
        let raw = state.issue_token().expect("issue");
        assert_eq!(raw.len(), 64, "32 random bytes, hex encoded");
        // The stored record holds the hash, never the token.
        {
            let guard = state.token.lock().expect("lock");
            let record = guard.as_ref().expect("record");
            assert_ne!(record.token_hash, raw);
            assert_eq!(record.token_hash, StoredToken::hash_of(&raw));
        }
        assert!(state.token_matches(&raw));
        assert!(!state.token_matches("not-the-token"));
        // No persistence backend may contain the RAW token: the file
        // fallback (used when the keychain is unavailable) stores the
        // hash only. (On a machine with a live keychain the file is
        // absent and the keychain holds the same hash-only JSON.)
        if let Ok(file) = std::fs::read_to_string(dir.join("word-assist-token.json")) {
            assert!(!file.contains(&raw));
            assert!(file.contains("token_hash"));
        }
        state.revoke_token().expect("revoke");
        assert!(!state.token_matches(&raw));
        std::fs::remove_dir_all(&dir).ok();
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
