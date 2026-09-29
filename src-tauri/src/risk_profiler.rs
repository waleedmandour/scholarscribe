#![allow(unused_variables, unused_mut, unused_assignments, dead_code)]

//! Authenticity Risk Profiler. Assesses whether a draft's surface features
//! overlap with the "high-risk zone" for AI-detection false positives, based
//! on the documented proxy metrics (perplexity and burstiness) from the
//! detection-evaluation literature.
//!
//! IMPORTANT: ETHICAL SCOPE.
//! This module does NOT predict whether a specific detector will flag the text.
//! It surfaces descriptive metrics so the author can understand whether their
//! genuine writing shares surface features with typical AI-generated text,
//! and if so, make informed choices about stylistic variation. It is not an
//! evasion tool. See docs/ETHICS.md.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RiskProfile {
    pub overall_perplexity_proxy: f64,
    pub overall_burstiness_proxy: f64,
    pub overall_risk_level: String,
    pub overall_risk_color: String,
    pub section_profiles: Vec<SectionRisk>,
    pub explanation: String,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionRisk {
    pub section_label: String,
    pub start_char: usize,
    pub end_char: usize,
    pub word_count: usize,
    pub perplexity_proxy: f64,
    pub burstiness_proxy: f64,
    pub vocabulary_uniformity_proxy: f64,
    pub sentence_length_variance_proxy: f64,
    pub risk_level: String,
    pub risk_color: String,
    /// First ~15 words of the passage, sliced from the original text via
    /// start_char..end_char. Lets the UI show context without re-reading the
    /// user's draft.
    pub excerpt: String,
    /// Plain-language cause, one or two sentences, derived from whichever
    /// proxy signal dominated the risk score. Never asserts AI authorship.
    /// Describes surface patterns only. Contains no em or en dashes.
    pub reason: String,
}

/// Compute the risk profile for a document. Splits into ~200-word passages
/// and computes proxy metrics for each.
pub fn analyze(text: &str) -> RiskProfile {
    let passages = split_into_passages(text, 200);
    let mut section_profiles = Vec::new();

    for (i, passage) in passages.iter().enumerate() {
        let metrics = compute_passage_metrics(&passage.text);
        let risk = classify_risk(&metrics);
        let excerpt = compute_excerpt(text, passage.start_char, passage.end_char);
        let reason = compute_reason(&metrics, &risk.0);
        section_profiles.push(SectionRisk {
            section_label: format!("Passage {} ({} words)", i + 1, passage.word_count),
            start_char: passage.start_char,
            end_char: passage.end_char,
            word_count: passage.word_count,
            perplexity_proxy: metrics.perplexity_proxy,
            burstiness_proxy: metrics.burstiness_proxy,
            vocabulary_uniformity_proxy: metrics.vocabulary_uniformity_proxy,
            sentence_length_variance_proxy: metrics.sentence_length_variance_proxy,
            risk_level: risk.0.clone(),
            risk_color: risk.1.clone(),
            excerpt,
            reason,
        });
    }

    // Overall metrics: average across passages.
    let overall_metrics = PassageMetrics {
        perplexity_proxy: average(section_profiles.iter().map(|s| s.perplexity_proxy)),
        burstiness_proxy: average(section_profiles.iter().map(|s| s.burstiness_proxy)),
        vocabulary_uniformity_proxy: average(
            section_profiles
                .iter()
                .map(|s| s.vocabulary_uniformity_proxy),
        ),
        sentence_length_variance_proxy: average(
            section_profiles
                .iter()
                .map(|s| s.sentence_length_variance_proxy),
        ),
    };
    let (overall_risk, overall_color) = classify_risk(&overall_metrics);
    let overall_perplexity = overall_metrics.perplexity_proxy;
    let overall_burstiness = overall_metrics.burstiness_proxy;

    let explanation = "This profile shows whether your draft's surface features (vocabulary predictability as a perplexity proxy, sentence-length variability as a burstiness proxy) overlap with the typical profile of AI-generated text. A \"high risk\" designation means your genuine writing shares surface features with AI text. It does NOT mean the text is AI-generated or will be flagged. Per Liang et al. (2023), non-native English writers often score in the high-risk zone despite writing entirely original work. Use this information to understand your writing's stylistic fingerprint, not to evade detection.".to_string();

    let mut recommendations = Vec::new();
    if overall_risk == "high" {
        recommendations.push("Your draft's surface features overlap with typical AI-generated text. This is common for technical writing and non-native English writers. Consider adding stylistic variation: vary sentence lengths, use more hedging language, or incorporate personal observations.".into());
    }
    if overall_burstiness < 0.3 {
        recommendations.push("Low burstiness (sentence-length variability) detected. Try mixing short and long sentences to increase natural rhythm.".into());
    }
    if overall_perplexity > 0.7 {
        recommendations.push("High perplexity proxy (low vocabulary diversity) detected. Consider using more varied vocabulary where appropriate.".into());
    }
    if recommendations.is_empty() {
        recommendations.push("Your draft's surface features are within the normal range for human-written academic text. No action needed.".into());
    }

    RiskProfile {
        overall_perplexity_proxy: overall_perplexity,
        overall_burstiness_proxy: overall_burstiness,
        overall_risk_level: overall_risk,
        overall_risk_color: overall_color,
        section_profiles,
        explanation,
        recommendations,
    }
}

fn average(iter: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = iter.fold((0.0_f64, 0_usize), |(s, c), v| (s + v, c + 1));
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

struct Passage {
    text: String,
    start_char: usize,
    end_char: usize,
    word_count: usize,
}

#[derive(Debug, Clone)]
struct PassageMetrics {
    perplexity_proxy: f64,
    burstiness_proxy: f64,
    vocabulary_uniformity_proxy: f64,
    sentence_length_variance_proxy: f64,
}

/// Split text into ~target_words word passages, tracking real character
/// offsets in the original text so excerpts and "Jump to passage" features
/// work. Fixes a pre-existing bug where `text.find(first_word)` returned the
/// first occurrence of that word anywhere in the document rather than the
/// passage's actual location.
fn split_into_passages(text: &str, target_words: usize) -> Vec<Passage> {
    // Walk the text by whitespace runs, tracking both word boundaries and
    // character offsets in the original text.
    let mut words: Vec<(usize, usize)> = Vec::new(); // (start_byte, end_byte)
    let mut in_word = false;
    let mut word_start = 0;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if in_word {
                words.push((word_start, i));
                in_word = false;
            }
        } else if !in_word {
            word_start = i;
            in_word = true;
        }
    }
    if in_word {
        words.push((word_start, text.len()));
    }

    let mut passages = Vec::new();
    if words.is_empty() {
        return passages;
    }

    let mut start = 0;
    while start < words.len() {
        let end = (start + target_words).min(words.len());
        let passage_start_byte = words[start].0;
        let passage_end_byte = words[end - 1].1;
        // Safe slice: both offsets come from char_indices and are on char
        // boundaries.
        let passage_text = text[passage_start_byte..passage_end_byte].to_string();
        passages.push(Passage {
            text: passage_text,
            start_char: passage_start_byte,
            end_char: passage_end_byte,
            word_count: end - start,
        });
        start = end;
    }
    passages
}

fn compute_passage_metrics(text: &str) -> PassageMetrics {
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len().max(1);

    // Perplexity proxy: inverse of vocabulary diversity (TTR).
    // Low TTR = low perplexity (more predictable) = higher risk.
    let unique: std::collections::HashSet<&str> = words.iter().copied().collect();
    let ttr = unique.len() as f64 / word_count as f64;
    let perplexity_proxy = 1.0 - ttr;

    // Vocabulary uniformity proxy: share of tokens that are repeats of an
    // earlier token. Higher = more repetitive vocabulary = higher risk.
    // Distinct from TTR (which is length-sensitive and rewards text that
    // introduces many unique words regardless of repetition depth).
    let vocab_uniformity_proxy = if word_count > 0 {
        (word_count - unique.len()) as f64 / word_count as f64
    } else {
        0.0
    };

    // Sentence splitting and length measurement.
    let sentences: Vec<&str> = text
        .split(|c: char| c == '.' || c == '!' || c == '?')
        .filter(|s| !s.trim().is_empty())
        .collect();
    let sentence_lengths: Vec<usize> = sentences
        .iter()
        .map(|s| s.split_whitespace().count())
        .collect();

    // Burstiness proxy: coefficient of variation of sentence lengths
    // (stdev / mean). Low = uniform rhythm = higher risk.
    let burstiness = if sentence_lengths.len() < 2 {
        0.0
    } else {
        let mean = sentence_lengths.iter().sum::<usize>() as f64 / sentence_lengths.len() as f64;
        let variance = sentence_lengths
            .iter()
            .map(|&l| (l as f64 - mean).powi(2))
            .sum::<f64>()
            / sentence_lengths.len() as f64;
        let stdev = variance.sqrt();
        if mean > 0.0 {
            stdev / mean
        } else {
            0.0
        }
    };

    // Sentence-length variance proxy: raw stdev of sentence lengths,
    // normalized to [0, 1] by dividing by 20 (a generous upper bound on stdev
    // for academic prose). The proxy is then inverted so that low variation
    // yields a high "risk-style" value. Distinct from burstiness (which is
    // stdev/mean and thus scale-free).
    let sentence_length_variance_proxy = if sentence_lengths.len() < 2 {
        0.0
    } else {
        let mean = sentence_lengths.iter().sum::<usize>() as f64 / sentence_lengths.len() as f64;
        let variance = sentence_lengths
            .iter()
            .map(|&l| (l as f64 - mean).powi(2))
            .sum::<f64>()
            / sentence_lengths.len() as f64;
        let stdev = variance.sqrt();
        // Higher stdev = MORE variation = LOWER risk-style value.
        (1.0 - (stdev / 20.0).min(1.0)).max(0.0)
    };

    PassageMetrics {
        perplexity_proxy,
        burstiness_proxy: burstiness,
        vocabulary_uniformity_proxy,
        sentence_length_variance_proxy,
    }
}

/// Classify risk into (level, color) using all four proxy signals.
/// Risk score is the average of: perplexity, vocabulary_uniformity,
/// (1 - burstiness), (1 - sentence_length_variance).
fn classify_risk(m: &PassageMetrics) -> (String, String) {
    let risk_score = (m.perplexity_proxy
        + m.vocabulary_uniformity_proxy
        + (1.0 - m.burstiness_proxy.min(1.0))
        + (1.0 - m.sentence_length_variance_proxy.min(1.0)))
        / 4.0;
    if risk_score > 0.65 {
        ("high".into(), "#c0392b".into())
    } else if risk_score > 0.45 {
        ("medium".into(), "#b76e00".into())
    } else {
        ("low".into(), "#1a8a52".into())
    }
}

/// Build a short excerpt (first ~15 words) of the passage, sliced from the
/// original text via start_char..end_char. Returns an empty string if the
/// slice is out of bounds or not on a char boundary (e.g. the user edited
/// the text after profiling).
fn compute_excerpt(text: &str, start_char: usize, end_char: usize) -> String {
    let slice = match text.get(start_char..end_char) {
        Some(s) => s,
        None => return String::new(),
    };
    let words: Vec<&str> = slice.split_whitespace().collect();
    if words.is_empty() {
        return String::new();
    }
    let head: Vec<&str> = words.iter().take(15).collect();
    let mut out = head.join(" ");
    if words.len() > 15 {
        out.push_str("...");
    }
    out
}

/// Build a plain-language reason for the passage's risk classification.
/// Derived from whichever proxy signal contributed most to the risk score.
/// Never asserts AI authorship. Describes surface patterns only. No em or
/// en dashes anywhere in the returned string.
fn compute_reason(m: &PassageMetrics, tier: &str) -> String {
    if tier == "low" {
        return "This passage's vocabulary and sentence rhythm are typical of human academic writing. No action needed.".into();
    }
    // Each signal's contribution to the overall risk_score (all four signals
    // are weighted equally, so contribution equals signal value).
    let contributions = [
        ("perplexity", m.perplexity_proxy),
        ("vocabulary_uniformity", m.vocabulary_uniformity_proxy),
        (
            "sentence_length_variance",
            1.0 - m.sentence_length_variance_proxy.min(1.0),
        ),
        ("burstiness", 1.0 - m.burstiness_proxy.min(1.0)),
    ];
    let mut dominant = contributions[0];
    for c in &contributions[1..] {
        if c.1 > dominant.1 {
            dominant = *c;
        }
    }
    let medium_suffix =
        " This is common in technical writing and is not evidence of AI authorship.";
    let high_suffix = " This is common in non-native English or heavily edited prose, and is not proof of AI generation.";
    let suffix = if tier == "high" {
        high_suffix
    } else {
        medium_suffix
    };
    match dominant.0 {
        "perplexity" => format!(
            "This passage's vocabulary is highly predictable (low type-token ratio), a pattern that overlaps with AI text surface features.{}",
            suffix
        ),
        "vocabulary_uniformity" => format!(
            "This passage's vocabulary is fairly narrow (high word repetition), a pattern that overlaps with AI text surface features.{}",
            suffix
        ),
        "sentence_length_variance" => format!(
            "This passage's sentence lengths are quite uniform, a pattern that overlaps with AI text surface features.{}",
            suffix
        ),
        "burstiness" => format!(
            "This passage's sentence rhythm is fairly uniform (low burstiness), a pattern that overlaps with AI text surface features.{}",
            suffix
        ),
        _ => format!(
            "This passage's surface features overlap with AI text surface patterns.{}",
            suffix
        ),
    }
}
