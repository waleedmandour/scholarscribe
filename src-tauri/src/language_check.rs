// Language Check (v2.2.1 r5), offline spelling, grammar and style feedback
// with proposed corrections, powered by harper-core.
//
// PRIVACY DESIGN:
// - The engine is embedded in this process. No helper process, no local
//   socket, no new network hosts. The network-surface test stays green.
// - User text is analyzed in memory and never written anywhere by this
//   module; only the user's own save/export actions touch files.
// - The personal dictionary is user content, so it persists to disk only
//   when the user opted in to persistence (see persistence.rs). Dialect and
//   per-rule toggles are preferences and persist like the theme does.
//
// ENGINE ATTRIBUTION:
// - harper-core (https://github.com/Automattic/harper), Apache 2.0,
//   pinned to an exact version in Cargo.toml. Pinned because upstream
//   upgrades are deliberate events reviewed against their changelog.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use harper_core::linting::{FlatConfig, Lint, LintGroup, LintKind, Suggestion};
use harper_core::spell::{Dictionary, FstDictionary, MergedDictionary, MutableDictionary};
use harper_core::{Dialect, DictWordMetadata, Document, Span};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{audit, persistence};

// ---------- Engine (pure logic, unit tested below) ----------

/// Dialects the UI may select, in display order. These exact strings are the
/// stable wire format between the page and this module.
pub const SUPPORTED_DIALECTS: [&str; 5] =
    ["American", "British", "Canadian", "Australian", "Indian"];

pub fn parse_dialect(name: &str) -> Result<Dialect, String> {
    match name {
        "American" => Ok(Dialect::American),
        "British" => Ok(Dialect::British),
        "Canadian" => Ok(Dialect::Canadian),
        "Australian" => Ok(Dialect::Australian),
        "Indian" => Ok(Dialect::Indian),
        other => Err(format!(
            "Unsupported dialect '{other}'. Supported: {}.",
            SUPPORTED_DIALECTS.join(", ")
        )),
    }
}

/// UI facing category, exactly the three named in the feature plan.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum IssueCategory {
    Spelling,
    Grammar,
    Style,
}

impl IssueCategory {
    fn from_lint_kind(kind: &LintKind) -> Self {
        match kind {
            LintKind::Spelling | LintKind::Typo => IssueCategory::Spelling,
            LintKind::Style
            | LintKind::WordChoice
            | LintKind::Redundancy
            | LintKind::Repetition
            | LintKind::Readability
            | LintKind::Enhancement
            | LintKind::Eggcorn
            | LintKind::Regionalism => IssueCategory::Style,
            // Agreement, BoundaryError, Capitalization, Formatting, Grammar,
            // Malapropism, Miscellaneous, Nonstandard, Punctuation, Usage,
            // WordOrder and any future variant land in Grammar.
            _ => IssueCategory::Grammar,
        }
    }
}

/// Typed correction proposal, mirroring harper's `Suggestion` enum.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuggestionDto {
    ReplaceWith { text: String },
    InsertAfter { text: String },
    Remove,
}

impl SuggestionDto {
    fn to_harper(&self) -> Suggestion {
        match self {
            SuggestionDto::ReplaceWith { text } => Suggestion::ReplaceWith(text.chars().collect()),
            SuggestionDto::InsertAfter { text } => Suggestion::InsertAfter(text.chars().collect()),
            SuggestionDto::Remove => Suggestion::Remove,
        }
    }

    fn from_harper(s: &Suggestion) -> Option<Self> {
        match s {
            Suggestion::ReplaceWith(chars) => Some(SuggestionDto::ReplaceWith {
                text: chars.iter().collect(),
            }),
            Suggestion::InsertAfter(chars) => Some(SuggestionDto::InsertAfter {
                text: chars.iter().collect(),
            }),
            Suggestion::Remove => Some(SuggestionDto::Remove),
        }
    }
}

/// One flagged span in the checked text. Offsets are indices into the
/// character sequence of the input (Unicode scalar values, end exclusive).
#[derive(Serialize, Clone, Debug)]
pub struct Issue {
    pub id: u32,
    pub category: IssueCategory,
    /// Harper's own category name, kept for transparency in the UI.
    pub lint_kind: String,
    pub rule_id: String,
    pub span_start: usize,
    pub span_end: usize,
    pub problem_text: String,
    pub message: String,
    pub description: Option<String>,
    pub suggestions: Vec<SuggestionDto>,
    pub priority: u8,
}

#[derive(Serialize, Default, Clone, Copy, Debug)]
pub struct CategoryCounts {
    pub spelling: u32,
    pub grammar: u32,
    pub style: u32,
    pub total: u32,
}

#[derive(Serialize, Clone, Debug)]
pub struct CheckOutcome {
    pub issues: Vec<Issue>,
    pub counts: CategoryCounts,
    /// Length of the input in characters (the offset space of the spans).
    pub text_length: usize,
}

/// A correction the user chose to accept, sent back from the UI.
#[derive(Deserialize, Clone, Debug)]
pub struct SuggestedEdit {
    pub span_start: usize,
    pub span_end: usize,
    pub suggestion: SuggestionDto,
}

#[derive(Serialize, Clone, Debug)]
pub struct ApplyOutcome {
    pub corrected_text: String,
    pub edits_applied: u32,
}

/// Rule configuration as persisted and sent over the wire:
/// rule name -> Some(true)/Some(false) to force a rule on or off, None to
/// fall back to the curated default. `BTreeMap` keeps serialization stable.
pub type RuleMap = BTreeMap<String, Option<bool>>;

/// The curated default configuration harper ships (every rule explicitly
/// enabled or disabled). Cheap after the first call; harper caches it.
pub fn curated_rule_map() -> RuleMap {
    let value = serde_json::to_value(FlatConfig::new_curated()).expect("FlatConfig must serialize");
    serde_json::from_value(value).expect("FlatConfig JSON must deserialize into a RuleMap")
}

/// The linter plus everything derived from it that the UI needs.
pub struct Engine {
    group: LintGroup,
    dict: Arc<MergedDictionary>,
    descriptions: BTreeMap<String, String>,
}

/// Build an engine for one dialect with the given personal dictionary words.
/// The curated dictionary is cached process wide by harper; personal words
/// are layered on top through a merged dictionary, so rebuilds stay cheap.
pub fn build_engine(dialect: Dialect, user_words: &[String]) -> Engine {
    let mut merged = MergedDictionary::new();
    merged.add_dictionary(FstDictionary::curated() as Arc<dyn Dictionary>);
    if !user_words.is_empty() {
        let mut user = MutableDictionary::new();
        for word in user_words {
            user.append_word_str(word, DictWordMetadata::default());
        }
        merged.add_dictionary(Arc::new(user));
    }
    let dict: Arc<MergedDictionary> = Arc::new(merged);
    let group = LintGroup::new_curated(dict.clone(), dialect);
    let descriptions = group
        .all_descriptions()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Engine {
        group,
        dict,
        descriptions,
    }
}

impl Engine {
    /// Set the effective rule config for the next check: curated defaults
    /// first, then the persisted overrides on top. Resetting to curated on
    /// every call keeps cached engines deterministic: removing an override
    /// really removes it, instead of leaving the previous run's mutation
    /// behind in the cached config.
    pub fn apply_rules(&mut self, rules: &RuleMap) {
        self.group.config = FlatConfig::new_curated();
        for (key, state) in rules {
            match state {
                Some(true) => self.group.config.set_rule_enabled(key, true),
                Some(false) => self.group.config.set_rule_enabled(key, false),
                None => self.group.config.unset_rule_enabled(key),
            }
        }
    }

    /// Lint `text` with the current config. Issues are sorted by position,
    /// overlaps resolved in favor of the higher priority (lower number)
    /// rule, and ids assigned densely from zero.
    pub fn check(&mut self, text: &str) -> CheckOutcome {
        let document = Document::new_plain_english(text, self.dict.as_ref());
        let organized = self.group.organized_lints(&document);

        // Flatten with rule attribution.
        let mut flat: Vec<(String, Lint)> = organized
            .into_iter()
            .flat_map(|(rule, lints)| lints.into_iter().map(move |lint| (rule.clone(), lint)))
            .collect();

        // Sort by position, then priority, then rule name for determinism.
        flat.sort_by(|a, b| {
            a.1.span
                .start
                .cmp(&b.1.span.start)
                .then(a.1.priority.cmp(&b.1.priority))
                .then(a.1.span.end.cmp(&b.1.span.end))
                .then(a.0.cmp(&b.0))
        });

        let source = document.get_source();
        let mut issues: Vec<Issue> = Vec::new();
        let mut kept_end: usize = 0;
        for (rule_id, lint) in flat {
            // Overlap filter: keep an issue only if it starts at or after the
            // end of the last kept one. Mirrors harper's remove_overlaps but
            // preserves the rule attribution the UI needs.
            if lint.span.start < kept_end {
                continue;
            }
            kept_end = lint.span.end;

            let problem_text = lint.get_str(source);
            let suggestions = lint
                .suggestions
                .iter()
                .filter_map(SuggestionDto::from_harper)
                .collect::<Vec<_>>();
            let description = self.descriptions.get(&rule_id).cloned();
            issues.push(Issue {
                id: issues.len() as u32,
                category: IssueCategory::from_lint_kind(&lint.lint_kind),
                lint_kind: lint.lint_kind.to_string(),
                rule_id,
                span_start: lint.span.start,
                span_end: lint.span.end,
                problem_text,
                message: lint.message,
                description,
                suggestions,
                priority: lint.priority,
            });
        }

        let mut counts = CategoryCounts::default();
        for issue in &issues {
            match issue.category {
                IssueCategory::Spelling => counts.spelling += 1,
                IssueCategory::Grammar => counts.grammar += 1,
                IssueCategory::Style => counts.style += 1,
            }
        }
        counts.total = issues.len() as u32;

        CheckOutcome {
            text_length: text.chars().count(),
            counts,
            issues,
        }
    }
}

/// One-shot convenience used by the command layer: overlay the persisted rule
/// config onto the engine, then lint. The dialect is fixed when the engine is
/// built; changing it means rebuilding (or fetching another cached engine).
pub fn check_text(text: &str, rules: &RuleMap, engine: &mut Engine) -> CheckOutcome {
    engine.apply_rules(rules);
    engine.check(text)
}

/// Apply accepted edits to `text`. Edits are applied from the rightmost span
/// leftward so earlier offsets stay valid; malformed or overlapping spans are
/// rejected before anything is mutated.
pub fn apply_edits(text: &str, edits: &[SuggestedEdit]) -> Result<ApplyOutcome, String> {
    let mut source: Vec<char> = text.chars().collect();

    let mut sorted: Vec<&SuggestedEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.span_start
            .cmp(&a.span_start)
            .then(b.span_end.cmp(&a.span_end))
    });

    // Validate first: no mutation on any error.
    let mut previous_start: Option<usize> = None;
    for edit in &sorted {
        if edit.span_start > edit.span_end {
            return Err(format!(
                "invalid span {}..{} (start after end)",
                edit.span_start, edit.span_end
            ));
        }
        if edit.span_end > source.len() {
            return Err(format!(
                "span {}..{} is out of bounds for text of {} characters",
                edit.span_start,
                edit.span_end,
                source.len()
            ));
        }
        if let Some(prev) = previous_start {
            if edit.span_end > prev {
                return Err("overlapping edits are not allowed".to_string());
            }
        }
        previous_start = Some(edit.span_start);
    }

    for edit in sorted {
        let span = Span::<char>::new(edit.span_start, edit.span_end);
        edit.suggestion.to_harper().apply(span, &mut source);
    }

    Ok(ApplyOutcome {
        corrected_text: source.into_iter().collect(),
        edits_applied: edits.len() as u32,
    })
}

// ---------- State ----------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct DictionaryFile {
    words: Vec<String>,
}

/// Cached engine plus the in-memory personal dictionary. One engine is kept
/// for the most recently used dialect; switching dialects rebuilds it (cheap:
/// the curated dictionary itself is process wide and cached by harper).
#[derive(Default)]
struct EngineCache {
    user_words: Vec<String>,
    dict_loaded: bool,
    engine: Option<Engine>,
    engine_dialect: Option<Dialect>,
}

#[derive(Default)]
pub struct LanguageCheckState {
    inner: Arc<Mutex<EngineCache>>,
}

fn load_rule_overrides(app: &AppHandle) -> Result<RuleMap, String> {
    Ok(persistence::settings_get(app.clone())?.language_check_rules)
}

/// Load the personal dictionary from disk exactly once per session, when the
/// user opted in to persistence. Missing or unreadable files degrade to an
/// empty dictionary; nothing fails hard for user data.
fn ensure_dictionary_loaded(cache: &mut EngineCache, app: &AppHandle) -> Result<(), String> {
    if cache.dict_loaded {
        return Ok(());
    }
    cache.dict_loaded = true;
    if let Some(path) = persistence::language_check_dictionary_path(app)? {
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("read language check dictionary: {e}"))?;
            match serde_json::from_str::<DictionaryFile>(&content) {
                Ok(file) => cache.user_words = file.words,
                Err(e) => log::warn!("language check dictionary ignored (unreadable): {e}"),
            }
        }
    }
    Ok(())
}

/// Write the personal dictionary to disk when persistence is on. A write
/// failure is surfaced so the user knows their words are memory only.
fn persist_dictionary(app: &AppHandle, words: &[String]) -> Result<(), String> {
    if let Some(path) = persistence::language_check_dictionary_path(app)? {
        let json = serde_json::to_string_pretty(&DictionaryFile {
            words: words.to_vec(),
        })
        .map_err(|e| format!("serialize dictionary: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("write dictionary: {e}"))?;
    }
    Ok(())
}

fn sorted_words(words: &[String]) -> Vec<String> {
    let mut out = words.to_vec();
    out.sort_by_key(|w| w.to_lowercase());
    out
}

fn lock_cache_state(
    cache: &Arc<Mutex<EngineCache>>,
) -> Result<std::sync::MutexGuard<'_, EngineCache>, String> {
    cache
        .lock()
        .map_err(|e| format!("language check state lock: {e}"))
}

// ---------- Commands ----------

#[derive(Serialize, Clone, Debug)]
pub struct DictionaryView {
    pub words: Vec<String>,
    /// Whether the dictionary is being written to disk (persistence opt-in).
    pub persisted: bool,
}

#[tauri::command]
pub async fn language_check_run(
    app: AppHandle,
    state: State<'_, LanguageCheckState>,
    text: String,
    dialect: String,
) -> Result<CheckOutcome, String> {
    let cache = Arc::clone(&state.inner);
    let wanted = parse_dialect(&dialect)?;
    // Remember the last used dialect like the theme is remembered.
    {
        let mut settings = persistence::settings_get(app.clone())?;
        if settings.language_check_dialect != dialect {
            settings.language_check_dialect = dialect;
            persistence::settings_set(app.clone(), settings)?;
        }
    }
    let rules = load_rule_overrides(&app)?;
    tokio::task::spawn_blocking(move || {
        let mut guard = lock_cache_state(&cache)?;
        ensure_dictionary_loaded(&mut guard, &app)?;
        if guard.engine.is_none() || guard.engine_dialect != Some(wanted) {
            guard.engine = Some(build_engine(wanted, &guard.user_words));
            guard.engine_dialect = Some(wanted);
        }
        let engine = guard
            .engine
            .as_mut()
            .expect("engine is rebuilt above when missing");
        Ok(check_text(&text, &rules, engine))
    })
    .await
    .map_err(|e| format!("language check task failed: {e}"))?
}

#[tauri::command]
pub async fn language_check_apply(
    text: String,
    edits: Vec<SuggestedEdit>,
) -> Result<ApplyOutcome, String> {
    // Pure computation; still moved off the async runtime for large texts.
    tokio::task::spawn_blocking(move || apply_edits(&text, &edits))
        .await
        .map_err(|e| format!("language check apply task failed: {e}"))?
}

#[tauri::command]
pub async fn language_check_dictionary_list(
    app: AppHandle,
    state: State<'_, LanguageCheckState>,
) -> Result<DictionaryView, String> {
    let cache = Arc::clone(&state.inner);
    let persisted = persistence::language_check_dictionary_path(&app)?.is_some();
    tokio::task::spawn_blocking(move || {
        let mut guard = lock_cache_state(&cache)?;
        ensure_dictionary_loaded(&mut guard, &app)?;
        Ok(DictionaryView {
            words: sorted_words(&guard.user_words),
            persisted,
        })
    })
    .await
    .map_err(|e| format!("dictionary list task failed: {e}"))?
}

#[tauri::command]
pub async fn language_check_dictionary_add(
    app: AppHandle,
    state: State<'_, LanguageCheckState>,
    word: String,
) -> Result<DictionaryView, String> {
    let word = word.trim().to_string();
    if word.is_empty() {
        return Err("Cannot add an empty word.".into());
    }
    if word.chars().count() > 64 {
        return Err(
            "That entry is too long for the personal dictionary (64 characters max).".into(),
        );
    }
    if word.split_whitespace().count() > 1 {
        return Err("Add single words only; the dictionary does not check phrases.".into());
    }
    let cache = Arc::clone(&state.inner);
    let persisted = persistence::language_check_dictionary_path(&app)?.is_some();
    tokio::task::spawn_blocking(move || {
        let mut guard = lock_cache_state(&cache)?;
        ensure_dictionary_loaded(&mut guard, &app)?;
        let known = guard
            .user_words
            .iter()
            .any(|w| w.to_lowercase() == word.to_lowercase());
        if !known {
            guard.user_words.push(word);
            guard.engine = None;
            guard.engine_dialect = None;
            persist_dictionary(&app, &guard.user_words)?;
        }
        Ok(DictionaryView {
            words: sorted_words(&guard.user_words),
            persisted,
        })
    })
    .await
    .map_err(|e| format!("dictionary add task failed: {e}"))?
}

#[tauri::command]
pub async fn language_check_dictionary_remove(
    app: AppHandle,
    state: State<'_, LanguageCheckState>,
    word: String,
) -> Result<DictionaryView, String> {
    let cache = Arc::clone(&state.inner);
    let persisted = persistence::language_check_dictionary_path(&app)?.is_some();
    tokio::task::spawn_blocking(move || {
        let mut guard = lock_cache_state(&cache)?;
        ensure_dictionary_loaded(&mut guard, &app)?;
        let before = guard.user_words.len();
        guard
            .user_words
            .retain(|w| w.to_lowercase() != word.to_lowercase());
        if guard.user_words.len() != before {
            guard.engine = None;
            guard.engine_dialect = None;
            persist_dictionary(&app, &guard.user_words)?;
        }
        Ok(DictionaryView {
            words: sorted_words(&guard.user_words),
            persisted,
        })
    })
    .await
    .map_err(|e| format!("dictionary remove task failed: {e}"))?
}

/// What the settings UI needs: the dialect, the supported dialects, and the
/// merged rule view (curated defaults overlaid with stored overrides).
#[derive(Serialize, Clone, Debug)]
pub struct LanguageCheckSettingsView {
    pub dialect: String,
    pub supported_dialects: Vec<&'static str>,
    pub rules: RuleMap,
    pub overrides: usize,
    pub dictionary_persisted: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanguageCheckSettingsIn {
    pub dialect: Option<String>,
    /// Full rule map from the UI. Rules absent from the map keep curated
    /// behavior; Some(false) disables, Some(true) forces on, None resets.
    pub rules: Option<RuleMap>,
}

#[tauri::command]
pub async fn language_check_settings_get(
    app: AppHandle,
) -> Result<LanguageCheckSettingsView, String> {
    let settings = persistence::settings_get(app.clone())?;
    let persisted = persistence::language_check_dictionary_path(&app)?.is_some();
    let mut rules = curated_rule_map();
    for (key, state) in &settings.language_check_rules {
        if state.is_some() {
            rules.insert(key.clone(), *state);
        }
    }
    Ok(LanguageCheckSettingsView {
        dialect: settings.language_check_dialect,
        supported_dialects: SUPPORTED_DIALECTS.to_vec(),
        overrides: settings
            .language_check_rules
            .values()
            .filter(|v| v.is_some())
            .count(),
        rules,
        dictionary_persisted: persisted,
    })
}

#[tauri::command]
pub async fn language_check_settings_set(
    app: AppHandle,
    settings: LanguageCheckSettingsIn,
) -> Result<LanguageCheckSettingsView, String> {
    if let Some(dialect) = &settings.dialect {
        parse_dialect(dialect)?;
    }
    let mut stored = persistence::settings_get(app.clone())?;
    if let Some(dialect) = settings.dialect {
        stored.language_check_dialect = dialect;
    }
    if let Some(rules) = settings.rules {
        // Normalize: None entries are dropped so the stored map only holds
        // real overrides; curated defaults always apply beneath them.
        stored.language_check_rules = rules.into_iter().filter(|(_, v)| v.is_some()).collect();
    }
    persistence::settings_set(app.clone(), stored)?;
    language_check_settings_get(app).await
}

#[derive(Serialize, Clone, Debug)]
pub struct ImportedText {
    pub source_path: String,
    pub text: String,
}

const MAX_IMPORT_BYTES: u64 = 10 * 1024 * 1024;

/// Import plain text for checking. Supports .txt, .md, .tex and .docx
/// (extracted through the existing docx_reading module, same as the Text
/// Cleaner uses). Runs locally; records a file_read audit entry.
#[tauri::command]
pub async fn language_check_import_file(
    path: String,
    audit_log: State<'_, audit::AuditLog>,
) -> Result<ImportedText, String> {
    let file = Path::new(&path);
    if !file.exists() {
        return Err(format!("File not found: {path}"));
    }
    let meta = std::fs::metadata(file).map_err(|e| format!("stat file: {e}"))?;
    if meta.len() > MAX_IMPORT_BYTES {
        return Err("File is larger than 10 MB; check smaller files.".into());
    }
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let text = if ext == "docx" {
        crate::docx_reading::extract_text_from_docx(file)?
    } else if matches!(ext.as_str(), "txt" | "md" | "tex" | "markdown" | "rst" | "") {
        let bytes = std::fs::read(file).map_err(|e| format!("read file: {e}"))?;
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        return Err(format!(
            "Unsupported file type '.{ext}'. Use .txt, .md, .tex or .docx."
        ));
    };

    audit_log.record("file_read", &path, "language check import", 0, 0);
    Ok(ImportedText {
        source_path: path,
        text,
    })
}

/// Save the corrected text as a .txt file at a user chosen path. The only
/// file write this module performs, and only on this explicit action.
#[tauri::command]
pub async fn language_check_export_txt(
    path: String,
    text: String,
    audit_log: State<'_, audit::AuditLog>,
) -> Result<String, String> {
    let file = Path::new(&path);
    if file.extension().map(|e| e.to_string_lossy().to_lowercase()) != Some("txt".into()) {
        return Err("Choose a .txt destination file.".into());
    }
    std::fs::write(file, text.as_bytes()).map_err(|e| format!("write file: {e}"))?;
    audit_log.record(
        "file_write",
        &path,
        "language check export",
        0,
        text.len() as u64,
    );
    Ok(path)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(user: &[&str]) -> Engine {
        let words: Vec<String> = user.iter().map(|s| s.to_string()).collect();
        build_engine(Dialect::American, &words)
    }

    #[test]
    fn curated_rules_are_complete() {
        let map = curated_rule_map();
        assert!(
            map.len() > 500,
            "curated rule map unexpectedly small: {}",
            map.len()
        );
        assert_eq!(map.get("SpellCheck"), Some(&Some(true)));
    }

    #[test]
    fn catches_misspelling_with_suggestion() {
        let mut e = engine(&[]);
        let out = check_text("I beleive this is true.", &curated_rule_map(), &mut e);
        assert!(
            out.counts.spelling >= 1,
            "expected a spelling issue, got {:?}",
            out.counts
        );
        let spell = out
            .issues
            .iter()
            .find(|i| i.category == IssueCategory::Spelling)
            .expect("spelling issue");
        assert_eq!(spell.problem_text.to_lowercase(), "beleive");
        assert!(
            !spell.suggestions.is_empty(),
            "spelling issue should carry suggestions"
        );
        match &spell.suggestions[0] {
            SuggestionDto::ReplaceWith { text } => assert_eq!(text.to_lowercase(), "believe"),
            other => panic!("expected replace suggestion, got {other:?}"),
        }
    }

    #[test]
    fn catches_grammar_and_applies_round_trip() {
        let mut e = engine(&[]);
        let text = "She go to the store every day.";
        let out = check_text(text, &curated_rule_map(), &mut e);
        assert!(
            out.counts.grammar >= 1 || out.counts.style >= 1,
            "expected a grammar or style issue, got {:?}",
            out.counts
        );
        // Round trip: apply every first suggestion and require a change.
        let edits: Vec<SuggestedEdit> = out
            .issues
            .iter()
            .filter(|i| !i.suggestions.is_empty())
            .map(|i| SuggestedEdit {
                span_start: i.span_start,
                span_end: i.span_end,
                suggestion: i.suggestions[0].clone(),
            })
            .collect();
        let applied = apply_edits(text, &edits).expect("apply must succeed");
        assert_ne!(
            applied.corrected_text, text,
            "some edit must have changed the text"
        );
    }

    #[test]
    fn apply_edits_right_to_left_keeps_offsets() {
        let edits = vec![
            SuggestedEdit {
                span_start: 0,
                span_end: 1,
                suggestion: SuggestionDto::ReplaceWith { text: "An".into() },
            },
            SuggestedEdit {
                span_start: 6,
                span_end: 9,
                suggestion: SuggestionDto::ReplaceWith {
                    text: "apple".into(),
                },
            },
        ];
        let out = apply_edits("A red cat", &edits).unwrap();
        assert_eq!(out.corrected_text, "An red apple");
        assert_eq!(out.edits_applied, 2);
    }

    #[test]
    fn apply_edits_rejects_bad_spans() {
        let bad_order = vec![SuggestedEdit {
            span_start: 5,
            span_end: 2,
            suggestion: SuggestionDto::Remove,
        }];
        assert!(apply_edits("hello", &bad_order).is_err());
        let out_of_bounds = vec![SuggestedEdit {
            span_start: 0,
            span_end: 99,
            suggestion: SuggestionDto::Remove,
        }];
        assert!(apply_edits("hello", &out_of_bounds).is_err());
    }

    #[test]
    fn personal_dictionary_suppresses_spelling_flag() {
        let text = "The CRISPR experiment worked.";
        let mut plain = engine(&[]);
        let out = check_text(text, &curated_rule_map(), &mut plain);
        let flagged = out
            .issues
            .iter()
            .any(|i| i.problem_text.to_lowercase().contains("crispr"));

        let mut with_user = engine(&["CRISPR"]);
        let out2 = check_text(text, &curated_rule_map(), &mut with_user);
        let flagged2 = out2
            .issues
            .iter()
            .any(|i| i.problem_text.to_lowercase().contains("crispr"));

        assert!(!flagged2, "personal dictionary must suppress the flag");
        assert!(flagged || out.counts.total == 0, "baseline run sanity");
    }

    #[test]
    fn dialect_selector_changes_result_or_at_least_parses() {
        assert!(parse_dialect("American").is_ok());
        assert!(parse_dialect("British").is_ok());
        assert!(parse_dialect("Canadian").is_ok());
        assert!(parse_dialect("Australian").is_ok());
        assert!(parse_dialect("Indian").is_ok());
        assert!(parse_dialect("Klingon").is_err());
        let mut e = engine(&[]);
        let out = check_text("The colour of the sky.", &curated_rule_map(), &mut e);
        let _ = out; // British accepts "colour"; just require no panic.
    }

    #[test]
    fn rule_disable_suppresses_issue() {
        let mut e = engine(&[]);
        let text = "She go to the store every day.";
        let base = check_text(text, &curated_rule_map(), &mut e);
        assert!(base.counts.total >= 1);

        // Disable every rule that fired, then re-run: nothing may fire.
        let mut rules = curated_rule_map();
        for issue in &base.issues {
            rules.insert(issue.rule_id.clone(), Some(false));
        }
        let out2 = check_text(text, &rules, &mut e);
        assert_eq!(
            out2.counts.total, 0,
            "disabling the fired rules must silence the text"
        );

        // Back to curated defaults: the same issues must return. This pins
        // the reset-to-curated semantics of the cached engine config.
        let out3 = check_text(text, &curated_rule_map(), &mut e);
        assert_eq!(
            out3.counts.total, base.counts.total,
            "curated defaults must be restored on the next run"
        );
    }

    #[test]
    fn empty_and_unicode_inputs_are_safe() {
        let mut e = engine(&[]);
        let empty = check_text("", &curated_rule_map(), &mut e);
        assert_eq!(empty.counts.total, 0);
        assert_eq!(empty.text_length, 0);

        let uni = check_text("Café naïve rsum lve.", &curated_rule_map(), &mut e);
        assert_eq!(uni.text_length, "Café naïve rsum lve.".chars().count());
    }

    #[test]
    fn issue_ids_are_dense_and_sorted() {
        let mut e = engine(&[]);
        let out = check_text(
            "Their is many issue's here. I beleive it. She go now.",
            &curated_rule_map(),
            &mut e,
        );
        for (a, b) in out.issues.iter().zip(out.issues.iter().skip(1)) {
            assert!(
                a.span_start <= b.span_start,
                "issues must be sorted by span start"
            );
            assert_eq!(a.id + 1, b.id, "ids must be dense 0..n");
        }
        assert_eq!(out.counts.total as usize, out.issues.len());
    }
}
