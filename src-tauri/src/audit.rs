//! Privacy audit log.
//!
//! Every time the backend reads a user file or makes an outbound HTTP call,
//! it appends an entry here. The log is in-memory only (cleared on app
//! restart) and is surfaced to the user in the Privacy Audit tab so they can
//! verify exactly what the app accessed during the session.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: u64, // unix seconds
    pub kind: String,   // "file_read" | "http_call" | "ollama_command"
    pub target: String, // file path or URL
    pub detail: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Default)]
pub struct AuditLog {
    entries: Mutex<Vec<AuditEntry>>,
}

impl AuditLog {
    pub fn record(&self, kind: &str, target: &str, detail: &str, bytes_in: u64, bytes_out: u64) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let entry = AuditEntry {
            timestamp: ts,
            kind: kind.to_string(),
            target: target.to_string(),
            detail: detail.to_string(),
            bytes_in,
            bytes_out,
        };
        if let Ok(mut entries) = self.entries.lock() {
            entries.push(entry);
            // Cap at 1000 entries to prevent unbounded growth.
            if entries.len() > 1000 {
                let drop = entries.len() - 1000;
                entries.drain(..drop);
            }
        }
    }

    pub fn snapshot(&self) -> Vec<AuditEntry> {
        self.entries.lock().map(|e| e.clone()).unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut e) = self.entries.lock() {
            e.clear();
        }
    }
}

#[tauri::command]
pub fn audit_list(state: State<'_, AuditLog>) -> Vec<AuditEntry> {
    state.snapshot()
}

#[tauri::command]
pub fn audit_clear(state: State<'_, AuditLog>) {
    state.clear();
}

#[tauri::command]
pub fn audit_summary(state: State<'_, AuditLog>) -> serde_json::Value {
    let entries = state.snapshot();
    let file_reads = entries.iter().filter(|e| e.kind == "file_read").count();
    let http_calls = entries.iter().filter(|e| e.kind == "http_call").count();
    let ollama_cmds = entries
        .iter()
        .filter(|e| e.kind == "ollama_command")
        .count();
    let bytes_in: u64 = entries.iter().map(|e| e.bytes_in).sum();
    let bytes_out: u64 = entries.iter().map(|e| e.bytes_out).sum();

    // Collect the unique outbound hosts (anything in http_call entries).
    let mut hosts: std::collections::HashSet<String> = std::collections::HashSet::new();
    for e in &entries {
        if e.kind == "http_call" {
            if let Ok(url) = url::Url::parse(&e.target) {
                if let Some(h) = url.host_str() {
                    hosts.insert(h.to_string());
                }
            }
        }
    }

    serde_json::json!({
        "total_events": entries.len(),
        "file_reads": file_reads,
        "http_calls": http_calls,
        "ollama_commands": ollama_cmds,
        "bytes_in": bytes_in,
        "bytes_out": bytes_out,
        "outbound_hosts": hosts.into_iter().collect::<Vec<_>>(),
    })
}
