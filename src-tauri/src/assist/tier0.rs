//! Tier 0 — the offline, deterministic lint pass.
//!
//! A thin adapter over the existing `language_check::Engine` (the app's
//! single source of truth for harper configuration, ruling D-4): the
//! engine is built through `language_check::build_engine`, configured with
//! the user's persisted rule overrides, and its issues are filtered by
//! strictness profile (E-1) and protected spans (D-4's S1/S2 arbitration)
//! before they reach Word.
//!
//! Tier 0 owns no rules, no dictionary, no dialect list. What it adds:
//! sentence bounding, protected-span filtering, the language gate, and the
//! wire shape the add-in needs (both offset spaces, stable ids).

use serde::Serialize;

use super::mask::{intersects_protected, mask, protected_spans, MaskMode, ProtectedSpan};
use super::offsets::OffsetMap;
use super::sentences::{dominant_script, split_sentences, Script};
use super::validate::StrictnessProfile;
use super::{ScalarOffset, Utf16Offset};
use crate::language_check::{build_engine, check_text, IssueCategory, RuleMap, SUPPORTED_DIALECTS};

/// Shown when the paragraph's dominant script is one harper cannot serve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LanguageStatus {
    /// The paragraph is (predominantly) Latin-script English.
    Supported,
    /// The paragraph is predominantly a script the engine cannot lint; the
    /// UI shows the "language not supported yet" line. Checks still run
    /// when the user enabled the script experimentally (1b setting).
    Unsupported { script: String },
    /// The user turned on the experimental toggle for this script; checks
    /// run and the UI must show the "experimental, unvalidated" line.
    Experimental { script: String },
}

/// One issue in the Word Assist wire shape. `start`/`end` are scalar
/// offsets into the SENTENCE text (canonical engine space);
/// `start_utf16`/`end_utf16` are the same span in UTF-16 code units (the
/// add-in's space). `sentence_start`/`sentence_end` locate the sentence
/// inside the paragraph, scalar space.
#[derive(Debug, Clone, Serialize)]
pub struct AssistIssue {
    /// Stable within one check response: `{rule}:{span}:{digest8}`.
    pub id: String,
    /// UI category: "grammar" | "spelling" | "clarity" (E-1 naming).
    pub category: String,
    pub rule_id: String,
    pub start: ScalarOffset,
    pub end: ScalarOffset,
    pub start_utf16: Utf16Offset,
    pub end_utf16: Utf16Offset,
    pub problem_text: String,
    pub message: String,
    pub suggestions: Vec<crate::language_check::SuggestionDto>,
}

/// One checked sentence with its surviving issues.
#[derive(Debug, Clone, Serialize)]
pub struct Tier0Sentence {
    /// Scalar offsets into the paragraph.
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub issues: Vec<AssistIssue>,
}

/// The full Tier 0 result for one paragraph.
#[derive(Debug, Clone, Serialize)]
pub struct Tier0Result {
    pub sentences: Vec<Tier0Sentence>,
    pub language_status: LanguageStatus,
    /// Offset map for the paragraph, so 1b can map additional spans without
    /// rebuilding the prefix table.
    #[serde(skip)]
    pub map: OffsetMap,
}

/// Gating is dominance-based (recon §6 Q5): the dominant script must be
/// Latin for `Supported`; anything else is `Unsupported` (or
/// `Experimental` when the user opted in). This constant documents the
/// share above which a Latin-dominant paragraph is treated as plain
/// English for the status line; kept for the Phase 4 precision pass.
#[allow(dead_code)]
const LATIN_SHARE_THRESHOLD: f64 = 0.8;

/// Which scripts can be enabled experimentally in settings (1b). harper is
/// English-only, so anything not Latin is "not supported yet"; the map is
/// per-script, not per-language, because script detection is all the
/// engine can do reliably.
pub fn script_name(script: Script) -> &'static str {
    match script {
        Script::Latin => "latin",
        Script::Arabic => "arabic",
        Script::Cjk => "cjk",
        Script::Cyrillic => "cyrillic",
        Script::Greek => "greek",
        Script::Other => "other",
    }
}

/// Decide the language status for a paragraph.
pub fn language_status(text: &str, experimental: &dyn Fn(&str) -> bool) -> LanguageStatus {
    match dominant_script(text) {
        None => LanguageStatus::Supported, // no letters: nothing to gate
        Some((Script::Latin, _)) => LanguageStatus::Supported,
        Some((script, _)) => {
            let name = script_name(script);
            if experimental(name) {
                LanguageStatus::Experimental {
                    script: name.to_string(),
                }
            } else {
                LanguageStatus::Unsupported {
                    script: name.to_string(),
                }
            }
        }
    }
}

/// Run Tier 0 over one paragraph.
///
/// * `dialect` — one of `SUPPORTED_DIALECTS` (persisted app setting).
/// * `rules` — the user's per-rule overrides (`language_check_rules`).
/// * `profile` — the strictness profile (E-1).
/// * `mode` — S1 (Filter) or S2 (Mask) protected-span strategy.
/// * `experimental` — closure consulting the 1b per-script opt-in map.
#[allow(clippy::too_many_arguments)]
pub fn check_paragraph(
    paragraph: &str,
    dialect: &str,
    rules: &RuleMap,
    profile: &StrictnessProfile,
    mode: MaskMode,
    experimental: &dyn Fn(&str) -> bool,
) -> Tier0Result {
    let status = language_status(paragraph, experimental);
    let map = OffsetMap::new(paragraph);
    let mut sentences_out: Vec<Tier0Sentence> = Vec::new();

    // Dialects are the exact strings language_check supports; an unknown
    // one degrades to American (same fallback language_check uses).
    let parsed_dialect = crate::language_check::parse_dialect(dialect)
        .unwrap_or(crate::language_check::parse_dialect("American").expect("static dialect"));

    // The engine is built once per paragraph: build_engine is cheap after
    // the first call (harper caches the curated dictionary process wide),
    // and 1b's state layer adds its own caching on top.
    let mut engine = build_engine(parsed_dialect, &[]);

    for sentence in split_sentences(paragraph) {
        let spans: Vec<ProtectedSpan> = protected_spans(&sentence.text);

        let issues: Vec<AssistIssue> = match mode {
            MaskMode::Filter => {
                let outcome = check_text(&sentence.text, rules, &mut engine);
                outcome
                    .issues
                    .into_iter()
                    .filter(|i| profile.keeps(i.category))
                    .filter(|i| !intersects_protected(&spans, i.span_start, i.span_end))
                    .map(|i| to_assist_issue(i, &map, sentence.start))
                    .collect()
            }
            MaskMode::Mask => {
                let masked = mask(&sentence.text, &spans);
                let outcome = check_text(&masked, rules, &mut engine);
                outcome
                    .issues
                    .into_iter()
                    .filter(|i| profile.keeps(i.category))
                    // Lints that touch a placeholder are artifacts of
                    // masking, not real issues (recon §6 Q5's predicted S2
                    // degradation) — drop them.
                    .filter(|i| !intersects_protected(&spans, i.span_start, i.span_end))
                    .map(|i| {
                        // problem_text may contain placeholders; restore the
                        // original characters so the UI never shows '¤'.
                        let mut fixed = i;
                        fixed.problem_text =
                            super::mask::unmask(&fixed.problem_text, &sentence.text);
                        to_assist_issue(fixed, &map, sentence.start)
                    })
                    .collect()
            }
        };

        if !issues.is_empty() {
            sentences_out.push(Tier0Sentence {
                start: sentence.start,
                end: sentence.end,
                text: sentence.text,
                issues,
            });
        }
    }

    Tier0Result {
        sentences: sentences_out,
        language_status: status,
        map,
    }
}

fn to_assist_issue(
    issue: crate::language_check::Issue,
    map: &OffsetMap,
    sentence_start: usize,
) -> AssistIssue {
    let (u0, u1) = map.span_scalar_to_utf16(
        sentence_start + issue.span_start,
        sentence_start + issue.span_end,
    );
    let category = match issue.category {
        IssueCategory::Grammar => "grammar",
        IssueCategory::Spelling => "spelling",
        IssueCategory::Style => "clarity",
    };
    AssistIssue {
        id: format!(
            "{}/{}/{:08x}",
            issue.rule_id,
            issue.span_start,
            fnv1a(&format!("{}|{}", issue.problem_text, issue.message))
        ),
        category: category.to_string(),
        rule_id: issue.rule_id,
        start: issue.span_start,
        end: issue.span_end,
        start_utf16: u0,
        end_utf16: u1,
        problem_text: issue.problem_text,
        message: issue.message,
        suggestions: issue.suggestions,
    }
}

/// Tiny FNV-1a for stable issue digests (not a cryptographic use; the
/// crypto rule "never hand-roll ciphers" does not cover a hash for ids,
/// and sha2 would be fine too — FNV keeps ids short).
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The supported dialect strings, re-exported for the 1b command layer so
/// no call site hardcodes them twice.
pub fn supported_dialects() -> &'static [&'static str] {
    &SUPPORTED_DIALECTS
}

#[cfg(test)]
mod tests {
    use super::{check_paragraph, language_status, LanguageStatus};
    use crate::assist::mask::MaskMode;
    use crate::assist::validate::StrictnessProfile;

    fn dev() -> StrictnessProfile {
        StrictnessProfile::dev_default()
    }

    #[test]
    fn grammar_issue_survives_dev_profile() {
        let result = check_paragraph(
            "They was late to the seminar. It replicated fine.",
            "American",
            &Default::default(),
            &dev(),
            MaskMode::Filter,
            &|_| false,
        );
        assert!(matches!(result.language_status, LanguageStatus::Supported));
        // The agreement error must be flagged; assert by message content
        // rather than a rule id (rule ids are harper's, not ours).
        let flagged: Vec<(String, String)> = result
            .sentences
            .iter()
            .flat_map(|s| {
                s.issues
                    .iter()
                    .map(|i| (i.rule_id.clone(), i.message.clone()))
            })
            .collect();
        assert!(
            flagged.iter().any(|(rule, msg)| {
                rule.contains("Agreement")
                    || rule.contains("Inflection")
                    || msg.to_lowercase().contains("agree")
            }),
            "expected an agreement lint, got {flagged:?}"
        );
        // Both offset spaces must agree on the problem span for BMP text.
        for s in &result.sentences {
            for i in &s.issues {
                let ascii_only = s.text.is_ascii();
                if ascii_only {
                    assert_eq!(i.start, i.start_utf16);
                    assert_eq!(i.end, i.end_utf16);
                }
            }
        }
    }

    #[test]
    fn citation_spans_never_flagged_s1() {
        // The citation and the year are protected; whatever harper thinks
        // of "(Smith, 2020)" must not reach Word.
        let result = check_paragraph(
            "Prior work (Smith, 2020) established the baseline for 1,234 samples.",
            "American",
            &Default::default(),
            &dev(),
            MaskMode::Filter,
            &|_| false,
        );
        for s in &result.sentences {
            for i in &s.issues {
                let issue_text: String =
                    s.text.chars().skip(i.start).take(i.end - i.start).collect();
                assert!(
                    !issue_text.contains("2020"),
                    "protected year flagged: {issue_text:?}"
                );
                assert!(
                    !issue_text.contains("Smith"),
                    "protected citation flagged: {issue_text:?}"
                );
            }
        }
    }

    #[test]
    fn spelling_off_in_dev_profile() {
        // A misspelling must NOT be flagged under the shipped dev default
        // (E-1): Word underlines misspellings itself; ours stay off.
        let result = check_paragraph(
            "The occurence of this misspeled word is documented.",
            "American",
            &Default::default(),
            &dev(),
            MaskMode::Filter,
            &|_| false,
        );
        let all: Vec<&str> = result
            .sentences
            .iter()
            .flat_map(|s| s.issues.iter().map(|i| i.category.as_str()))
            .collect();
        assert!(!all.contains(&"spelling"), "spelling must be off: {all:?}");
    }

    #[test]
    fn spelling_on_in_balanced_profile() {
        let result = check_paragraph(
            "The occurence of this misspeled word is documented.",
            "American",
            &Default::default(),
            &StrictnessProfile::balanced(),
            MaskMode::Filter,
            &|_| false,
        );
        let all: Vec<&str> = result
            .sentences
            .iter()
            .flat_map(|s| s.issues.iter().map(|i| i.category.as_str()))
            .collect();
        assert!(
            all.contains(&"spelling"),
            "balanced keeps spelling: {all:?}"
        );
    }

    #[test]
    fn arabic_paragraph_is_unsupported() {
        let result = check_paragraph(
            "هذه فقرة باللغة العربية لا يدعمها المحرك بعد.",
            "American",
            &Default::default(),
            &dev(),
            MaskMode::Filter,
            &|_| false,
        );
        assert!(matches!(
            result.language_status,
            LanguageStatus::Unsupported { .. }
        ));
        // And no issues are produced for unsupported scripts (precision).
        assert!(
            result.sentences.is_empty() || result.sentences.iter().all(|s| s.issues.is_empty())
        );
    }

    #[test]
    fn experimental_opt_in_runs_anyway() {
        let result = check_paragraph(
            "هذه فقرة عربية تجريبية.",
            "American",
            &Default::default(),
            &dev(),
            MaskMode::Filter,
            &|name| name == "arabic",
        );
        assert!(matches!(
            result.language_status,
            LanguageStatus::Experimental { .. }
        ));
    }

    #[test]
    fn mask_mode_round_trip_no_placeholders_in_output() {
        let result = check_paragraph(
            "Per Smith (2020) the method was robust and the the duplication slipped through.",
            "American",
            &Default::default(),
            &dev(),
            MaskMode::Mask,
            &|_| false,
        );
        for s in &result.sentences {
            for i in &s.issues {
                assert!(!i.problem_text.contains('¤'), "placeholder leaked: {i:?}");
            }
        }
    }

    #[test]
    fn dialects_reexport() {
        assert!(!super::supported_dialects().is_empty());
        assert!(super::supported_dialects().contains(&"American"));
    }

    #[test]
    fn language_status_gate() {
        assert!(matches!(
            language_status("Plain English here.", &|_| false),
            LanguageStatus::Supported
        ));
        assert!(matches!(
            language_status("3.14 + 2 = 5.", &|_| false),
            LanguageStatus::Supported // no letters -> not gated
        ));
        assert!(matches!(
            language_status("Ελληνικά κείμενο εδώ.", &|_| false),
            LanguageStatus::Unsupported { .. }
        ));
    }
}
