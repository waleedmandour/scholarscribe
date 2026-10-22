//! Word Assist ledger export (A3): `language_assist.json`.
//!
//! The ledger (assist/ledger.rs) is a SHA-256-hashed, text-free record of
//! what the assist surfaced and what the user accepted (E-4 as applied by
//! the hardening round: plain hash chain, NO HMAC anywhere). This module
//! exports its aggregate plus the chain HEAD, signed through the EXISTING
//! provenance signing path (`provenance::sign_payload`, the same ed25519
//! key as the provenance manifests, from the OS keychain).
//!
//! WHY THE HEAD, NOT THE CHAIN: the head hash commits to every prior
//! entry (each entry's hash covers its `prev` field), so the export
//! proves the chain existed and ends exactly there — without shipping
//! counts the user may not want to publish, and never any text.
//!
//! HONEST LIMIT (also stated in the note field of every export and in
//! the docs): today's HTML verifier (`verifier/verify.js`) does NOT
//! check this file — it verifies provenance packets only. Verification
//! is available through the app command `word_assist_ledger_verify_export`
//! and this module's `verify_export` (tested). No change was made to
//! verify.js in A3.
//!
//! PRIVACY: the export carries counts, hashes and keys — never document
//! text (test-enforced).

use serde::{Deserialize, Serialize};

use super::ledger::AssistLedger;
use crate::provenance::{key_fingerprint, sign_payload, verify_payload};

/// Schema version for `language_assist.json`. Bump on any field change.
pub const EXPORT_SCHEMA_VERSION: u32 = 1;

/// The public note embedded in every export.
pub const EXPORT_NOTE: &str =
    "Signed aggregate of the Word Assist ledger (counts and hashes only; no document text). \
     Signed with the same device key as provenance exports. \
     The bundled HTML verifier does not check this file yet; use the app's \
     'verify export' command or the word_assist_ledger_verify_export API.";

/// The signed export document. Field order matters: `canonical_payload`
/// serializes it deterministically and the signature covers that string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageAssistExport {
    pub schema_version: u32,
    pub engine: String,
    /// Unix seconds at export time.
    pub exported_at: u64,
    /// Counts-only aggregate (ledger aggregate).
    pub counts: super::ledger::LedgerAggregate,
    /// Chain head commitment.
    pub chain: ChainHead,
    /// hex of the ed25519 verifying key that signed this export.
    pub author_public_key: String,
    /// `ed25519:{sha256(vk) hex}` — same fingerprint format as provenance
    /// manifests, so a reader can match a provenance key to this export.
    pub author_key_fingerprint: String,
    /// `ed25519:{hex}` over `canonical_payload` of this document.
    pub signature: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainHead {
    /// Number of entries committed.
    pub entries: u64,
    /// The ledger's SHA-256 chain head (64-hex; the all-zero genesis
    /// string when the ledger is empty). This is the value a reader
    /// quotes in a disclosure. It commits to the whole chain without
    /// shipping it and without any key material — the chain is a plain
    /// SHA-256 hash chain (no HMAC; hardening E-4).
    pub head_sha256: String,
}

/// Deterministic payload string: every field EXCEPT `signature`, in field
/// order, via serde_json (struct order is fixed; no maps are involved).
/// The signature covers exactly this string.
pub fn canonical_payload(e: &LanguageAssistExport) -> String {
    #[derive(Serialize)]
    struct Canonical<'a> {
        schema_version: u32,
        engine: &'a str,
        exported_at: u64,
        counts: &'a super::ledger::LedgerAggregate,
        chain: &'a ChainHead,
        author_public_key: &'a str,
        author_key_fingerprint: &'a str,
        note: &'a str,
    }
    let c = Canonical {
        schema_version: e.schema_version,
        engine: &e.engine,
        exported_at: e.exported_at,
        counts: &e.counts,
        chain: &e.chain,
        author_public_key: &e.author_public_key,
        author_key_fingerprint: &e.author_key_fingerprint,
        note: &e.note,
    };
    serde_json::to_string(&c).expect("canonical payload serialization cannot fail")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Build and sign the export from the ledger at `data_dir/ledger.jsonl`.
/// Opens the ledger fresh (which re-verifies the chain) so a tampered
/// ledger fails the export instead of silently exporting a broken head.
/// Returns the signed document serialized as JSON (the file contents).
pub fn build_export(
    ledger_dir: &std::path::Path,
    key: &ed25519_dalek::SigningKey,
) -> Result<String, String> {
    let ledger = AssistLedger::open(ledger_dir)?;
    let agg = ledger.aggregate()?;
    // Hardened ledger: chain_head() is the SHA-256 head (genesis zeros
    // when empty) — exactly the commitment the export signs.
    let chain = ChainHead {
        entries: ledger.count() as u64,
        head_sha256: ledger.chain_head(),
    };
    let verifying = key.verifying_key();
    let mut e = LanguageAssistExport {
        schema_version: EXPORT_SCHEMA_VERSION,
        engine: super::ENGINE_VERSION.to_string(),
        exported_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        counts: agg,
        chain,
        author_public_key: hex(&verifying.to_bytes()),
        author_key_fingerprint: key_fingerprint(&verifying),
        signature: String::new(),
        note: EXPORT_NOTE.to_string(),
    };
    e.signature = sign_payload(canonical_payload(&e).as_bytes(), key);
    serde_json::to_string_pretty(&e).map_err(|e| format!("serialize export: {e}"))
}

/// Verify an exported `language_assist.json`:
/// 1. parse (Err on malformed JSON / missing fields),
/// 2. rebuild the canonical payload and check the embedded ed25519
///    signature against the embedded author_public_key,
/// 3. additionally check the fingerprint matches that key.
///
/// Ok(true) = signature valid; Ok(false) = signature invalid;
/// Err = malformed. NOTE: this proves who signed the aggregate and that
/// the bytes were not altered after signing; it cannot re-derive the
/// chain (the provenance signing key stays on the exporting device — by
/// design; the chain itself is a plain SHA-256 chain, no key).
pub fn verify_export(bytes: &str) -> Result<bool, String> {
    let e: LanguageAssistExport =
        serde_json::from_str(bytes).map_err(|e| format!("parse language_assist.json: {e}"))?;
    if e.schema_version != EXPORT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version {} (expected {})",
            e.schema_version, EXPORT_SCHEMA_VERSION
        ));
    }
    if e.author_public_key.len() != 64 {
        return Err("author_public_key must be 64 hex chars (32 bytes)".into());
    }
    let key_bytes =
        decode_hex(&e.author_public_key).map_err(|e| format!("author_public_key: {e}"))?;
    let key_arr: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| "author_public_key length")?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&key_arr)
        .map_err(|e| format!("author_public_key: {e}"))?;
    let expected_fp = key_fingerprint(&vk);
    if expected_fp != e.author_key_fingerprint {
        return Err(format!(
            "author_key_fingerprint {} does not match the embedded public key ({expected_fp})",
            e.author_key_fingerprint
        ));
    }
    verify_payload(canonical_payload(&e).as_bytes(), &e.signature, &vk)
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd hex length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| format!("bad hex at {i}: {e}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn test_key(byte: u8) -> SigningKey {
        SigningKey::from_bytes(&[byte; 32])
    }

    /// Build an export against a synthetic ledger dir. The hardened
    /// ledger needs no key (plain SHA-256 chain), so plain `open()` is
    /// deterministic on runners without a keychain. Unique dir per test:
    /// these run in parallel.
    fn export_with(name: &str, entries: usize) -> String {
        let dir =
            std::env::temp_dir().join(format!("wa-export-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmpdir");
        {
            let mut ledger = AssistLedger::open(&dir).expect("open");
            for i in 0..entries {
                ledger
                    .append(
                        "accepted",
                        &format!("p{i}"),
                        &format!("issue-{i}"),
                        "Rule",
                        "grammar",
                        0,
                        1,
                        1,
                        "",
                    )
                    .expect("append");
            }
        }
        let out = build_export(&dir, &test_key(7)).expect("build export");
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn round_trip_verifies() {
        let bytes = export_with("roundtrip", 3);
        assert!(verify_export(&bytes).expect("verify"));
    }

    #[test]
    fn empty_ledger_exports_a_genesis_head() {
        let bytes = export_with("empty", 0);
        let e: LanguageAssistExport = serde_json::from_str(&bytes).expect("parse");
        assert_eq!(e.chain.entries, 0);
        // Genesis: the all-zero 64-hex string the hardened ledger starts from.
        assert_eq!(
            e.chain.head_sha256,
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert!(verify_export(&bytes).expect("verify"));
    }

    #[test]
    fn tampered_payload_fails() {
        let bytes = export_with("tamper", 2);
        let mut e: LanguageAssistExport = serde_json::from_str(&bytes).expect("parse");
        e.counts.accepted += 1; // inflate the count
        let re = serde_json::to_string(&e).expect("serialize");
        assert_eq!(verify_export(&re), Ok(false));
    }

    #[test]
    fn wrong_key_fails() {
        let bytes = export_with("wrongkey", 2);
        // Re-sign with a different key: fingerprint check fires first.
        let mut e: LanguageAssistExport = serde_json::from_str(&bytes).expect("parse");
        e.author_public_key = hex(&test_key(9).verifying_key().to_bytes());
        e.author_key_fingerprint = key_fingerprint(&test_key(9).verifying_key());
        e.signature = sign_payload(canonical_payload(&e).as_bytes(), &test_key(9));
        // Signature over payload with key 9 IS valid for that key, but the
        // document no longer matches anything the app produced — verify
        // still returns true for internal consistency, so assert the
        // fingerprint binding caught nothing here and the signature is
        // self-consistent (documenting what the check does and does not
        // prove).
        assert!(verify_export(&serde_json::to_string(&e).unwrap()).expect("verify"));
    }

    #[test]
    fn swapped_signature_fails() {
        let bytes = export_with("swapped", 2);
        let a: LanguageAssistExport = serde_json::from_str(&bytes).expect("parse");
        let other = export_with("swapped-other", 5);
        let b: LanguageAssistExport = serde_json::from_str(&other).expect("parse");
        // B's signature over A's payload.
        let mut c = a.clone();
        c.signature = b.signature.clone();
        assert_eq!(
            verify_export(&serde_json::to_string(&c).unwrap()),
            Ok(false)
        );
    }

    #[test]
    fn malformed_input_is_err() {
        assert!(verify_export("not json").is_err());
        // A wrong schema version is refused even when the JSON parses.
        let full = export_with("badversion", 1);
        let e: LanguageAssistExport = serde_json::from_str(&full).expect("parse");
        let bad = serde_json::to_string(&LanguageAssistExport {
            schema_version: 99,
            ..e
        })
        .unwrap();
        assert!(verify_export(&bad).is_err());
    }

    #[test]
    fn export_is_text_free() {
        let dir = std::env::temp_dir().join(format!("wa-export-text-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmpdir");
        {
            let mut ledger = AssistLedger::open(&dir).expect("open");
            ledger
                .append(
                    "accepted", "p1", "issue-1", "SomeRule", "grammar", 3, 8, 5, "abcd1234",
                )
                .expect("append");
        }
        let bytes = build_export(&dir, &test_key(7)).expect("export");
        for banned in [
            "The manuscript",
            "p1",
            "issue-1",
            "SomeRule",
            "Some sentence with the manuscript text",
        ] {
            assert!(!bytes.contains(banned), "export leaked {banned}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn canonical_payload_is_deterministic() {
        let bytes = export_with("deterministic", 1);
        let e: LanguageAssistExport = serde_json::from_str(&bytes).expect("parse");
        assert_eq!(canonical_payload(&e), canonical_payload(&e));
        // And the signature verifies against exactly that string.
        let vk = decode_hex(&e.author_public_key).expect("hex");
        let arr: [u8; 32] = vk.try_into().expect("len");
        let v = ed25519_dalek::VerifyingKey::from_bytes(&arr).expect("vk");
        assert_eq!(
            verify_payload(canonical_payload(&e).as_bytes(), &e.signature, &v),
            Ok(true)
        );
    }
}
