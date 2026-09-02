//! Writing Provenance — Tauri command layer (Phase 1).
//!
//! Wraps the pure `provenance` core with:
//! - OS-keychain key management (signing key never leaves the keychain
//!   except as a derived public key the user explicitly exports),
//! - the privacy audit log (every file read is recorded; Phase 1 performs
//!   ZERO outbound HTTP calls — if that ever changes, it must be reflected
//!   in the audit trail and SECURITY.md),
//! - the opt-in gate (the whole feature is off until the user accepts the
//!   disclosure dialog, mirroring the persistence design).
//!
//! ETHICAL SCOPE: evidence, not verdict (see provenance.rs header).

use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::audit::AuditLog;
use crate::docx_reading;
use crate::persistence;
use crate::provenance::{
    self, build_chain, build_export_zip, group_into_sessions, interpret_style_distance,
    key_fingerprint, sign_manifest, verify_chain, verify_manifest_signature, CitationValidation,
    ProvenanceError, ProvenanceManifest, RawSession, SessionRecord, StyleConsistency,
    MANIFEST_VERSION,
};

const KEYCHAIN_SERVICE: &str = "scholarscribe";
const KEYCHAIN_USER: &str = "provenance-signing-key-v1";
const GENERATOR: &str = concat!("scholarscribe-", env!("CARGO_PKG_VERSION"));

// ---------------------------------------------------------------------------
// Signing key (OS keychain)
// ---------------------------------------------------------------------------

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(|e| {
        format!(
            "OS keychain is not available ({e}). Writing Provenance needs it to keep your \
             signing key safe. On Linux, install a Secret Service provider \
             (e.g. gnome-keyring / KWallet) and try again."
        )
    })
}

/// Load the signing key from the keychain, creating it on first use.
fn load_or_create_signing_key() -> Result<SigningKey, String> {
    let entry = keyring_entry()?;
    match entry.get_password() {
        Ok(seed_hex) => {
            let seed = decode_hex32(&seed_hex)?;
            Ok(SigningKey::from_bytes(&seed))
        }
        Err(keyring::Error::NoEntry) => {
            // First use: generate a fresh key and store the seed.
            let mut seed = [0u8; 32];
            getrandom::getrandom(&mut seed)
                .map_err(|e| format!("could not generate signing key: {e}"))?;
            let key = SigningKey::from_bytes(&seed);
            entry
                .set_password(&hex_str(&seed))
                .map_err(|e| format!("could not store signing key in OS keychain: {e}"))?;
            seed.fill(0); // wipe the stack copy
            Ok(key)
        }
        Err(e) => Err(format!("could not read signing key from OS keychain: {e}")),
    }
}

/// True when the keychain already holds a signing key.
#[allow(dead_code)]
fn signing_key_exists() -> bool {
    match keyring_entry() {
        Ok(entry) => entry.get_password().is_ok(),
        Err(_) => false,
    }
}

fn hex_str(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn decode_hex32(s: &str) -> Result<[u8; 32], String> {
    if s.len() % 2 != 0 {
        return Err("odd-length hex".into());
    }
    let b = s.as_bytes();
    if b.len() != 64 {
        return Err(format!("expected 64 hex chars (32 bytes), got {}", b.len()));
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        let hi = (b[i * 2] as char).to_digit(16).ok_or("bad hex digit")?;
        let lo = (b[i * 2 + 1] as char).to_digit(16).ok_or("bad hex digit")?;
        *o = (hi * 16 + lo) as u8;
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct ProvenanceKeyStatus {
    pub has_key: bool,
    pub fingerprint: Option<String>,
    pub keyring_available: bool,
    pub note: String,
}

#[tauri::command]
pub fn provenance_key_status() -> ProvenanceKeyStatus {
    match keyring_entry() {
        Ok(entry) => match entry.get_password() {
            Ok(seed_hex) => match decode_hex32(&seed_hex) {
                Ok(seed) => {
                    let key = SigningKey::from_bytes(&seed);
                    ProvenanceKeyStatus {
                        has_key: true,
                        fingerprint: Some(key_fingerprint(&key.verifying_key())),
                        keyring_available: true,
                        note: "Signing key lives in your OS keychain and never leaves this device.".into(),
                    }
                }
                Err(_) => ProvenanceKeyStatus {
                    has_key: false,
                    fingerprint: None,
                    keyring_available: true,
                    note: "Keychain entry exists but is unreadable; re-export will create a new key. Any manifest signed with the old key will need its old key to verify.".into(),
                },
            },
            Err(keyring::Error::NoEntry) => ProvenanceKeyStatus {
                has_key: false,
                fingerprint: None,
                keyring_available: true,
                note: "No signing key yet. One will be created in your OS keychain the first time you export a provenance package.".into(),
            },
            Err(_) => ProvenanceKeyStatus {
                has_key: false,
                fingerprint: None,
                keyring_available: false,
                note: "OS keychain unavailable. On Linux, install gnome-keyring or KWallet.".into(),
            },
        },
        Err(_) => ProvenanceKeyStatus {
            has_key: false,
            fingerprint: None,
            keyring_available: false,
            note: "OS keychain unavailable. On Linux, install gnome-keyring or KWallet.".into(),
        },
    }
}

#[derive(Debug, Serialize)]
pub struct PublicKeyExport {
    pub fingerprint: String,
    pub public_key: String,
    pub written_to: String,
}

/// Export the PUBLIC key so reviewers can verify manifests without the app.
/// The private key never leaves the keychain.
#[tauri::command]
pub fn provenance_export_public_key(
    app: AppHandle,
    output_path: String,
) -> Result<PublicKeyExport, String> {
    require_provenance_enabled(&app)?;
    let key = load_or_create_signing_key()?;
    let vk: VerifyingKey = key.verifying_key();
    let payload = serde_json::json!({
        "type": "scholarscribe-signing-key",
        "algorithm": "ed25519",
        "fingerprint": key_fingerprint(&vk),
        "public_key": format!("ed25519-pub:{}", hex_str(&vk.to_bytes())),
        "note": "Public key for verifying ScholarScribe Writing Provenance manifests. The matching private key stays in the author's OS keychain.",
    });
    let json = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, json).map_err(|e| format!("write public key file: {e}"))?;
    Ok(PublicKeyExport {
        fingerprint: key_fingerprint(&vk),
        public_key: format!("ed25519-pub:{}", hex_str(&vk.to_bytes())),
        written_to: output_path,
    })
}

// ---------------------------------------------------------------------------
// Opt-in gate
// ---------------------------------------------------------------------------

pub(crate) fn require_provenance_enabled(app: &AppHandle) -> Result<(), String> {
    let settings = persistence::settings_get(app.clone())?;
    if settings.provenance_enabled {
        Ok(())
    } else {
        Err(
            "Writing Provenance is switched off. Turn it on in the Provenance tab first; \
             it is opt-in by design."
                .into(),
        )
    }
}

// ---------------------------------------------------------------------------
// Analysis (no key operations, no export)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ProvenanceAnalysis {
    pub document_hash: String,
    pub revision_count: u32,
    pub skipped_unparsable_dates: u32,
    pub sessions: Vec<SessionRecord>,
    pub anomalies: Vec<String>,
    pub authors: Vec<String>,
    pub total_chars_added: u32,
    pub total_chars_removed: u32,
    pub time_span_hours: f64,
    pub largest_insertion_pct: f64,
    pub has_track_changes_markup: bool,
    pub note: String,
}

fn analysis_note() -> String {
    "This is a record of process: when edits happened and how large they were. \
     It is not an AI-detection score and not proof of authorship."
        .into()
}

#[tauri::command]
pub fn provenance_analyze_docx(
    app: AppHandle,
    audit: State<'_, AuditLog>,
    path: String,
) -> Result<ProvenanceAnalysis, String> {
    require_provenance_enabled(&app)?;
    let p = std::path::PathBuf::from(&path);
    let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);

    let extraction =
        docx_reading::extract_track_changes(&p).map_err(provenance_error_to_string_with_hint)?;
    audit.record(
        "file_read",
        &path,
        "provenance: read .docx revision history",
        bytes,
        0,
    );

    if extraction.revisions.is_empty() {
        return Err(if extraction.has_track_changes_markup {
            "Track Changes is on, but no revisions were recorded. The document may have been \
             re-saved with all revisions accepted or rejected."
                .into()
        } else {
            "No tracked changes found in this document. Provenance needs an edit history. \
             Make sure Track Changes was switched on in Word while the document was edited."
                .into()
        });
    }

    let grouping = group_into_sessions(&extraction.revisions);
    let records = build_chain(&grouping.sessions);
    let authors: Vec<String> = {
        let mut a: Vec<String> = records.iter().map(|r| r.author.clone()).collect();
        a.sort();
        a.dedup();
        a
    };
    let total_added: u32 = records.iter().map(|r| r.chars_added).sum();
    let total_removed: u32 = records.iter().map(|r| r.chars_removed).sum();
    let largest: u32 = records
        .iter()
        .map(|r| r.largest_insertion)
        .max()
        .unwrap_or(0);
    let span_hours = if records.is_empty() {
        0.0
    } else {
        let min = records.iter().map(|r| r.start_time).min().unwrap_or(0);
        let max = records.iter().map(|r| r.end_time).max().unwrap_or(0);
        (max - min).max(0) as f64 / 3600.0
    };

    Ok(ProvenanceAnalysis {
        document_hash: extraction.document_xml_sha256,
        revision_count: extraction.revisions.len() as u32,
        skipped_unparsable_dates: extraction.skipped_unparsable_dates as u32,
        sessions: records,
        anomalies: grouping.anomalies,
        authors,
        total_chars_added: total_added,
        total_chars_removed: total_removed,
        time_span_hours: (span_hours * 10.0).round() / 10.0,
        largest_insertion_pct: if total_added == 0 {
            0.0
        } else {
            ((largest as f64 / total_added as f64) * 1000.0).round() / 10.0
        },
        has_track_changes_markup: extraction.has_track_changes_markup,
        note: analysis_note(),
    })
}

fn provenance_error_to_string_with_hint(e: ProvenanceError) -> String {
    match &e {
        ProvenanceError::NoTrackChanges => e.to_string(),
        ProvenanceError::NoRevisions => e.to_string(),
        ProvenanceError::CorruptOoxml(_) => e.to_string(),
        ProvenanceError::Io(_) => e.to_string(),
        ProvenanceError::Other(_) => e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Export (.zip bundle, signed)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ProvenanceExportResult {
    pub output_path: String,
    pub manifest_fingerprint: String,
    pub session_count: usize,
    pub signature_valid: bool,
    pub chain_intact: bool,
    pub anomalies: Vec<String>,
    pub style_distance_score: Option<f32>,
    pub largest_insertion_pct: f64,
    pub time_span_hours: f64,
}

#[tauri::command]
pub fn provenance_export_zip(
    app: AppHandle,
    audit: State<'_, AuditLog>,
    path: String,
    output_path: String,
    baseline_text: Option<String>,
) -> Result<ProvenanceExportResult, String> {
    require_provenance_enabled(&app)?;
    let p = std::path::PathBuf::from(&path);
    let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);

    // 1. Parse + group + chain (all local, all audited).
    let extraction =
        docx_reading::extract_track_changes(&p).map_err(provenance_error_to_string_with_hint)?;
    audit.record(
        "file_read",
        &path,
        "provenance: read .docx revision history for export",
        bytes,
        0,
    );
    if extraction.revisions.is_empty() {
        return Err(if extraction.has_track_changes_markup {
            "Track Changes is on, but no revisions were recorded; nothing to export.".into()
        } else {
            "No tracked changes found in this document; nothing to export.".into()
        });
    }
    let grouping = group_into_sessions(&extraction.revisions);

    // 2-5. Shared pipeline (style metrics, key, manifest, signature, zip).
    let final_text = docx_reading::extract_text_from_docx(&p)
        .map_err(|e| format!("could not extract document text: {e}"))?;
    finalize_export(
        audit.inner(),
        &grouping.sessions,
        extraction.revisions.len() as u32,
        extraction.document_xml_sha256,
        &final_text,
        baseline_text,
        &output_path,
        "word_docx",
    )
    .map(|mut r| {
        r.anomalies.extend(grouping.anomalies);
        r
    })
}

/// Shared export pipeline for both sources (.docx and Google Docs).
/// Steps: style/citation metrics → signing key → manifest → signature →
/// self-verification → zip bundle → audit entry. Returns the summary.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_export(
    audit: &AuditLog,
    raw_sessions: &[RawSession],
    revision_count: u32,
    document_hash: String,
    final_text: &str,
    baseline_text: Option<String>,
    output_path: &str,
    source: &str,
) -> Result<ProvenanceExportResult, String> {
    // 2. Style consistency + citation inventory (aggregate metrics only).
    let (baseline, baseline_source): (String, String) = match baseline_text {
        Some(t) if t.trim().chars().count() >= 40 => (t, "user_reference_text".to_string()),
        _ => {
            // Earliest tracked session's insertions as the fallback baseline.
            let earliest: Option<&RawSession> = raw_sessions.iter().min_by_key(|s| s.start_time);
            match earliest {
                Some(s) if !s.revisions.is_empty() => (
                    s.revisions
                        .iter()
                        .map(|r| r.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    "first_tracked_session".to_string(),
                ),
                _ => (String::new(), "unavailable".to_string()),
            }
        }
    };
    let style_consistency: StyleConsistency = if baseline.trim().is_empty() {
        StyleConsistency {
            distance_score: None,
            baseline_source: "unavailable".into(),
            metrics_compared: provenance::STYLE_METRICS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            interpretation: "Not enough text to compute a style comparison.".into(),
        }
    } else {
        provenance::compute_style_consistency(&baseline, final_text, &baseline_source)
    };
    let citation_validation: CitationValidation =
        provenance::validate_citations_basic(final_text, raw_sessions);

    // 3. Key + manifest + signature.
    let key = load_or_create_signing_key()?;
    let fingerprint = key_fingerprint(&key.verifying_key());
    let records = build_chain(raw_sessions);
    let mut manifest = ProvenanceManifest {
        version: MANIFEST_VERSION.to_string(),
        document_hash,
        author_key_fingerprint: fingerprint.clone(),
        sessions: records,
        style_consistency,
        signature: String::new(),
        generated_at: now_unix(),
        generator: GENERATOR.to_string(),
        revision_count,
    };
    manifest.signature = sign_manifest(&manifest, &key);

    // 4. Self-check BEFORE handing the file to the user: chain + signature.
    let (chain_intact, _chain_anomalies) = verify_chain(&manifest.sessions);
    let signature_valid =
        verify_manifest_signature(&manifest, &key.verifying_key(), &fingerprint).unwrap_or(false);
    if !chain_intact || !signature_valid {
        return Err(
            "Internal error: the manifest failed its own verification and was not exported. \
             Nothing was written."
                .into(),
        );
    }

    // 5. Bundle + write.
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    let style_json = serde_json::to_string_pretty(
        &serde_json::json!({
            "privacy_notice": "Aggregate stylometric metrics only. No document text is included.",
            "source": source,
            "style_consistency": manifest.style_consistency,
            "interpretation_bands": {
                "0.0-0.2": "very close to baseline",
                "0.2-0.4": "broadly consistent",
                "0.4-0.6": "noticeable differences",
                "0.6-1.0": "substantially different"
            },
            "caveat": "Descriptive statistics for the author's own review. This is not an AI-detection score."
        }),
    )
    .map_err(|e| e.to_string())?;
    let citation_json =
        serde_json::to_string_pretty(&citation_validation).map_err(|e| e.to_string())?;
    let readme = build_readme(&fingerprint, manifest.sessions.len(), source);
    let zip_bytes = build_export_zip(
        &manifest_json,
        provenance::DISCLOSURE_TXT,
        &style_json,
        &citation_json,
        &readme,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(output_path, &zip_bytes).map_err(|e| format!("write export file: {e}"))?;
    audit.record(
        "file_write",
        output_path,
        "provenance: wrote signed export bundle (hashes only, no document text)",
        0,
        zip_bytes.len() as u64,
    );

    let summary = provenance::summarize_verification(&manifest);
    Ok(ProvenanceExportResult {
        output_path: output_path.to_string(),
        manifest_fingerprint: fingerprint,
        session_count: summary.session_count,
        signature_valid: true,
        chain_intact: true,
        anomalies: summary.anomalies,
        style_distance_score: summary.style_distance_score,
        largest_insertion_pct: summary.largest_insertion_pct,
        time_span_hours: summary.time_span_hours,
    })
}

fn build_readme(fingerprint: &str, session_count: usize, source: &str) -> String {
    let source_line = match source {
        "word_docx" => "Source: Microsoft Word Track Changes history (.docx).",
        "google_docs" => "Source: Google Drive revision history (read-only import).",
        other => return format!("Source: {other}"),
    };
    format!(
        "\
ScholarScribe Writing Provenance: Export Package
===================================================

Files in this package
---------------------
manifest.json             Signed record of the document's tracked-change
                          sessions (hashes + counts, no document text).
disclosure.txt            What this package is and is not. Read first.
style_analysis.json       Aggregate style-consistency metrics (descriptive).
citation_validation.json  Inventory of citation-like patterns (descriptive).
README.txt                This file.

{source_line}

How to verify
-------------
1. Open verifier/index.html from the ScholarScribe repository in any
   modern browser (works offline; double-click the file).
2. Load this .zip and, for a .docx source, optionally the original
   document so the verifier can bind it to the manifest.
3. The verifier re-derives every hash and checks the Ed25519 signature.

Signing key fingerprint
-----------------------
{fingerprint}

Sessions recorded: {session_count}

Before you rely on this package
-------------------------------
The signature proves the package has not changed since it was produced by
the holder of the signing key. It does NOT prove the text is human-written,
does NOT prove authorship, and only covers edits made while history
tracking was enabled (Word Track Changes, or Google Docs version history).
See disclosure.txt for the full limitations.

Generated by ScholarScribe ({GENERATOR}). Evidence, not verdict.
"
    )
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn provenance_interpret_score(score: f32) -> String {
    interpret_style_distance(score)
}

/// Single source of truth for the opt-in disclosure dialog: the frontend
/// renders exactly what the backend enforces in DISCLOSURE_TXT.
#[derive(Debug, Serialize)]
pub struct DisclosureCopy {
    pub title: String,
    pub body: String,
}

#[tauri::command]
pub fn provenance_disclosure_text() -> DisclosureCopy {
    DisclosureCopy {
        title: provenance::DISCLOSURE_DIALOG_TITLE.to_string(),
        body: provenance::DISCLOSURE_DIALOG_BODY.to_string(),
    }
}
