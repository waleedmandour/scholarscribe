//! Generates the deterministic cross-language verifier fixture (T1.7 / D8).
//!
//! Run from `src-tauri/`:
//!   cargo run --example gen_fixture
//!
//! Writes four files under `src-tauri/fixtures/cross-check/`:
//!   manifest.json   - the signed manifest (fixture content, no document text)
//!   payload.txt     - the canonical signed payload string (SCHOLARSCRIBE-MANIFEST-v1)
//!   public-key.txt  - ed25519-pub:{hex} for the fixture key
//!   meta.json       - how the fixture was produced (for humans)
//!
//! The fixture uses a FIXED seed key and FIXED content so the files are
//! byte-stable across machines. `tests/cross_language.rs` verifies them with
//! the Rust library; `scripts/cross-verify-fixture.mjs` verifies the same
//! files with the same vendored crypto the browser verifier uses. If Rust and
//! JavaScript ever disagree about the canonical payload, this pair fails.

use ed25519_dalek::SigningKey;
use scholarscribe_lib::provenance::{
    build_chain, canonical_manifest_payload, key_fingerprint, sign_manifest, verify_chain,
    verify_manifest_signature, ProvenanceManifest, RawRevision, RawSession, RevisionKind,
    StyleConsistency,
};

fn main() {
    // Fixed 32-byte seed (arbitrary constants; stability matters, secrecy does
    // not: this key signs only public fixture data).
    let seed: [u8; 32] = [
        0x53, 0x43, 0x48, 0x4f, 0x4c, 0x41, 0x52, 0x53, 0x43, 0x52, 0x49, 0x42, 0x45, 0x46, 0x49,
        0x58, 0x54, 0x55, 0x52, 0x45, 0x4b, 0x45, 0x59, 0x31, 0x30, 0x32, 0x34, 0x46, 0x49, 0x58,
        0x54, 0x32,
    ];
    let key = SigningKey::from_bytes(&seed);

    // Two sessions from one author, deterministic content and times.
    let revs = vec![
        RawRevision {
            author: "Fixture Author".into(),
            date: 1_758_000_000,
            kind: RevisionKind::Insertion,
            text: "The introduction states the research question clearly. ".repeat(3),
        },
        RawRevision {
            author: "Fixture Author".into(),
            date: 1_758_000_600,
            kind: RevisionKind::Insertion,
            text: "Methods were preregistered; deviations are reported. ".repeat(2),
        },
        RawRevision {
            author: "Fixture Author".into(),
            date: 1_758_100_000,
            kind: RevisionKind::Deletion,
            text: "Delete this tentative sentence about the sample size.".into(),
        },
    ];
    let session = RawSession {
        author: "Fixture Author".into(),
        start_time: 1_758_000_000,
        end_time: 1_758_100_000,
        chars_added: revs
            .iter()
            .filter(|r| r.kind == RevisionKind::Insertion)
            .map(|r| r.text.chars().count() as u32)
            .sum(),
        chars_removed: revs
            .iter()
            .filter(|r| r.kind == RevisionKind::Deletion)
            .map(|r| r.text.chars().count() as u32)
            .sum(),
        largest_insertion: revs
            .iter()
            .filter(|r| r.kind == RevisionKind::Insertion)
            .map(|r| r.text.chars().count() as u32)
            .max()
            .unwrap_or(0),
        revisions: revs,
    };
    let raw_sessions = vec![session];
    let records = build_chain(&raw_sessions);

    let manifest = ProvenanceManifest {
        version: "1.0".into(),
        document_hash: "sha256:fixturedocumenthash000000000000000000000000000000000000000000000000"
            .into(),
        author_key_fingerprint: key_fingerprint(&key.verifying_key()),
        sessions: records,
        style_consistency: StyleConsistency {
            distance_score: Some(0.1234),
            baseline_source: "user_reference_text".into(),
            metrics_compared: vec!["avg_sentence_length".into(), "hedge_density".into()],
            interpretation: "Fixture interpretation text.".into(),
        },
        signature: String::new(),
        generated_at: 1_758_200_000,
        generator: "scholarscribe-fixture-1.0".into(),
        revision_count: 3,
    };

    let mut signed = manifest;
    signed.signature = sign_manifest(&signed, &key);

    // Self-check before writing anything.
    let (chain_ok, anomalies) = verify_chain(&signed.sessions);
    assert!(chain_ok, "fixture chain broken: {anomalies:?}");
    let sig_ok = verify_manifest_signature(
        &signed,
        &key.verifying_key(),
        &signed.author_key_fingerprint,
    )
    .unwrap_or(false);
    assert!(sig_ok, "fixture signature does not verify");

    let payload = canonical_manifest_payload(&signed);
    let out_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/cross-check");
    std::fs::create_dir_all(out_dir).expect("create fixture dir");
    std::fs::write(
        format!("{out_dir}/manifest.json"),
        serde_json::to_string_pretty(&signed).unwrap(),
    )
    .unwrap();
    // payload.txt carries exactly one trailing newline for git-friendliness;
    // the canonical payload itself is the file content WITHOUT that newline.
    std::fs::write(format!("{out_dir}/payload.txt"), format!("{payload}\n")).unwrap();
    let verifying_key = key.verifying_key();
    let vk_bytes = verifying_key.as_bytes();
    let mut pub_hex = String::from("ed25519-pub:");
    for b in vk_bytes.iter() {
        pub_hex.push_str(&format!("{b:02x}"));
    }
    std::fs::write(format!("{out_dir}/public-key.txt"), format!("{pub_hex}\n")).unwrap();
    std::fs::write(
        format!("{out_dir}/meta.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "generator": "cargo run --example gen_fixture (src-tauri/examples/gen_fixture.rs)",
            "purpose": "Deterministic Rust-vs-JS verifier cross-check fixture (work order T1.7, D8)",
            "key": "fixed 32-byte seed compiled into the example; fixture data is public by construction",
            "manifest_payload_label": "SCHOLARSCRIBE-MANIFEST-v1",
            "sessions": signed.sessions.len(),
            "generated_at": signed.generated_at
        }))
        .unwrap(),
    )
    .unwrap();
    println!("fixture written to {out_dir}");
    println!("fingerprint: {}", signed.author_key_fingerprint);
}
