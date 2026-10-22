//! Word Assist ledger (Phase 3).
//!
//! An append-only, HMAC-chained JSONL record of what the assist surfaced
//! and what the user accepted, following the PROVENANCE_SPEC §0.3/§11
//! pattern: **hashes and counts, never text**. The ledger exists so the
//! user can later show *that* assistance happened (and how much) — for a
//! disclosure statement or an integrity conversation — without ever
//! storing the manuscript content itself.
//!
//! - File: `<data>/word-assist/ledger.jsonl`, created on first append.
//! - Key: a per-install 32-byte secret in the OS keychain
//!   (`scholarscribe` / `word-assist-ledger-v1`), falling back to a 0600
//!   file next to the ledger when the keychain is unavailable (disclosed
//!   either way, same policy as the TLS key).
//! - Chain: `hmac_n = HMAC-SHA256(key, canonical_entry_n || prev_hmac)`.
//!   `verify()` walks the file and re-derives every link; a broken chain
//!   is surfaced, never silently truncated.
//! - The ledger only grows while the feature is enabled; disable() stops
//!   writes but keeps the file (it is the user's record).

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::io::BufRead;
#[allow(unused_imports)] // Write is used under cfg(unix) and in doc examples
use std::io::Write as _;
use std::path::{Path, PathBuf};

type HmacSha256 = Hmac<Sha256>;

const KEYCHAIN_SERVICE: &str = "scholarscribe";
const KEYCHAIN_USER: &str = "word-assist-ledger-v1";
const KEY_FILE: &str = "word-assist-ledger-key";

/// One ledger row. Text-free by construction: paragraph ids are hashed,
/// suggestions are counted, not stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub seq: u64,
    /// Unix seconds.
    pub ts: u64,
    /// Engine version that produced the event (traceability).
    pub engine: String,
    /// "accepted" | "rejected" | "failed" | "paired"
    pub kind: String,
    /// SHA-256 of the paragraph id (the add-in's id is opaque, but we hash
    /// anyway so the file never carries anything but hashes).
    pub paragraph_hash: String,
    pub issue_id: String,
    pub rule_id: String,
    pub category: String,
    pub applied_start: usize,
    pub applied_end: usize,
    pub replacement_len: usize,
    /// Pre-edit paragraph hash as reported by the add-in ("-" when absent).
    pub paragraph_text_hash: String,
    /// HMAC of the previous entry (genesis: all zeros).
    pub prev_hmac: String,
    /// HMAC-SHA256 over the canonical serialization of this entry
    /// (everything above, in field order, tab-joined) under the per-install
    /// key.
    pub hmac: String,
}

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub fn canonical_entry(e: &LedgerEntry) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        e.seq,
        e.ts,
        e.engine,
        e.kind,
        e.paragraph_hash,
        e.issue_id,
        e.rule_id,
        e.category,
        e.applied_start,
        e.applied_end,
        e.replacement_len,
        e.paragraph_text_hash,
        e.prev_hmac,
    )
}

fn compute_hmac(key: &[u8], e: &LedgerEntry) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key length");
    mac.update(canonical_entry(e).as_bytes());
    hex(&mac.finalize().into_bytes())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Load or create the per-install ledger key. Keychain first, 0600 file
/// fallback (same disclosure policy as the TLS key).
fn ensure_key(data_dir: &Path) -> Result<Vec<u8>, String> {
    std::fs::create_dir_all(data_dir).map_err(|e| format!("create assist dir: {e}"))?;
    let key_path = data_dir.join(KEY_FILE);
    if let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER) {
        if let Ok(stored) = entry.get_password() {
            if let Some(bytes) = unhex(stored.trim()) {
                return Ok(bytes);
            }
        }
    }
    if key_path.exists() {
        let stored =
            std::fs::read_to_string(&key_path).map_err(|e| format!("read ledger key file: {e}"))?;
        if let Some(bytes) = unhex(stored.trim()) {
            return Ok(bytes);
        }
        return Err("ledger key file is corrupt".into());
    }
    let mut key = vec![0u8; 32];
    super::scheduler::rand_compat::fill_random_bytes(&mut key);
    let hexkey = hex(&key);
    let stored_in_keychain = match keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER) {
        Ok(entry) => entry.set_password(&hexkey).is_ok(),
        Err(_) => false,
    };
    if !stored_in_keychain {
        use std::io::Write as _;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut f = std::fs::File::create(&key_path)
                .map_err(|e| format!("create ledger key file: {e}"))?;
            f.write_all(hexkey.as_bytes())
                .map_err(|e| format!("write ledger key file: {e}"))?;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("restrict ledger key file: {e}"))?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&key_path, hexkey.as_bytes())
                .map_err(|e| format!("write ledger key file: {e}"))?;
        }
    }
    log::info!(
        "Word Assist ledger key created (in {})",
        if stored_in_keychain {
            "OS keychain"
        } else {
            "app data (0600)"
        }
    );
    Ok(key)
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || s.len() != 64 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// The open ledger. Appends are serialized through the mutex by the caller
/// (AssistState owns one instance behind a Mutex).
pub struct AssistLedger {
    path: PathBuf,
    key: Vec<u8>,
    next_seq: u64,
    last_hmac: String,
    count: usize,
}

impl std::fmt::Debug for AssistLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The key is secret; never derive Debug over it.
        f.debug_struct("AssistLedger")
            .field("path", &self.path)
            .field("next_seq", &self.next_seq)
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

impl AssistLedger {
    /// Open (or prepare) the ledger at `data_dir/ledger.jsonl`. An absent
    /// file is fine; a present file is verified up front so a tampered
    /// ledger is reported at open time, not at the worst moment later.
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        let key = ensure_key(data_dir)?;
        let path = data_dir.join("ledger.jsonl");
        let mut ledger = Self {
            path,
            key,
            next_seq: 0,
            last_hmac: GENESIS.to_string(),
            count: 0,
        };
        if ledger.path.exists() {
            let (next, last, count) = ledger.scan_and_verify()?;
            ledger.next_seq = next;
            ledger.last_hmac = last;
            ledger.count = count;
        }
        Ok(ledger)
    }

    /// Read the whole file, verify the chain, return (next_seq, last_hmac,
    /// entry_count).
    fn scan_and_verify(&self) -> Result<(u64, String, usize), String> {
        let file = std::fs::File::open(&self.path).map_err(|e| format!("open ledger: {e}"))?;
        let reader = std::io::BufReader::new(file);
        let mut expected_prev = GENESIS.to_string();
        let mut last_hmac = GENESIS.to_string();
        let mut count = 0usize;
        for (line_no, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| format!("read ledger: {e}"))?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: LedgerEntry = serde_json::from_str(&line)
                .map_err(|e| format!("ledger line {}: {e}", line_no + 1))?;
            if entry.prev_hmac != expected_prev {
                return Err(format!(
                    "ledger chain broken at entry {} (sequence {}): prev_hmac mismatch",
                    line_no + 1,
                    entry.seq
                ));
            }
            let expect = compute_hmac(&self.key, &entry);
            if expect != entry.hmac {
                return Err(format!(
                    "ledger chain broken at entry {} (sequence {}): hmac mismatch",
                    line_no + 1,
                    entry.seq
                ));
            }
            expected_prev = entry.hmac.clone();
            last_hmac = entry.hmac;
            count += 1;
        }
        Ok((count as u64, last_hmac, count))
    }

    /// Append one event. `paragraph_id` is hashed here so no call site can
    /// accidentally store text or raw ids.
    #[allow(clippy::too_many_arguments)] // the row shape is fixed by the spec; a struct would obscure the call sites
    pub fn append(
        &mut self,
        kind: &str,
        paragraph_id: &str,
        issue_id: &str,
        rule_id: &str,
        category: &str,
        applied_start: usize,
        applied_end: usize,
        replacement_len: usize,
        paragraph_text_hash: &str,
    ) -> Result<LedgerEntry, String> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut entry = LedgerEntry {
            seq: self.next_seq,
            ts,
            engine: super::ENGINE_VERSION.to_string(),
            kind: kind.to_string(),
            paragraph_hash: super::cache::text_hash(paragraph_id),
            issue_id: issue_id.to_string(),
            rule_id: rule_id.to_string(),
            category: category.to_string(),
            applied_start,
            applied_end,
            replacement_len,
            paragraph_text_hash: if paragraph_text_hash.is_empty() {
                "-".to_string()
            } else {
                paragraph_text_hash.chars().take(12).collect()
            },
            prev_hmac: self.last_hmac.clone(),
            hmac: String::new(),
        };
        entry.hmac = compute_hmac(&self.key, &entry);

        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open ledger for append: {e}"))?;
        let line = serde_json::to_string(&entry).map_err(|e| format!("serialize ledger: {e}"))?;
        writeln!(file, "{line}").map_err(|e| format!("append ledger: {e}"))?;

        self.last_hmac = entry.hmac.clone();
        self.next_seq += 1;
        self.count += 1;
        Ok(entry)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Aggregate view for the Disclosure Assistant pre-fill (counts only).
    pub fn aggregate(&self) -> Result<LedgerAggregate, String> {
        let file = std::fs::File::open(&self.path).map_err(|e| format!("open ledger: {e}"))?;
        let reader = std::io::BufReader::new(file);
        let mut agg = LedgerAggregate::default();
        for line in reader.lines() {
            let line = line.map_err(|e| format!("read ledger: {e}"))?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: LedgerEntry =
                serde_json::from_str(&line).map_err(|e| format!("ledger parse: {e}"))?;
            match entry.kind.as_str() {
                "accepted" => {
                    agg.accepted += 1;
                    match entry.category.as_str() {
                        "grammar" => agg.grammar += 1,
                        "spelling" => agg.spelling += 1,
                        "clarity" => agg.clarity += 1,
                        _ => {}
                    }
                    if entry.rule_id == "tier1" {
                        agg.rewrites += 1;
                    }
                }
                "rejected" => agg.rejected += 1,
                "paired" => agg.sessions += 1,
                _ => {}
            }
        }
        Ok(agg)
    }
}

/// Counts-only aggregate (PROVENANCE_SPEC §0.3: numbers, never text).
#[derive(Debug, Clone, Default, Serialize)]
pub struct LedgerAggregate {
    pub accepted: u64,
    pub rejected: u64,
    pub sessions: u64,
    pub grammar: u64,
    pub spelling: u64,
    pub clarity: u64,
    pub rewrites: u64,
}

#[cfg(test)]
mod tests {
    use super::{canonical_entry, AssistLedger};
    use std::path::PathBuf;

    fn tmpdir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("wa-ledger-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmpdir");
        dir
    }

    #[test]
    fn append_verify_and_aggregate() {
        let dir = tmpdir("append");
        {
            let mut ledger = AssistLedger::open(&dir).expect("open");
            ledger
                .append(
                    "accepted", "p1", "issue-1", "SomeRule", "grammar", 3, 8, 5, "abcd1234",
                )
                .expect("append 1");
            ledger
                .append(
                    "accepted", "p2", "issue-2", "tier1", "clarity", 0, 30, 31, "",
                )
                .expect("append 2");
            ledger
                .append("rejected", "p1", "issue-3", "Other", "grammar", 0, 0, 0, "")
                .expect("append 3");
            assert_eq!(ledger.count(), 3);
        }
        // Re-open: verification must pass and counting must continue.
        let mut ledger = AssistLedger::open(&dir).expect("reopen");
        assert_eq!(ledger.count(), 3);
        ledger
            .append("accepted", "p3", "issue-4", "R", "spelling", 1, 2, 3, "")
            .expect("append after reopen");
        let agg = ledger.aggregate().expect("aggregate");
        assert_eq!(agg.accepted, 3);
        assert_eq!(agg.rejected, 1);
        assert_eq!(agg.rewrites, 1);
        assert_eq!(agg.grammar, 1);
        assert_eq!(agg.spelling, 1);
        assert_eq!(agg.clarity, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampering_is_detected() {
        let dir = tmpdir("tamper");
        {
            let mut ledger = AssistLedger::open(&dir).expect("open");
            ledger
                .append("accepted", "p1", "issue-1", "R", "grammar", 0, 1, 2, "")
                .expect("append");
            ledger
                .append("accepted", "p1", "issue-2", "R", "grammar", 0, 1, 2, "")
                .expect("append");
        }
        // Tamper with the first line's issue_id.
        let path = dir.join("ledger.jsonl");
        let raw = std::fs::read_to_string(&path).expect("read");
        let tampered = raw.replacen("\"issue-1\"", "\"issue-9\"", 1);
        std::fs::write(&path, tampered).expect("write");
        let err = AssistLedger::open(&dir).expect_err("tampered ledger must fail");
        assert!(err.contains("hmac mismatch"), "got: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn canonical_is_stable_and_text_free() {
        let e = super::LedgerEntry {
            seq: 1,
            ts: 42,
            engine: "word-assist/0.1.0".into(),
            kind: "accepted".into(),
            paragraph_hash: "ab".into(),
            issue_id: "i".into(),
            rule_id: "r".into(),
            category: "grammar".into(),
            applied_start: 0,
            applied_end: 2,
            replacement_len: 2,
            paragraph_text_hash: "-".into(),
            prev_hmac: super::GENESIS.into(),
            hmac: String::new(),
        };
        let c = canonical_entry(&e);
        assert_eq!(c.matches('\t').count(), 12);
        assert!(!c.contains("sentence text"));
    }
}
