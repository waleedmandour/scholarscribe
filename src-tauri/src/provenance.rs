//! Writing Provenance — Phase 1 core.
//!
//! ETHICAL SCOPE — READ THIS FIRST:
//! This module produces *evidence about process*, not verdicts about text.
//! It reads the revision history that Microsoft Word's Track Changes feature
//! genuinely recorded, groups edits into work sessions, chains those sessions
//! into a tamper-evident SHA-256 hash chain, and signs the chain with an
//! Ed25519 key stored in the OS keychain.
//!
//! What this module deliberately does NOT do:
//! - No AI-detection score of any kind. `style_consistency.distance_score`
//!   measures how similar the *final* text is to a *baseline the author
//!   supplied* (or to the earliest tracked session) — it is a descriptive
//!   statistic, never a "humanness" verdict, and the UI presents it with
//!   interpretation bands, not pass/fail.
//! - No fabrication. Every number in the manifest is derived from revisions
//!   Word itself recorded. We never invent history (see docs/ETHICS.md §2.3).
//! - No document content in the manifest. Only hashes, timestamps, edit
//!   sizes, author display names as recorded by Word, and aggregate metrics.
//! - No network access. This module is pure, offline code. (The Phase 1.5
//!   Google bridge lives in google_docs*.rs and logs every outbound call to
//!   the privacy audit log.)
//!
//! VERIFICATION MODEL
//! The manifest is a JSON file plus a detached Ed25519 signature over a
//! *canonical payload string* (not the JSON bytes — this avoids any float /
//! key-ordering ambiguity between Rust and the JavaScript verifier). The
//! canonical payload format is specified in docs/PROVENANCE_SPEC.md and
//! mirrored byte-for-byte by verifier/verify.js. Any change to the format
//! must update all three (this file, the spec, the verifier) in one commit.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Genesis marker: the `prev_record_hash` of the first record in each
/// author's chain. 64 zeros — the verifier expects exactly this.
pub const GENESIS_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// Manifest format version. Bump only with a spec change + verifier update.
pub const MANIFEST_VERSION: &str = "1.0";

/// Session-grouping window: edits by the same author within this many
/// seconds of each other belong to the same session (30 minutes).
pub const SESSION_GAP_SECS: i64 = 30 * 60;

/// A gap longer than this between consecutive sessions of the same author
/// is flagged as an anomaly (7 days).
pub const ANOMALY_GAP_SECS: i64 = 7 * 24 * 3600;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Public API surface kept for spec compliance and future in-app verification;
/// not every item is exercised by the current command layer.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ProvenanceError {
    /// The document contains no `w:ins` / `w:del` elements at all.
    #[error("No tracked changes found in this document. Provenance needs an edit history. Make sure Track Changes was switched on in Word while the document was edited.")]
    NoTrackChanges,

    /// Track-changes markup exists but no usable revisions were extracted.
    #[error("Track Changes markup was found, but no revisions were recorded. The document may have been opened and re-saved with all revisions already accepted or rejected.")]
    NoRevisions,

    /// The OOXML inside the .docx could not be parsed.
    #[error("Could not parse Track Changes data: the document.xml inside this file is not valid OOXML. Re-save the document from Word and try again. ({0})")]
    CorruptOoxml(String),

    /// Filesystem / IO problem.
    #[error("Could not read the document file. ({0})")]
    Io(String),

    /// Anything else.
    #[error("{0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// Raw revision input (produced by the .docx parser or the Google bridge)
// ---------------------------------------------------------------------------

/// One recorded revision, straight out of the document's revision markup.
/// This is the only structure that ever touches revision *text*; it lives in
/// memory for the duration of an analysis and is never persisted.
#[derive(Debug, Clone, PartialEq)]
pub struct RawRevision {
    /// Author display name as recorded by the editing tool.
    pub author: String,
    /// Revision timestamp (unix seconds, UTC).
    pub date: i64,
    /// `Insertion` (w:ins) or `Deletion` (w:del).
    pub kind: RevisionKind,
    /// The inserted or deleted text.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionKind {
    Insertion,
    Deletion,
}

impl RevisionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RevisionKind::Insertion => "insertion",
            RevisionKind::Deletion => "deletion",
        }
    }
}

/// A contiguous burst of edits by one author. Produced by grouping
/// `RawRevision`s with `group_into_sessions`.
#[derive(Debug, Clone)]
pub struct RawSession {
    pub author: String,
    pub start_time: i64,
    pub end_time: i64,
    pub chars_added: u32,
    pub chars_removed: u32,
    /// Longest single insertion in this session (chars). Part of the record
    /// hash — large unedited blocks are the signature of pasted-in text.
    pub largest_insertion: u32,
    /// Revisions in this session, chronological.
    pub revisions: Vec<RawRevision>,
}

/// Result of session grouping: sessions plus any anomalies observed.
#[derive(Debug, Default)]
pub struct SessionGrouping {
    pub sessions: Vec<RawSession>,
    pub anomalies: Vec<String>,
}

/// Group raw revisions into sessions.
///
/// Rules (see docs/PROVENANCE_SPEC.md §4):
/// - Revisions are sorted by timestamp (stable — ties keep markup order).
/// - Consecutive revisions by the SAME author whose timestamps are at most
///   `SESSION_GAP_SECS` apart join the current session of that author.
/// - Each author owns an independent session sequence (and, later, an
///   independent hash chain). Different authors never share a session.
/// - A gap of more than `ANOMALY_GAP_SECS` (7 days) between consecutive
///   sessions of the same author is reported as an anomaly.
/// - A revision timestamped in the future (> now + 1 day) is reported as an
///   anomaly and clamped out of grouping order concerns (still recorded).
pub fn group_into_sessions(revisions: &[RawRevision]) -> SessionGrouping {
    let mut out = SessionGrouping::default();
    if revisions.is_empty() {
        return out;
    }

    // Stable index sort by (date, original order).
    let mut order: Vec<usize> = (0..revisions.len()).collect();
    order.sort_by_key(|&i| (revisions[i].date, i));

    let now_plus_day: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 86_400;

    // Per-author state: index into out.sessions of that author's open session.
    let mut open: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for &i in &order {
        let rev = &revisions[i];
        if rev.date > now_plus_day {
            out.anomalies.push(format!(
                "future_timestamp: revision by '{}' is dated after the current date",
                rev.author
            ));
        }

        let text_chars = rev.text.chars().count() as u32;
        match open.get(&rev.author).copied() {
            Some(idx) => {
                let sess = &mut out.sessions[idx];
                let gap = rev.date - sess.end_time;
                if gap <= SESSION_GAP_SECS {
                    // Extend the open session.
                    sess.end_time = rev.date.max(sess.end_time);
                    match rev.kind {
                        RevisionKind::Insertion => {
                            sess.chars_added += text_chars;
                            if text_chars > sess.largest_insertion {
                                sess.largest_insertion = text_chars;
                            }
                        }
                        RevisionKind::Deletion => sess.chars_removed += text_chars,
                    }
                    sess.revisions.push(rev.clone());
                } else {
                    // Close it; open a new one for the same author.
                    if rev.date - sess.end_time > ANOMALY_GAP_SECS {
                        out.anomalies.push(format!(
                            "time_gap: {:.1} days between sessions by '{}'",
                            (rev.date - sess.end_time) as f64 / 86_400.0,
                            rev.author
                        ));
                    }
                    let new_idx = out.sessions.len();
                    out.sessions.push(RawSession {
                        author: rev.author.clone(),
                        start_time: rev.date,
                        end_time: rev.date,
                        chars_added: if rev.kind == RevisionKind::Insertion {
                            text_chars
                        } else {
                            0
                        },
                        chars_removed: if rev.kind == RevisionKind::Deletion {
                            text_chars
                        } else {
                            0
                        },
                        largest_insertion: if rev.kind == RevisionKind::Insertion {
                            text_chars
                        } else {
                            0
                        },
                        revisions: vec![rev.clone()],
                    });
                    open.insert(rev.author.clone(), new_idx);
                }
            }
            None => {
                // First session for this author.
                let new_idx = out.sessions.len();
                out.sessions.push(RawSession {
                    author: rev.author.clone(),
                    start_time: rev.date,
                    end_time: rev.date,
                    chars_added: if rev.kind == RevisionKind::Insertion {
                        text_chars
                    } else {
                        0
                    },
                    chars_removed: if rev.kind == RevisionKind::Deletion {
                        text_chars
                    } else {
                        0
                    },
                    largest_insertion: if rev.kind == RevisionKind::Insertion {
                        text_chars
                    } else {
                        0
                    },
                    revisions: vec![rev.clone()],
                });
                open.insert(rev.author.clone(), new_idx);
            }
        }
    }

    out.sessions
        .sort_by_key(|s| (s.start_time, s.author.clone()));
    out
}

// ---------------------------------------------------------------------------
// Hash chain
// ---------------------------------------------------------------------------

/// The published, privacy-safe record of one session. This is what goes in
/// the manifest — note there is no revision text here, only aggregates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    pub session_id: String,
    pub author: String,
    /// Unix seconds, UTC.
    pub start_time: i64,
    pub end_time: i64,
    /// SHA-256 over the session's revision content (canonical form — see
    /// `snapshot_hash`). Hash only: no text.
    pub snapshot_hash: String,
    pub chars_added: u32,
    pub chars_removed: u32,
    pub largest_insertion: u32,
    /// Hash of the previous record by the SAME author (or `GENESIS_HASH`).
    pub prev_record_hash: String,
    /// `sha256:{hex}` over the canonical record string.
    pub record_hash: String,
}

/// SHA-256 hex digest helper (lowercase, no prefix).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Deterministic snapshot hash of a session's revision content.
/// Format (docs/PROVENANCE_SPEC.md §5.1):
///   scholarscribe-session-v1 \n author \n start \n end \n
///   then per revision (chronological):  kind \n date \n len:text
/// where `len` is the text's UTF-8 byte length (length-prefix defeats
/// concatenation ambiguity).
pub fn compute_snapshot_hash(session: &RawSession) -> String {
    let mut buf = String::new();
    buf.push_str("scholarscribe-session-v1\n");
    buf.push_str(&session.author);
    buf.push('\n');
    buf.push_str(&session.start_time.to_string());
    buf.push('\n');
    buf.push_str(&session.end_time.to_string());
    buf.push('\n');
    for r in &session.revisions {
        buf.push_str(r.kind.as_str());
        buf.push('\n');
        buf.push_str(&r.date.to_string());
        buf.push('\n');
        buf.push_str(&r.text.len().to_string());
        buf.push(':');
        buf.push_str(&r.text);
    }
    format!("sha256:{}", sha256_hex(buf.as_bytes()))
}

/// Canonical record string (docs/PROVENANCE_SPEC.md §5.2). The verifier
/// rebuilds this exact string from the manifest JSON and re-hashes it.
pub fn canonical_record_string(rec: &SessionRecord) -> String {
    // Fixed field order, fixed count — unambiguous without delimiters.
    format!(
        "scholarscribe-record-v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        rec.session_id,
        rec.author,
        rec.start_time,
        rec.end_time,
        rec.snapshot_hash,
        rec.chars_added,
        rec.chars_removed,
        rec.largest_insertion,
        rec.prev_record_hash,
    )
}

/// Compute a record hash over the chain inputs.
#[allow(clippy::too_many_arguments)]
pub fn compute_record_hash(
    session_id: &str,
    author: &str,
    start_time: i64,
    end_time: i64,
    snapshot_hash: &str,
    chars_added: u32,
    chars_removed: u32,
    largest_insertion: u32,
    prev_record_hash: &str,
) -> String {
    // The canonical layout is implemented via canonical_record_string on a
    // fully-populated record; build that record here to keep one definition.
    let rec = SessionRecord {
        session_id: session_id.to_string(),
        author: author.to_string(),
        start_time,
        end_time,
        snapshot_hash: snapshot_hash.to_string(),
        chars_added,
        chars_removed,
        largest_insertion,
        prev_record_hash: prev_record_hash.to_string(),
        record_hash: String::new(),
    };
    format!(
        "sha256:{}",
        sha256_hex(canonical_record_string(&rec).as_bytes())
    )
}

/// Build the hash chain from raw sessions.
/// Each author's records chain independently; `sessions` in the result are
/// ordered chronologically by session start.
pub fn build_chain(sessions: &[RawSession]) -> Vec<SessionRecord> {
    let mut last_hash: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut records = Vec::with_capacity(sessions.len());
    for s in sessions {
        let snapshot_hash = compute_snapshot_hash(s);
        let prev = last_hash
            .get(&s.author)
            .cloned()
            .unwrap_or_else(|| GENESIS_HASH.to_string());
        let session_id = uuid::Uuid::new_v4().to_string();
        let record_hash = compute_record_hash(
            &session_id,
            &s.author,
            s.start_time,
            s.end_time,
            &snapshot_hash,
            s.chars_added,
            s.chars_removed,
            s.largest_insertion,
            &prev,
        );
        last_hash.insert(s.author.clone(), record_hash.clone());
        records.push(SessionRecord {
            session_id,
            author: s.author.clone(),
            start_time: s.start_time,
            end_time: s.end_time,
            snapshot_hash,
            chars_added: s.chars_added,
            chars_removed: s.chars_removed,
            largest_insertion: s.largest_insertion,
            prev_record_hash: prev,
            record_hash,
        });
    }
    records
}

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StyleConsistency {
    /// 1 − cosine similarity between the baseline and final style vectors.
    /// None when no usable baseline exists (e.g. a single-session document).
    pub distance_score: Option<f32>,
    /// Where the baseline came from: "user_reference_text" |
    /// "first_tracked_session" | "unavailable".
    pub baseline_source: String,
    /// Names of the metrics that formed the comparison vector.
    pub metrics_compared: Vec<String>,
    /// Plain-language interpretation of the score band.
    pub interpretation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenanceManifest {
    pub version: String,
    /// SHA-256 of the document's word/document.xml (`sha256:{hex}`).
    pub document_hash: String,
    /// `ed25519:{sha256-hex-of-public-key}`.
    pub author_key_fingerprint: String,
    pub sessions: Vec<SessionRecord>,
    pub style_consistency: StyleConsistency,
    /// `ed25519:{hex}` over the canonical manifest payload (§6). Empty string
    /// in unsigned previews.
    pub signature: String,
    /// When the manifest was produced (unix seconds).
    pub generated_at: i64,
    /// Which tool produced it ("scholarscribe-x.y.z").
    pub generator: String,
    /// Revision count that fed the chain (audit convenience).
    pub revision_count: u32,
}

/// Summary a verifier (or the app's own re-check) produces from a manifest.
#[derive(Debug, Clone, Serialize)]
pub struct VerificationResult {
    pub chain_intact: bool,
    pub signature_valid: bool,
    pub session_count: usize,
    pub time_span_hours: f64,
    pub largest_insertion_pct: f64,
    pub style_distance_score: Option<f32>,
    pub anomalies: Vec<String>,
}

/// Human-readable interpretation bands for the style distance score
/// (docs/PROVENANCE_SPEC.md §7). Deliberately *not* pass/fail.
pub fn interpret_style_distance(score: f32) -> String {
    if score < 0.2 {
        "Very close to the baseline style. (Descriptive only; this is not a human-writing score.)"
            .into()
    } else if score < 0.4 {
        "Broadly consistent with the baseline style. (Descriptive only.)".into()
    } else if score < 0.6 {
        "Noticeable differences from the baseline. Could be a different register, co-authors, or heavy editing. (Descriptive only.)".into()
    } else {
        "Substantially different from the baseline. Worth reviewing which sessions introduced the shift. (Descriptive only.)".into()
    }
}

/// Build the canonical manifest payload string (docs/PROVENANCE_SPEC.md §6).
/// This — not the JSON file — is what the Ed25519 signature covers. Both this
/// function and verifier/verify.js must construct it byte-identically.
pub fn canonical_manifest_payload(m: &ProvenanceManifest) -> String {
    let mut p = String::new();
    p.push_str("SCHOLARSCRIBE-MANIFEST-v1\n");
    p.push_str(&m.version);
    p.push('\n');
    p.push_str(&m.document_hash);
    p.push('\n');
    p.push_str(&m.author_key_fingerprint);
    p.push('\n');
    p.push_str(&m.generated_at.to_string());
    p.push('\n');
    p.push_str(&m.generator);
    p.push('\n');
    p.push_str(&m.revision_count.to_string());
    p.push('\n');
    for s in &m.sessions {
        p.push_str(&format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
            s.session_id,
            s.author,
            s.start_time,
            s.end_time,
            s.snapshot_hash,
            s.chars_added,
            s.chars_removed,
            s.largest_insertion,
            s.prev_record_hash,
            s.record_hash,
        ));
    }
    // Style block: score formatted to exactly 4 decimal places (toFixed(4)
    // in the verifier) or the literal string "null".
    p.push_str(&match m.style_consistency.distance_score {
        Some(v) => format!("{:.4}", round4(v)),
        None => "null".to_string(),
    });
    p.push('\n');
    p.push_str(&m.style_consistency.baseline_source);
    p.push('\n');
    p.push_str(&m.style_consistency.metrics_compared.join(","));
    p.push('\n');
    p
}

fn round4(v: f32) -> f32 {
    (v * 10_000.0).round() / 10_000.0
}

/// Sign a manifest with the given signing key. Returns the signature string
/// (`ed25519:{hex}`) to place in `manifest.signature`.
pub fn sign_manifest(m: &ProvenanceManifest, key: &SigningKey) -> String {
    let payload = canonical_manifest_payload(m);
    let sig = key.sign(payload.as_bytes());
    format!("ed25519:{}", hex(&sig.to_bytes()))
}

/// Verify a manifest's signature against the public key recorded in its own
/// fingerprint (the caller passes the actual `VerifyingKey`; binding between
/// fingerprint and key is checked by the caller via `key_fingerprint`).
pub fn verify_manifest_signature(
    m: &ProvenanceManifest,
    verifying_key: &VerifyingKey,
    expected_fingerprint: &str,
) -> Result<bool, String> {
    if m.author_key_fingerprint != expected_fingerprint {
        return Err(format!(
            "manifest fingerprint {} does not match provided key fingerprint {}",
            m.author_key_fingerprint, expected_fingerprint
        ));
    }
    if m.signature.is_empty() {
        return Ok(false);
    }
    let hex_sig = m
        .signature
        .strip_prefix("ed25519:")
        .ok_or("signature is not in ed25519:{hex} form")?;
    let sig_bytes = decode_hex(hex_sig).map_err(|e| format!("bad signature hex: {e}"))?;
    if sig_bytes.len() != 64 {
        return Err("signature must be 64 bytes".into());
    }
    let sig = Signature::from_bytes(&sig_bytes.try_into().unwrap());
    let payload = canonical_manifest_payload(m);
    Ok(verifying_key.verify(payload.as_bytes(), &sig).is_ok())
}

/// Fingerprint for a verifying key: `ed25519:{sha256(vk bytes) hex}`.
pub fn key_fingerprint(vk: &VerifyingKey) -> String {
    format!("ed25519:{}", sha256_hex(&vk.to_bytes()))
}

/// Walk the chain: for each author, each record's `prev_record_hash` must
/// match the previous record that author issued (or GENESIS), and each
/// `record_hash` must recompute exactly. Returns anomaly strings for any
/// break. A chain of one author is the normal case.
pub fn verify_chain(records: &[SessionRecord]) -> (bool, Vec<String>) {
    let mut anomalies = Vec::new();
    let mut last: std::collections::HashMap<String, SessionRecord> =
        std::collections::HashMap::new();
    for r in records {
        // Recompute the record hash from its own fields.
        let recomputed = compute_record_hash(
            &r.session_id,
            &r.author,
            r.start_time,
            r.end_time,
            &r.snapshot_hash,
            r.chars_added,
            r.chars_removed,
            r.largest_insertion,
            &r.prev_record_hash,
        );
        if recomputed != r.record_hash {
            anomalies.push(format!(
                "chain_break: record hash mismatch for session {} (author '{}')",
                r.session_id, r.author
            ));
        }
        match last.get(&r.author) {
            Some(prev) => {
                if r.prev_record_hash != prev.record_hash {
                    anomalies.push(format!(
                        "chain_break: session {} by '{}' does not link to that author's previous record",
                        r.session_id, r.author
                    ));
                }
                if r.start_time < prev.start_time {
                    anomalies.push(format!(
                        "order: session {} by '{}' starts before the author's previous session",
                        r.session_id, r.author
                    ));
                }
            }
            None => {
                if r.prev_record_hash != GENESIS_HASH {
                    anomalies.push(format!(
                        "chain_break: first session {} by '{}' does not use the genesis marker",
                        r.session_id, r.author
                    ));
                }
            }
        }
        last.insert(r.author.clone(), r.clone());
    }
    (anomalies.is_empty(), anomalies)
}

/// Produce a full VerificationResult for a manifest (chain + aggregate stats).
/// Signature checking needs the key, so it is a separate step (see
/// `verify_manifest_signature`) — callers merge the two flags.
pub fn summarize_verification(m: &ProvenanceManifest) -> VerificationResult {
    let (chain_intact, anomalies) = verify_chain(&m.sessions);
    let time_span_hours = if m.sessions.is_empty() {
        0.0
    } else {
        let min = m.sessions.iter().map(|s| s.start_time).min().unwrap_or(0);
        let max = m.sessions.iter().map(|s| s.end_time).max().unwrap_or(0);
        (max - min).max(0) as f64 / 3600.0
    };
    let total_added: u64 = m.sessions.iter().map(|s| s.chars_added as u64).sum();
    let largest: u64 = m.sessions.iter().map(|s| s.largest_insertion as u64).sum();
    let largest_insertion_pct = if total_added == 0 {
        0.0
    } else {
        (largest as f64 / total_added as f64) * 100.0
    };
    VerificationResult {
        chain_intact,
        signature_valid: false, // caller fills after signature check
        session_count: m.sessions.len(),
        time_span_hours: (time_span_hours * 10.0).round() / 10.0,
        largest_insertion_pct: (largest_insertion_pct * 10.0).round() / 10.0,
        style_distance_score: m.style_consistency.distance_score,
        anomalies,
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd-length hex".into());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let hi = (b[i] as char).to_digit(16).ok_or("bad hex digit")?;
        let lo = (b[i + 1] as char).to_digit(16).ok_or("bad hex digit")?;
        out.push((hi * 16 + lo) as u8);
    }
    Ok(out)
}

/// Decode an `ed25519:{hex}` fingerprint's key component? No — fingerprints
/// are SHA-256 digests, not keys. Public keys are shared separately as
/// `ed25519-pub:{hex}` (64 bytes → 32 bytes raw) — see provenance_commands.
#[allow(dead_code)]
pub fn verifying_key_from_hex(hex_key: &str) -> Result<VerifyingKey, String> {
    let bytes = decode_hex(hex_key).map_err(|e| format!("bad public key hex: {e}"))?;
    if bytes.len() != 32 {
        return Err("public key must be 32 bytes".into());
    }
    VerifyingKey::from_bytes(&bytes.try_into().unwrap()).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Style consistency + citation validation (aggregate metrics only)
// ---------------------------------------------------------------------------

/// The 12 style.rs metrics that form the comparison vector. Raw counts
/// (word_count, sentence_count) are excluded on purpose: they scale with
/// document length, not with style shape. metrics_compared in the manifest
/// records this list so the choice is transparent.
pub const STYLE_METRICS: [&str; 12] = [
    "avg_sentence_length",
    "sentence_length_stdev",
    "type_token_ratio",
    "avg_paragraph_length",
    "passive_ratio",
    "hedge_density",
    "connector_density",
    "first_person_singular_ratio",
    "first_person_plural_ratio",
    "citation_density",
    "flesch_reading_ease",
    "gunning_fog",
];

fn style_vector(profile: &crate::style::StyleProfile) -> Vec<f64> {
    vec![
        profile.avg_sentence_length,
        profile.sentence_length_stdev,
        profile.type_token_ratio,
        profile.avg_paragraph_length,
        profile.passive_ratio,
        profile.hedge_density,
        profile.connector_density,
        profile.first_person_singular_ratio,
        profile.first_person_plural_ratio,
        profile.citation_density,
        profile.flesch_reading_ease,
        profile.gunning_fog,
    ]
}

/// 1 − cosine similarity between baseline and final style vectors, clamped
/// to [0, 1] and rounded to 4 decimals (the canonical-payload format).
/// Descriptive only — see the interpretation bands, never a verdict.
pub fn style_distance(baseline_text: &str, final_text: &str) -> Option<f32> {
    let a = style_vector(&crate::style::analyze(baseline_text));
    let b = style_vector(&crate::style::analyze(final_text));
    let dot: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return None; // empty or degenerate sample — no honest number available
    }
    let cos = (dot / (na * nb)).clamp(-1.0, 1.0);
    Some(round4((1.0 - cos) as f32))
}

/// Build the StyleConsistency block for a manifest.
/// `baseline_text` is the author's prior writing (pasted reference) or the
/// text of the earliest tracked session; `final_text` is the document's
/// current text.
pub fn compute_style_consistency(
    baseline_text: &str,
    final_text: &str,
    baseline_source: &str,
) -> StyleConsistency {
    match style_distance(baseline_text, final_text) {
        Some(score) => StyleConsistency {
            distance_score: Some(score),
            baseline_source: baseline_source.to_string(),
            metrics_compared: STYLE_METRICS.iter().map(|s| s.to_string()).collect(),
            interpretation: interpret_style_distance(score),
        },
        None => StyleConsistency {
            distance_score: None,
            baseline_source: "unavailable".to_string(),
            metrics_compared: STYLE_METRICS.iter().map(|s| s.to_string()).collect(),
            interpretation:
                "Not enough text to compute a style comparison. This is normal for short documents."
                    .into(),
        },
    }
}

/// A lightweight, transparent citation inventory for the final text.
/// Evidence about structure — not a "citation quality" judgment.
#[derive(Debug, Clone, Serialize)]
pub struct CitationValidation {
    /// Author-year or numeric citations detected in the final text.
    pub citations_detected: usize,
    /// Sessions whose inserted text contained citation-like patterns.
    pub sessions_with_citation_edits: usize,
    /// Approximate citations per sentence in the final text.
    pub citation_density: f64,
    pub note: String,
}

pub fn validate_citations_basic(
    final_text: &str,
    raw_sessions: &[RawSession],
) -> CitationValidation {
    let count = crate::style::count_citations(final_text);
    let sentence_count = crate::style::analyze(final_text).sentence_count.max(1);
    // Which sessions inserted citation-like text? (Raw sessions carry the
    // revision fragments in memory; nothing textual is published.)
    let with_cites = raw_sessions
        .iter()
        .filter(|s| {
            s.revisions.iter().any(|r| {
                r.kind == RevisionKind::Insertion && crate::style::count_citations(&r.text) > 0
            })
        })
        .count();
    CitationValidation {
        citations_detected: count,
        sessions_with_citation_edits: with_cites,
        citation_density: (count as f64 / sentence_count as f64 * 1000.0).round() / 1000.0,
        note: "Counts use a simple author-year/numeric pattern match. They are an inventory, not a validity check; use the Citations tab for source-level validation.".into(),
    }
}

// ---------------------------------------------------------------------------
// Export bundle (.zip) — built in-memory, caller writes to disk
// ---------------------------------------------------------------------------

/// Build the provenance export bundle as a ZIP archive in memory.
/// Contents: manifest.json, disclosure.txt, style_analysis.json,
/// citation_validation.json, README.txt.
pub fn build_export_zip(
    manifest_json: &str,
    disclosure_txt: &str,
    style_analysis_json: &str,
    citation_validation_json: &str,
    readme_txt: &str,
) -> Result<Vec<u8>, ProvenanceError> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut add = |name: &str, content: &str| -> Result<(), ProvenanceError> {
        zip.start_file(name, options)
            .map_err(|e| ProvenanceError::Other(format!("zip write {name}: {e}")))?;
        use std::io::Write as _;
        zip.write_all(content.as_bytes())
            .map_err(|e| ProvenanceError::Other(format!("zip write {name}: {e}")))?;
        Ok(())
    };
    add("manifest.json", manifest_json)?;
    add("disclosure.txt", disclosure_txt)?;
    add("style_analysis.json", style_analysis_json)?;
    add("citation_validation.json", citation_validation_json)?;
    add("README.txt", readme_txt)?;
    let cursor = zip
        .finish()
        .map_err(|e| ProvenanceError::Other(format!("zip finish: {e}")))?;
    Ok(cursor.into_inner())
}

// ---------------------------------------------------------------------------
// Disclosure copy (single source of truth — UI dialog and export both use it)
// ---------------------------------------------------------------------------

pub const DISCLOSURE_DIALOG_TITLE: &str = "Before you turn on Writing Provenance";

pub const DISCLOSURE_DIALOG_BODY: &str = "\
This feature will let you export a cryptographically signed record of the tracked changes in your document: when edits were made, how large they were, and by which author name.

What this does NOT do:

1. It does not tell you whether text was AI-generated. This is not an AI-detection score.
2. It does not prove authorship. It records the revision history your editor kept, nothing more.
3. It only covers edits made while Track Changes was switched on. Untracked edits leave no evidence.

No document content leaves your device. The exported file contains hashes and counts, not text.";

pub const DISCLOSURE_TXT: &str = "\
ScholarScribe Writing Provenance: Disclosure
================================================

What this package is
--------------------
A cryptographically signed summary of the revision history that Microsoft
Word's Track Changes feature recorded in the accompanying document. It
contains, per work session: the author name Word recorded, the start and end
time, how many characters were inserted and deleted, the size of the largest
single insertion, and SHA-256 hashes binding each session to the next.

What this package is NOT
------------------------
1. It is not an AI-detection score. Nothing here tells you whether any text
   was written by a human or by AI.
2. It is not proof of authorship. It is a record of the revision history the
   document carried, nothing more, nothing less.
3. It is not a complete edit history. Only edits made while Track Changes
   was switched on are recorded. Untracked edits leave no evidence.

Privacy
-------
No document content leaves the author's device. This package contains
hashes, counts, timestamps and author display names, never manuscript text.

How to verify
-------------
See README.txt in this package, or docs/PROVENANCE_SPEC.md in the
ScholarScribe repository.

Limitations (read before relying on this)
-----------------------------------------
- The signature proves the package was produced by the holder of the signing
  key and has not been altered since. It does not prove when the underlying
  edits happened; timestamps come from the document's own revision metadata.
- A determined actor with access to the original .docx could in principle
  hand-edit revision XML before analysis. Treat this as one piece of
  evidence alongside drafts, notes and version history, not as a verdict.
- Files exported from other tools (Google Docs, LibreOffice) carry metadata
  with different fidelity; the manifest records the source pipeline used.

Generated by ScholarScribe. Evidence, not verdict.";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rev(author: &str, date: i64, kind: RevisionKind, text: &str) -> RawRevision {
        RawRevision {
            author: author.to_string(),
            date,
            kind,
            text: text.to_string(),
        }
    }

    const T0: i64 = 1_700_000_000; // arbitrary fixed epoch

    // ---- session grouping ----

    #[test]
    fn same_author_within_30min_is_one_session() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "hello "),
            rev("A", T0 + 20 * 60, RevisionKind::Insertion, "world"),
        ]);
        assert_eq!(g.sessions.len(), 1);
        assert_eq!(
            g.sessions[0].chars_added,
            "hello ".chars().count() as u32 + 5
        );
        assert!(g.anomalies.is_empty());
    }

    #[test]
    fn gap_over_30min_starts_new_session() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "one"),
            rev("A", T0 + 31 * 60, RevisionKind::Insertion, "two"),
        ]);
        assert_eq!(g.sessions.len(), 2);
        assert!(g.anomalies.is_empty()); // 31 min < 7 days → no anomaly
    }

    #[test]
    fn gap_over_7_days_flags_anomaly() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "one"),
            rev("A", T0 + 8 * 86_400, RevisionKind::Insertion, "two"),
        ]);
        assert_eq!(g.sessions.len(), 2);
        assert!(g.anomalies.iter().any(|a| a.starts_with("time_gap:")),);
    }

    #[test]
    fn different_authors_never_share_a_session() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "aaa"),
            rev("B", T0 + 10, RevisionKind::Insertion, "bbb"),
            rev("A", T0 + 20, RevisionKind::Insertion, "ccc"),
        ]);
        // A's two edits are within 30 min of each other → they join into ONE
        // session of A's own; B never joins it.
        assert_eq!(g.sessions.len(), 2);
        assert_eq!(g.sessions[0].author, "A");
        assert_eq!(g.sessions[0].chars_added, 6);
        assert_eq!(g.sessions[1].author, "B");
        assert_eq!(g.sessions[1].chars_added, 3);
    }

    #[test]
    fn deletions_track_chars_removed() {
        let g = group_into_sessions(&[rev("A", T0, RevisionKind::Deletion, "xyz")]);
        assert_eq!(g.sessions[0].chars_removed, 3);
        assert_eq!(g.sessions[0].chars_added, 0);
        assert_eq!(g.sessions[0].largest_insertion, 0);
    }

    #[test]
    fn future_timestamp_is_flagged() {
        let far_future = T0 + 100 * 86_400 * 365;
        let g = group_into_sessions(&[rev("A", far_future, RevisionKind::Insertion, "x")]);
        assert!(g
            .anomalies
            .iter()
            .any(|a| a.starts_with("future_timestamp:")));
    }

    // ---- hash chain ----

    #[test]
    fn chain_builds_and_verifies() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "first"),
            rev("A", T0 + 60, RevisionKind::Insertion, " second"),
            rev("A", T0 + 120, RevisionKind::Deletion, "second"),
        ]);
        let records = build_chain(&g.sessions);
        assert_eq!(records.len(), 1); // all within 30 min → one session
        let (ok, anomalies) = verify_chain(&records);
        assert!(ok, "unexpected anomalies: {anomalies:?}");
        assert_eq!(records[0].prev_record_hash, GENESIS_HASH);
    }

    #[test]
    fn chain_links_per_author() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "a1"),
            rev("B", T0 + 60, RevisionKind::Insertion, "b1"),
            rev("A", T0 + 31 * 60, RevisionKind::Insertion, "a2"),
            rev("B", T0 + 32 * 60, RevisionKind::Insertion, "b2"),
        ]);
        let records = build_chain(&g.sessions);
        assert_eq!(records.len(), 4);
        let (ok, anomalies) = verify_chain(&records);
        assert!(ok, "unexpected anomalies: {anomalies:?}");
        // Author A's second record must chain to A's first, not to B's.
        let a_records: Vec<&SessionRecord> = records.iter().filter(|r| r.author == "A").collect();
        let b_records: Vec<&SessionRecord> = records.iter().filter(|r| r.author == "B").collect();
        assert_eq!(a_records[1].prev_record_hash, a_records[0].record_hash);
        assert_eq!(b_records[1].prev_record_hash, b_records[0].record_hash);
        assert_ne!(a_records[0].record_hash, b_records[0].record_hash);
    }

    #[test]
    fn tampered_field_breaks_chain() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "a1"),
            rev("A", T0 + 31 * 60, RevisionKind::Insertion, "a2"),
        ]);
        let mut records = build_chain(&g.sessions);
        // Tamper: inflate chars_added on the first record.
        records[0].chars_added += 1;
        let (ok, anomalies) = verify_chain(&records);
        assert!(!ok);
        assert!(anomalies
            .iter()
            .any(|a| a.starts_with("chain_break: record hash mismatch")));
    }

    #[test]
    fn tampered_prev_hash_breaks_chain() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "a1"),
            rev("A", T0 + 31 * 60, RevisionKind::Insertion, "a2"),
        ]);
        let mut records = build_chain(&g.sessions);
        records[1].prev_record_hash = GENESIS_HASH.to_string(); // claim to be first
        let (ok, _) = verify_chain(&records);
        assert!(!ok);
    }

    #[test]
    fn snapshot_hash_is_deterministic_and_content_sensitive() {
        let s1 = group_into_sessions(&[rev("A", T0, RevisionKind::Insertion, "abc")]).sessions[0]
            .clone();
        let s2 = group_into_sessions(&[rev("A", T0, RevisionKind::Insertion, "abd")]).sessions[0]
            .clone();
        let h1 = compute_snapshot_hash(&s1);
        let h2 = compute_snapshot_hash(&s2);
        assert_eq!(h1, compute_snapshot_hash(&s1));
        assert_ne!(h1, h2);
        assert!(h1.starts_with("sha256:"));
    }

    // ---- signing ----

    #[test]
    fn sign_and_verify_roundtrip() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let g = group_into_sessions(&[rev("A", T0, RevisionKind::Insertion, "text")]);
        let records = build_chain(&g.sessions);
        let manifest = ProvenanceManifest {
            version: MANIFEST_VERSION.to_string(),
            document_hash: "sha256:abc".to_string(),
            author_key_fingerprint: key_fingerprint(&key.verifying_key()),
            sessions: records,
            style_consistency: StyleConsistency {
                distance_score: Some(0.25),
                baseline_source: "user_reference_text".to_string(),
                metrics_compared: STYLE_METRICS.iter().map(|s| s.to_string()).collect(),
                interpretation: "test".to_string(),
            },
            signature: String::new(),
            generated_at: T0,
            generator: "scholarscribe-test".to_string(),
            revision_count: 1,
        };
        let sig = sign_manifest(&manifest, &key);
        assert!(sig.starts_with("ed25519:"));

        let mut signed = manifest.clone();
        signed.signature = sig;
        let ok = verify_manifest_signature(
            &signed,
            &key.verifying_key(),
            &key_fingerprint(&key.verifying_key()),
        )
        .unwrap();
        assert!(ok);
    }

    #[test]
    fn tampered_manifest_fails_signature() {
        let key = SigningKey::from_bytes(&[9u8; 32]);
        let g = group_into_sessions(&[rev("A", T0, RevisionKind::Insertion, "text")]);
        let manifest = ProvenanceManifest {
            version: MANIFEST_VERSION.to_string(),
            document_hash: "sha256:abc".to_string(),
            author_key_fingerprint: key_fingerprint(&key.verifying_key()),
            sessions: build_chain(&g.sessions),
            style_consistency: StyleConsistency {
                distance_score: None,
                baseline_source: "unavailable".to_string(),
                metrics_compared: vec![],
                interpretation: String::new(),
            },
            signature: String::new(),
            generated_at: T0,
            generator: "scholarscribe-test".to_string(),
            revision_count: 1,
        };
        let mut signed = manifest.clone();
        signed.signature = sign_manifest(&manifest, &key);
        // Tamper with a session field after signing.
        signed.sessions[0].chars_added += 100;
        let ok = verify_manifest_signature(
            &signed,
            &key.verifying_key(),
            &key_fingerprint(&key.verifying_key()),
        )
        .unwrap();
        assert!(!ok, "tampered manifest must not verify");
    }

    #[test]
    fn wrong_key_fails_signature() {
        let key = SigningKey::from_bytes(&[1u8; 32]);
        let other = SigningKey::from_bytes(&[2u8; 32]);
        let g = group_into_sessions(&[rev("A", T0, RevisionKind::Insertion, "text")]);
        let manifest = ProvenanceManifest {
            version: MANIFEST_VERSION.to_string(),
            document_hash: "sha256:abc".to_string(),
            author_key_fingerprint: key_fingerprint(&key.verifying_key()),
            sessions: build_chain(&g.sessions),
            style_consistency: StyleConsistency {
                distance_score: None,
                baseline_source: "unavailable".to_string(),
                metrics_compared: vec![],
                interpretation: String::new(),
            },
            signature: String::new(),
            generated_at: T0,
            generator: "g".into(),
            revision_count: 1,
        };
        let mut signed = manifest.clone();
        signed.signature = sign_manifest(&manifest, &key);
        let ok = verify_manifest_signature(
            &signed,
            &other.verifying_key(),
            &key_fingerprint(&other.verifying_key()),
        );
        assert!(ok.is_err()); // fingerprint mismatch → hard error
    }

    // ---- privacy invariants ----

    #[test]
    fn manifest_json_contains_no_document_text() {
        let secret = "CONFIDENTIAL-CLINICAL-DATA-XYZ";
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, secret),
            rev(
                "A",
                T0 + 60,
                RevisionKind::Insertion,
                "more secret prose here",
            ),
        ]);
        let key = SigningKey::from_bytes(&[3u8; 32]);
        let manifest = ProvenanceManifest {
            version: MANIFEST_VERSION.to_string(),
            document_hash: "sha256:abc".to_string(),
            author_key_fingerprint: key_fingerprint(&key.verifying_key()),
            sessions: build_chain(&g.sessions),
            style_consistency: StyleConsistency {
                distance_score: None,
                baseline_source: "unavailable".to_string(),
                metrics_compared: vec![],
                interpretation: String::new(),
            },
            signature: String::new(),
            generated_at: T0,
            generator: "g".into(),
            revision_count: 2,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        assert!(!json.contains(secret), "manifest leaked document text!");
        assert!(
            !json.contains("more secret prose"),
            "manifest leaked document text!"
        );
        // Author names ARE included by design (they are part of the evidence).
        assert!(json.contains("A"));
    }

    // ---- verification summary ----

    #[test]
    fn summarize_reports_stats() {
        let g = group_into_sessions(&[
            rev("A", T0, RevisionKind::Insertion, "short"),
            rev(
                "A",
                T0 + 4 * 3600,
                RevisionKind::Insertion,
                "a much larger insertion here",
            ),
        ]);
        let manifest_sessions = build_chain(&g.sessions);
        let m = ProvenanceManifest {
            version: MANIFEST_VERSION.into(),
            document_hash: "sha256:abc".into(),
            author_key_fingerprint: "ed25519:x".into(),
            sessions: manifest_sessions,
            style_consistency: StyleConsistency {
                distance_score: Some(0.3),
                baseline_source: "first_tracked_session".into(),
                metrics_compared: vec![],
                interpretation: String::new(),
            },
            signature: String::new(),
            generated_at: T0,
            generator: "g".into(),
            revision_count: 2,
        };
        let v = summarize_verification(&m);
        assert!(v.chain_intact);
        assert_eq!(v.session_count, 2);
        assert!(v.time_span_hours >= 4.0);
        assert!(v.largest_insertion_pct > 50.0);
        assert_eq!(v.style_distance_score, Some(0.3));
    }

    // ---- style consistency ----

    #[test]
    fn style_distance_zero_for_identical_text() {
        let text = "Researchers often report mixed findings. The method was robust; however, sample sizes varied. We argue that replication matters (Smith, 2020).";
        let d = style_distance(text, text).unwrap();
        assert!(d < 0.01, "identical text should score ~0, got {d}");
    }

    #[test]
    fn style_distance_none_for_empty_text() {
        assert!(style_distance("", "some text").is_none());
    }

    #[test]
    fn style_consistency_interpretation_bands() {
        assert!(interpret_style_distance(0.1).contains("Very close"));
        assert!(interpret_style_distance(0.3).contains("Broadly consistent"));
        assert!(interpret_style_distance(0.5).contains("Noticeable"));
        assert!(interpret_style_distance(0.9).contains("Substantially"));
    }

    // ---- canonical payload determinism ----

    #[test]
    fn canonical_payload_is_stable_and_readable() {
        let key = SigningKey::from_bytes(&[5u8; 32]);
        let g = group_into_sessions(&[rev("A", T0, RevisionKind::Insertion, "text")]);
        let m = ProvenanceManifest {
            version: MANIFEST_VERSION.into(),
            document_hash: "sha256:abc".into(),
            author_key_fingerprint: key_fingerprint(&key.verifying_key()),
            sessions: build_chain(&g.sessions),
            style_consistency: StyleConsistency {
                distance_score: Some(0.12344),
                baseline_source: "user_reference_text".into(),
                metrics_compared: STYLE_METRICS.iter().map(|s| s.to_string()).collect(),
                interpretation: String::new(),
            },
            signature: String::new(),
            generated_at: T0,
            generator: "g".into(),
            revision_count: 1,
        };
        let p1 = canonical_manifest_payload(&m);
        let p2 = canonical_manifest_payload(&m);
        assert_eq!(p1, p2);
        assert!(p1.starts_with("SCHOLARSCRIBE-MANIFEST-v1\n"));
        // 0.12344 rounds to 4 places — the exact string the JS verifier builds.
        assert!(p1.contains("0.1234\n"));
    }

    // ---- export zip ----

    #[test]
    fn export_zip_contains_all_entries() {
        let bytes = build_export_zip("{}", "d", "{}", "{}", "r").unwrap();
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut names = Vec::new();
        for i in 0..archive.len() {
            names.push(archive.by_index(i).unwrap().name().to_string());
        }
        for expected in [
            "manifest.json",
            "disclosure.txt",
            "style_analysis.json",
            "citation_validation.json",
            "README.txt",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }

    // ---- hex helpers ----

    #[test]
    fn hex_roundtrip() {
        let bytes = vec![0xde, 0xad, 0xbe, 0xef];
        let h = hex(&bytes);
        assert_eq!(h, "deadbeef");
        assert_eq!(decode_hex(&h).unwrap(), bytes);
    }
}
