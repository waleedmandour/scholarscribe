//! Cross-language verifier fixture test (work order T1.7, closes D8's missing
//! cross-check). The committed fixture in `fixtures/cross-check/` was produced
//! by `cargo run --example gen_fixture` with a fixed key and fixed content.
//!
//! This test re-verifies it with the Rust library: chain, signature, and
//! byte-stability of the canonical payload. `scripts/cross-verify-fixture.mjs`
//! verifies the SAME files with the same vendored crypto the browser verifier
//! uses; CI runs both. If Rust and JS ever diverge on the canonical payload,
//! one of the two fails.

use scholarscribe_lib::provenance::{canonical_manifest_payload, verify_chain, ProvenanceManifest};

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/cross-check");

#[test]
fn fixture_chain_and_signature_verify() {
    let raw = std::fs::read(format!("{FIXTURE_DIR}/manifest.json")).expect(
        "fixture manifest.json is committed; run cargo run --example gen_fixture to regenerate",
    );
    let manifest: ProvenanceManifest =
        serde_json::from_str(&String::from_utf8(raw).expect("utf8")).expect("valid manifest");

    assert_eq!(manifest.version, "1.0");
    assert!(manifest.author_key_fingerprint.starts_with("ed25519:"));
    assert!(manifest.signature.starts_with("ed25519:"));

    let (chain_ok, anomalies) = verify_chain(&manifest.sessions);
    assert!(chain_ok, "fixture chain failed: {anomalies:?}");
    assert!(anomalies.is_empty(), "unexpected anomalies: {anomalies:?}");

    // Signature check against the fixture public key.
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let pub_hex = std::fs::read_to_string(format!("{FIXTURE_DIR}/public-key.txt"))
        .expect("public-key.txt committed")
        .trim()
        .trim_start_matches("ed25519-pub:")
        .to_string();
    let vk_full = scholarscribe_lib::provenance::decode_hex(&pub_hex).expect("hex key");
    assert_eq!(vk_full.len(), 32);
    let mut vk_bytes = [0u8; 32];
    vk_bytes.copy_from_slice(&vk_full);
    let vk = VerifyingKey::from_bytes(&vk_bytes).expect("valid verifying key");

    let sig_hex = manifest
        .signature
        .trim_start_matches("ed25519:")
        .to_string();
    let sig_bytes = scholarscribe_lib::provenance::decode_hex(&sig_hex).expect("hex sig");
    let sig = Signature::from_slice(&sig_bytes).expect("valid signature");

    let payload = canonical_manifest_payload(&manifest);
    let committed_payload = std::fs::read_to_string(format!("{FIXTURE_DIR}/payload.txt"))
        .expect("payload.txt committed");
    assert_eq!(
        // payload.txt = canonical payload + exactly one newline added by the
        // generator for git-friendliness.
        committed_payload.strip_suffix('\n').expect("payload.txt must end with one newline"),
        payload,
        "canonical payload drifted from the committed fixture; regenerate with cargo run --example gen_fixture and re-run the JS check"
    );

    vk.verify(payload.as_bytes(), &sig)
        .expect("fixture Ed25519 signature must verify against the fixture public key");

    // The fingerprint in the manifest must equal sha256 of the fixture key.
    let fp = scholarscribe_lib::provenance::sha256_hex(&vk_full);
    assert_eq!(
        manifest.author_key_fingerprint,
        format!("ed25519:{fp}"),
        "manifest fingerprint must match the fixture public key"
    );
}

/// Tamper check: flipping any byte of the fixture payload must break the
/// signature (guards against accidentally wiring the verifier to accept
/// anything).
#[test]
fn fixture_signature_rejects_tampered_payload() {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let raw = std::fs::read(format!("{FIXTURE_DIR}/manifest.json")).unwrap();
    let manifest: ProvenanceManifest =
        serde_json::from_str(&String::from_utf8(raw).unwrap()).unwrap();
    let pub_hex = std::fs::read_to_string(format!("{FIXTURE_DIR}/public-key.txt"))
        .unwrap()
        .trim()
        .trim_start_matches("ed25519-pub:")
        .to_string();
    let vk_full = scholarscribe_lib::provenance::decode_hex(&pub_hex).unwrap();
    let mut vk_bytes = [0u8; 32];
    vk_bytes.copy_from_slice(&vk_full);
    let vk = VerifyingKey::from_bytes(&vk_bytes).unwrap();
    let sig_bytes = scholarscribe_lib::provenance::decode_hex(
        manifest.signature.trim_start_matches("ed25519:"),
    )
    .unwrap();
    let sig = Signature::from_slice(&sig_bytes).unwrap();

    let mut payload = canonical_manifest_payload(&manifest).into_bytes();
    let last = payload.len() - 1;
    payload[last] ^= 0x01;
    assert!(
        vk.verify(&payload, &sig).is_err(),
        "tampered payload must not verify"
    );
}
