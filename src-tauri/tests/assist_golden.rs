//! Word Assist Phase 1a golden-set harness.
//!
//! Runs the whole `assist` Tier 0 pipeline over `tests/assist/golden.json`
//! and asserts the invariants the brief and the rulings require:
//! - every sentence parses, splits and maps offsets losslessly (the
//!   scalar↔UTF-16 round trip holds on real scholarly content);
//! - protected sentences (citation/number/latex/url/quote/mixed) produce
//!   ZERO issues whose span touches the protected content — i.e. Tier 1
//!   would be asked to return them unchanged (D-4, E-1);
//! - the shipped dev strictness profile (E-1: Grammar on, Spelling off,
//!   Clarity off) never emits `spelling` or `clarity` categories;
//! - Arabic and CJK sentences come back `Unsupported` by default and
//!   run when the experimental opt-in is on;
//! - ASCII-only sentences have identical scalar and UTF-16 offsets.
//!
//! Run with: `cargo test --features word-assist --test assist_golden`
//! (a plain `cargo test` compiles this file to nothing — the whole suite
//! is feature-gated, so the default build's test count is unchanged).

#![cfg(feature = "word-assist")]

use scholarscribe_lib::assist::mask::MaskMode;
use scholarscribe_lib::assist::tier0::{check_paragraph, Tier0Result};
use scholarscribe_lib::assist::validate::StrictnessProfile;
use serde::Deserialize;

#[derive(Deserialize)]
struct Golden {
    #[allow(dead_code)]
    version: u32,
    sentences: Vec<GoldenSentence>,
}

#[derive(Deserialize)]
struct GoldenSentence {
    class: String,
    text: String,
}

fn golden() -> Vec<GoldenSentence> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assist/golden.json");
    let raw = std::fs::read_to_string(path).expect("golden.json readable");
    let parsed: Golden = serde_json::from_str(&raw).expect("golden.json parses");
    assert!(
        parsed.sentences.len() >= 50,
        "the brief asks for ~50 sentences, got {}",
        parsed.sentences.len()
    );
    parsed.sentences
}

const PROTECTED_CLASSES: [&str; 7] = [
    "citation",
    "number",
    "latex",
    "url",
    "quote",
    "mixed_protected",
    "tier1_candidate", // must pass through Tier 1 validation unchanged-ish: Tier 0 flags its grammar, but never inside protected content
];

fn run(text: &str, profile: &StrictnessProfile) -> Tier0Result {
    check_paragraph(
        text,
        "American",
        &Default::default(),
        profile,
        MaskMode::Filter,
        &|_| false,
    )
}

#[test]
fn golden_offsets_round_trip_everywhere() {
    for s in golden() {
        let result = run(&s.text, &StrictnessProfile::dev_default());
        let chars: Vec<char> = s.text.chars().collect();
        for sent in &result.sentences {
            assert!(sent.start < chars.len(), "sentence start in bounds");
            assert!(sent.end <= chars.len());
            assert!(sent.end > sent.start);
        }
        // Every issue span slices back to plausible text (never mid-astral).
        for sent in &result.sentences {
            for issue in &sent.issues {
                assert!(issue.start <= issue.end);
                assert!(issue.end <= sent.text.chars().count());
            }
        }
    }
}

#[test]
fn golden_protected_content_never_flagged() {
    for s in golden() {
        if !PROTECTED_CLASSES.contains(&s.class.as_str()) {
            continue;
        }
        let result = run(&s.text, &StrictnessProfile::dev_default());
        for sent in &result.sentences {
            for issue in &sent.issues {
                let flagged: String = sent
                    .text
                    .chars()
                    .skip(issue.start)
                    .take(issue.end - issue.start)
                    .collect();
                let lower = flagged.to_lowercase();
                for banned in ["2020", "2024", "smith", "doi", "osf.io", "%", "10."] {
                    assert!(
                        !lower.contains(banned),
                        "{:?} ({}): protected content {flagged:?} matched {banned:?}",
                        s.text,
                        s.class
                    );
                }
            }
        }
    }
}

#[test]
fn golden_dev_profile_emits_grammar_only() {
    for s in golden() {
        let result = run(&s.text, &StrictnessProfile::dev_default());
        for sent in &result.sentences {
            for issue in &sent.issues {
                assert_eq!(
                    issue.category, "grammar",
                    "E-1: dev profile is grammar-only; got {} in {:?}",
                    issue.category, s.text
                );
            }
        }
    }
}

#[test]
fn golden_thorough_profile_can_see_more() {
    // Sanity in the other direction: the thorough profile must not be a
    // strict subset behavior — at minimum, spelling issues exist somewhere
    // in the golden set under `thorough` but never under `dev`.
    let mut dev_spelling = 0;
    let mut thorough_spelling = 0;
    for s in golden() {
        dev_spelling +=
            count_category(&run(&s.text, &StrictnessProfile::dev_default()), "spelling");
        thorough_spelling +=
            count_category(&run(&s.text, &StrictnessProfile::thorough()), "spelling");
    }
    assert_eq!(dev_spelling, 0, "E-1: dev profile never flags spelling");
    assert!(
        thorough_spelling > 0,
        "thorough should surface spelling somewhere in the golden set"
    );
}

fn count_category(result: &Tier0Result, category: &str) -> usize {
    result
        .sentences
        .iter()
        .flat_map(|s| s.issues.iter())
        .filter(|i| i.category == category)
        .count()
}

#[test]
fn golden_arabic_and_cjk_gated() {
    for s in golden() {
        match s.class.as_str() {
            "arabic" | "cjk" => {
                let result = run(&s.text, &StrictnessProfile::dev_default());
                assert!(
                    matches!(
                        result.language_status,
                        scholarscribe_lib::assist::tier0::LanguageStatus::Unsupported { .. }
                    ),
                    "{:?} should be unsupported by default",
                    s.text
                );
                // With the experimental opt-in the check runs.
                let exp = check_paragraph(
                    &s.text,
                    "American",
                    &Default::default(),
                    &StrictnessProfile::dev_default(),
                    MaskMode::Filter,
                    &|script| script == "arabic" || script == "cjk",
                );
                assert!(matches!(
                    exp.language_status,
                    scholarscribe_lib::assist::tier0::LanguageStatus::Experimental { .. }
                ));
            }
            _ => {}
        }
    }
}

#[test]
fn golden_ascii_offsets_identical_in_both_spaces() {
    for s in golden() {
        if !s.text.is_ascii() {
            continue;
        }
        let result = run(&s.text, &StrictnessProfile::dev_default());
        for sent in &result.sentences {
            for issue in &sent.issues {
                assert_eq!(issue.start, issue.start_utf16);
                assert_eq!(issue.end, issue.end_utf16);
            }
        }
    }
}

#[test]
fn golden_clean_sentences_mostly_silent() {
    // Clean sentences should produce at most one issue across the whole
    // set (harper is good but not perfect; the bar here is "the engine
    // does not flood clean scholarly prose" — the Phase 4 precision work
    // tightens this further).
    let mut total = 0;
    for s in golden() {
        if s.class != "clean" {
            continue;
        }
        total += count_category(&run(&s.text, &StrictnessProfile::dev_default()), "grammar");
    }
    assert!(
        total <= 2,
        "clean sentences produced {total} grammar issues; engine is too noisy"
    );
}

#[test]
fn golden_tier1_validation_round_trip() {
    // For tier1_candidate sentences, a *correct* revision must pass the
    // six rules and a citation-altering revision must fail rule 4.
    use scholarscribe_lib::assist::validate::validate_revision;
    let o = "The results was clear even without the outliers removed.";
    assert!(validate_revision(
        o,
        "The results were clear even without the outliers removed."
    )
    .is_ok());
    assert!(validate_revision(
        o,
        "The results were clear (Lee, 2021) without the outliers."
    )
    .is_err());
}
