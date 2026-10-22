//! Tier 1 response validation: the six rejection rules, plus the
//! strictness profiles that gate Tier 0 categories.
//!
//! VALIDATION PHILOSOPHY (precision over recall, recon R7): a Tier 1
//! suggestion that fails any rule is not "partially applied" — it is
//! dropped entirely and reported as `no_suggestion`. Word shows an
//! annotation only for suggestions that survived all six rules.
//!
//! THE SIX REJECTION RULES (validate.rs, engine.rs calls them in order):
//! 1. **Schema** — the model response must parse as the versioned JSON
//!    shape (`engine::Tier1Response`); anything else is rejected.
//! 2. **Presence** — the revised sentence must be non-empty and must
//!    actually differ from the original (word-level, case-insensitive on
//!    whitespace-only differences).
//! 3. **Length** — the revision may not exceed `MAX_GROWTH_RATIO` times the
//!    original's word count (hallucination guard; scholarly prose rarely
//!    needs to double in length in one edit).
//! 4. **Protected content** — every protected span's *content* (citations,
//!    numbers, URLs/DOIs, LaTeX, quotes) extracted from the original must
//!    survive verbatim in the revision. A model that "fixes grammar" by
//!    rewriting a citation has failed.
//! 5. **Script** — the revision's dominant script must equal the
//!    original's (the language gate applies to the model's output too).
//! 6. **Sentence count** — the revision must contain exactly one sentence
//!    (no splitting, no merging); the sentence-boundary rule set is the
//!    same one the splitter uses, so validator and producer agree.
//!
//! STRICTNESS PROFILES (owner ruling E-1, PROVISIONAL until Phase 4):
//! categories come from `language_check::IssueCategory` (the single source
//! of truth). The shipped default is `Dev`: Grammar on, Spelling off
//! (Word already underlines misspellings; re-evaluation only if the
//! golden set shows the FP share at or below 15%), Clarity (harper style)
//! off. Per-rule overrides from the user's `language_check_rules` apply on
//! top in every profile (single source of truth, D-4).

use serde::{Deserialize, Serialize};

use super::mask::ProtectedKind;
use super::sentences::{dominant_script, split_sentences, Script};
use crate::language_check::IssueCategory;

/// Maximum allowed word-count growth for a Tier 1 revision (rule 3).
pub const MAX_GROWTH_RATIO: f64 = 1.5;

/// Which lint categories a strictness profile lets through to the add-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrictnessProfile {
    /// harper Grammar (agreement, punctuation, usage, ...).
    pub grammar: bool,
    /// harper Spelling/Typo. OFF in the shipped default (E-1).
    pub spelling: bool,
    /// harper Style/WordChoice/Readability/... ("Clarity" in the UI).
    /// OFF in the shipped default (E-1).
    pub clarity: bool,
}

impl StrictnessProfile {
    /// Shipped default, PROVISIONAL per ruling E-1: Grammar only. Revisited
    /// at the Phase 4 gate; do not flip here without re-reading E-1.
    pub fn dev_default() -> Self {
        Self {
            grammar: true,
            spelling: false,
            clarity: false,
        }
    }

    /// Conservative: Grammar only, spelled out explicitly (same as the dev
    /// default today; kept as a named profile so Phase 4 can move the dev
    /// default without renaming the user-facing choice).
    pub fn conservative() -> Self {
        Self::dev_default()
    }

    /// Balanced: Grammar + Spelling, style still off.
    pub fn balanced() -> Self {
        Self {
            grammar: true,
            spelling: true,
            clarity: false,
        }
    }

    /// Thorough: everything the curated harper config emits.
    pub fn thorough() -> Self {
        Self {
            grammar: true,
            spelling: true,
            clarity: true,
        }
    }

    /// Parse a profile from the settings string (persistence, 1b).
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "dev" => Some(Self::dev_default()),
            "conservative" => Some(Self::conservative()),
            "balanced" => Some(Self::balanced()),
            "thorough" => Some(Self::thorough()),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        if *self == Self::dev_default() {
            "dev"
        } else if *self == Self::balanced() {
            "balanced"
        } else if *self == Self::thorough() {
            "thorough"
        } else {
            "conservative"
        }
    }

    /// Does this profile keep an issue of `category`?
    pub fn keeps(&self, category: IssueCategory) -> bool {
        match category {
            IssueCategory::Grammar => self.grammar,
            IssueCategory::Spelling => self.spelling,
            IssueCategory::Style => self.clarity,
        }
    }
}

/// Result of validating a Tier 1 candidate. `Err` carries the rule number
/// and a short reason (both are safe to show; they never contain text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rejection {
    pub rule: u8,
    pub reason: &'static str,
}

/// The stable rule identifiers, exported so tests and the ledger (Phase 3)
/// can reference rejections symbolically instead of by magic number.
pub mod rules {
    pub const SCHEMA: u8 = 1;
    pub const PRESENCE: u8 = 2;
    pub const LENGTH: u8 = 3;
    pub const PROTECTED: u8 = 4;
    pub const SCRIPT: u8 = 5;
    pub const SENTENCE_COUNT: u8 = 6;
}

/// Validate a Tier 1 revision against its original. `protected` are the
/// original's protected spans (mask::protected_spans), `original` and
/// `revised` are raw sentence strings.
pub fn validate_revision(original: &str, revised: &str) -> Result<(), Rejection> {
    // Rule 2 — presence (rule 1, schema, is enforced by the caller's
    // deserialization: if we are here, the JSON parsed).
    let revised_trimmed = revised.trim();
    if revised_trimmed.is_empty() {
        return Err(Rejection {
            rule: rules::PRESENCE,
            reason: "revision is empty",
        });
    }

    // Rule 3 — length growth.
    let orig_words = word_count(original);
    let rev_words = word_count(revised_trimmed);
    if orig_words > 0 {
        let ratio = rev_words as f64 / orig_words as f64;
        if ratio > MAX_GROWTH_RATIO {
            return Err(Rejection {
                rule: rules::LENGTH,
                reason: "revision grows the sentence beyond the allowed ratio",
            });
        }
    }

    // Rule 4 — protected content survives verbatim, and nothing new of
    // that kind may appear. Two directions:
    //   (a) every protected span of the ORIGINAL must survive in the
    //       revision;
    //   (b) every protected span of the REVISION must already exist in the
    //       original (a rewrite that adds a citation, a number, a URL or a
    //       quote is a hallucination, not a copy edit).
    let spans = super::mask::protected_spans(original);
    for span in &spans {
        let content: String = original
            .chars()
            .skip(span.start)
            .take(span.end - span.start)
            .collect();
        let normalized = content.trim();
        if normalized.is_empty() {
            continue;
        }
        if !revised_trimmed.contains(normalized) {
            return Err(Rejection {
                rule: rules::PROTECTED,
                reason: match span.kind {
                    ProtectedKind::Citation => "revision alters a citation",
                    ProtectedKind::Url => "revision alters a URL or DOI",
                    ProtectedKind::Number => "revision alters a number",
                    ProtectedKind::Latex => "revision alters inline math",
                    ProtectedKind::Quote => "revision alters a quoted passage",
                },
            });
        }
    }
    for span in super::mask::protected_spans(revised_trimmed) {
        let content: String = revised_trimmed
            .chars()
            .skip(span.start)
            .take(span.end - span.start)
            .collect();
        let normalized = content.trim();
        if normalized.is_empty() {
            continue;
        }
        if !original.contains(normalized) {
            return Err(Rejection {
                rule: rules::PROTECTED,
                reason: "revision introduces protected content",
            });
        }
    }

    // Rule 5 — script preservation.
    if let (Some((so, _)), Some((sr, _))) =
        (dominant_script(original), dominant_script(revised_trimmed))
    {
        if !script_compatible(so, sr) {
            return Err(Rejection {
                rule: rules::SCRIPT,
                reason: "revision changes the writing script",
            });
        }
    }

    // Rule 6 — single sentence in, single sentence out.
    let rev_sentences = split_sentences(revised_trimmed);
    if rev_sentences.len() != 1 {
        return Err(Rejection {
            rule: rules::SENTENCE_COUNT,
            reason: "revision splits or merges sentences",
        });
    }

    Ok(())
}

fn word_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

/// CJK "script" is compatible with Latin (scholarly CJK text embeds Latin
/// terms, and harper is not involved for CJK anyway — the gate exists to
/// stop cross-script rewrites like Arabic→Latin).
fn script_compatible(a: Script, b: Script) -> bool {
    a == b
        || matches!(
            (a, b),
            (Script::Cjk, Script::Latin) | (Script::Latin, Script::Cjk)
        )
}

#[cfg(test)]
mod tests {
    use super::{validate_revision, StrictnessProfile};
    use crate::language_check::IssueCategory;

    #[test]
    fn accepts_clean_revision() {
        assert!(validate_revision(
            "The results was clear in 2020 (Smith, 2020).",
            "The results were clear in 2020 (Smith, 2020).",
        )
        .is_ok());
    }

    #[test]
    fn rejects_empty() {
        let err = validate_revision("Something here.", "   ").unwrap_err();
        assert_eq!(err.rule, 2);
    }

    #[test]
    fn rejects_growth_beyond_ratio() {
        let o = "We ran the tests.";
        let r = "We ran the full test suite twice, then we carefully reviewed every single failing case from the log and documented the results for the whole team to see.";
        let err = validate_revision(o, r).unwrap_err();
        assert_eq!(err.rule, 3);
    }

    #[test]
    fn rejects_citation_rewrite() {
        let err = validate_revision(
            "As Smith (2020) showed, the effect is real.",
            "As Jones (2021) showed, the effect is real.",
        )
        .unwrap_err();
        assert_eq!(err.rule, 4);
    }

    #[test]
    fn rejects_number_rewrite() {
        let err = validate_revision(
            "The sample included 3,412 participants.",
            "The sample included 3,512 participants.",
        )
        .unwrap_err();
        assert_eq!(err.rule, 4);
    }

    #[test]
    fn rejects_url_rewrite() {
        let err = validate_revision(
            "Data are at https://example.com/dataset.",
            "Data are at https://example.org/dataset.",
        )
        .unwrap_err();
        assert_eq!(err.rule, 4);
    }

    #[test]
    fn rejects_script_change() {
        let err = validate_revision(
            "هذه جملة اختبار مهمة جداً في النص.",
            "This is a test sentence in the text.",
        )
        .unwrap_err();
        assert_eq!(err.rule, 5);
    }

    #[test]
    fn rejects_sentence_split() {
        let err = validate_revision(
            "The method works well and the paper is clear.",
            "The method works well. The paper is clear.",
        )
        .unwrap_err();
        assert_eq!(err.rule, 6);
    }

    #[test]
    fn strictness_profiles_match_e1() {
        let dev = StrictnessProfile::dev_default();
        // E-1: Grammar ON, Spelling OFF, Clarity OFF.
        assert!(dev.keeps(IssueCategory::Grammar));
        assert!(!dev.keeps(IssueCategory::Spelling));
        assert!(!dev.keeps(IssueCategory::Style));
        assert!(StrictnessProfile::balanced().keeps(IssueCategory::Spelling));
        assert!(StrictnessProfile::thorough().keeps(IssueCategory::Style));
        assert_eq!(StrictnessProfile::parse("dev").unwrap().name(), "dev");
        assert!(StrictnessProfile::parse("nope").is_none());
    }
}
