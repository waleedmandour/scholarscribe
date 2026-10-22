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
            "Trust the CA certificate ('ScholarScribe Word Assist Root CA (ephemeral)'), NOT the leaf: the CA is name-constrained to localhost / 127.0.0.1 and its signing key was destroyed after issuing your leaf.".into(),
            "Windows: double-click the CA certificate file, 'Install Certificate', scope 'Current User', store 'Trusted Root Certification Authorities'.".into(),
            "Or: certmgr.msc > Trusted Root Certification Authorities > Import.".into(),
            format!("Sideload: Word > Insert > My Add-ins / Get Add-ins > 'Upload My Add-in' > {}", format!("https://localhost:{port}/addin/manifest.xml")),
            "Renewal: the CA + leaf are regenerated when they expire (CA 3 years, leaf 1 year); re-trust the NEW CA — this re-asks your consent.".into(),
            "Removal: delete 'ScholarScribe Word Assist Root CA (ephemeral)' from Manage User Certificates > Trusted Root Certification Authorities, and remove the add-in from My Add-ins.".into(),
        ],
    })
}

/// Stub for builds without the feature.
#[cfg(not(feature = "word-assist"))]
#[tauri::command]
pub async fn word_assist_registration(_app: AppHandle) -> Result<AssistRegistration, String> {
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

#[cfg(all(test, feature = "word-assist"))]
mod tests {
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
