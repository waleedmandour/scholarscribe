// ScholarScribe — library entry point.
//
// ETHICAL DESIGN NOTICE:
// This software does NOT include any feature whose purpose is to evade
// AI-detection systems (Turnitin, GPTZero, Originality.ai, etc.). The
// style-analysis module compares a draft against the *author's own* prior
// writing — it is not designed to lower detector scores. See README.md
// "Ethical Use" section for the full policy.

mod abstract_generator;
mod audit;
mod citation_manager;
mod commands;
mod disclosure;
mod document_stats;
mod docx_reading;
mod ollama;
mod persistence;
mod structure_analyzer;
mod style;
mod text_cleaner;

pub use audit::AuditLog;

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(ollama::OllamaState::default())
        .manage(audit::AuditLog::default())
        .setup(|app| {
            log::info!(
                "ScholarScribe v{} starting up. All processing is local.",
                app.package_info().version
            );
            log::info!("No telemetry. No cloud calls. No third-party APIs.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::ollama_status,
            commands::ollama_list_models,
            commands::ollama_pull_model,
            commands::ollama_delete_model,
            commands::ollama_chat,
            commands::recommended_models,
            commands::read_text_file,
            commands::analyze_style,
            commands::compare_style,
            commands::list_venue_templates,
            commands::generate_disclosure,
            commands::system_info,
            commands::ollama_import_gguf,
            commands::check_gguf_compatibility,
            commands::clean_text,
            commands::clean_text_strict,
            commands::strict_clean_options,
            commands::clean_docx_file,
            commands::clean_docx_preserve_format,
            commands::validate_citations,
            commands::document_stats,
            commands::analyze_structure,
            commands::analyze_structure_text,
            commands::generate_abstract,
            audit::audit_list,
            audit::audit_clear,
            audit::audit_summary,
            persistence::settings_get,
            persistence::settings_set,
            persistence::persistence_enable,
            persistence::persistence_disable,
            persistence::persistence_status,
            persistence::draft_save,
            persistence::draft_update,
            persistence::draft_load,
            persistence::draft_list,
            persistence::draft_delete,
            persistence::draft_delete_all,
            persistence::data_dir_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ScholarScribe");
}
