//! Google Docs Bridge (Phase 1.5) — Tauri command layer.
//!
//! PRIVACY CONTRACT (enforced here, verified by the Privacy Audit tab):
//! - Every outbound HTTP call performed by this module is written to the
//!   audit log with the exact URL. A provenance export that used Google
//!   imports is therefore fully auditable end-to-end.
//! - The OAuth refresh token is stored ONLY in the OS keychain.
//! - Revision text is held in memory for diffing and then dropped; it is
//!   never written to disk and never included in any export.
//!
//! ETHICAL SCOPE: this bridge imports REAL revision history from Google's
//! API. It never synthesizes or edits that history (docs/ETHICS.md §2.3).

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::audit::AuditLog;
use crate::google_docs::{self, GoogleRevision, RevisionMeta};
use crate::google_docs_net as net;
use crate::provenance_commands::{
    finalize_export, require_provenance_enabled, ProvenanceExportResult,
};

const KEYCHAIN_SERVICE: &str = "scholarscribe";
const KEYCHAIN_GOOGLE_USER: &str = "google-refresh-token-v1";
/// Safety cap for very long histories (v1: first 500 revisions).
const MAX_REVISIONS: usize = 500;
const OAUTH_TIMEOUT_SECS: u64 = 300;

// ---------------------------------------------------------------------------
// Status / connect / disconnect
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct GoogleStatus {
    pub connected: bool,
    pub scope: &'static str,
    pub note: String,
}

fn google_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_GOOGLE_USER)
        .map_err(|e| format!("OS keychain unavailable ({e}). Google import needs it to store the refresh token safely."))
}

#[tauri::command]
pub async fn google_status(app: AppHandle) -> Result<GoogleStatus, String> {
    let _ = &app; // reserved for future per-profile state
    let connected = tauri::async_runtime::spawn_blocking(|| {
        google_entry()
            .ok()
            .and_then(|e| e.get_password().ok())
            .map(|t| !t.is_empty())
            .unwrap_or(false)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(GoogleStatus {
        connected,
        scope: google_docs::GOOGLE_SCOPE,
        note: "Read-only access (drive.readonly). The refresh token lives in your OS keychain. Every outbound call appears in the Privacy Audit tab.".into(),
    })
}

#[tauri::command]
pub async fn google_disconnect(app: AppHandle) -> Result<(), String> {
    let _ = &app;
    tauri::async_runtime::spawn_blocking(|| {
        let entry = google_entry()?;
        match entry.get_password() {
            Ok(_) => entry
                .delete_credential()
                .map_err(|e| format!("could not remove Google token from keychain: {e}")),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keychain error: {e}")),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Serialize)]
pub struct GoogleConnectResult {
    pub connected: bool,
    pub note: String,
}

/// Full OAuth 2.0 (installed-app flow, PKCE, loopback redirect):
/// 1. start a one-shot loopback HTTP server on a random port,
/// 2. open the system browser at Google's consent screen,
/// 3. capture the authorization code, exchange it for tokens,
/// 4. store the refresh token in the OS keychain.
/// Every network hop is audited. Requires `client_id` (see UI help text).
#[tauri::command]
pub async fn google_connect(
    app: AppHandle,
    client_id: String,
) -> Result<GoogleConnectResult, String> {
    require_provenance_enabled(&app)?;
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("A Google OAuth Client ID is required. See the help text under the Connect button for 2-minute setup instructions.".into());
    }

    // PKCE (S256) + state.
    let mut verifier_bytes = [0u8; 32];
    getrandom::getrandom(&mut verifier_bytes).map_err(|e| e.to_string())?;
    let code_verifier = b64url(&verifier_bytes);
    let code_challenge = b64url(&{
        let mut h = Sha256::new();
        h.update(code_verifier.as_bytes());
        h.finalize().to_vec()
    });
    let mut state_bytes = [0u8; 16];
    getrandom::getrandom(&mut state_bytes).map_err(|e| e.to_string())?;
    let state = b64url(&state_bytes);

    // Loopback listener on a random free port.
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("could not open loopback listener: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let url = net::consent_url(&client_id, &redirect_uri, &code_challenge, &state);

    // Open the system browser (the webview is NOT used for OAuth — no CSP
    // changes, and the token never enters the JS context).
    {
        use tauri_plugin_shell::ShellExt;
        // shell.open is deprecated in favor of tauri-plugin-opener; the
        // project already ships shell and we avoid adding another plugin.
        #[allow(deprecated)]
        app.shell()
            .open(&url, None)
            .map_err(|e| format!("could not open browser: {e}"))?;
    }

    // Wait for the redirect (blocking work on a dedicated thread).
    let app_for_task = app.clone();
    let state_clone = state.clone();
    let handle = tauri::async_runtime::spawn_blocking(move || {
        wait_for_oauth_code(listener, &state_clone, &app_for_task)
    });
    let auth_code = handle
        .await
        .map_err(|e| format!("OAuth listener task failed: {e}"))??;

    // Exchange the code. AUDIT: outbound call to oauth2.googleapis.com.
    let audit = app.state::<AuditLog>();
    audit.record(
        "http_call",
        net::OAUTH_TOKEN_URL,
        "google_docs: exchange authorization code for tokens (PKCE)",
        0,
        0,
    );
    let tokens = net::exchange_code(&client_id, &auth_code, &code_verifier, &redirect_uri).await?;

    // Store the refresh token (OS keychain only).
    let refresh = tokens
        .refresh_token
        .clone()
        .ok_or("Google did not return a refresh token. Reconnect and make sure 'consent' was requested (this is automatic in ScholarScribe).")?;
    tauri::async_runtime::spawn_blocking(move || {
        let entry = google_entry()?;
        entry
            .set_password(&refresh)
            .map_err(|e| format!("could not store refresh token in keychain: {e}"))
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(GoogleConnectResult {
        connected: true,
        note: "Connected. ScholarScribe can now read the revision history of documents you point it at (read-only).".into(),
    })
}

/// One-shot loopback OAuth receiver. Verifies `state`, returns the code.
fn wait_for_oauth_code(
    listener: TcpListener,
    expected_state: &str,
    app: &AppHandle,
) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(OAUTH_TIMEOUT_SECS);
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    loop {
        if Instant::now() > deadline {
            return Err("Timed out waiting for the Google sign-in (5 minutes). Close this window and try again.".into());
        }
        let (mut stream, _) = match listener.accept() {
            Ok(x) => x,
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(format!("loopback listener error: {e}")),
        };
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]).into_owned();
        let query = req
            .split_whitespace()
            .nth(1) // "GET /?code=... HTTP/1.1"
            .and_then(|path| path.split_once('?'))
            .map(|(_, q)| q.to_string())
            .unwrap_or_default();

        let mut code: Option<String> = None;
        let mut got_state: Option<String> = None;
        let mut error: Option<String> = None;
        for kv in query.split('&') {
            let mut it = kv.splitn(2, '=');
            let k = it.next().unwrap_or("");
            let v = it.next().unwrap_or("");
            match k {
                "code" => code = Some(v.to_string()),
                "state" => got_state = Some(v.to_string()),
                "error" => error = Some(v.to_string()),
                _ => {}
            }
        }

        let (ok_html, ok_status) = if error.is_some() {
            (
                "<html><body><h2>Authorization declined.</h2>ScholarScribe did not receive permission. You can close this window.</body></html>",
                "200 OK",
            )
        } else if got_state.as_deref() != Some(expected_state) {
            (
                "<html><body><h2>State mismatch.</h2>ScholarScribe rejected this redirect (possible tampering). You can close this window.</body></html>",
                "400 Bad Request",
            )
        } else if code.is_some() {
            (
                "<html><body><h2>ScholarScribe is connected.</h2>Authorization received. You can close this window and return to the app.</body></html>",
                "200 OK",
            )
        } else {
            ("<html><body>Waiting…</body></html>", "200 OK")
        };
        let _ = stream.write_all(&format!(
            "HTTP/1.1 {ok_status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            ok_html.len(),
            ok_html
        ).into_bytes());
        let _ = stream.flush();

        if let Some(err) = error {
            return Err(format!("Google authorization was declined ({err})."));
        }
        if got_state.as_deref() != Some(expected_state) {
            return Err("OAuth state mismatch — the redirect did not come from the sign-in window we opened.".into());
        }
        if let Some(c) = code {
            let audit = app.state::<AuditLog>();
            audit.record(
                "http_call",
                "http://127.0.0.1 (loopback OAuth redirect, local only)",
                "google_docs: received authorization code on loopback listener",
                0,
                0,
            );
            return Ok(c);
        }
        // Otherwise keep waiting (e.g. favicon request).
    }
}

fn b64url(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[n as usize & 63] as char);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Import + export
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct GoogleImportAnalysis {
    pub file_id: String,
    pub revision_count: u32,
    pub sessions: Vec<crate::provenance::SessionRecord>,
    pub anomalies: Vec<String>,
    pub authors: Vec<String>,
    pub document_hash: String,
    pub total_chars_added: u32,
    pub total_chars_removed: u32,
    pub note: String,
}

/// Fetch revisions for a Google Doc and run the SAME provenance pipeline as
/// the .docx path. Returns the analysis (no key operations, no export).
#[tauri::command]
pub async fn google_import_doc(
    app: AppHandle,
    client_id: String,
    doc_ref: String,
) -> Result<GoogleImportAnalysis, String> {
    require_provenance_enabled(&app)?;
    let file_id = google_docs::parse_google_doc_id(&doc_ref)
        .ok_or("Could not read a document ID from that input. Paste the Google Docs URL or the bare file ID.")?;
    let (revisions, document_hash, rev_meta_count) =
        fetch_all_revisions(&app, &client_id, &file_id).await?;
    let raw = google_docs::revisions_to_raw_revisions(&revisions);
    let grouping = crate::provenance::group_into_sessions(&raw);
    let records = crate::provenance::build_chain(&grouping.sessions);
    let mut authors: Vec<String> = records.iter().map(|r| r.author.clone()).collect();
    authors.sort();
    authors.dedup();
    let total_added: u32 = records.iter().map(|r| r.chars_added).sum();
    let total_removed: u32 = records.iter().map(|r| r.chars_removed).sum();
    Ok(GoogleImportAnalysis {
        file_id,
        revision_count: rev_meta_count as u32,
        sessions: records,
        anomalies: grouping.anomalies,
        authors,
        document_hash,
        total_chars_added: total_added,
        total_chars_removed: total_removed,
        note: "Imported from Google Drive's revision history via the read-only scope. Revision text was diffed in memory and discarded — nothing was stored.".into(),
    })
}

/// Shared fetcher: refresh token → access token → list → download texts.
/// Audits EVERY outbound call. Returns revisions + document hash
/// (sha256 of the final revision text) + count of listed revisions.
async fn fetch_all_revisions(
    app: &AppHandle,
    client_id: &str,
    file_id: &str,
) -> Result<(Vec<GoogleRevision>, String, usize), String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err(
            "A Google OAuth Client ID is required (set it under the Connect button).".into(),
        );
    }

    // 1. Refresh token from keychain.
    let refresh = tauri::async_runtime::spawn_blocking(|| {
        let entry = google_entry()?;
        entry
            .get_password()
            .map_err(|_| "Not connected to Google yet. Click Connect Google Doc first.".to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    // 2. Refresh access token. AUDIT.
    let audit = app.state::<AuditLog>();
    audit.record(
        "http_call",
        net::OAUTH_TOKEN_URL,
        "google_docs: refresh access token",
        0,
        0,
    );
    let tokens = net::refresh_access_token(&client_id, &refresh).await?;

    // 3. List revisions. AUDIT.
    let list_url = format!("{}/files/{}/revisions", net::DRIVE_API_BASE, file_id);
    audit.record("http_call", &list_url, "google_docs: list revisions", 0, 0);
    let metas: Vec<RevisionMeta> = net::list_revisions(&tokens.access_token, file_id).await?;
    let total = metas.len();
    if total == 0 {
        return Err(
            "Google reports no revisions for this document — there is no history to analyze."
                .into(),
        );
    }

    // 4. Download each revision's text. AUDIT each call. Emit progress.
    let mut revisions: Vec<GoogleRevision> = Vec::with_capacity(total.min(MAX_REVISIONS));
    for (i, meta) in metas.iter().enumerate() {
        if i >= MAX_REVISIONS {
            let _ = tauri::Emitter::emit(
                app,
                "provenance://progress",
                serde_json::json!({
                    "stage": "google_revisions_capped",
                    "current": i,
                    "total": total,
                    "detail": format!("history capped at {MAX_REVISIONS} revisions for v1"),
                }),
            );
            break;
        }
        let fetch_url_1 = format!(
            "{}/files/{}/revisions/{}?alt=media",
            net::DRIVE_API_BASE,
            file_id,
            meta.id
        );
        audit.record(
            "http_call",
            &fetch_url_1,
            "google_docs: fetch revision text",
            0,
            0,
        );
        let text = net::fetch_revision_text(&tokens.access_token, file_id, &meta.id).await?;
        let when = google_docs::parse_drive_time(&meta.modified_time)
            .ok_or_else(|| format!("revision {} has an unreadable timestamp", meta.id))?;
        revisions.push(GoogleRevision {
            id: meta.id.clone(),
            modified_time: when,
            text,
            author: meta.author.clone(),
        });
        let _ = tauri::Emitter::emit(
            app,
            "provenance://progress",
            serde_json::json!({
                "stage": "google_revisions",
                "current": i + 1,
                "total": total.min(MAX_REVISIONS),
                "detail": "downloading revision history",
            }),
        );
    }

    // 5. Document hash = sha256 of the final revision's text (the doc's
    // current content as Drive serves it). See PROVENANCE_SPEC.md §3.2.
    let final_text = revisions.last().map(|r| r.text.clone()).unwrap_or_default();
    let document_hash = format!(
        "sha256:{}",
        crate::provenance::sha256_hex(final_text.as_bytes())
    );
    Ok((revisions, document_hash, total))
}

#[tauri::command]
pub async fn google_export_zip(
    app: AppHandle,
    client_id: String,
    doc_ref: String,
    output_path: String,
    baseline_text: Option<String>,
) -> Result<ProvenanceExportResult, String> {
    require_provenance_enabled(&app)?;
    let file_id = google_docs::parse_google_doc_id(&doc_ref)
        .ok_or("Could not read a document ID from that input.")?;
    let (revisions, document_hash, _total) =
        fetch_all_revisions(&app, &client_id, &file_id).await?;
    let raw = google_docs::revisions_to_raw_revisions(&revisions);
    if raw.is_empty() {
        return Err("No usable revision history came back from Google — nothing to export.".into());
    }
    let grouping = crate::provenance::group_into_sessions(&raw);
    let final_text = revisions.last().map(|r| r.text.clone()).unwrap_or_default();
    let audit = app.state::<AuditLog>();
    finalize_export(
        audit.inner(),
        &grouping.sessions,
        raw.len() as u32,
        document_hash,
        &final_text,
        baseline_text,
        &output_path,
        "google_docs",
    )
}
