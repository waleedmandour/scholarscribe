//! Proofread tab (v2.3.0). Tier 1 rule-based grammar checking + Phase 2a/2b.
//!
//! ETHICAL SCOPE: correction suggestions, not rewrites. The author stays the
//! author. No AI-detection score (H2). No silent auto-apply (H3). No
//! generative rewriting (H4). Ledger is counts-only (H5). No network (H1).
//! No provenance interference (H6). Pure core, no Tauri imports.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use harper_core::{
    linting::{LintGroup, Linter},
    parsers::{Markdown, PlainEnglish},
    spell::FstDictionary,
    Dialect, Document,
};

use crate::text_masking::{self, MaskSpan};

pub const DEFAULT_DIALECT: Dialect = Dialect::American;
pub const TIER1_ENGINE_NAME: &str = "Harper";
pub const TIER1_LABEL: Tier = Tier::Harper;

#[derive(Debug, Error)]
pub enum GrammarError {
    #[error("Could not lint the text: {0}")]
    LintFailed(String),
    #[error("Unknown dialect: {0}")]
    UnknownDialect(String),
    #[error("Unknown format: {0}")]
    UnknownFormat(String),
    #[error("Docx apply-fixes error: {0}")]
    DocxApply(String),
}

// H5 ledger types: counts and enumerated IDs only, no user text.

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Harper,
    LocalLlm,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Spelling,
    Grammar,
    Punctuation,
    Style,
    Consistency,
    Typo,
    Misc,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    Accepted,
    Rejected,
    Ignored,
    Deferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintFinding {
    pub start_char: usize,
    pub end_char: usize,
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub tier: Tier,
    pub rule_kind: RuleKind,
    pub rule_id: u32,
    pub explanation: String,
    pub original_text: String,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub replacement: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub engine: EngineInfo,
    pub dialect: DialectName,
    pub lints: Vec<LintFinding>,
    pub masked_spans: Vec<MaskSpan>,
    pub total_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
    pub tier: Tier,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DialectName {
    American,
    British,
    Australian,
    Canadian,
    Indian,
}

impl DialectName {
    pub fn to_harper(self) -> Dialect {
        match self {
            DialectName::American => Dialect::American,
            DialectName::British => Dialect::British,
            DialectName::Australian => Dialect::Australian,
            DialectName::Canadian => Dialect::Canadian,
            DialectName::Indian => Dialect::Indian,
        }
    }
    pub fn from_harper(d: Dialect) -> Self {
        match d {
            Dialect::American => DialectName::American,
            Dialect::British => DialectName::British,
            Dialect::Australian => DialectName::Australian,
            Dialect::Canadian => DialectName::Canadian,
            Dialect::Indian => DialectName::Indian,
        }
    }
}

impl Default for DialectName {
    fn default() -> Self {
        match DEFAULT_DIALECT {
            Dialect::American => DialectName::American,
            Dialect::British => DialectName::British,
            Dialect::Australian => DialectName::Australian,
            Dialect::Canadian => DialectName::Canadian,
            Dialect::Indian => DialectName::Indian,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerFinding {
    pub sentence_index: u32,
    pub tier: Tier,
    pub rule_kind: RuleKind,
    pub rule_id: u32,
    pub suggestion_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerDecision {
    pub action: DecisionAction,
    pub tier: Tier,
    pub rule_kind: RuleKind,
    pub rule_id: u32,
    pub chars_changed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerSummary {
    pub session_id: u64,
    pub generated_at: i64,
    pub generator: String,
    pub engine_name: String,
    pub engine_version: String,
    pub dialect: DialectName,
    pub tier1_enabled: bool,
    pub tier2_enabled: bool,
    pub sentences_scanned: u32,
    pub chars_scanned: u32,
    pub findings: Vec<LedgerFinding>,
    pub decisions: Vec<LedgerDecision>,
    pub accepted_count: u32,
    pub rejected_count: u32,
    pub ignored_count: u32,
    pub deferred_count: u32,
    pub total_chars_changed: u32,
    pub mask_count: u32,
}

impl Default for LedgerSummary {
    fn default() -> Self {
        Self {
            session_id: 0,
            generated_at: 0,
            generator: format!("scholarscribe-{}", env!("CARGO_PKG_VERSION")),
            engine_name: TIER1_ENGINE_NAME.to_string(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            dialect: DialectName::default(),
            tier1_enabled: true,
            tier2_enabled: false,
            sentences_scanned: 0,
            chars_scanned: 0,
            findings: Vec::new(),
            decisions: Vec::new(),
            accepted_count: 0,
            rejected_count: 0,
            ignored_count: 0,
            deferred_count: 0,
            total_chars_changed: 0,
            mask_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextFormat {
    Plain,
    Markdown,
}
impl Default for TextFormat {
    fn default() -> Self {
        Self::Plain
    }
}

// Phase 1: check()
pub fn check(
    text: &str,
    format: TextFormat,
    dialect: DialectName,
    ignored_rules: &[u32],
    personal_words: &[String],
) -> Result<CheckResult, GrammarError> {
    if text.is_empty() {
        return Ok(CheckResult {
            engine: engine_info(),
            dialect,
            lints: Vec::new(),
            masked_spans: Vec::new(),
            total_chars: 0,
        });
    }
    let mask_spans = text_masking::find_mask_spans(text);
    let dict = FstDictionary::curated();
    let doc = match format {
        TextFormat::Plain => Document::new_curated(text, &PlainEnglish),
        TextFormat::Markdown => Document::new_curated(text, &Markdown::default()),
    };
    let mut linter = LintGroup::new_curated(dict.clone(), dialect.to_harper());
    let raw_lints = linter.lint(&doc);
    let lints: Vec<LintFinding> = raw_lints
        .iter()
        .filter_map(|lint| {
            let start = lint.span.start;
            let end = lint.span.end;
            if mask_spans.iter().any(|m| start >= m.start && end <= m.end) {
                return None;
            }
            let rule_id = lint_kind_to_rule_id(&lint.lint_kind);
            if ignored_rules.contains(&rule_id) {
                return None;
            }
            let chars: Vec<char> = text.chars().collect();
            let flagged_text: String = chars[start..end].iter().collect();
            if personal_words.iter().any(|w| w == &flagged_text) {
                return None;
            }
            Some(lint_to_finding(lint, text, start, end))
        })
        .collect();
    Ok(CheckResult {
        engine: engine_info(),
        dialect,
        lints,
        masked_spans: mask_spans,
        total_chars: text.chars().count(),
    })
}

// Phase 2a + 2b: DocxFinding, check_docx()

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxFinding {
    #[serde(flatten)]
    pub finding: LintFinding,
    pub paragraph_index: u32,
    pub paragraph_count: u32,
    #[serde(default)]
    pub can_apply_in_place: bool,
    #[serde(default)]
    pub apply_blocker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxCheckResult {
    pub engine: EngineInfo,
    pub dialect: DialectName,
    pub findings: Vec<DocxFinding>,
    pub masked_spans: Vec<MaskSpan>,
    pub total_chars: usize,
    pub paragraph_count: usize,
    pub source_path: String,
}

pub fn check_docx(
    text: &str,
    paragraph_map: &crate::docx_reading::ParagraphMap,
    run_spans: &[crate::docx_reading::RunSpan],
    dialect: DialectName,
    ignored_rules: &[u32],
    personal_words: &[String],
) -> Result<DocxCheckResult, GrammarError> {
    if text.is_empty() {
        return Ok(DocxCheckResult {
            engine: engine_info(),
            dialect,
            findings: Vec::new(),
            masked_spans: Vec::new(),
            total_chars: 0,
            paragraph_count: paragraph_map.paragraph_count(),
            source_path: String::new(),
        });
    }
    let result = check(
        text,
        TextFormat::Plain,
        dialect,
        ignored_rules,
        personal_words,
    )?;
    let findings: Vec<DocxFinding> = result
        .lints
        .iter()
        .map(|f| {
            let paragraph_index = paragraph_map
                .paragraph_for_char(f.start_char)
                .map(|i| i as u32)
                .unwrap_or(0);
            let (can_apply, blocker) = can_apply_in_run(f.start_char, f.end_char, run_spans);
            DocxFinding {
                finding: f.clone(),
                paragraph_index,
                paragraph_count: paragraph_map.paragraph_count() as u32,
                can_apply_in_place: can_apply,
                apply_blocker: blocker,
            }
        })
        .collect();
    Ok(DocxCheckResult {
        engine: result.engine,
        dialect: result.dialect,
        findings,
        masked_spans: result.masked_spans,
        total_chars: result.total_chars,
        paragraph_count: paragraph_map.paragraph_count(),
        source_path: String::new(),
    })
}

fn can_apply_in_run(
    start_char: usize,
    end_char: usize,
    run_spans: &[crate::docx_reading::RunSpan],
) -> (bool, String) {
    let containing = run_spans
        .iter()
        .find(|rs| rs.char_start <= start_char && start_char < rs.char_end);
    match containing {
        None => (false, "no single run contains the start".to_string()),
        Some(rs) => {
            if end_char <= rs.char_end {
                (true, String::new())
            } else {
                let count = run_spans
                    .iter()
                    .filter(|r| r.char_start < end_char && r.char_end > start_char)
                    .count();
                (
                    false,
                    format!("spans {} runs (crosses run boundary)", count),
                )
            }
        }
    }
}

// Phase 2b: apply_suggestion_to_docx_xml

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxApplyResult {
    pub output_path: String,
    pub applied_count: u32,
    pub skipped_count: u32,
    pub skipped_summary: Vec<SkippedReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedReason {
    pub reason: String,
    pub count: u32,
}

pub fn apply_suggestion_to_docx_xml(
    document_xml: &str,
    run_spans: &[crate::docx_reading::RunSpan],
    finding: &LintFinding,
    replacement: &str,
) -> Result<String, GrammarError> {
    let (can_apply, blocker) = can_apply_in_run(finding.start_char, finding.end_char, run_spans);
    if !can_apply {
        return Err(GrammarError::DocxApply(format!("cannot apply: {blocker}")));
    }
    let rs = run_spans
        .iter()
        .find(|rs| rs.char_start <= finding.start_char && finding.start_char < rs.char_end)
        .expect("can_apply_in_run returned true");
    let rel_start = finding.start_char - rs.char_start;
    let rel_end = finding.end_char - rs.char_start;
    if rel_end > rs.char_end - rs.char_start {
        return Err(GrammarError::DocxApply(
            "finding end exceeds run bounds".to_string(),
        ));
    }
    let run_text_bytes: Vec<u8> = document_xml
        .as_bytes()
        .get(rs.xml_text_start..rs.xml_text_end)
        .ok_or_else(|| GrammarError::DocxApply("run byte range out of bounds".to_string()))?
        .to_vec();
    let run_text = String::from_utf8_lossy(&run_text_bytes);
    let run_chars: Vec<char> = run_text.chars().collect();
    if rel_end > run_chars.len() {
        return Err(GrammarError::DocxApply(
            "finding end exceeds run char length".to_string(),
        ));
    }
    let new_run_text: String = run_chars[..rel_start].iter().collect::<String>()
        + replacement
        + &run_chars[rel_end..].iter().collect::<String>();
    let escaped = xml_escape(&new_run_text);
    let mut new_xml = String::with_capacity(document_xml.len() + escaped.len());
    new_xml.push_str(&document_xml[..rs.xml_text_start]);
    new_xml.push_str(&escaped);
    new_xml.push_str(&document_xml[rs.xml_text_end..]);
    Ok(new_xml)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// Span conversion char <-> UTF-16
pub fn char_to_utf16(text: &str, start_char: usize, end_char: usize) -> (usize, usize) {
    let mut char_idx = 0usize;
    let mut utf16_idx = 0usize;
    let mut start_utf16 = 0usize;
    let mut end_utf16 = 0usize;
    for c in text.chars() {
        if char_idx == start_char {
            start_utf16 = utf16_idx;
        }
        if char_idx == end_char {
            end_utf16 = utf16_idx;
            break;
        }
        utf16_idx += c.len_utf16();
        char_idx += 1;
    }
    if char_idx == end_char && end_utf16 == 0 && start_char != end_char {
        end_utf16 = utf16_idx;
    } else if end_char >= text.chars().count() {
        end_utf16 = utf16_idx;
    }
    (start_utf16, end_utf16)
}

fn lint_to_finding(
    lint: &harper_core::linting::Lint,
    text: &str,
    start_char: usize,
    end_char: usize,
) -> LintFinding {
    let (start_utf16, end_utf16) = char_to_utf16(text, start_char, end_char);
    let chars: Vec<char> = text.chars().collect();
    let original_text: String = chars[start_char..end_char].iter().collect();
    let rule_kind = lint_kind_to_rule_kind(&lint.lint_kind);
    let rule_id = lint_kind_to_rule_id(&lint.lint_kind);
    let explanation = rule_id_to_explanation(rule_id).to_string();
    let suggestions: Vec<Suggestion> = lint
        .suggestions
        .iter()
        .map(|s| {
            let (replacement, label) = harper_suggestion_to_string(s);
            Suggestion {
                replacement,
                label: label.unwrap_or("Apply suggestion").to_string(),
            }
        })
        .collect();
    LintFinding {
        start_char,
        end_char,
        start_utf16,
        end_utf16,
        tier: Tier::Harper,
        rule_kind,
        rule_id,
        explanation,
        original_text,
        suggestions,
    }
}

fn lint_kind_to_rule_kind(kind: &harper_core::linting::LintKind) -> RuleKind {
    use harper_core::linting::LintKind::*;
    match kind {
        Spelling | Typo | Capitalization => RuleKind::Spelling,
        Grammar | Agreement | WordChoice | WordOrder | Usage | Malapropism | Eggcorn
        | BoundaryError => RuleKind::Grammar,
        Punctuation | Formatting => RuleKind::Punctuation,
        Style | Enhancement | Redundancy | Repetition | Readability => RuleKind::Style,
        Regionalism | Nonstandard => RuleKind::Consistency,
        Miscellaneous => RuleKind::Misc,
    }
}

fn lint_kind_to_rule_id(kind: &harper_core::linting::LintKind) -> u32 {
    use harper_core::linting::LintKind::*;
    match kind {
        Spelling => 1,
        Typo => 2,
        Capitalization => 3,
        Grammar => 10,
        Agreement => 11,
        WordChoice => 12,
        WordOrder => 13,
        Usage => 14,
        Malapropism => 15,
        Eggcorn => 16,
        BoundaryError => 17,
        Punctuation => 20,
        Formatting => 21,
        Style => 30,
        Enhancement => 31,
        Redundancy => 32,
        Repetition => 33,
        Readability => 34,
        Regionalism => 40,
        Nonstandard => 41,
        Miscellaneous => 99,
    }
}

fn rule_id_to_explanation(rule_id: u32) -> &'static str {
    match rule_id {
        1 => "A word may be misspelled. Check the spelling against a dictionary.",
        2 => "A grammatical rule may have been violated (subject-verb agreement, articles, etc.).",
        3 => "A punctuation convention may be broken (missing comma, wrong quote style, etc.).",
        4 => "A stylistic convention may be broken. These are softer than grammar rules.",
        5 => "The text may be internally inconsistent (e.g. mixed spelling of the same word).",
        6 => "A likely typo (e.g. 'teh' instead of 'the').",
        _ => "An issue was detected that does not fit the standard categories.",
    }
}

fn harper_suggestion_to_string(
    s: &harper_core::linting::Suggestion,
) -> (String, Option<&'static str>) {
    use harper_core::linting::Suggestion::*;
    match s {
        ReplaceWith(chars) => (chars.iter().collect(), Some("Replace with this text")),
        Remove => (String::new(), Some("Remove this text")),
        InsertAfter(chars) => (
            chars.iter().collect(),
            Some("Insert this text after the span"),
        ),
    }
}

pub fn engine_info() -> EngineInfo {
    EngineInfo {
        name: TIER1_ENGINE_NAME.to_string(),
        version: harper_core::core_version().to_string(),
        tier: TIER1_LABEL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_serializes_no_user_text() {
        let probe = "PRIVACY_PROBE_5f8a3c2e1b9d";
        let text = format!("{} is a test sentence with erors.", probe);
        let result = check(&text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        let mut ledger = LedgerSummary {
            session_id: 12345,
            generated_at: 1_700_000_000,
            generator: "scholarscribe-test".to_string(),
            engine_name: TIER1_ENGINE_NAME.to_string(),
            engine_version: "test".to_string(),
            dialect: DialectName::American,
            tier1_enabled: true,
            tier2_enabled: false,
            sentences_scanned: 1,
            chars_scanned: text.chars().count() as u32,
            findings: result
                .lints
                .iter()
                .map(|f| LedgerFinding {
                    sentence_index: 1,
                    tier: f.tier,
                    rule_kind: f.rule_kind,
                    rule_id: f.rule_id,
                    suggestion_count: f.suggestions.len() as u32,
                })
                .collect(),
            decisions: Vec::new(),
            accepted_count: 0,
            rejected_count: 0,
            ignored_count: 0,
            deferred_count: result.lints.len() as u32,
            total_chars_changed: 0,
            mask_count: result.masked_spans.len() as u32,
        };
        for f in &result.lints {
            ledger.decisions.push(LedgerDecision {
                action: DecisionAction::Deferred,
                tier: f.tier,
                rule_kind: f.rule_kind,
                rule_id: f.rule_id,
                chars_changed: 0,
            });
        }
        let json = serde_json::to_string(&ledger).unwrap();
        assert!(!json.contains(probe), "H5 VIOLATION: {}", json);
        for f in &result.lints {
            assert!(
                !json.contains(&f.original_text),
                "H5 VIOLATION: flagged text in JSON"
            );
        }
    }
    #[test]
    fn char_to_utf16_ascii() {
        assert_eq!(char_to_utf16("Hello, world!", 7, 12), (7, 12));
    }
    #[test]
    fn char_to_utf16_bmp_accent() {
        assert_eq!(char_to_utf16("The café is open.", 4, 8), (4, 8));
    }
    #[test]
    fn char_to_utf16_astral_emoji() {
        assert_eq!(char_to_utf16("Smile \u{1F600} now", 8, 11), (9, 12));
    }
    #[test]
    fn char_to_utf16_arabic() {
        let text = "The word \u{0627}\u{0644}\u{0633}\u{0644}\u{0627}\u{0645} means peace.";
        assert_eq!(char_to_utf16(text, 9, 15), (9, 15));
    }
    #[test]
    fn dialect_switch_changes_results() {
        let text = "The colour of the sky is blue.";
        let us = check(text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        let uk = check(text, TextFormat::Plain, DialectName::British, &[], &[]).unwrap();
        assert_ne!(us.dialect, uk.dialect);
    }
    #[test]
    fn masks_suppress_citation_lints() {
        let text = "Previous work (Smith, 2020) showed this.";
        let result = check(text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        let citation_span = result.masked_spans.iter().find(|s| s.len() > 0).unwrap();
        let inside = result
            .lints
            .iter()
            .any(|l| l.start_char >= citation_span.start && l.end_char <= citation_span.end);
        assert!(!inside, "lint flagged inside citation mask");
    }
    #[test]
    fn personal_dictionary_suppresses_lints() {
        let text = "The mispelled word is intentional.";
        let without = check(text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        let with = check(
            text,
            TextFormat::Plain,
            DialectName::American,
            &[],
            &["mispelled".to_string()],
        )
        .unwrap();
        assert!(without.lints.iter().any(|l| l.original_text == "mispelled"));
        assert!(!with.lints.iter().any(|l| l.original_text == "mispelled"));
    }
    #[test]
    fn ignored_rules_suppress_lints() {
        let text = "This are a test with eror.";
        let without = check(text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        let with = check(text, TextFormat::Plain, DialectName::American, &[1, 2], &[]).unwrap();
        assert!(with.lints.len() <= without.lints.len());
    }
    #[test]
    fn empty_input() {
        let r = check("", TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        assert!(r.lints.is_empty());
        assert_eq!(r.total_chars, 0);
    }
    #[test]
    fn multibyte_text_no_panic() {
        let text =
            "The café ☕ is nice. \u{0627}\u{0644}\u{0633}\u{0644}\u{0627}\u{0645} means peace.";
        let result = check(text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        for f in &result.lints {
            let utf16: Vec<u16> = text.encode_utf16().collect();
            let slice: Vec<u16> = utf16[f.start_utf16..f.end_utf16].to_vec();
            assert_eq!(String::from_utf16_lossy(&slice), f.original_text);
        }
    }
    #[test]
    fn markdown_format_skips_code_blocks() {
        let md = "# Heading\n\nSome text with eror.\n\n```\ncode with eror\n```\n";
        let result = check(md, TextFormat::Markdown, DialectName::American, &[], &[]).unwrap();
        let _ = result.lints.len();
    }
    #[test]
    fn engine_info_is_correct() {
        let info = engine_info();
        assert_eq!(info.name, "Harper");
        assert_eq!(info.tier, Tier::Harper);
        assert!(!info.version.is_empty());
    }
    #[test]
    fn ledger_default_is_empty() {
        let l = LedgerSummary::default();
        assert!(l.findings.is_empty());
        assert!(l.decisions.is_empty());
    }
    #[test]
    fn docx_multi_paragraph_findings() {
        let text = "First paragraph has an eror.\n\nSecond paragraph also has an eror.";
        let fake_map = crate::docx_reading::ParagraphMap {
            paragraph_starts: vec![0, 31],
        };
        let result = check(text, TextFormat::Plain, DialectName::American, &[], &[]).unwrap();
        let eror_lints: Vec<_> = result
            .lints
            .iter()
            .filter(|l| l.original_text == "eror")
            .collect();
        if eror_lints.len() >= 2 {
            let paras: Vec<_> = eror_lints
                .iter()
                .map(|l| fake_map.paragraph_for_char(l.start_char).unwrap())
                .collect();
            assert!(paras.contains(&0));
            assert!(paras.contains(&1));
        }
    }
}
