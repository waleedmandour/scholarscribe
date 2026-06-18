//! Tauri command handlers — the bridge between the Svelte frontend and Rust backend.

use crate::{disclosure, ollama, style};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn app_info(app: AppHandle) -> serde_json::Value {
    let pkg = app.package_info();
    serde_json::json!({
        "name": pkg.name,
        "version": pkg.version.to_string(),
        "authors": pkg.authors,
        "license": "MIT",
        "ollama_base_url": ollama::base_url(),
    })
}

#[tauri::command]
pub async fn ollama_status(
    state: State<'_, ollama::OllamaState>,
) -> Result<bool, ollama::OllamaError> {
    ollama::status(state.inner()).await
}

#[tauri::command]
pub async fn ollama_list_models(
    state: State<'_, ollama::OllamaState>,
) -> Result<Vec<ollama::ModelInfo>, ollama::OllamaError> {
    ollama::list_models(state.inner()).await
}

#[tauri::command]
pub async fn ollama_pull_model(
    app: AppHandle,
    state: State<'_, ollama::OllamaState>,
    name: String,
) -> Result<(), ollama::OllamaError> {
    // Tell the frontend this pull is starting.
    let _ = app.emit("ollama://pull-start", &name);
    let result = ollama::pull_model(app.clone(), state.inner(), name.clone()).await;
    let _ = app.emit(
        "ollama://pull-end",
        serde_json::json!({ "model": name, "ok": result.is_ok() }),
    );
    result
}

#[tauri::command]
pub async fn ollama_delete_model(
    state: State<'_, ollama::OllamaState>,
    name: String,
) -> Result<(), ollama::OllamaError> {
    ollama::delete_model(state.inner(), &name).await
}

#[tauri::command]
pub async fn ollama_chat(
    state: State<'_, ollama::OllamaState>,
    request: ollama::ChatRequest,
) -> Result<ollama::ChatMessage, ollama::OllamaError> {
    ollama::chat(state.inner(), request).await
}

#[tauri::command]
pub fn recommended_models() -> Vec<ollama::RecommendedModel> {
    ollama::recommended_models()
}

#[derive(Debug, Deserialize)]
pub struct ReadTextFileArgs {
    pub path: String,
}

#[tauri::command]
pub async fn read_text_file(args: ReadTextFileArgs) -> Result<String, String> {
    let path = PathBuf::from(&args.path);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "txt" | "md" | "markdown" | "tex" | "rst" | "csv" | "json" => {
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))
        }
        "docx" => {
            // We do not pull in docx parsing as a hard dependency to keep the
            // bundle small. The user can convert .docx to .txt or .md via
            // Word's "Save As", or we accept the .docx and return a helpful
            // pointer. Future versions may bundle `docx-rs` behind a feature.
            Err("ScholarScribe cannot read .docx directly yet. Please export your document as .txt, .md, or .pdf-extracted text. Support for .docx is planned for v0.2.".into())
        }
        other => Err(format!(
            "Unsupported file type: .{}. Supported: .txt, .md, .tex, .rst, .csv, .json",
            other
        )),
    }
}

#[tauri::command]
pub fn analyze_style(text: String) -> style::StyleProfile {
    style::analyze(&text)
}

#[tauri::command]
pub fn compare_style(
    draft: style::StyleProfile,
    reference: style::StyleProfile,
) -> style::StyleComparison {
    style::compare(&draft, &reference)
}

#[tauri::command]
pub fn list_venue_templates() -> Vec<disclosure::VenueTemplate> {
    disclosure::venue_templates()
}

#[tauri::command]
pub fn generate_disclosure(
    input: disclosure::DisclosureInput,
) -> Result<disclosure::DisclosureOutput, String> {
    disclosure::generate(&input)
}
