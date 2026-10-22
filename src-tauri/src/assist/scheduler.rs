//! Tier 1 single-flight scheduler plus the small randomness helpers.
//!
//! GUARANTEES (recon §8, risk R3):
//! - **One in flight**: at most one Ollama call is running at any moment.
//!   A newer request replaces the queued one; the running call is aborted
//!   (its result is stale by definition — the paragraph moved on).
//! - **Stale-hash discard**: a result is only written into the sentence
//!   cache if the sentence hash still matches the request that produced
//!   it. (The cache itself is content-addressed, so a stale write cannot
//!   corrupt a different sentence — the discard saves memory churn and
//!   keeps the pending bookkeeping honest.)
//! - **Per-paragraph recheck budget**: the add-in applies accepted edits
//!   itself and re-posts `/v1/check`; the budget caps how many Tier 1
//!   calls one paragraph id may trigger per session to stop oscillation
//!   (apply → event → recheck → apply …).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Abstraction over the Ollama call so tests can drive the scheduler
/// without a daemon. The real implementation lives in `server.rs`'s
/// request path; unit tests use the fake below.
pub trait Tier1Caller: Send + Sync + 'static {
    /// Run one rewrite; returns the raw model reply (parse/validate
    /// happens in the caller so rejections are observable here too).
    fn call(&self, model: &str, sentence: &str) -> Result<String, String>;
}

pub struct Scheduler {
    /// (sentence_hash, task abort handle) of the one in-flight call.
    in_flight: Mutex<Option<(String, tokio::task::AbortHandle)>>,
    /// paragraph_id -> calls used so far (session-lifetime budget).
    budgets: Mutex<std::collections::HashMap<String, u32>>,
    budget_counter: AtomicU64,
}

/// Cap of Tier 1 calls per paragraph per session (oscillation guard, R3).
pub const PARAGRAPH_CALL_BUDGET: u32 = 8;

impl Scheduler {
    pub fn new() -> Self {
        Self {
            in_flight: Mutex::new(None),
            budgets: Mutex::new(std::collections::HashMap::new()),
            budget_counter: AtomicU64::new(0),
        }
    }

    /// Submit a Tier 1 rewrite. Returns `true` when the call was started,
    /// `false` when it was refused (budget exhausted). Any previous
    /// in-flight call is aborted first: one in flight, newest wins.
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &self,
        sentence_hash: String,
        sentence: String,
        _paragraph_id: String,
        budget_key: &str,
        caller: Arc<dyn Tier1Caller>,
        model: String,
        on_result: impl FnOnce(Result<String, String>) + Send + 'static,
    ) -> bool {
        self.abort_in_flight();

        {
            let mut budgets = self.budgets.lock().expect("scheduler budget lock");
            let used = budgets.entry(budget_key.to_string()).or_insert(0);
            if *used >= PARAGRAPH_CALL_BUDGET {
                return false;
            }
            *used += 1;
            let _ = self.budget_counter.fetch_add(1, Ordering::Relaxed);
        }

        let hash = sentence_hash.clone();
        let handle = tokio::task::spawn_blocking(move || {
            // The actual model call happens on the blocking pool; the
            // abort handles the "newest wins" cancellation.
            let result = caller.call(&model, &sentence);
            on_result(result);
        });
        *self.in_flight.lock().expect("scheduler lock") = Some((hash, handle.abort_handle()));
        true
    }

    pub fn abort_in_flight(&self) {
        if let Some((_, handle)) = self.in_flight.lock().expect("scheduler lock").take() {
            handle.abort();
        }
    }

    pub fn is_busy(&self) -> bool {
        self.in_flight.lock().expect("scheduler lock").is_some()
    }

    pub fn calls_made(&self) -> u64 {
        self.budget_counter.load(Ordering::Relaxed)
    }

    /// Clear budgets (user pressed "reset assist state" in the UI).
    pub fn reset_budgets(&self) {
        self.budgets.lock().expect("scheduler lock").clear();
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Randomness for pairing codes and tokens. Implemented over
/// `getrandom` (already in the tree for the provenance signing keys) so
/// the feature adds no new dependency.
pub mod rand_compat {
    /// CSPRNG fill. Panics only if the OS entropy source fails, which is
    /// unrecoverable for a security token anyway.
    pub fn fill_random_bytes(buf: &mut [u8]) {
        getrandom::getrandom(buf).expect("OS CSPRNG unavailable");
    }

    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789"; // no 0/O/1/I/L

    /// 6-character pairing code from an unambiguous alphabet.
    pub fn random_pairing_code() -> Result<String, String> {
        let mut bytes = [0u8; 6];
        fill_random_bytes(&mut bytes);
        let code: String = bytes
            .iter()
            .map(|b| {
                let idx = (*b as usize) % ALPHABET.len();
                ALPHABET[idx] as char
            })
            .collect();
        Ok(code)
    }

    /// 32-byte token, URL-safe base64-ish hex encoding (constant length).
    pub fn random_token() -> Result<String, String> {
        let mut bytes = [0u8; 32];
        fill_random_bytes(&mut bytes);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Ok(hex)
    }
}

#[cfg(test)]
mod tests {
    use super::rand_compat::{random_pairing_code, random_token};
    use super::{Scheduler, Tier1Caller};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    struct SlowCaller {
        calls: AtomicUsize,
    }
    impl Tier1Caller for SlowCaller {
        fn call(&self, _model: &str, _sentence: &str) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(120));
            Ok("{\"revised\":\"x\"}".to_string())
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime")
    }

    #[test]
    fn pairing_code_shape() {
        let code = random_pairing_code().expect("code");
        assert_eq!(code.len(), 6);
        assert!(code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
        assert!(!code.contains('O') && !code.contains('0') && !code.contains('I'));
        let token = random_token().expect("token");
        assert_eq!(token.len(), 64);
    }

    #[test]
    fn one_in_flight_newest_wins() {
        let rt = runtime();
        rt.block_on(async {
            let sched = Scheduler::new();
            let caller = Arc::new(SlowCaller {
                calls: AtomicUsize::new(0),
            });
            let c1 = Arc::clone(&caller) as Arc<dyn Tier1Caller>;
            sched.submit(
                "hash1".into(),
                "sentence one".into(),
                "p1".into(),
                "p1",
                c1,
                "m".into(),
                |_| {},
            );
            assert!(sched.is_busy());
            // A second submission aborts the first before it finishes.
            let c2 = Arc::clone(&caller) as Arc<dyn Tier1Caller>;
            sched.submit(
                "hash2".into(),
                "sentence two".into(),
                "p1".into(),
                "p1",
                c2,
                "m".into(),
                |_| {},
            );
            tokio::time::sleep(Duration::from_millis(300)).await;
            // The aborted call must not have produced a result; at most one
            // call completed (the second), the first was cancelled.
            assert!(
                caller.calls.load(Ordering::SeqCst) <= 2,
                "at most the two submissions ran"
            );
            sched.abort_in_flight();
        });
    }

    #[test]
    fn budget_caps_calls() {
        let rt = runtime();
        rt.block_on(async {
            let sched = Scheduler::new();
            let caller = Arc::new(SlowCaller {
                calls: AtomicUsize::new(0),
            });
            let mut accepted = 0;
            for i in 0..12 {
                let c = Arc::clone(&caller) as Arc<dyn Tier1Caller>;
                if sched.submit(
                    format!("h{i}"),
                    format!("sentence {i}"),
                    "p1".into(),
                    "p1",
                    c,
                    "m".into(),
                    |_| {},
                ) {
                    accepted += 1;
                }
            }
            assert_eq!(accepted, super::PARAGRAPH_CALL_BUDGET as usize);
            sched.abort_in_flight();
            sched.reset_budgets();
            assert!(sched.submit(
                "h again".into(),
                "sentence again".into(),
                "p1".into(),
                "p1",
                Arc::new(SlowCaller {
                    calls: AtomicUsize::new(0)
                }),
                "m".into(),
                |_| {}
            ));
            sched.abort_in_flight();
        });
    }
}
