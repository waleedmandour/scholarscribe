//! Persistence — opt-in local storage of drafts, chat history, and settings.
//!
//! PRIVACY DESIGN:
//! - Persistence is OFF by default. The user must explicitly enable it.
//! - All data is stored in the Tauri app_data_dir (e.g. %APPDATA%\com.scholarscribe.app\data\
//!   on Windows). Never synced to cloud.
//! - The audit log is NEVER persisted — it stays in-memory only and is cleared
//!   on app close. This is intentional: the audit log exists so users can verify
//!   the app's behavior in-session; persisting it would create a record of every
//!   file they read, which is the opposite of privacy.
//! - Drafts are stored as plain JSON files the user can open in any text editor.
//! - The user can view, delete, or "open in file explorer" any saved data from
//!   the Saved Work tab in the UI.

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
    /// Last-used theme: "light" | "dark" | "auto". Default: "auto".
    pub theme: String,
    /// App version that wrote this settings file. Used for migrations.
    pub version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            persistence_enabled: false,
            theme: "auto".into(),
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
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
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
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if std::fs::remove_file(&path).is_ok() {
                count += 1;
            }
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
