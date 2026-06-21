//! Tauri command handlers — the bridge between the Svelte frontend and Rust backend.

use crate::{audit, disclosure, docx_reading, ollama, style, text_cleaner};
use serde::Deserialize;
use std::path::{Path, PathBuf};
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
        "docx" => {
            // .docx files are ZIP archives containing XML — we use the
            // `docx-rs` crate to extract plain text. The extracted text is
            // suitable for cleaning, style analysis, or any other text op.
            let path_for_extract = path.clone();
            let extracted =
                tokio::task::spawn_blocking(move || docx_reading::extract_text_from_docx(&path_for_extract))
                    .await
                    .map_err(|e| format!("docx extraction task failed: {}", e))??;
            audit.record(
                "file_read",
                &args.path,
                "extracted text from .docx",
                extracted.len() as u64,
                0,
            );
            Ok(extracted)
        }
        "doc" => Err("ScholarScribe cannot read legacy .doc files (binary Word format). Please re-save as .docx in Word, or export as .txt/.md.".into()),
        other => Err(format!(
            "Unsupported file type: .{}. Supported: .txt, .md, .tex, .rst, .csv, .json, .docx",
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

// ---------------- v0.1.3 new commands ----------------

#[derive(Debug, Deserialize)]
pub struct CleanTextArgs {
    pub text: String,
    #[serde(default)]
    pub options: Option<text_cleaner::CleanOptions>,
}

#[tauri::command]
pub fn clean_text(args: CleanTextArgs) -> text_cleaner::CleanResult {
    let opts = args.options.unwrap_or_default();
    text_cleaner::clean(&args.text, &opts)
}

// ---------------- v0.1.7 strict cleaning ----------------

#[derive(Debug, Deserialize)]
pub struct CleanTextStrictArgs {
    pub text: String,
}

/// Strict cleaning — applies ALL 24 cleaning operations (the 12 default ones
/// plus the 11 new v0.1.7 strict ones, with normalize_quotes also enabled).
/// Use this when you want a maximally-clean plain-text version of the input.
#[tauri::command]
pub fn clean_text_strict(args: CleanTextStrictArgs) -> text_cleaner::CleanResult {
    let opts = text_cleaner::strict_options();
    text_cleaner::clean(&args.text, &opts)
}

/// Returns the strict-cleaning options preset (so the UI can show the user
/// which operations will be applied before they click "Strict clean").
#[tauri::command]
pub fn strict_clean_options() -> text_cleaner::CleanOptions {
    text_cleaner::strict_options()
}

// ---------------- v0.1.4 new commands ----------------

#[derive(Debug, Deserialize)]
pub struct CleanDocxArgs {
    pub path: String,
    #[serde(default)]
    pub options: Option<text_cleaner::CleanOptions>,
}

#[derive(Debug, serde::Serialize)]
pub struct CleanDocxResult {
    pub source_path: String,
    pub extracted: text_cleaner::CleanResult,
}

/// One-shot: read a .docx file, extract its text, run the text cleaner on it.
/// Returns the cleaned text plus transformation stats. The original .docx is
/// never modified — output is plain text the user can copy or save.
#[tauri::command]
pub async fn clean_docx_file(
    args: CleanDocxArgs,
    audit: State<'_, audit::AuditLog>,
) -> Result<CleanDocxResult, String> {
    let path = PathBuf::from(&args.path);
    if !path.exists() {
        return Err(format!("File not found: {}", args.path));
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext != "docx" {
        return Err(format!(
            "Expected a .docx file, got .{}. Use clean_text for plain-text input.",
            ext
        ));
    }

    audit.record("file_read", &args.path, "extract + clean .docx", 0, 0);

    // Extract text in a blocking task (docx-rs is sync).
    let path_for_extract = path.clone();
    let text = tokio::task::spawn_blocking(move || {
        docx_reading::extract_text_from_docx(&path_for_extract)
    })
    .await
    .map_err(|e| format!("docx extraction task failed: {}", e))??;

    // Run the cleaner (CPU-bound, also blocking).
    let opts = args.options.unwrap_or_default();
    let result = tokio::task::spawn_blocking(move || text_cleaner::clean(&text, &opts))
        .await
        .map_err(|e| format!("clean task failed: {}", e))?;

    Ok(CleanDocxResult {
        source_path: args.path,
        extracted: result,
    })
}

// ---------------- v0.1.5 new commands ----------------

#[derive(Debug, Deserialize)]
pub struct CleanDocxPreserveArgs {
    pub input_path: String,
    pub output_path: String,
    #[serde(default)]
    pub options: Option<text_cleaner::CleanOptions>,
}

#[derive(Debug, serde::Serialize)]
pub struct CleanDocxPreserveResult {
    pub output_path: String,
    pub parts_cleaned: Vec<String>,
    pub runs_cleaned: usize,
    pub stats: text_cleaner::CleanStats,
    pub transformations_applied: Vec<String>,
    pub skipped_operations: Vec<String>,
}

/// Clean a .docx file in place, preserving ALL formatting (tables, images,
/// hyperlinks, headers/footers, styles, theme, embedded objects).
///
/// How it works: a .docx is a ZIP of XML parts. We unzip, find every XML
/// part that contains document text (document.xml, header*.xml, footer*.xml,
/// footnotes.xml, endnotes.xml), and for each `<w:t>` element inside those
/// parts, apply per-run cleaning operations to the text content. Then we
/// write a new .docx ZIP with the modified XML parts, copying every other
/// part (images, styles, etc.) byte-for-byte from the original.
#[tauri::command]
pub async fn clean_docx_preserve_format(
    args: CleanDocxPreserveArgs,
    audit: State<'_, audit::AuditLog>,
) -> Result<CleanDocxPreserveResult, String> {
    let input = PathBuf::from(&args.input_path);
    let output = PathBuf::from(&args.output_path);

    if !input.exists() {
        return Err(format!("Input file not found: {}", args.input_path));
    }
    let in_ext = input
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if in_ext != "docx" {
        return Err(format!("Expected input .docx, got .{}", in_ext));
    }
    let out_ext = output
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if out_ext != "docx" {
        return Err(format!("Output path must end in .docx, got .{}", out_ext));
    }

    audit.record(
        "file_read",
        &args.input_path,
        "in-place clean .docx (preserve format)",
        0,
        0,
    );
    audit.record("file_write", &args.output_path, "write cleaned .docx", 0, 0);

    let opts = args.options.unwrap_or_default();
    let input_for_task = input.clone();
    let output_for_task = output.clone();
    let result = tokio::task::spawn_blocking(move || {
        clean_docx_in_place(&input_for_task, &output_for_task, &opts)
    })
    .await
    .map_err(|e| format!("docx preserve-format task failed: {}", e))??;

    Ok(result)
}

/// Implementation: walks every relevant XML part in the .docx, applies
/// per-run cleaning to each `<w:t>` element, writes a new .docx ZIP.
fn clean_docx_in_place(
    input: &Path,
    output: &Path,
    opts: &text_cleaner::CleanOptions,
) -> Result<CleanDocxPreserveResult, String> {
    use std::io::{Read, Write};

    let bytes = std::fs::read(input).map_err(|e| format!("read input: {}", e))?;
    let cursor = std::io::Cursor::new(bytes.clone());
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("open input .docx: {}", e))?;

    let text_part_pattern =
        regex::Regex::new(r"^word/(document|header\d+|footer\d+|footnotes|endnotes)\.xml$")
            .unwrap();

    let mut parts_cleaned: Vec<String> = Vec::new();
    let mut runs_cleaned: usize = 0;
    let mut stats = text_cleaner::CleanStats::default();
    let mut modified_parts: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {}: {}", i, e))?;
        let name = entry.name().to_string();
        if text_part_pattern.is_match(&name) {
            let mut xml = String::new();
            entry
                .read_to_string(&mut xml)
                .map_err(|e| format!("read {}: {}", name, e))?;
            let (cleaned_xml, run_count) = apply_per_run_cleaning(&xml, opts, &mut stats);
            runs_cleaned += run_count;
            parts_cleaned.push(name.clone());
            modified_parts.insert(name, cleaned_xml.into_bytes());
        }
    }

    // Write a new .docx by copying all entries from the original,
    // substituting the cleaned XML for the parts we modified.
    let output_cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(output_cursor);
    let opts_zip = zip::write::SimpleFileOptions::default();

    let cursor2 = std::io::Cursor::new(bytes);
    let mut archive2 =
        zip::ZipArchive::new(cursor2).map_err(|e| format!("reopen input .docx: {}", e))?;

    for i in 0..archive2.len() {
        let mut entry = archive2
            .by_index(i)
            .map_err(|e| format!("zip entry {} (pass 2): {}", i, e))?;
        let name = entry.name().to_string();
        let mut data: Vec<u8> = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| format!("read {} for copy: {}", name, e))?;
        drop(entry);

        let final_data: Vec<u8> = if let Some(cleaned) = modified_parts.remove(&name) {
            cleaned
        } else {
            data
        };

        writer
            .start_file(&name, opts_zip)
            .map_err(|e| format!("start_file {}: {}", name, e))?;
        writer
            .write_all(&final_data)
            .map_err(|e| format!("write {}: {}", name, e))?;
    }

    let final_bytes = writer
        .finish()
        .map_err(|e| format!("finalize .docx: {}", e))?
        .into_inner();
    std::fs::write(output, &final_bytes).map_err(|e| format!("write output file: {}", e))?;

    let mut transformations_applied = Vec::new();
    if opts.fix_mojibake && stats.mojibake_fixed > 0 {
        transformations_applied.push(format!(
            "Fixed mojibake ({} substitutions)",
            stats.mojibake_fixed
        ));
    }
    if opts.expand_ligatures && stats.ligatures_expanded > 0 {
        transformations_applied.push(format!(
            "Expanded ligatures ({} substitutions)",
            stats.ligatures_expanded
        ));
    }
    if opts.normalize_quotes && stats.quotes_normalized > 0 {
        transformations_applied.push(format!(
            "Normalized quotes ({} substitutions)",
            stats.quotes_normalized
        ));
    }
    if opts.normalize_dashes && stats.dashes_normalized > 0 {
        transformations_applied.push(format!(
            "Normalized dashes ({} substitutions)",
            stats.dashes_normalized
        ));
    }
    if opts.strip_zero_width && stats.zero_width_chars_stripped > 0 {
        transformations_applied.push(format!(
            "Stripped zero-width chars ({} removed)",
            stats.zero_width_chars_stripped
        ));
    }
    if opts.strip_control_chars && stats.control_chars_stripped > 0 {
        transformations_applied.push(format!(
            "Stripped control chars ({} removed)",
            stats.control_chars_stripped
        ));
    }
    if opts.join_hyphenated_words && stats.hyphenated_words_joined > 0 {
        transformations_applied.push(format!(
            "Joined hyphenated line breaks ({} joined)",
            stats.hyphenated_words_joined
        ));
    }
    if opts.collapse_whitespace && stats.whitespace_collapsed > 0 {
        transformations_applied.push(format!(
            "Collapsed whitespace ({} runs trimmed)",
            stats.whitespace_collapsed
        ));
    }
    if transformations_applied.is_empty() {
        transformations_applied
            .push("No per-run transformations needed — document was already clean.".into());
    }

    Ok(CleanDocxPreserveResult {
        output_path: output.to_string_lossy().into_owned(),
        parts_cleaned,
        runs_cleaned,
        stats,
        transformations_applied,
        skipped_operations: text_cleaner::skipped_docx_operations()
            .iter()
            .map(|s| s.to_string())
            .collect(),
    })
}

/// Apply per-run cleaning to every `<w:t>` element in the XML.
/// Returns (cleaned_xml, number_of_runs_cleaned).
fn apply_per_run_cleaning(
    xml: &str,
    opts: &text_cleaner::CleanOptions,
    stats: &mut text_cleaner::CleanStats,
) -> (String, usize) {
    let re = regex::Regex::new(r"(<w:t[^>]*>)([^<]*)(</w:t>)").unwrap();
    let mut runs_cleaned: usize = 0;

    let cleaned_xml = re
        .replace_all(xml, |caps: &regex::Captures| {
            let open = &caps[1];
            let text = &caps[2];
            let close = &caps[3];

            if text.is_empty() {
                return format!("{}{}{}", open, text, close);
            }

            let before = stats.clone();
            let cleaned = text_cleaner::clean_text_run(text, opts, stats);
            let changed = stats.mojibake_fixed != before.mojibake_fixed
                || stats.ligatures_expanded != before.ligatures_expanded
                || stats.quotes_normalized != before.quotes_normalized
                || stats.dashes_normalized != before.dashes_normalized
                || stats.zero_width_chars_stripped != before.zero_width_chars_stripped
                || stats.control_chars_stripped != before.control_chars_stripped
                || stats.hyphenated_words_joined != before.hyphenated_words_joined
                || stats.whitespace_collapsed != before.whitespace_collapsed;
            if changed {
                runs_cleaned += 1;
            }

            let escaped = xml_escape_safe(&cleaned);
            format!("{}{}{}", open, escaped, close)
        })
        .into_owned();

    (cleaned_xml, runs_cleaned)
}

/// XML-escape a string for safe insertion into OOXML. Only escapes chars
/// that aren't already part of a valid XML entity reference.
fn xml_escape_safe(s: &str) -> String {
    if !s.contains('&') && !s.contains('<') && !s.contains('>') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 16);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'&' {
            if let Some(end) = s[i..].find(';') {
                let entity = &s[i..i + end + 1];
                if is_valid_entity(entity) {
                    out.push_str(entity);
                    i += end + 1;
                    continue;
                }
            }
            out.push_str("&amp;");
            i += 1;
        } else if c == b'<' {
            out.push_str("&lt;");
            i += 1;
        } else if c == b'>' {
            out.push_str("&gt;");
            i += 1;
        } else {
            let ch_len = utf8_char_len(c);
            if i + ch_len <= bytes.len() {
                if let Ok(slice) = std::str::from_utf8(&bytes[i..i + ch_len]) {
                    out.push_str(slice);
                }
                i += ch_len;
            } else {
                i += 1;
            }
        }
    }
    out
}

fn is_valid_entity(s: &str) -> bool {
    if !s.starts_with('&') || !s.ends_with(';') {
        return false;
    }
    let inner = &s[1..s.len() - 1];
    if inner.is_empty() {
        return false;
    }
    if let Some(num) = inner.strip_prefix('#') {
        return num.chars().all(|c| c.is_ascii_digit())
            || num
                .strip_prefix('x')
                .map(|h| h.chars().all(|c| c.is_ascii_hexdigit()))
                .unwrap_or(false);
    }
    inner.chars().all(|c| c.is_ascii_alphanumeric())
}

fn utf8_char_len(first_byte: u8) -> usize {
    if first_byte < 0x80 {
        1
    } else if first_byte < 0xC0 {
        1
    } else if first_byte < 0xE0 {
        2
    } else if first_byte < 0xF0 {
        3
    } else {
        4
    }
}

// ---------------- v0.1.6 new commands ----------------

#[derive(Debug, Deserialize)]
pub struct ValidateCitationsArgs {
    pub draft_path: String,
    pub bib_path: String,
}

#[tauri::command]
pub async fn validate_citations(
    args: ValidateCitationsArgs,
    audit: State<'_, audit::AuditLog>,
) -> Result<crate::citation_manager::CitationReport, String> {
    use crate::citation_manager;

    let draft_path = PathBuf::from(&args.draft_path);
    let bib_path = PathBuf::from(&args.bib_path);

    if !draft_path.exists() {
        return Err(format!("Draft file not found: {}", args.draft_path));
    }
    if !bib_path.exists() {
        return Err(format!(".bib file not found: {}", args.bib_path));
    }

    audit.record("file_read", &args.draft_path, "citation validation", 0, 0);
    audit.record("file_read", &args.bib_path, "citation validation", 0, 0);

    let draft_path_for_task = draft_path.clone();
    let bib_path_for_task = bib_path.clone();

    let result = tokio::task::spawn_blocking(
        move || -> Result<crate::citation_manager::CitationReport, String> {
            // Read draft (handles .docx and text)
            let draft_text = if draft_path_for_task
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                == Some("docx".into())
            {
                crate::docx_reading::extract_text_from_docx(&draft_path_for_task)?
            } else {
                std::fs::read_to_string(&draft_path_for_task)
                    .map_err(|e| format!("read draft: {}", e))?
            };
            let bib_content = citation_manager::read_bib_file(&bib_path_for_task)?;
            let mut report = citation_manager::validate(&draft_text, &bib_content);
            report.draft_path = Some(draft_path_for_task.to_string_lossy().into_owned());
            report.bib_path = Some(bib_path_for_task.to_string_lossy().into_owned());
            Ok(report)
        },
    )
    .await
    .map_err(|e| format!("citation validation task failed: {}", e))??;

    Ok(result)
}

#[tauri::command]
pub async fn document_stats(text: String) -> Result<crate::document_stats::DocStats, String> {
    let result = tokio::task::spawn_blocking(move || crate::document_stats::analyze(&text))
        .await
        .map_err(|e| format!("document stats task failed: {}", e))?;
    Ok(result)
}

// ---------------- v0.1.8 new commands ----------------

#[derive(Debug, Deserialize)]
pub struct AnalyzeStructureArgs {
    pub path: String,
}

#[tauri::command]
pub async fn analyze_structure(
    args: AnalyzeStructureArgs,
    audit: State<'_, audit::AuditLog>,
) -> Result<crate::structure_analyzer::StructureReport, String> {
    let path = PathBuf::from(&args.path);
    if !path.exists() {
        return Err(format!("File not found: {}", args.path));
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    audit.record("file_read", &args.path, "structure analysis", 0, 0);

    let path_for_task = path.clone();
    let result = tokio::task::spawn_blocking(
        move || -> Result<crate::structure_analyzer::StructureReport, String> {
            if ext == "docx" {
                crate::structure_analyzer::analyze_docx(&path_for_task)
            } else {
                let text =
                    std::fs::read_to_string(&path_for_task).map_err(|e| format!("read: {}", e))?;
                Ok(crate::structure_analyzer::analyze_text(&text))
            }
        },
    )
    .await
    .map_err(|e| format!("structure analysis task failed: {}", e))??;

    Ok(result)
}

#[tauri::command]
pub async fn analyze_structure_text(
    text: String,
) -> Result<crate::structure_analyzer::StructureReport, String> {
    let result =
        tokio::task::spawn_blocking(move || crate::structure_analyzer::analyze_text(&text))
            .await
            .map_err(|e| format!("structure analysis task failed: {}", e))?;
    Ok(result)
}

#[derive(Debug, Deserialize)]
pub struct GenerateAbstractArgs {
    pub model: String,
    pub draft_text: String,
    pub max_words: Option<usize>,
    pub venue: Option<String>,
}

#[tauri::command]
pub async fn generate_abstract(
    args: GenerateAbstractArgs,
    state: State<'_, ollama::OllamaState>,
    audit: State<'_, audit::AuditLog>,
) -> Result<crate::abstract_generator::AbstractResult, String> {
    audit.record(
        "ollama_command",
        &format!("{}/api/chat", ollama::base_url()),
        &format!("generate abstract with model {}", args.model),
        0,
        0,
    );

    let req = crate::abstract_generator::AbstractRequest {
        model: args.model,
        draft_text: args.draft_text,
        max_words: args.max_words,
        venue: args.venue,
    };

    let client = &state.inner().client;
    crate::abstract_generator::generate_abstract(client, req)
        .await
        .map_err(|e| format!("[{}] {}", e.kind, e.message))
}
