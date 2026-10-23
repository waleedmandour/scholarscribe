//! Word Assist Tauri command surface (1b).
//!
//! This module is ALWAYS compiled: the `invoke_handler` list is static, so
//! the command paths must resolve in every build. tauri's command macro
//! generates argument glue from the token stream and cannot handle
//! `#[cfg]`-stripped parameters, so every command is defined twice — the
//! real implementation under `feature = "word-assist"` and a minimal stub
//! under `not(...)`. Keep the stub signatures minimal (no State): the
//! frontend never shows the tab when the feature is absent.
//!
//! Lifecycle (ruling 5): nothing here starts a listener until the user
//! opted in through the disclosure dialog, which sets
//! `word_assist_enabled` in settings. `word_assist_enable` re-checks that
//! flag before binding.

use serde::Serialize;
use tauri::AppHandle;

/// Mirrors `crate::assist::ENGINE_VERSION` (kept as a literal because that
/// module is compiled out of non-feature builds; mod.rs's own test pins
/// the real value's shape).
#[cfg(not(feature = "word-assist"))]
const ENGINE_VERSION_STUB: &str = "word-assist/0.1.0";

#[cfg(not(feature = "word-assist"))]
fn feature_absent() -> String {
    "This build of ScholarScribe was compiled without the Word Assist feature.".into()
}

// ------------------------------------------------------------------ DTOs

/// What the UI sees (word_assist_status and /v1/health).
#[derive(Serialize, Clone, Debug)]
pub struct AssistStatus {
    pub running: bool,
    pub port: u16,
    pub paired: bool,
    pub strictness: String,
    pub tier1_enabled: bool,
    pub engine: String,
    /// Present while a pairing code is live (never the code itself).
    pub pairing_active: bool,
    /// False in builds compiled without the feature.
    pub feature_compiled: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct AssistEnableOutcome {
    pub ok: bool,
    pub port: u16,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct AssistAuditEntry {
    pub timestamp: u64,
    pub kind: String,
    pub target: String,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct AssistRegistration {
    pub manifest_url: String,
    pub taskpane_url: String,
    /// H5: the CA certificate — the ONE cert the user trusts. Path + PEM.
    pub cert_path: String,
    pub cert_pem: String,
    /// The leaf (public, informational; never to be trusted directly).
    pub leaf_path: String,
    pub leaf_pem: String,
    pub instructions: Vec<String>,
}

/// Counts-only ledger aggregate + a ready-to-paste disclosure sentence.
#[derive(Serialize, Clone, Debug)]
pub struct AssistDisclosurePrefill {
    pub accepted: u64,
    pub rejected: u64,
    pub sessions: u64,
    pub grammar: u64,
    pub spelling: u64,
    pub clarity: u64,
    pub rewrites: u64,
    pub note: String,
}

// ------------------------------------------------- feature-build helpers

/// Managed handle to the running listener (feature builds only).
#[cfg(feature = "word-assist")]
#[derive(Default)]
pub struct AssistListener {
    handle: std::sync::Mutex<Option<crate::assist::server::ServerHandle>>,
}

#[cfg(feature = "word-assist")]
impl AssistListener {
    fn stop(&self) {
        if let Some(h) = self.handle.lock().expect("listener lock").take() {
            h.stop();
        }
    }
}

/// Resolve the assist data directory (certificate storage).
#[cfg(feature = "word-assist")]
fn assist_data_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot resolve app data dir: {e}"))?;
    Ok(base.join("data").join("word-assist"))
}

// ------------------------------------------------------------- commands

/// Start the listener. Idempotent on the same port.
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_enable(
    app: AppHandle,
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
    listener: tauri::State<'_, AssistListener>,
) -> Result<AssistEnableOutcome, String> {
    let settings_value = persistence::settings_get(app.clone())?;
    if !settings_value.word_assist_enabled {
        return Err(
            "Word Assist is not opted in. Enable it from the Word Assist tab (the disclosure dialog)."
                .into(),
        );
    }
    let settings = crate::assist::settings::AssistSettings::from_persisted(&settings_value)?;

    // Already running? Report and leave it alone (idempotent enable).
    if state.is_running() {
        let port = state
            .bound_port
            .lock()
            .map_err(|e| e.to_string())?
            .unwrap_or(settings.port);
        return Ok(AssistEnableOutcome {
            ok: true,
            port,
            error: None,
        });
    }

    // Session hygiene: a fresh listener starts unpaired.
    *state.settings.lock().map_err(|e| e.to_string())? = Some(settings.clone());

    let dir = assist_data_dir(&app)?;
    // The ledger is a commitment: if it cannot be opened (broken chain),
    // the enable FAILS rather than running unrecorded.
    state.attach_ledger(&dir)?;
    // H2: attach token persistence and restore any saved pairing (an
    // app restart must not force a re-pair). Enable still clears the
    // in-memory session FIRST (clear_session no longer touches the
    // persisted record), so restore runs after it.
    state
        .clear_session()
        .map_err(|e| format!("session reset: {e}"))?;
    state.attach_token_store(&dir)?;
    match crate::assist::server::start(settings, std::sync::Arc::clone(&state), dir).await {
        Ok(handle) => {
            let port = handle.port;
            *listener.handle.lock().expect("listener lock") = Some(handle);
            log::info!("Word Assist enabled on port {port}");
            Ok(AssistEnableOutcome {
                ok: true,
                port,
                error: None,
            })
        }
        Err(e) => {
            // Roll the runtime settings back so /v1/* and the status
            // report the truth (the feature is NOT running).
            *state.settings.lock().map_err(|e| e.to_string())? = None;
            state
                .running
                .store(false, std::sync::atomic::Ordering::SeqCst);
            log::warn!("Word Assist enable failed: {e}");
            Ok(AssistEnableOutcome {
                ok: false,
                port: settings_value.word_assist_port,
                error: Some(e),
            })
        }
    }
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_enable(_app: AppHandle) -> Result<AssistEnableOutcome, String> {
    Err(feature_absent())
}

/// Stop the listener and wipe the session. H2 rotate-on-disable: the
/// persisted token is revoked, so the next session always re-pairs.
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_disable(
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
    listener: tauri::State<'_, AssistListener>,
) -> Result<(), String> {
    listener.stop();
    state.scheduler.abort_in_flight();
    state.cache.clear();
    // H2: rotate on feature disable = the stored token dies here.
    state.revoke_token()?;
    state.clear_session()?;
    *state.settings.lock().map_err(|e| e.to_string())? = None;
    state
        .running
        .store(false, std::sync::atomic::Ordering::SeqCst);
    *state.bound_port.lock().map_err(|e| e.to_string())? = None;
    state.audit.record(
        "assist_listener",
        "127.0.0.1",
        "word assist listener stopped; session wiped; token rotated (revoked)",
        0,
        0,
    );
    Ok(())
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_disable() -> Result<(), String> {
    Err(feature_absent())
}

#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_status(
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
) -> Result<AssistStatus, String> {
    let running = state.is_running();
    let (strictness, tier1_enabled, port) = {
        let guard = state.settings.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(s) => (s.strictness.name().to_string(), s.tier1_enabled, s.port),
            None => (
                "dev".to_string(),
                false,
                state
                    .bound_port
                    .lock()
                    .map_err(|e| e.to_string())?
                    .unwrap_or(38080),
            ),
        }
    };
    let pairing_active = state.pairing.has_live_code();
    Ok(AssistStatus {
        running,
        port,
        paired: state.token.lock().map(|g| g.is_some()).unwrap_or(false),
        strictness,
        tier1_enabled,
        engine: crate::assist::ENGINE_VERSION.to_string(),
        pairing_active,
        feature_compiled: true,
    })
}

/// Stub for builds without the feature: reports the E-1 provisional
/// defaults so the (hidden) tab would render coherent values.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_status() -> Result<AssistStatus, String> {
    Ok(AssistStatus {
        running: false,
        port: 38080,
        paired: false,
        strictness: "dev".into(),
        tier1_enabled: false,
        engine: ENGINE_VERSION_STUB.to_string(),
        pairing_active: false,
        feature_compiled: false,
    })
}

/// Issue a fresh 6-character pairing code (5-minute TTL, one-time use).
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_new_pairing_code(
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
) -> Result<String, String> {
    if !state.is_running() {
        return Err("Word Assist is not running. Enable it first.".into());
    }
    state.new_pairing_code()
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_new_pairing_code() -> Result<String, String> {
    Err(feature_absent())
}

/// The assist-surface audit log (see assist::settings module docs).
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_audit(
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
) -> Result<Vec<AssistAuditEntry>, String> {
    Ok(state
        .audit
        .snapshot()
        .into_iter()
        .map(|e| AssistAuditEntry {
            timestamp: e.timestamp,
            kind: e.kind,
            target: e.target,
            detail: e.detail,
        })
        .collect())
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_audit() -> Result<Vec<AssistAuditEntry>, String> {
    Ok(Vec::new())
}

/// "Unpair everything": revoke the token (memory + persisted record) and
/// drop any live pairing code without stopping the listener (H2 revoke).
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_unpair(
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
) -> Result<(), String> {
    state.revoke_token()?;
    state.clear_session()
}

/// H2 rotate: revoke the current token and hand back a fresh pairing
/// code so the user can re-pair. The token is NEVER pushed — the add-in
/// only ever receives it through pairing — so this is the exact shape a
/// rotation can take without new add-in surfaces.
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_rotate_token(
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
) -> Result<String, String> {
    if !state.is_running() {
        return Err("Word Assist is not running. Enable it first.".into());
    }
    state.rotate_token()?;
    state.clear_session()?;
    state.new_pairing_code()
}

/// Export the assist ledger aggregate + chain head as a signed
/// `language_assist.json` (A3), signed through the existing provenance
/// ed25519 key (OS keychain). Counts, hashes and keys only - never
/// document text. The verifier limitation is stated inside the file
/// itself (EXPORT_NOTE) and in the docs.
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_ledger_export(app: AppHandle, path: String) -> Result<String, String> {
    let dir = assist_data_dir(&app)?;
    let key = crate::provenance_commands::load_or_create_signing_key()?;
    let json = crate::assist::export::build_export(&dir, &key)?;
    let target = std::path::PathBuf::from(&path);
    if target.parent().map(|p| !p.exists()).unwrap_or(false) {
        return Err("target folder does not exist".into());
    }
    std::fs::write(&target, json).map_err(|e| format!("write export: {e}"))?;
    Ok(format!("signed ledger export written: {path}"))
}

/// Verify an exported `language_assist.json` (A3). Returns a human-
/// readable verdict; the structured result is the bool (true = the
/// embedded signature verifies against the embedded key).
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_ledger_verify_export(path: String) -> Result<String, String> {
    let bytes = std::fs::read_to_string(&path).map_err(|e| format!("read export: {e}"))?;
    match crate::assist::export::verify_export(&bytes)? {
        true => Ok(format!(
            "VALID: {path} is signed by the key it embeds (fingerprint matches) and the bytes are unchanged since signing. Note: this does not re-derive the ledger chain; the exporting device holds the provenance signing key."
        )),
        false => Ok(format!(
            "INVALID: {path} failed signature verification (the aggregate was altered after signing, or the signature does not belong to the embedded key)."
        )),
    }
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_ledger_export(_app: AppHandle, _path: String) -> Result<String, String> {
    Err(feature_absent())
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_ledger_verify_export(_path: String) -> Result<String, String> {
    Err(feature_absent())
}

/// Counts-only aggregate of the assist ledger for the Disclosure
/// Assistant pre-fill (Phase 3). Numbers, never text.
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_disclosure_prefill(
    app: AppHandle,
) -> Result<AssistDisclosurePrefill, String> {
    let dir = assist_data_dir(&app)?;
    // Open read-only for the aggregate: this works even when the listener
    // is not running (the ledger file is the user's record, not the
    // session's).
    let ledger = crate::assist::ledger::AssistLedger::open(&dir)?;
    let agg = ledger.aggregate()?;
    let note = crate::disclosure::word_assist_note(
        agg.accepted,
        agg.grammar,
        agg.spelling,
        agg.clarity,
        agg.rewrites,
    );
    Ok(AssistDisclosurePrefill {
        accepted: agg.accepted,
        rejected: agg.rejected,
        sessions: agg.sessions,
        grammar: agg.grammar,
        spelling: agg.spelling,
        clarity: agg.clarity,
        rewrites: agg.rewrites,
        note,
    })
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_disclosure_prefill(
    _app: AppHandle,
) -> Result<AssistDisclosurePrefill, String> {
    Err(feature_absent())
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_unpair() -> Result<(), String> {
    Err(feature_absent())
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_rotate_token() -> Result<String, String> {
    Err(feature_absent())
}

/// Registration info for the manual side-load flow. H5: the user trusts
/// the CA certificate (ephemeral, name-constrained), never the leaf. This
/// command only reads the generated certificates and returns copy/paste
/// instructions; nothing writes a trust store.
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_registration(
    app: AppHandle,
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
) -> Result<AssistRegistration, String> {
    let port = state
        .bound_port
        .lock()
        .map_err(|e| e.to_string())?
        .ok_or("Word Assist is not running. Enable it first.")?;
    let dir = assist_data_dir(&app)?;
    let ca_path = dir.join("word-assist-ca.der");
    let leaf_path = dir.join("word-assist-cert.der");
    let ca_der = std::fs::read(&ca_path)
        .map_err(|e| format!("read assist CA certificate: {e} (enable Word Assist first)"))?;
    let leaf_der = std::fs::read(&leaf_path).unwrap_or_default();
    Ok(AssistRegistration {
        manifest_url: format!("https://localhost:{port}/addin/manifest.xml"),
        taskpane_url: format!("https://localhost:{port}/addin/taskpane.html"),
        cert_path: ca_path.display().to_string(),
        cert_pem: pem_encode(&ca_der),
        leaf_path: leaf_path.display().to_string(),
        leaf_pem: if leaf_der.is_empty() { String::new() } else { pem_encode(&leaf_der) },
        instructions: vec![
            // A4 x H5 merge: the CA-not-leaf framing is hardening's; the
            // per-OS copy/paste commands with uninstall/revoke are Stage A's,
            // re-pointed at the hardened CA (file word-assist-ca.der, subject
            // "ScholarScribe Word Assist Root CA (ephemeral)"). The app only
            // displays these; the USER runs them; nothing writes a store.
            //
            // P6: the Step 1 / uninstall commands are now EMITTED by the
            // per-shell builders below (quote rules unit-tested on spaces,
            // apostrophes, non-ASCII and long paths). The placeholder
            // `<assist-data-dir>` is passed as the path so the displayed
            // command arrives pre-quoted for its shell: the user replaces
            // the placeholder with their real data dir (which the tab shows
            // elsewhere) and the quoting is already correct.
            "Trust the CA certificate ('ScholarScribe Word Assist Root CA (ephemeral)'), NOT the leaf: the CA is name-constrained to localhost / 127.0.0.1 and its signing key was destroyed after issuing your leaf.".into(),
            format!(
                "Step 1 - trust the CA (Windows, cmd.exe): {} (installs into the CURRENT USER Trusted Root Certification Authorities store; the -user flag keeps this to your account; the app does not run this).",
                ca_trust_command_windows(std::path::Path::new("<assist-data-dir>"))
            ),
            format!(
                "Step 1 - trust the CA (macOS, zsh/bash): {} (adds to your login keychain; macOS shows an authentication dialog for per-user trust - that prompt is the OS, not ScholarScribe). CA name constraints against real OS stores are untested until spike (q).",
                ca_trust_command_macos(std::path::Path::new("<assist-data-dir>"))
            ),
            format!(
                "Step 1 (Windows, PowerShell variant): {} (same store as cmd.exe; PowerShell single-quote rules).",
                format!(
                    "certutil -user -addstore Root {}",
                    quote_powershell(r"<assist-data-dir>\word-assist-ca.der")
                )
            ),
            "Step 2 - sideload (both OS): Word > Insert > My Add-ins / Get Add-ins > 'Upload My Add-in' > the manifest URL below.".into(),
            "Step 2 (alt, Windows): save the manifest with 'Export manifest file', share its folder, then Word > File > Options > Trust Center > Trusted Add-in Catalogs > add the network path of that folder, and Insert > My Add-ins > SHARED FOLDER.".into(),
            "Step 2 (alt, macOS): copy the exported manifest to ~/Library/Containers/com.microsoft.Word/Data/Documents/wef (create the folder if needed) and restart Word.".to_string(),
            "Step 3 - pair: open the add-in pane and enter the pairing code shown in this tab.".into(),
            "Renewal: the CA + leaf are regenerated when they expire (CA 3 years, leaf 1 year); re-trust the NEW CA — this re-asks your consent.".into(),
            format!(
                "Uninstall - remove the add-in first (My Add-ins > right-click > Remove, or clear the trusted catalog / delete the manifest from the wef folder), then revoke CA trust (Windows, cmd.exe: {}; macOS, zsh/bash: {}).",
                ca_revoke_command_windows(),
                ca_revoke_command_macos()
            ),
            "Full verified walkthrough with uninstall and revoke for every step: docs/WORD_ASSIST_INSTALL.md.".into(),
        ],
    })
}

/// Export the {PORT}-substituted manifest to a user-chosen file (the
/// Windows shared-folder catalog consumes a manifest FILE; macOS sideload
/// copies the same file). The add-in GUID is stable in the manifest itself
/// (word-addin/dist/manifest.xml <Id>), so every export re-uses one
/// identity - reinstalling or re-exporting never duplicates the add-in.
#[cfg(feature = "word-assist")]
#[tauri::command]
pub async fn word_assist_export_manifest(
    state: tauri::State<'_, std::sync::Arc<crate::assist::settings::AssistState>>,
    path: String,
) -> Result<String, String> {
    let port = state
        .bound_port
        .lock()
        .map_err(|e| e.to_string())?
        .ok_or("Word Assist is not running. Enable it first.")?;
    // Fail fast on a missing parent dir instead of a half-written export.
    let target = std::path::PathBuf::from(&path);
    if target.parent().map(|p| !p.exists()).unwrap_or(false) {
        return Err(format!(
            "target folder does not exist: {}",
            target
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        ));
    }
    let xml =
        include_str!("../../word-addin/dist/manifest.xml").replace("{PORT}", &port.to_string());
    std::fs::write(&target, xml).map_err(|e| format!("write manifest: {e}"))?;
    // P6: attach the generated, correctly-quoted macOS sideload copy command
    // (zsh/bash; $HOME form works for every username shape — spaces,
    // apostrophes, non-ASCII). Windows users use the shared-folder catalog
    // instead (no shell command).
    let mac_sideload = sideload_copy_command_macos(&target);
    let written = format!(
        "manifest written for port {port} (SourceLocation https://localhost:{port}/addin/taskpane.html): {path}\nmacOS sideload (zsh/bash): {mac_sideload}"
    );
    log::info!("{written}");
    Ok(written)
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_registration(_app: AppHandle) -> Result<AssistRegistration, String> {
    Err(feature_absent())
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_export_manifest(_app: AppHandle, _path: String) -> Result<String, String> {
    Err(feature_absent())
}

// ----------------------------------------------------------------- util

/// DER→PEM (base64 at 64 chars per line). A base64 encoder is not a
/// cipher; no new dependency is pulled for this. Only the feature build's
/// registration command calls it.
#[cfg(feature = "word-assist")]
fn pem_encode(der: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    let mut line_len = 0usize;
    for chunk in der.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2])) as usize;
        let chars = [
            TABLE[(n >> 18) & 63],
            TABLE[(n >> 12) & 63],
            if chunk.len() > 1 {
                TABLE[(n >> 6) & 63]
            } else {
                b'='
            },
            if chunk.len() > 2 { TABLE[n & 63] } else { b'=' },
        ];
        for c in chars {
            out.push(c as char);
            line_len += 1;
            if line_len == 64 {
                out.push('\n');
                line_len = 0;
            }
        }
    }
    if line_len != 0 {
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}

// ---------------------------------------------------------------------------
// P6: per-shell, copy-pasteable command builders for the CA trust / revoke /
// sideload steps. Pure functions so the exact quoting a user must type for a
// REAL data dir (spaces, apostrophes, non-ASCII usernames, long paths) is
// generated and unit-tested in one place. The registration UI keeps using the
// PLACEHOLDER text (`<assist-data-dir>`); nothing here writes to any trust
// store — the USER runs these commands.
//
// Shell targets (stated explicitly, as required by the Gate-1 review):
//   * Windows instructions target cmd.exe. (PowerShell would need different
//     quoting; see quote_powershell — provided and tested so a future UI can
//     print a PowerShell variant without re-deriving the rules.)
//   * macOS instructions target zsh/bash (POSIX single-quote rules).
//
// Elevation (verified against vendor docs in docs/ADVISORIES_A6-adjacent
// install notes and re-checked in the P6 report): `certutil -user -addstore`
// writes the CURRENT USER store (no admin); macOS `security add-trusted-cert
// -r trustRoot` without `-d` targets the login keychain (no admin; the OS
// shows its own auth prompt). These builders assert the no-elevation shape.
// ---------------------------------------------------------------------------

/// The ephemeral CA subject (single source of truth; also used by the cert
/// generator in assist::server).
#[cfg(feature = "word-assist")]
pub(crate) const CA_COMMON_NAME: &str = "ScholarScribe Word Assist Root CA (ephemeral)";

/// cmd.exe quoting: wrap in double quotes. Inside double quotes cmd treats
/// `^` and most metacharacters literally for a single file argument; `%VAR%`
/// would still expand (documented in the tests — paths with `%` are out of
/// scope for the trust step because the app renders the real path, which
/// never contains `%` unless the OS did something extraordinary).
#[cfg(feature = "word-assist")]
pub fn quote_windows_cmd(s: &str) -> String {
    format!("\"{s}\"")
}

/// PowerShell single-quoted string: fully literal; double any inner `'`.
#[cfg(feature = "word-assist")]
pub fn quote_powershell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// POSIX (zsh/bash) single-quoted string with the standard `'\''` re-entry
/// trick for inner single quotes.
#[cfg(feature = "word-assist")]
pub fn quote_posix(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Windows CA trust command (cmd.exe). CURRENT USER Trusted Root store — the
/// `-user` flag keeps it to the calling user's profile, so NO elevation.
#[cfg(feature = "word-assist")]
pub fn ca_trust_command_windows(data_dir: &std::path::Path) -> String {
    let ca = data_dir.join("word-assist-ca.der");
    format!(
        "certutil -user -addstore Root {}",
        quote_windows_cmd(&ca.display().to_string())
    )
}

/// macOS CA trust command (zsh/bash). Login keychain (per-user) — no `-d`
/// (that would target the admin domain and require elevation); the OS shows
/// its own authentication prompt for per-user trust-root changes.
#[cfg(feature = "word-assist")]
pub fn ca_trust_command_macos(data_dir: &std::path::Path) -> String {
    let ca = data_dir.join("word-assist-ca.der");
    format!(
        "security add-trusted-cert -r trustRoot {}",
        quote_posix(&ca.display().to_string())
    )
}

/// Windows CA revoke/uninstall command (cmd.exe). CURRENT USER store — no
/// elevation.
#[cfg(feature = "word-assist")]
pub fn ca_revoke_command_windows() -> String {
    format!(
        "certutil -user -delstore Root {}",
        quote_windows_cmd(CA_COMMON_NAME)
    )
}

/// macOS CA revoke/uninstall command (zsh/bash): drop per-user trust then
/// delete the login-keychain certificate. Omitting `-d` on BOTH commands
/// keeps them in the user's login domain — no `-d`, no sudo.
#[cfg(feature = "word-assist")]
pub fn ca_revoke_command_macos() -> String {
    format!(
        "security remove-trusted-cert {} && security delete-certificate -c {}",
        quote_posix("word-assist-ca.der"),
        quote_posix(CA_COMMON_NAME)
    )
}

/// macOS sideload copy command (zsh/bash): copy an exported manifest into
/// Word's per-user wef folder (created if missing). `$HOME` keeps the
/// command working for every username shape (spaces, apostrophes, non-ASCII).
#[cfg(feature = "word-assist")]
pub fn sideload_copy_command_macos(manifest_path: &std::path::Path) -> String {
    let wef = "$HOME/Library/Containers/com.microsoft.Word/Data/Documents/wef";
    format!(
        "mkdir -p {wef} && cp {} {wef}/manifest.xml",
        quote_posix(&manifest_path.display().to_string())
    )
}

#[cfg(all(test, feature = "word-assist"))]
mod tests {
    // ---- P6: hostile-path unit tests for the command builders ------------
    // Each case exercises a path shape the owner named: spaces, apostrophes,
    // non-ASCII (Arabic / accented) usernames, long paths — and asserts the
    // no-elevation shape of every command.
    #[test]
    fn p6_windows_trust_quotes_paths_with_spaces() {
        let dir = std::path::Path::new(r"C:\Users\John Doe\AppData\Roaming\scholarscribe");
        let cmd = super::ca_trust_command_windows(dir);
        assert!(cmd.starts_with("certutil -user -addstore Root \""), "{cmd}");
        assert!(cmd.ends_with("word-assist-ca.der\""), "{cmd}");
        assert!(cmd.contains("John Doe"), "{cmd}");
        // No elevation: -user store, never the machine store flag pair.
        assert!(!cmd.contains(" -enterprise"), "{cmd}");
        assert!(!cmd.contains(" -gpo"), "{cmd}");
    }

    #[test]
    fn p6_posix_quoting_survives_apostrophes() {
        let dir = std::path::Path::new("/Users/O'Brien/Library/Application Support/scholarscribe");
        let cmd = super::ca_trust_command_macos(dir);
        // The '\'' re-entry trick must be applied, and the name must survive
        // round-trip: stripping the quoting yields the original path.
        assert!(cmd.contains("'\\''"), "{cmd}");
        assert!(!cmd.starts_with("sudo"), "{cmd}");
        assert!(!cmd.contains(" -d "), "{cmd}");
        let quoted = cmd
            .strip_prefix("security add-trusted-cert -r trustRoot ")
            .expect("command shape")
            .to_string();
        let unquoted = quoted
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .unwrap()
            .replace("'\\''", "'");
        assert_eq!(
            unquoted,
            "/Users/O'Brien/Library/Application Support/scholarscribe/word-assist-ca.der"
        );
    }

    #[test]
    fn p6_powershell_quoting_doubles_inner_apostrophes() {
        let q = super::quote_powershell("/Users/O'Brien/x");
        assert_eq!(q, "'/Users/O''Brien/x'");
    }

    #[test]
    fn p6_non_ascii_usernames_pass_through_verbatim() {
        // Arabic username (Windows shape) — UTF-8 passthrough, quoted.
        let dir = std::path::Path::new(r"C:\Users\مستخدم\AppData\Roaming\scholarscribe");
        let cmd = super::ca_trust_command_windows(dir);
        assert!(cmd.contains("مستخدم"), "{cmd}");
        assert!(cmd.starts_with("certutil -user"), "{cmd}");
        // Accented username (macOS shape).
        let dir2 = std::path::Path::new("/Users/José/Library/Application Support/scholarscribe");
        let cmd2 = super::ca_trust_command_macos(dir2);
        assert!(cmd2.contains("José"), "{cmd2}");
        assert!(
            cmd2.starts_with("security add-trusted-cert -r trustRoot"),
            "{cmd2}"
        );
    }

    #[test]
    fn p6_long_paths_are_not_truncated() {
        let mut long = std::path::PathBuf::from(r"C:\Users\jane");
        for i in 0..40 {
            long.push(format!("directory-with-a-very-long-name-{i:02}"));
        }
        let cmd = super::ca_trust_command_windows(&long);
        assert!(
            cmd.len() > 1200,
            "path should survive intact: len={}",
            cmd.len()
        );
        assert!(cmd.starts_with("certutil -user -addstore Root \""), "{cmd}");
        assert!(cmd.ends_with("\""), "{cmd}");
    }

    #[test]
    fn p6_revoke_commands_target_user_stores_only() {
        let win = super::ca_revoke_command_windows();
        assert!(win.starts_with("certutil -user -delstore Root \""), "{win}");
        assert!(win.contains(super::CA_COMMON_NAME), "{win}");
        let mac = super::ca_revoke_command_macos();
        assert!(mac.starts_with("security remove-trusted-cert '"), "{mac}");
        assert!(!mac.contains(" -d "), "{mac}");
        assert!(!mac.contains("sudo"), "{mac}");
        assert!(mac.contains(super::CA_COMMON_NAME), "{mac}");
    }

    #[test]
    fn p6_sideload_copy_uses_home_and_quotes_manifest() {
        let manifest = std::path::Path::new("/Users/Ann Lee/Downloads/word assist manifest.xml");
        let cmd = super::sideload_copy_command_macos(manifest);
        assert!(
            cmd.starts_with(
                "mkdir -p $HOME/Library/Containers/com.microsoft.Word/Data/Documents/wef && cp '"
            ),
            "{cmd}"
        );
        assert!(cmd.contains("Ann Lee"), "{cmd}");
        assert!(cmd.contains("manifest.xml"), "{cmd}");
    }

    #[test]
    fn pem_encoder_matches_shape() {
        let pem = super::pem_encode(&[0x30, 0x82, 0x01]);
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----\nMIIB"));
        assert!(pem.ends_with("-----END CERTIFICATE-----\n"));
        let one = super::pem_encode(&[0x30]);
        assert!(one.contains("MA=="));
        // Line width is 64 at most.
        for line in super::pem_encode(&vec![7u8; 300]).lines() {
            assert!(line.len() <= 64 || line.starts_with('-'));
        }
    }
}

#[cfg(feature = "word-assist")]
use crate::persistence;
