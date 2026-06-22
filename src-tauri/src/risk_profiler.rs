//! Authenticity Risk Profiler — assesses whether a draft's surface features
//! overlap with the "high-risk zone" for AI-detection false positives, based
//! on the documented proxy metrics (perplexity and burstiness) from the
//! detection-evaluation literature.
//!
//! IMPORTANT — ETHICAL SCOPE:
//! This module does NOT predict whether a specific detector will flag the text.
//! It surfaces descriptive metrics so the author can understand whether their
//! genuine writing shares surface features with typical AI-generated text —
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
    pub risk_level: String,
    pub risk_color: String,
}

/// Compute the risk profile for a document. Splits into ~200-word passages
/// and computes proxy metrics for each.
pub fn analyze(text: &str) -> RiskProfile {
    let passages = split_into_passages(text, 200);
    let mut section_profiles = Vec::new();

    for (i, passage) in passages.iter().enumerate() {
        let metrics = compute_passage_metrics(passage.text);
        let risk = classify_risk(metrics.perplexity_proxy, metrics.burstiness_proxy);
        section_profiles.push(SectionRisk {
            section_label: format!("Passage {} ({} words)", i + 1, passage.word_count),
            start_char: passage.start_char,
            end_char: passage.end_char,
            word_count: passage.word_count,
            perplexity_proxy: metrics.perplexity_proxy,
            burstiness_proxy: metrics.burstiness_proxy,
            risk_level: risk.0.clone(),
            risk_color: risk.1.clone(),
        });
    }

    // Overall metrics: average across passages
    let overall_perplexity = if section_profiles.is_empty() {
        0.0
    } else {
        section_profiles
            .iter()
            .map(|s| s.perplexity_proxy)
            .sum::<f64>()
            / section_profiles.len() as f64
    };
    let overall_burstiness = if section_profiles.is_empty() {
        0.0
    } else {
        section_profiles
            .iter()
            .map(|s| s.burstiness_proxy)
            .sum::<f64>()
            / section_profiles.len() as f64
    };
    let (overall_risk, overall_color) = classify_risk(overall_perplexity, overall_burstiness);

    let explanation = format!(
        "This profile shows whether your draft's surface features (vocabulary diversity as a perplexity proxy, sentence-length variability as a burstiness proxy) overlap with the typical profile of AI-generated text. A \"high risk\" designation means your genuine writing shares surface features with AI text — it does NOT mean the text is AI-generated or will be flagged. Per Liang et al. (2023), non-native English writers often score in the high-risk zone despite writing entirely original work. Use this information to understand your writing's stylistic fingerprint, not to evade detection."
    );

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

struct Passage {
    text: String,
    start_char: usize,
    end_char: usize,
    word_count: usize,
}

struct PassageMetrics {
    perplexity_proxy: f64,
    burstiness_proxy: f64,
}

fn split_into_passages(text: &str, target_words: usize) -> Vec<Passage> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut passages = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = (start + target_words).min(words.len());
        let passage_words = &words[start..end];
        let passage_text = passage_words.join(" ");
        let start_char = text.find(passage_words.first().unwrap_or(&"")).unwrap_or(0);
        passages.push(Passage {
            text: passage_text,
            start_char,
            end_char: start_char + passage_text.len(),
            word_count: passage_words.len(),
        });
        start = end;
    }
    passages
}

fn compute_passage_metrics(text: &str) -> PassageMetrics {
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len().max(1);

    // Perplexity proxy: inverse of vocabulary diversity.
    // Low TTR = low perplexity (more predictable) = higher risk.
    let unique: std::collections::HashSet<&str> = words.iter().copied().collect();
    let ttr = unique.len() as f64 / word_count as f64;
    let perplexity_proxy = 1.0 - ttr; // Higher = more predictable = higher risk

    // Burstiness proxy: coefficient of variation of sentence lengths.
    let sentences: Vec<&str> = text
        .split(|c: char| c == '.' || c == '!' || c == '?')
        .filter(|s| !s.trim().is_empty())
        .collect();
    let sentence_lengths: Vec<usize> = sentences
        .iter()
        .map(|s| s.split_whitespace().count())
        .collect();
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

    PassageMetrics {
        perplexity_proxy,
        burstiness_proxy: burstiness,
    }
}

fn classify_risk(perplexity: f64, burstiness: f64) -> (String, String) {
    // Combined risk score: high perplexity proxy (predictable vocabulary) +
    // low burstiness (uniform sentence lengths) = higher risk.
    let risk_score = perplexity * 0.5 + (1.0 - burstiness.min(1.0)) * 0.5;
    if risk_score > 0.65 {
        ("high".into(), "#c0392b".into())
    } else if risk_score > 0.45 {
        ("medium".into(), "#b76e00".into())
    } else {
        ("low".into(), "#1a8a52".into())
    }
}
