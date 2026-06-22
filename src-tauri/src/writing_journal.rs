//! Writing Process Journal — auto-saves timestamped snapshots of draft text,
//! creating a verifiable process record that can serve as evidence of
//! authentic authorship. Aligns with the opt-in persistence architecture.
//!
//! Snapshots are stored as JSON files in the app data directory under
//! `journals/`. Each snapshot includes: timestamp, content, word count,
//! and a diff summary from the previous snapshot.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::persistence;

const JOURNALS_DIR: &str = "journals";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub session_id: String,
    pub timestamp: u64,
    pub content: String,
    pub word_count: usize,
    pub char_count: usize,
    pub diff_from_previous: Option<DiffSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub words_added: usize,
    pub words_removed: usize,
    pub chars_added: usize,
    pub chars_removed: usize,
    pub similarity_pct: f64,
}

#[derive(Debug, Serialize)]
pub struct JournalSession {
    pub session_id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub snapshot_count: usize,
    pub total_words_final: usize,
}

fn journals_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {}", e))?;
    let dir = base.join("data").join(JOURNALS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create journals dir: {}", e))?;
    Ok(dir)
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

#[tauri::command]
pub fn journal_create_session(app: AppHandle, title: String) -> Result<Snapshot, String> {
    if !persistence::persistence_status(app.clone())? {
        return Err("Persistence is not enabled. Enable it in the Saved Work tab first.".into());
    }
    let dir = journals_dir(&app)?;
    let session_id = new_id();
    let now = now_unix();
    let snapshot = Snapshot {
        id: new_id(),
        session_id: session_id.clone(),
        timestamp: now,
        content: String::new(),
        word_count: 0,
        char_count: 0,
        diff_from_previous: None,
    };
    let path = dir.join(format!("{}_{}.json", session_id, now));
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("write snapshot: {}", e))?;
    Ok(snapshot)
}

#[tauri::command]
pub fn journal_save_snapshot(
    app: AppHandle,
    session_id: String,
    content: String,
) -> Result<Snapshot, String> {
    if !persistence::persistence_status(app.clone())? {
        return Err("Persistence is not enabled.".into());
    }
    let dir = journals_dir(&app)?;

    // Find the previous snapshot for this session to compute diff
    let mut prev_content: Option<String> = None;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(&format!("{}_", session_id))
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());
    if let Some(last) = entries.last() {
        if let Ok(text) = std::fs::read_to_string(last.path()) {
            if let Ok(prev) = serde_json::from_str::<Snapshot>(&text) {
                prev_content = Some(prev.content);
            }
        }
    }

    let word_count = content.split_whitespace().count();
    let now = now_unix();

    let diff = prev_content.map(|prev| {
        let prev_words = prev.split_whitespace().count();
        let prev_chars = prev.len();
        let curr_chars = content.len();
        let words_added = word_count.saturating_sub(prev_words);
        let words_removed = prev_words.saturating_sub(word_count);
        let chars_added = curr_chars.saturating_sub(prev_chars);
        let chars_removed = prev_chars.saturating_sub(curr_chars);
        let max_len = prev_chars.max(curr_chars).max(1);
        let similarity = 1.0 - (chars_added + chars_removed) as f64 / (2.0 * max_len as f64);
        DiffSummary {
            words_added,
            words_removed,
            chars_added,
            chars_removed,
            similarity_pct: (similarity * 100.0).round(1),
        }
    });

    let snapshot = Snapshot {
        id: new_id(),
        session_id: session_id.clone(),
        timestamp: now,
        content,
        word_count,
        char_count: content.len(),
        diff_from_previous: diff,
    };

    let path = dir.join(format!("{}_{}.json", session_id, now));
    let json = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("write snapshot: {}", e))?;
    Ok(snapshot)
}

#[tauri::command]
pub fn journal_list_sessions(app: AppHandle) -> Result<Vec<JournalSession>, String> {
    let dir = journals_dir(&app)?;
    let mut sessions: std::collections::HashMap<String, Vec<Snapshot>> =
        std::collections::HashMap::new();

    for entry in std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let snap: Snapshot = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(_) => continue,
        };
        sessions
            .entry(snap.session_id.clone())
            .or_default()
            .push(snap);
    }

    let mut out = Vec::new();
    for (session_id, mut snaps) in sessions {
        snaps.sort_by_key(|s| s.timestamp);
        let last = snaps.last().unwrap();
        out.push(JournalSession {
            session_id,
            title: format!("Session with {} snapshots", snaps.len()),
            created_at: snaps.first().unwrap().timestamp,
            updated_at: last.timestamp,
            snapshot_count: snaps.len(),
            total_words_final: last.word_count,
        });
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}

#[tauri::command]
pub fn journal_get_snapshots(app: AppHandle, session_id: String) -> Result<Vec<Snapshot>, String> {
    let dir = journals_dir(&app)?;
    let mut snaps = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let fname = entry.file_name().to_string_lossy().to_string();
        if !fname.starts_with(&format!("{}_", session_id)) {
            continue;
        }
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Ok(snap) = serde_json::from_str::<Snapshot>(&content) {
            snaps.push(snap);
        }
    }
    snaps.sort_by_key(|s| s.timestamp);
    Ok(snaps)
}

#[tauri::command]
pub fn journal_delete_session(app: AppHandle, session_id: String) -> Result<usize, String> {
    let dir = journals_dir(&app)?;
    let mut count = 0;
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let fname = entry.file_name().to_string_lossy().to_string();
        if fname.starts_with(&format!("{}_", session_id)) {
            if std::fs::remove_file(entry.path()).is_ok() {
                count += 1;
            }
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn journal_export_session(
    app: AppHandle,
    session_id: String,
    output_path: String,
) -> Result<(), String> {
    let snaps = journal_get_snapshots(app, session_id)?;
    let mut out = String::new();
    out.push_str(&format!("# Writing Process Journal Export\n\n"));
    out.push_str(&format!("Session ID: {}\n", session_id));
    out.push_str(&format!("Snapshots: {}\n", snaps.len()));
    out.push_str(&format!(
        "Created: {}\n",
        snaps
            .first()
            .map(|s| format!("{}", s.timestamp))
            .unwrap_or_default()
    ));
    out.push_str(&format!(
        "Last updated: {}\n\n",
        snaps
            .last()
            .map(|s| format!("{}", s.timestamp))
            .unwrap_or_default()
    ));
    out.push_str("---\n\n");
    for (i, snap) in snaps.iter().enumerate() {
        out.push_str(&format!("## Snapshot {} — {}\n", i + 1, snap.timestamp));
        out.push_str(&format!(
            "Words: {} | Characters: {}\n",
            snap.word_count, snap.char_count
        ));
        if let Some(diff) = &snap.diff_from_previous {
            out.push_str(&format!(
                "Diff: +{} / -{} words, {}% similarity\n",
                diff.words_added, diff.words_removed, diff.similarity_pct
            ));
        }
        out.push_str(&format!("\n{}\n\n---\n\n", snap.content));
    }
    std::fs::write(&output_path, out).map_err(|e| format!("write export: {}", e))?;
    Ok(())
}
