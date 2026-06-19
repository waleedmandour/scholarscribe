//! Tauri command handlers — the bridge between the Svelte frontend and Rust backend.

use crate::{audit, disclosure, ollama, style};
use serde::Deserialize;
use std::path::PathBuf;
use sysinfo::System;
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
    audit: State<'_, audit::AuditLog>,
    name: String,
) -> Result<(), ollama::OllamaError> {
    audit.record(
        "ollama_command",
        &format!("{}/api/pull", ollama::base_url()),
        &format!("pull model {}", name),
        0,
        0,
    );
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
    audit: State<'_, audit::AuditLog>,
    name: String,
) -> Result<(), ollama::OllamaError> {
    audit.record(
        "ollama_command",
        &format!("{}/api/delete", ollama::base_url()),
        &format!("delete model {}", name),
        0,
        0,
    );
    ollama::delete_model(state.inner(), &name).await
}

#[tauri::command]
pub async fn ollama_chat(
    state: State<'_, ollama::OllamaState>,
    audit: State<'_, audit::AuditLog>,
    request: ollama::ChatRequest,
) -> Result<ollama::ChatMessage, ollama::OllamaError> {
    audit.record(
        "ollama_command",
        &format!("{}/api/chat", ollama::base_url()),
        &format!("chat with model {}", request.model),
        0,
        0,
    );
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
pub async fn read_text_file(
    args: ReadTextFileArgs,
    audit: State<'_, audit::AuditLog>,
) -> Result<String, String> {
    let path = PathBuf::from(&args.path);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    audit.record(
        "file_read",
        &args.path,
        &format!("read .{} file", ext),
        0,
        0,
    );

    match ext.as_str() {
        "txt" | "md" | "markdown" | "tex" | "rst" | "csv" | "json" => {
            tokio::fs::read_to_string(&path)
                .await
                .map(|content| {
                    // Update the audit entry's bytes_in retroactively would require mut access;
                    // simpler: record a second entry reflecting the actual byte count.
                    audit.record(
                        "file_read",
                        &args.path,
                        "bytes read",
                        content.len() as u64,
                        0,
                    );
                    content
                })
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))
        }
        "docx" => Err("ScholarScribe cannot read .docx directly yet. Please export your document as .txt, .md, or .pdf-extracted text. Support for .docx is planned for v0.2.".into()),
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

// ---------------- v0.1.1 new commands ----------------

#[derive(Debug, serde::Serialize)]
pub struct SystemInfo {
    pub total_ram_gb: f64,
    pub available_ram_gb: f64,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub os_name: String,
}

#[tauri::command]
pub fn system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();
    let total_ram_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let available_ram_gb = sys.available_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "unknown".into());
    let cpu_cores = sys.cpus().len();
    // sysinfo 0.32: System::name() returns Option<String>
    let os_name = System::name().unwrap_or_else(|| "unknown".to_string());

    SystemInfo {
        total_ram_gb,
        available_ram_gb,
        cpu_brand,
        cpu_cores,
        os_name,
    }
}

#[derive(Debug, serde::Serialize)]
pub struct GgufCompatResult {
    pub file_path: String,
    pub file_size_gb: f64,
    pub recommended_ram_gb: f64,
    pub total_ram_gb: f64,
    pub available_ram_gb: f64,
    pub verdict: String, // "ok" | "tight" | "insufficient"
    pub message: String,
}

/// Check whether a GGUF file's size is compatible with the device's RAM.
/// We can't read the parameter count from a GGUF without a binary parser,
/// so we use the file size as a proxy: a model typically needs ~1.5x its
/// file size in RAM during inference (file + KV cache + activations).
#[tauri::command]
pub async fn check_gguf_compatibility(path: String) -> Result<GgufCompatResult, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }
    let metadata = tokio::fs::metadata(&p)
        .await
        .map_err(|e| format!("Cannot stat {}: {}", path, e))?;
    let file_size_bytes = metadata.len();
    let file_size_gb = file_size_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    // Llama.cpp / Ollama typically needs ~1.5x model file size in RAM for inference
    // (file is mmap'd, plus KV cache grows with context length, plus activations).
    let recommended_ram_gb = file_size_gb * 1.5;

    let mut sys = System::new_all();
    sys.refresh_all();
    let total_ram_gb = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let available_ram_gb = sys.available_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

    let (verdict, message) = if available_ram_gb >= recommended_ram_gb {
        (
            "ok".to_string(),
            format!(
                "Your available RAM ({:.1} GB) comfortably covers the recommended {:.1} GB for this model.",
                available_ram_gb, recommended_ram_gb
            ),
        )
    } else if total_ram_gb >= recommended_ram_gb {
        (
            "tight".to_string(),
            format!(
                "Your total RAM ({:.1} GB) covers the recommended {:.1} GB, but only {:.1} GB is currently free. Close other memory-heavy apps (browsers, etc.) before running inference.",
                total_ram_gb, recommended_ram_gb, available_ram_gb
            ),
        )
    } else {
        (
            "insufficient".to_string(),
            format!(
                "Your total RAM ({:.1} GB) is below the recommended {:.1} GB for this model. Inference will likely be very slow or fail with an out-of-memory error. Consider a smaller model.",
                total_ram_gb, recommended_ram_gb
            ),
        )
    };

    Ok(GgufCompatResult {
        file_path: path,
        file_size_gb,
        recommended_ram_gb,
        total_ram_gb,
        available_ram_gb,
        verdict,
        message,
    })
}

#[derive(Debug, Deserialize)]
pub struct ImportGgufArgs {
    pub path: String,
    pub model_name: String,
}

/// Import a local .gguf file into Ollama's model registry.
///
/// Calls Ollama's `/api/create` endpoint. Ollama 0.5+ accepts a structured
/// `from` field pointing directly at the GGUF file path; older versions
/// require the `modelfile` field with `FROM <path>` text. We send both so
/// the call works across versions.
///
/// Windows path handling: Ollama's Modelfile parser sometimes mishandles
/// backslashes. We convert to forward slashes AND wrap in a `file://` URL
/// for the `from` field, which is the documented form for absolute paths.
#[tauri::command]
pub async fn ollama_import_gguf(
    args: ImportGgufArgs,
    state: State<'_, ollama::OllamaState>,
    audit: State<'_, audit::AuditLog>,
) -> Result<(), String> {
    let p = PathBuf::from(&args.path);
    if !p.exists() {
        return Err(format!("File not found: {}", args.path));
    }
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext != "gguf" {
        return Err(format!(
            "Expected a .gguf file, got .{}. Ollama can only import GGUF files directly.",
            ext
        ));
    }
    let name = args.model_name.trim();
    if name.is_empty() {
        return Err("Model name is required.".into());
    }

    audit.record(
        "ollama_command",
        &format!("{}/api/create", ollama::base_url()),
        &format!("import gguf {} as {}", args.path, name),
        0,
        0,
    );

    // Canonicalize to an absolute path. On Windows this returns a `\\?\`
    // prefix (e.g. `\\?\C:\Users\...`) which Ollama's parser doesn't like.
    // We strip it and normalize backslashes to forward slashes.
    let abs_path = {
        let raw = p
            .canonicalize()
            .map_err(|e| format!("Cannot resolve path: {}", e))?
            .to_string_lossy()
            .to_string();
        // Strip the Windows extended-length path prefix if present.
        let stripped = raw
            .strip_prefix(r"\\?\")
            .or_else(|| raw.strip_prefix(r"\\.\"))
            .unwrap_or(&raw);
        stripped.replace('\\', "/")
    };

    // Modelfile text (older Ollama API): "FROM /path/to/file.gguf"
    let modelfile = format!("FROM {}\n", abs_path);

    let create_url = format!("{}/api/create", ollama::base_url());

    // Attempt 1: modern API — `from` field + `modelfile` text.
    let body_v1 = serde_json::json!({
        "name": name,
        "from": abs_path,
        "modelfile": modelfile,
        "stream": false,
    });

    let resp = state
        .client
        .post(&create_url)
        .json(&body_v1)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await
        .map_err(|e| format!("Failed to call Ollama: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();

        // If the modern API rejected our request, try the legacy API:
        // modelfile text only, no `from` field. This handles older Ollama
        // versions that don't recognize `from` and reject the whole request.
        if status == 400 {
            let body_v2 = serde_json::json!({
                "name": name,
                "modelfile": modelfile,
                "stream": false,
            });
            let resp2 = state
                .client
                .post(&create_url)
                .json(&body_v2)
                .timeout(std::time::Duration::from_secs(600))
                .send()
                .await
                .map_err(|e| format!("Failed to call Ollama (retry): {}", e))?;

            if resp2.status().is_success() {
                return Ok(());
            }
            let status2 = resp2.status();
            let body2 = resp2.text().await.unwrap_or_default();
            return Err(format!(
                "Ollama rejected the import on both API variants.\n\nAttempt 1 (modern `from` field): HTTP {} — {}\nAttempt 2 (legacy `modelfile` only): HTTP {} — {}\n\nLikely causes:\n  • Your Ollama version is older than 0.5. Update from https://ollama.com/download\n  • The GGUF file is corrupt or incomplete. Re-download it from the source.\n  • The file path contains characters Ollama can't parse. Move the file to a simple path like C:\\\\models\\\\model.gguf and retry.",
                status, body_text, status2, body2
            ));
        }

        return Err(format!("Ollama returned HTTP {}: {}", status, body_text));
    }

    Ok(())
}
