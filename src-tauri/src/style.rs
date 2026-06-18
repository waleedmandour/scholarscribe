//! Style analysis module.
//!
//! IMPORTANT — ETHICAL SCOPE:
//! This module compares a draft to the author's *own* prior writing sample(s)
//! and reports stylistic *consistency* between them. It is NOT designed to
//! lower AI-detection scores. We do not implement any of the "6 markers"
//! evasion logic. The metrics here are descriptive statistics about prose,
//! surfaced to the *author* so they can decide whether the draft sounds like
//! them. The author always retains final say over what gets revised.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

static SENTENCE_END: Lazy<Regex> = Lazy::new(|| {
    // Split on whitespace that follows a sentence-ending punctuation mark.
    // The Rust `regex` crate does not support look-behind, so we use a
    // capture group and split_with() to keep the punctuation attached.
    Regex::new(r"([.!?]+)\s+").expect("regex")
});
static WORD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z][A-Za-z'-]*").expect("regex"));
static HEDGES: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "perhaps",
        "possibly",
        "probably",
        "likely",
        "may",
        "might",
        "could",
        "arguably",
        "seemingly",
        "appear",
        "appears",
        "suggest",
        "suggests",
        "tend",
        "tends",
        "largely",
        "generally",
        "broadly",
        "often",
        "frequently",
        "in part",
        "to some extent",
        "somewhat",
        "rather",
    ]
});
static CONNECTORS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "however",
        "moreover",
        "furthermore",
        "nevertheless",
        "nonetheless",
        "therefore",
        "thus",
        "consequently",
        "accordingly",
        "hence",
        "additionally",
        "in addition",
        "by contrast",
        "in contrast",
        "on the other hand",
        "for instance",
        "for example",
        "specifically",
        "in particular",
        "notably",
        "indeed",
        "in fact",
        "in sum",
        "in short",
    ]
});

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StyleProfile {
    pub word_count: usize,
    pub sentence_count: usize,
    pub avg_sentence_length: f64,
    pub sentence_length_stdev: f64,
    pub type_token_ratio: f64,
    pub avg_paragraph_length: f64,
    pub passive_ratio: f64,
    pub hedge_density: f64,
    pub connector_density: f64,
    pub first_person_singular_ratio: f64,
    pub first_person_plural_ratio: f64,
    pub citation_density: f64,
}

pub fn analyze(text: &str) -> StyleProfile {
    let words: Vec<String> = WORD_RE
        .find_iter(text)
        .map(|m| m.as_str().to_lowercase())
        .collect();
    let word_count = words.len();

    // Split into sentences. The regex captures the sentence-ending punctuation,
    // so after split() each piece lacks its trailing punctuation — that's fine
    // for word-counting purposes.
    let sentences: Vec<&str> = SENTENCE_END
        .split(text)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let sentence_count = sentences.len().max(1);

    let sentence_lengths: Vec<usize> = sentences
        .iter()
        .map(|s| WORD_RE.find_iter(s).count())
        .collect();

    let avg_sentence_length = if sentence_lengths.is_empty() {
        0.0
    } else {
        sentence_lengths.iter().sum::<usize>() as f64 / sentence_lengths.len() as f64
    };
    let sentence_length_stdev = stdev(&sentence_lengths, avg_sentence_length);

    let type_token_ratio = {
        let unique: std::collections::HashSet<&str> = words.iter().map(|s| s.as_str()).collect();
        if word_count == 0 {
            0.0
        } else {
            unique.len() as f64 / word_count as f64
        }
    };

    let paragraphs: Vec<&str> = text
        .split('\n')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let avg_paragraph_length = if paragraphs.is_empty() {
        0.0
    } else {
        paragraphs
            .iter()
            .map(|p| WORD_RE.find_iter(p).count())
            .sum::<usize>() as f64
            / paragraphs.len() as f64
    };

    let passive_ratio = count_passive(text) as f64 / sentence_count as f64;
    let hedge_density = count_matches(&words, &HEDGES) as f64 / sentence_count as f64;
    let connector_density = count_phrase_matches(text, &CONNECTORS) as f64 / sentence_count as f64;
    let first_person_singular_ratio = count_word(&words, "i") as f64 / sentence_count as f64;
    let first_person_plural_ratio = count_word(&words, "we") as f64 / sentence_count as f64;
    let citation_density = count_citations(text) as f64 / sentence_count as f64;

    StyleProfile {
        word_count,
        sentence_count,
        avg_sentence_length,
        sentence_length_stdev,
        type_token_ratio,
        avg_paragraph_length,
        passive_ratio,
        hedge_density,
        connector_density,
        first_person_singular_ratio,
        first_person_plural_ratio,
        citation_density,
    }
}

fn stdev(xs: &[usize], mean: f64) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let var: f64 =
        xs.iter().map(|x| (*x as f64 - mean).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    var.sqrt()
}

fn count_passive(text: &str) -> usize {
    let re = Regex::new(r"(?i)\b(?:is|are|was|were|be|been|being)\s+(\w+ed|written|done|made|taken|given|seen|known|shown|found|considered|regarded|thought|assumed)\b").unwrap();
    re.find_iter(text).count()
}

fn count_matches(words: &[String], targets: &[&str]) -> usize {
    let target_set: std::collections::HashSet<&str> = targets.iter().copied().collect();
    words
        .iter()
        .filter(|w| target_set.contains(w.as_str()))
        .count()
}

fn count_word(words: &[String], target: &str) -> usize {
    words.iter().filter(|w| w.as_str() == target).count()
}

fn count_phrase_matches(text: &str, targets: &[&str]) -> usize {
    let lower = text.to_lowercase();
    targets.iter().map(|t| lower.matches(t).count()).sum()
}

fn count_citations(text: &str) -> usize {
    // Author-year (Smith, 2020) or (Smith et al., 2020; Jones, 2021)
    // or numeric [1, 2, 3-5]
    let re = Regex::new(r"\((?:[A-Z][a-zA-Z'-]+(?:\s+(?:et\s+al\.?|and|&)\s+[A-Z][a-zA-Z'-]+)?,?\s*\d{4}[a-z]?[;,]?\s*)+\)|\[[\d,\s\-]+\]").unwrap();
    re.find_iter(text).count()
}

#[derive(Debug, Serialize)]
pub struct StyleComparison {
    pub draft: StyleProfile,
    pub reference: StyleProfile,
    pub overall_distance: f64,
    pub feature_distances: Vec<FeatureDistance>,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FeatureDistance {
    pub feature: String,
    pub draft_value: f64,
    pub reference_value: f64,
    pub relative_diff_pct: f64,
    pub interpretation: String,
}

pub fn compare(draft: &StyleProfile, reference: &StyleProfile) -> StyleComparison {
    let features = [
        (
            "avg_sentence_length",
            draft.avg_sentence_length,
            reference.avg_sentence_length,
        ),
        (
            "sentence_length_stdev",
            draft.sentence_length_stdev,
            reference.sentence_length_stdev,
        ),
        (
            "type_token_ratio",
            draft.type_token_ratio,
            reference.type_token_ratio,
        ),
        (
            "passive_ratio",
            draft.passive_ratio,
            reference.passive_ratio,
        ),
        (
            "hedge_density",
            draft.hedge_density,
            reference.hedge_density,
        ),
        (
            "connector_density",
            draft.connector_density,
            reference.connector_density,
        ),
        (
            "first_person_plural_ratio",
            draft.first_person_plural_ratio,
            reference.first_person_plural_ratio,
        ),
        (
            "citation_density",
            draft.citation_density,
            reference.citation_density,
        ),
    ];

    let mut feature_distances = Vec::new();
    let mut sum_sq = 0.0;
    for (name, d, r) in features {
        let diff = (d - r).abs();
        let denom = r.abs().max(0.001);
        let rel = (diff / denom) * 100.0;
        sum_sq += (diff / denom).powi(2);
        feature_distances.push(FeatureDistance {
            feature: name.to_string(),
            draft_value: d,
            reference_value: r,
            relative_diff_pct: rel,
            interpretation: interpret_diff(name, rel),
        });
    }

    let overall_distance = sum_sq.sqrt();

    let mut notes = Vec::new();
    if overall_distance < 0.5 {
        notes.push(
            "The draft's stylistic profile is closely aligned with your reference writing.".into(),
        );
    } else if overall_distance < 1.2 {
        notes.push("The draft is broadly consistent with your reference writing, with a few features standing out (see per-feature notes).".into());
    } else {
        notes.push("The draft diverges noticeably from your reference writing. This may simply mean you wrote in a different register — but if you expected it to sound like 'you', review the highlighted features.".into());
    }
    for fd in &feature_distances {
        if fd.relative_diff_pct > 50.0 {
            notes.push(format!(
                "{} differs by {:.0}% between draft and reference. ({} → {})",
                fd.feature, fd.relative_diff_pct, fd.draft_value, fd.reference_value
            ));
        }
    }

    StyleComparison {
        draft: draft.clone(),
        reference: reference.clone(),
        overall_distance,
        feature_distances,
        notes,
    }
}

fn interpret_diff(_feature: &str, rel: f64) -> String {
    if rel < 15.0 {
        "very close".to_string()
    } else if rel < 35.0 {
        "minor difference".to_string()
    } else if rel < 75.0 {
        "notable difference".to_string()
    } else {
        "substantial difference".to_string()
    }
}
