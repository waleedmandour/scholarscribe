//! Persistence, opt-in local storage of drafts, chat history, and settings.
//!
//! PRIVACY DESIGN:
//! - Persistence is OFF by default. The user must explicitly enable it.
//! - All data is stored in the Tauri app_data_dir (e.g. %APPDATA%\com.scholarscribe.app\data\
//!   on Windows). Never synced to cloud.
//! - The audit log is NEVER persisted, it stays in-memory only and is cleared
//!   on app close. This is intentional: the audit log exists so users can verify
//!   the app's behavior in-session; persisting it would create a record of every
//!   file they read, which is the opposite of privacy.
//! - Drafts are stored as plain JSON files the user can open in any text editor.
//! - The user can view, delete, or "open in file explorer" any saved data from
//!   the Saved Work tab in the UI.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const DATA_DIR_NAME: &str = "data";
const DRAFTS_DIR_NAME: &str = "drafts";
const SETTINGS_FILE: &str = "settings.json";

/// In-process cache of the data directory path (resolved on first use).
static DATA_DIR: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Whether the user has opted in to local persistence. Default: false.
    pub persistence_enabled: bool,
    /// Whether the user has opted in to the Writing Provenance feature
    /// (v2.1.0). Default: false, the disclosure dialog sets this to true.
    #[serde(default)]
    pub provenance_enabled: bool,
    /// Last-used theme: "light" | "dark" | "auto". Default: "auto".
    pub theme: String,
    /// Language Check (v2.2.1 r5): last-used dialect name. One of the
    /// supported harper dialect strings, e.g. "American".
    #[serde(default = "default_language_check_dialect")]
    pub language_check_dialect: String,
    /// Language Check (v2.2.1 r5): per-rule overrides. Some(true/false) forces
    /// a rule on or off, None falls back to the curated default. Preferences
    /// like this persist unconditionally, mirroring how `theme` is stored;
    /// the personal dictionary (user content) is only written to disk when
    /// `persistence_enabled` is on.
    #[serde(default)]
    pub language_check_rules: BTreeMap<String, Option<bool>>,
    /// Word Assist (feature/word-assist): master opt-in. Default false; the
    /// disclosure dialog on the Word Assist tab is the only code path that
    /// sets it to true (ruling 5: explicit consent before any listener).
    #[serde(default)]
    pub word_assist_enabled: bool,
    /// Word Assist: loopback listener port. Default 38080 (below the
    /// Windows ephemeral range, ruling D-3). Changing it forces re-pairing
    /// (localStorage is per-origin including port) — the UI says so.
    #[serde(default = "default_word_assist_port")]
    pub word_assist_port: u16,
    /// Word Assist: strictness profile name. Shipped default "dev"
    /// (Grammar only, PROVISIONAL per ruling E-1; final defaults are a
    /// Phase 4 decision). One of: dev | conservative | balanced | thorough.
    #[serde(default = "default_word_assist_strictness")]
    pub word_assist_strictness: String,
    /// Word Assist: whether the Tier 1 (local model) pass runs at all.
    /// Tier 0 is always on while the feature is enabled; Tier 1 sends one
    /// sentence at a time to the local Ollama daemon, so it is opt-in on
    /// top of the feature opt-in.
    #[serde(default)]
    pub word_assist_tier1_enabled: bool,
    /// Word Assist: model name for Tier 1. Empty = use the app's last-used
    /// chat model. No validation here; the scheduler surfaces daemon errors.
    #[serde(default)]
    pub word_assist_tier1_model: String,
    /// Word Assist: per-script experimental opt-in for scripts the engine
    /// cannot validate ("experimental: other languages", off by default,
    /// ruling 2). Keys are script names from assist::tier0::script_name
    /// ("arabic", "cjk", "cyrillic", "greek", "other"); true runs checks
    /// with the "experimental, unvalidated" status line.
    #[serde(default)]
    pub word_assist_experimental_languages: BTreeMap<String, bool>,
    /// App version that wrote this settings file. Used for migrations.
    pub version: String,
}

fn default_language_check_dialect() -> String {
    "American".to_string()
}

fn default_word_assist_port() -> u16 {
    38080
}

fn default_word_assist_strictness() -> String {
    "dev".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            persistence_enabled: false,
            provenance_enabled: false,
            theme: "auto".into(),
            language_check_dialect: default_language_check_dialect(),
            language_check_rules: BTreeMap::new(),
            word_assist_enabled: false,
            word_assist_port: default_word_assist_port(),
            word_assist_strictness: default_word_assist_strictness(),
            word_assist_tier1_enabled: false,
            word_assist_tier1_model: String::new(),
            word_assist_experimental_languages: BTreeMap::new(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    pub title: String,
    pub kind: String, // "style_draft" | "style_reference" | "chat" | "disclosure" | "cleaner"
    pub content: String,
    pub created_at: u64, // unix seconds
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DraftMeta {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub size_bytes: usize,
}

/// Resolve the data directory. Caches the result so subsequent calls are fast.
fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    {
        let cache = DATA_DIR.lock().map_err(|e| e.to_string())?;
        if let Some(p) = cache.as_ref() {
            return Ok(p.clone());
        }
    }
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {}", e))?;
    let data = base.join(DATA_DIR_NAME);
    std::fs::create_dir_all(&data).map_err(|e| format!("Cannot create data dir: {}", e))?;
    let drafts = data.join(DRAFTS_DIR_NAME);
    std::fs::create_dir_all(&drafts).map_err(|e| format!("Cannot create drafts dir: {}", e))?;
    let mut cache = DATA_DIR.lock().map_err(|e| e.to_string())?;
    *cache = Some(data.clone());
    Ok(data)
}

fn drafts_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join(DRAFTS_DIR_NAME))
}

/// Path for the Language Check personal dictionary file (v2.2.1 r5).
/// Returns None when persistence is disabled: the dictionary is user content,
/// so it is only written to disk after the user opted in to persistence. The
/// in-session dictionary keeps working either way (memory only).
pub(crate) fn language_check_dictionary_path(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    if !settings_get(app.clone())?.persistence_enabled {
        return Ok(None);
    }
    Ok(Some(data_dir(app)?.join("language-check-dictionary.json")))
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join(SETTINGS_FILE))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ---------- Settings ----------

#[tauri::command]
pub fn settings_get(app: AppHandle) -> Result<Settings, String> {
    let path = settings_path(&app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("read settings: {}", e))?;
    let mut s: Settings = serde_json::from_str(&content).unwrap_or_default();
    // Migrate: ensure version is current
    if s.version.is_empty() {
        s.version = env!("CARGO_PKG_VERSION").into();
    }
    Ok(s)
}

#[tauri::command]
pub fn settings_set(app: AppHandle, settings: Settings) -> Result<(), String> {
    let path = settings_path(&app)?;
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| format!("write settings: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn persistence_enable(app: AppHandle) -> Result<(), String> {
    let mut s = settings_get(app.clone())?;
    s.persistence_enabled = true;
    settings_set(app, s)
}

#[tauri::command]
pub fn persistence_disable(app: AppHandle) -> Result<(), String> {
    let mut s = settings_get(app.clone())?;
    s.persistence_enabled = false;
    settings_set(app, s)
}

#[tauri::command]
pub fn persistence_status(app: AppHandle) -> Result<bool, String> {
    Ok(settings_get(app)?.persistence_enabled)
}

// ---------- Writing Provenance opt-in (v2.1.0) ----------

#[tauri::command]
pub fn provenance_enable(app: AppHandle) -> Result<(), String> {
    let mut s = settings_get(app.clone())?;
    s.provenance_enabled = true;
    settings_set(app, s)
}

#[tauri::command]
pub fn provenance_disable(app: AppHandle) -> Result<(), String> {
    let mut s = settings_get(app.clone())?;
    s.provenance_enabled = false;
    settings_set(app, s)
}

#[tauri::command]
pub fn provenance_status(app: AppHandle) -> Result<bool, String> {
    Ok(settings_get(app)?.provenance_enabled)
}

// ---------- Drafts ----------

#[tauri::command]
pub fn draft_save(
    app: AppHandle,
    title: String,
    kind: String,
    content: String,
) -> Result<Draft, String> {
    if !persistence_status(app.clone())? {
        return Err("Persistence is not enabled. Enable it in the Saved Work tab first.".into());
    }
    let dir = drafts_dir(&app)?;
    let now = now_unix();
    let id = new_id();
    let draft = Draft {
        id: id.clone(),
        title: title.clone(),
        kind: kind.clone(),
        content: content.clone(),
        created_at: now,
        updated_at: now,
    };
    let path = dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&draft).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("write draft: {}", e))?;
    Ok(draft)
}

#[tauri::command]
pub fn draft_update(
    app: AppHandle,
    id: String,
    title: Option<String>,
    content: Option<String>,
) -> Result<Draft, String> {
    if !persistence_status(app.clone())? {
        return Err("Persistence is not enabled.".into());
    }
    let dir = drafts_dir(&app)?;
    let path = dir.join(format!("{}.json", id));
    if !path.exists() {
        return Err(format!("Draft {} not found", id));
    }
    let content_str = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut draft: Draft =
        serde_json::from_str(&content_str).map_err(|e| format!("parse draft: {}", e))?;
    if let Some(t) = title {
        draft.title = t;
    }
    if let Some(c) = content {
        draft.content = c;
    }
    draft.updated_at = now_unix();
    let json = serde_json::to_string_pretty(&draft).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("write draft: {}", e))?;
    Ok(draft)
}

#[tauri::command]
pub fn draft_load(app: AppHandle, id: String) -> Result<Draft, String> {
    let dir = drafts_dir(&app)?;
    let path = dir.join(format!("{}.json", id));
    if !path.exists() {
        return Err(format!("Draft {} not found", id));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let draft: Draft = serde_json::from_str(&content).map_err(|e| format!("parse: {}", e))?;
    Ok(draft)
}

#[tauri::command]
pub fn draft_list(app: AppHandle) -> Result<Vec<DraftMeta>, String> {
    if !persistence_status(app.clone())? {
        return Ok(Vec::new());
    }
    let dir = drafts_dir(&app)?;
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let draft: Draft = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(_) => continue,
        };
        out.push(DraftMeta {
            id: draft.id,
            title: draft.title,
            kind: draft.kind,
            created_at: draft.created_at,
            updated_at: draft.updated_at,
            size_bytes: content.len(),
        });
    }
    // Sort by updated_at descending (most recent first)
    out.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
    Ok(out)
}

#[tauri::command]
pub fn draft_delete(app: AppHandle, id: String) -> Result<(), String> {
    let dir = drafts_dir(&app)?;
    let path = dir.join(format!("{}.json", id));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn draft_delete_all(app: AppHandle) -> Result<usize, String> {
    let dir = drafts_dir(&app)?;
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && std::fs::remove_file(&path).is_ok()
        {
            count += 1;
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn data_dir_path(app: AppHandle) -> Result<String, String> {
    let p = data_dir(&app)?;
    Ok(p.to_string_lossy().into_owned())
}

// ---------- Cleanup ----------

/// Placeholder for any startup-time migration logic. Currently a no-op.
#[allow(dead_code)]
pub fn migrate(_app: &AppHandle) -> Result<(), String> {
    // Future versions will use this to migrate settings/draft formats
    // across app version bumps.
    Ok(())
}
