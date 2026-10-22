//! Word Assist ledger (Phase 3).
//!
//! An append-only, SHA-256-hash-chained JSONL record of what the assist
//! surfaced and what the user accepted, following the PROVENANCE_SPEC
//! §0.3/§11 pattern: **hashes and counts, never text**. The ledger exists
//! so the user can later show *that* assistance happened (and how much) —
//! for a disclosure statement or an integrity conversation — without ever
//! storing the manuscript content itself.
//!
//! - File: `<data>/word-assist/ledger.jsonl`, created on first append.
//! - Chain: `hash_n = SHA-256(canonical_entry_n)`, where the canonical
//!   serialization *includes* `prev_hash` (genesis: all zeros). `verify()`
//!   walks the file and re-derives every link; a broken chain is surfaced,
//!   never silently truncated.
//! - NO MAC, NO LOCAL KEY (owner ruling E-4, restated 2026-10-23): a key
//!   stored on the same machine protects nothing against an adversary who
//!   can already rewrite the file — see `docs/LEDGER_THREAT_MODEL.md`.
//!   Tamper *evidence* comes from the chain; tamper *resistance* for an
//!   exported snapshot comes from signing the chain head with the existing
//!   provenance signing key at export time (the export surface itself is a
//!   Phase 4 gate item — nothing signs at rest today).
//! - The ledger only grows while the feature is enabled; disable() stops
//!   writes but keeps the file (it is the user's record).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::BufRead;
use std::path::Path;

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
    /// Hash of the previous entry (genesis: all zeros).
    pub prev_hash: String,
    /// SHA-256 over the canonical serialization of this entry (everything
    /// above, in field order, tab-joined — `prev_hash` included, so each
    /// link commits to its predecessor).
    pub hash: String,
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
        e.prev_hash,
    )
}

fn compute_hash(e: &LedgerEntry) -> String {
    let digest = Sha256::digest(canonical_entry(e).as_bytes());
    hex(&digest)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The open ledger. Appends are serialized through the mutex by the caller
/// (AssistState owns one instance behind a Mutex).
pub struct AssistLedger {
    path: std::path::PathBuf,
    next_seq: u64,
    last_hash: String,
    count: usize,
}

impl std::fmt::Debug for AssistLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
        std::fs::create_dir_all(data_dir).map_err(|e| format!("create assist dir: {e}"))?;
        let path = data_dir.join("ledger.jsonl");
        let mut ledger = Self {
            path,
            next_seq: 0,
            last_hash: GENESIS.to_string(),
            count: 0,
        };
        if ledger.path.exists() {
            let (next, last, count) = ledger.scan_and_verify()?;
            ledger.next_seq = next;
            ledger.last_hash = last;
            ledger.count = count;
        }
        Ok(ledger)
    }

    /// Read the whole file, verify the chain, return (next_seq, last_hash,
    /// entry_count).
    fn scan_and_verify(&self) -> Result<(u64, String, usize), String> {
        let file = std::fs::File::open(&self.path).map_err(|e| format!("open ledger: {e}"))?;
        let reader = std::io::BufReader::new(file);
        let mut expected_prev = GENESIS.to_string();
        let mut last_hash = GENESIS.to_string();
        let mut count = 0usize;
        for (line_no, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| format!("read ledger: {e}"))?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: LedgerEntry = serde_json::from_str(&line)
                .map_err(|e| format!("ledger line {}: {e}", line_no + 1))?;
            if entry.prev_hash != expected_prev {
                return Err(format!(
                    "ledger chain broken at entry {} (sequence {}): prev_hash mismatch",
                    line_no + 1,
                    entry.seq
                ));
            }
            let expect = compute_hash(&entry);
            if expect != entry.hash {
                return Err(format!(
                    "ledger chain broken at entry {} (sequence {}): hash mismatch",
                    line_no + 1,
                    entry.seq
                ));
            }
            expected_prev = entry.hash.clone();
            last_hash = entry.hash;
            count += 1;
        }
        Ok((count as u64, last_hash, count))
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
            prev_hash: self.last_hash.clone(),
            hash: String::new(),
        };
        entry.hash = compute_hash(&entry);

        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open ledger for append: {e}"))?;
        let line = serde_json::to_string(&entry).map_err(|e| format!("serialize ledger: {e}"))?;
        writeln!(file, "{line}").map_err(|e| format!("append ledger: {e}"))?;

        self.last_hash = entry.hash.clone();
        self.next_seq += 1;
        self.count += 1;
        Ok(entry)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// The hash of the most recent entry (genesis zeros when empty). This
    /// is the value a verifier pins when the chain head is signed at
    /// export: an exported snapshot is evidence iff its head hash matches
    /// a signature from the provenance signing key. (A3 integration: the
    /// export signs this SHA-256 head; the pre-hardening HMAC pair form
    /// is gone — hardening wins on the ledger scheme.)
    pub fn chain_head(&self) -> String {
        self.last_hash.clone()
    }

    /// Aggregate view for the Disclosure Assistant pre-fill (counts only).
    /// An absent ledger file is an empty record, not an error — a fresh
    /// install must be able to open the Disclosure Assistant (found via
    /// A3's export tests).
    pub fn aggregate(&self) -> Result<LedgerAggregate, String> {
        if !self.path.exists() {
            return Ok(LedgerAggregate::default());
        }
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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
            // The chain head is the last entry's hash, not the genesis value.
            assert_ne!(ledger.chain_head(), super::GENESIS);
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
        assert!(err.contains("hash mismatch"), "got: {err}");
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
            prev_hash: super::GENESIS.into(),
            hash: String::new(),
        };
        let c = canonical_entry(&e);
        assert_eq!(c.matches('\t').count(), 12);
        assert!(!c.contains("sentence text"));
    }

    #[test]
    fn chain_links_commit_to_their_predecessor() {
        // The second entry's canonical form must embed the first entry's
        // hash — the property that makes the chain re-derivable.
        let dir = tmpdir("links");
        let mut ledger = AssistLedger::open(&dir).expect("open");
        let first = ledger
            .append("accepted", "p1", "i1", "R", "grammar", 0, 1, 2, "")
            .expect("append 1");
        let second = ledger
            .append("accepted", "p2", "i2", "R", "grammar", 0, 1, 2, "")
            .expect("append 2");
        assert_eq!(second.prev_hash, first.hash);
        assert_eq!(ledger.chain_head(), second.hash);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
