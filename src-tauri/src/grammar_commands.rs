//! Tauri command layer for the Proofread tab (v2.3.0 — Phases 1, 2a, 2b, 3).
//!
//! Commands: grammar_check, grammar_check_docx, grammar_apply_docx_fix,
//! grammar_record_decision, grammar_ledger_export, grammar_engine_info,
//! grammar_session_reset, grammar_tier2_enable/disable/status/suggest.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::audit::AuditLog;
use crate::grammar::{
    self, CheckResult, DecisionAction, DialectName, GrammarError, LedgerDecision, LedgerFinding,
    LedgerSummary, TextFormat, Tier,
};
use crate::ollama;
use crate::tier2::{self, Tier2Error, Tier2Request, Tier2Result};

pub struct ProofreadSession {
    ledger: Mutex<LedgerSummary>,
    session_id: AtomicU64,
    tier2_enabled: std::sync::atomic::AtomicBool,
}

impl Default for ProofreadSession {
    fn default() -> Self {
        let mut id_bytes = [0u8; 8];
        let _ = getrandom::getrandom(&mut id_bytes);
        let session_id = u64::from_le_bytes(id_bytes);
        Self {
            ledger: Mutex::new(LedgerSummary {
                session_id,
                ..LedgerSummary::default()
            }),
            session_id: AtomicU64::new(session_id),
            tier2_enabled: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl ProofreadSession {
    async fn record_findings(&self, result: &CheckResult) {
        let mut ledger = self.ledger.lock().await;
        ledger.engine_name = result.engine.name.clone();
        ledger.engine_version = result.engine.version.clone();
        ledger.dialect = result.dialect;
        ledger.chars_scanned = result.total_chars as u32;
        ledger.sentences_scanned = ledger.sentences_scanned.saturating_add(1);
        ledger.mask_count = ledger
            .mask_count
            .saturating_add(result.masked_spans.len() as u32);
        for f in &result.lints {
            ledger.findings.push(LedgerFinding {
                sentence_index: 0,
                tier: f.tier,
                rule_kind: f.rule_kind,
                rule_id: f.rule_id,
                suggestion_count: f.suggestions.len() as u32,
            });
        }
        ledger.deferred_count = ledger
            .deferred_count
            .saturating_add(result.lints.len() as u32);
    }
    async fn record_decision(&self, decision: LedgerDecision) {
        let mut ledger = self.ledger.lock().await;
        match decision.action {
            DecisionAction::Accepted => {
                ledger.accepted_count = ledger.accepted_count.saturating_add(1);
                ledger.total_chars_changed = ledger
                    .total_chars_changed
                    .saturating_add(decision.chars_changed);
            }
            DecisionAction::Rejected => {
                ledger.rejected_count = ledger.rejected_count.saturating_add(1);
            }
            DecisionAction::Ignored => {
                ledger.ignored_count = ledger.ignored_count.saturating_add(1);
            }
            DecisionAction::Deferred => {
                ledger.deferred_count = ledger.deferred_count.saturating_add(1);
            }
        }
        ledger.decisions.push(decision);
    }
    async fn ledger_snapshot(&self) -> LedgerSummary {
        let mut ledger = self.ledger.lock().await.clone();
        ledger.generated_at = now_unix();
        ledger.generator = generator_string().to_string();
        ledger.tier2_enabled = self.tier2_enabled.load(Ordering::Relaxed);
        ledger
    }
    async fn reset(&self) {
        let session_id = self.session_id.load(Ordering::Relaxed);
        let mut ledger = self.ledger.lock().await;
        *ledger = LedgerSummary {
            session_id,
            ..LedgerSummary::default()
        };
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn generator_string() -> &'static str {
    static GENERATOR: once_cell::sync::Lazy<&'static str> = once_cell::sync::Lazy::new(|| {
        Box::leak(format!("scholarscribe-{}", env!("CARGO_PKG_VERSION")).into_boxed_str())
    });
    *GENERATOR
}

#[derive(Debug, Deserialize)]
pub struct GrammarCheckArgs {
    pub text: String,
    #[serde(default)]
    pub format: Option<TextFormat>,
    #[serde(default)]
    pub dialect: Option<DialectName>,
    #[serde(default)]
    pub ignored_rules: Option<Vec<u32>>,
    #[serde(default)]
    pub personal_words: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct GrammarRecordDecisionArgs {
    pub action: DecisionAction,
    pub tier: Tier,
    pub rule_kind: grammar::RuleKind,
    pub rule_id: u32,
    #[serde(default)]
    pub chars_changed: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GrammarCheckDocxArgs {
    pub path: String,
    #[serde(default)]
    pub dialect: Option<DialectName>,
    #[serde(default)]
    pub ignored_rules: Option<Vec<u32>>,
    #[serde(default)]
    pub personal_words: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct GrammarApplyDocxFixArgs {
    pub input_path: String,
    pub output_path: String,
    pub finding: grammar::LintFinding,
    pub replacement: String,
}

#[derive(Debug, Deserialize)]
pub struct GrammarTier2SuggestArgs {
    pub sentence: String,
    pub dialect: DialectName,
    pub model: String,
}

// Phase 1 commands
#[tauri::command]
pub async fn grammar_check(
    args: GrammarCheckArgs,
    audit: State<'_, AuditLog>,
    session: State<'_, ProofreadSession>,
) -> Result<CheckResult, String> {
    let format = args.format.unwrap_or_default();
    let dialect = args.dialect.unwrap_or_default();
    let ignored_rules = args.ignored_rules.unwrap_or_default();
    let personal_words = args.personal_words.unwrap_or_default();
    audit.record(
        "proofread_check",
        "local",
        &format!(
            "tier1 check, {} chars, dialect={:?}",
            args.text.chars().count(),
            dialect
        ),
        0,
        0,
    );
    let result = grammar::check(&args.text, format, dialect, &ignored_rules, &personal_words)
        .map_err(grammar_error_to_string)?;
    session.record_findings(&result).await;
    Ok(result)
}

#[tauri::command]
pub async fn grammar_record_decision(
    args: GrammarRecordDecisionArgs,
    session: State<'_, ProofreadSession>,
) -> Result<(), String> {
    session
        .record_decision(LedgerDecision {
            action: args.action,
            tier: args.tier,
            rule_kind: args.rule_kind,
            rule_id: args.rule_id,
            chars_changed: args.chars_changed.unwrap_or(0),
        })
        .await;
    Ok(())
}

#[tauri::command]
pub async fn grammar_ledger_export(
    session: State<'_, ProofreadSession>,
) -> Result<LedgerSummary, String> {
    Ok(session.ledger_snapshot().await)
}

#[tauri::command]
pub fn grammar_engine_info() -> grammar::EngineInfo {
    grammar::engine_info()
}

#[tauri::command]
pub async fn grammar_session_reset(session: State<'_, ProofreadSession>) -> Result<(), String> {
    session.reset().await;
    Ok(())
}

// Phase 2a/2b commands
#[tauri::command]
pub async fn grammar_check_docx(
    args: GrammarCheckDocxArgs,
    audit: State<'_, AuditLog>,
    session: State<'_, ProofreadSession>,
) -> Result<grammar::DocxCheckResult, String> {
    let dialect = args.dialect.unwrap_or_default();
    let ignored_rules = args.ignored_rules.unwrap_or_default();
    let personal_words = args.personal_words.unwrap_or_default();
    let path = std::path::PathBuf::from(&args.path);
    let path_for_extract = path.clone();
    let (text, paragraph_map, run_spans) = tokio::task::spawn_blocking(move || {
        crate::docx_reading::extract_text_with_runs(&path_for_extract)
    })
    .await
    .map_err(|e| format!("docx extraction task failed: {e}"))??;
    let byte_count = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    audit.record(
        "file_read",
        &args.path,
        &format!(
            "proofread .docx, {} paragraphs, {} chars",
            paragraph_map.paragraph_count(),
            text.chars().count()
        ),
        byte_count,
        0,
    );
    let mut result = grammar::check_docx(
        &text,
        &paragraph_map,
        &run_spans,
        dialect,
        &ignored_rules,
        &personal_words,
    )
    .map_err(grammar_error_to_string)?;
    result.source_path = args.path.clone();
    let check_for_ledger = grammar::CheckResult {
        engine: result.engine.clone(),
        dialect: result.dialect,
        lints: result.findings.iter().map(|f| f.finding.clone()).collect(),
        masked_spans: result.masked_spans.clone(),
        total_chars: result.total_chars,
    };
    session.record_findings(&check_for_ledger).await;
    Ok(result)
}

#[tauri::command]
pub async fn grammar_apply_docx_fix(
    args: GrammarApplyDocxFixArgs,
    audit: State<'_, AuditLog>,
    session: State<'_, ProofreadSession>,
) -> Result<grammar::DocxApplyResult, String> {
    if args.input_path == args.output_path {
        return Err(
            "output_path must differ from input_path (H7: never overwrite original)".into(),
        );
    }
    let input_path = std::path::PathBuf::from(&args.input_path);
    let input_for_extract = input_path.clone();
    let (_text, _map, run_spans) = tokio::task::spawn_blocking(move || {
        crate::docx_reading::extract_text_with_runs(&input_for_extract)
    })
    .await
    .map_err(|e| format!("docx extraction task failed: {e}"))??;

    let input_bytes =
        std::fs::read(&input_path).map_err(|e| format!("read {}: {}", input_path.display(), e))?;
    let cursor = std::io::Cursor::new(input_bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("not a readable .docx ({e})"))?;
    let mut document_xml_bytes = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {}: {}", i, e))?;
        if entry.name() == "word/document.xml" {
            use std::io::Read as _;
            entry
                .read_to_end(&mut document_xml_bytes)
                .map_err(|e| format!("read document.xml: {}", e))?;
            break;
        }
    }
    let document_xml = String::from_utf8_lossy(&document_xml_bytes).into_owned();
    let new_xml = grammar::apply_suggestion_to_docx_xml(
        &document_xml,
        &run_spans,
        &args.finding,
        &args.replacement,
    )
    .map_err(grammar_error_to_string)?;

    // Write new .docx: copy every entry, substituting modified document.xml.
    let output_path = std::path::PathBuf::from(&args.output_path);
    let output_cursor = std::io::Cursor::new(Vec::new());
    let mut zip_writer = zip::ZipWriter::new(output_cursor);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    let cursor_in = std::io::Cursor::new(std::fs::read(&input_path).map_err(|e| e.to_string())?);
    let mut archive_in = zip::ZipArchive::new(cursor_in).map_err(|e| e.to_string())?;
    for i in 0..archive_in.len() {
        let mut entry = archive_in.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        let is_compressed = entry.compression() != zip::CompressionMethod::Stored;
        let mode = (entry.unix_mode() as u16).wrapping_add(0);
        drop(entry);
        if name == "word/document.xml" {
            zip_writer
                .start_file(&name, options)
                .map_err(|e| e.to_string())?;
            use std::io::Write as _;
            zip_writer
                .write_all(new_xml.as_bytes())
                .map_err(|e| e.to_string())?;
        } else {
            let mut data = Vec::new();
            use std::io::Read as _;
            archive_in
                .by_index(i)
                .map_err(|e| e.to_string())?
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            let opts = if is_compressed {
                options
            } else {
                options.compression_method(zip::CompressionMethod::Stored)
            };
            zip_writer
                .start_file(&name, opts)
                .map_err(|e| e.to_string())?;
            use std::io::Write as _;
            zip_writer.write_all(&data).map_err(|e| e.to_string())?;
        }
    }
    let zip_bytes = zip_writer.finish().map_err(|e| e.to_string())?.into_inner();
    std::fs::write(&output_path, &zip_bytes)
        .map_err(|e| format!("write {}: {}", output_path.display(), e))?;

    let output_bytes = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    audit.record(
        "file_write",
        &args.output_path,
        "proofread apply-docx-fix (single run-span edit)",
        0,
        output_bytes,
    );
    session
        .record_decision(LedgerDecision {
            action: DecisionAction::Accepted,
            tier: Tier::Harper,
            rule_kind: args.finding.rule_kind,
            rule_id: args.finding.rule_id,
            chars_changed: (args.finding.end_char - args.finding.start_char) as u32,
        })
        .await;
    Ok(grammar::DocxApplyResult {
        output_path: args.output_path,
        applied_count: 1,
        skipped_count: 0,
        skipped_summary: vec![],
    })
}

// Phase 3 commands (gated, off by default)
#[tauri::command]
pub fn grammar_tier2_enable(session: State<'_, ProofreadSession>) -> Result<(), String> {
    session.tier2_enabled.store(true, Ordering::Relaxed);
    Ok(())
}
#[tauri::command]
pub fn grammar_tier2_disable(session: State<'_, ProofreadSession>) -> Result<(), String> {
    session.tier2_enabled.store(false, Ordering::Relaxed);
    Ok(())
}
#[tauri::command]
pub fn grammar_tier2_status(session: State<'_, ProofreadSession>) -> Result<bool, String> {
    Ok(session.tier2_enabled.load(Ordering::Relaxed))
}

#[tauri::command]
pub async fn grammar_tier2_suggest(
    args: GrammarTier2SuggestArgs,
    audit: State<'_, AuditLog>,
    session: State<'_, ProofreadSession>,
    ollama_state: State<'_, ollama::OllamaState>,
) -> Result<Tier2Result, String> {
    if !session.tier2_enabled.load(Ordering::Relaxed) {
        return Err(Tier2Error::NotEnabled.to_string());
    }
    let base = ollama::base_url();
    audit.record(
        "ollama_command",
        &format!("{}/api/chat", base),
        &format!(
            "tier2 minimal-edit, {} chars, model={}",
            args.sentence.chars().count(),
            args.model
        ),
        0,
        0,
    );
    let request = Tier2Request {
        sentence: args.sentence,
        dialect: args.dialect,
        model: args.model,
    };
    let result = tier2::suggest_minimal_edit(&ollama_state.client, base, &request)
        .await
        .map_err(tier2_error_to_string)?;
    Ok(result)
}

fn grammar_error_to_string(e: GrammarError) -> String {
    e.to_string()
}
fn tier2_error_to_string(e: Tier2Error) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_records_findings_and_decisions() {
        let session = ProofreadSession::default();
        let result = grammar::check(
            "This are a test with eror.",
            TextFormat::Plain,
            DialectName::American,
            &[],
            &[],
        )
        .unwrap();
        session.record_findings(&result).await;
        session
            .record_decision(LedgerDecision {
                action: DecisionAction::Accepted,
                tier: Tier::Harper,
                rule_kind: grammar::RuleKind::Spelling,
                rule_id: 1,
                chars_changed: 5,
            })
            .await;
        let ledger = session.ledger_snapshot().await;
        assert!(!ledger.findings.is_empty());
        assert_eq!(ledger.accepted_count, 1);
    }

    #[test]
    fn ledger_summary_serializes_no_user_text() {
        let probe = "PRIVACY_PROBE_5f8a3c2e1b9d";
        let text = format!("{} is a test sentence with erors.", probe);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let session = rt.block_on(async {
            let s = ProofreadSession::default();
            let result =
                grammar::check(&text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
            s.record_findings(&result).await;
            s
        });
        let ledger = rt.block_on(session.ledger_snapshot());
        let json = serde_json::to_string(&ledger).unwrap();
        assert!(!json.contains(probe), "H5 VIOLATION: {}", json);
    }
}
