//! Voice Consistency Checker — flags within-document stylistic inconsistencies
//! where sentence length, hedging density, or vocabulary diversity abruptly
//! shifts. Sudden stylistic shifts within a single paper are a legitimate
//! editorial concern and a documented AI-detection signal.
//!
//! This helps researchers ensure cohesion regardless of AI involvement.

use crate::style;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ConsistencyReport {
    pub passages: Vec<PassageMetrics>,
    pub inconsistencies: Vec<Inconsistency>,
    pub overall_consistency_score: f64,
    pub explanation: String,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PassageMetrics {
    pub label: String,
    pub word_count: usize,
    pub avg_sentence_length: f64,
    pub type_token_ratio: f64,
    pub hedge_density: f64,
    pub passive_ratio: f64,
    pub flesch_reading_ease: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Inconsistency {
    pub passage_index: usize,
    pub passage_label: String,
    pub metric: String,
    pub value: f64,
    pub document_average: f64,
    pub deviation_pct: f64,
    pub severity: String,
    pub note: String,
}

pub fn check(text: &str) -> ConsistencyReport {
    let passages = split_into_sections(text, 300);
    let mut passage_metrics = Vec::new();
    let mut inconsistencies = Vec::new();

    for (i, passage) in passages.iter().enumerate() {
        let profile = style::analyze(passage);
        passage_metrics.push(PassageMetrics {
            label: format!("Section {} ({} words)", i + 1, profile.word_count),
            word_count: profile.word_count,
            avg_sentence_length: profile.avg_sentence_length,
            type_token_ratio: profile.type_token_ratio,
            hedge_density: profile.hedge_density,
            passive_ratio: profile.passive_ratio,
            flesch_reading_ease: profile.flesch_reading_ease,
        });
    }

    if passage_metrics.len() < 2 {
        return ConsistencyReport {
            passages: passage_metrics,
            inconsistencies,
            overall_consistency_score: 1.0,
            explanation:
                "Not enough text to check consistency. Need at least 2 sections (~600 words)."
                    .into(),
            recommendations: vec!["Add more text to enable consistency analysis.".into()],
        };
    }

    // Compute document averages
    let n = passage_metrics.len() as f64;
    let avg_sentence_length: f64 = passage_metrics
        .iter()
        .map(|p| p.avg_sentence_length)
        .sum::<f64>()
        / n;
    let avg_ttr: f64 = passage_metrics
        .iter()
        .map(|p| p.type_token_ratio)
        .sum::<f64>()
        / n;
    let avg_hedge: f64 = passage_metrics.iter().map(|p| p.hedge_density).sum::<f64>() / n;
    let avg_passive: f64 = passage_metrics.iter().map(|p| p.passive_ratio).sum::<f64>() / n;
    let avg_flesch: f64 = passage_metrics
        .iter()
        .map(|p| p.flesch_reading_ease)
        .sum::<f64>()
        / n;

    // Check each passage for deviations > 30% from the document average
    for (i, p) in passage_metrics.iter().enumerate() {
        check_deviation(
            &mut inconsistencies,
            i,
            &p.label,
            "avg_sentence_length",
            p.avg_sentence_length,
            avg_sentence_length,
        );
        check_deviation(
            &mut inconsistencies,
            i,
            &p.label,
            "type_token_ratio",
            p.type_token_ratio,
            avg_ttr,
        );
        check_deviation(
            &mut inconsistencies,
            i,
            &p.label,
            "hedge_density",
            p.hedge_density,
            avg_hedge,
        );
        check_deviation(
            &mut inconsistencies,
            i,
            &p.label,
            "passive_ratio",
            p.passive_ratio,
            avg_passive,
        );
        check_deviation(
            &mut inconsistencies,
            i,
            &p.label,
            "flesch_reading_ease",
            p.flesch_reading_ease,
            avg_flesch,
        );
    }

    // Overall consistency: 1 - (inconsistencies / total checks)
    let total_checks = passage_metrics.len() * 5;
    let consistency = 1.0 - (inconsistencies.len() as f64 / total_checks as f64).min(1.0);

    let explanation = format!(
        "This checker compares stylistic metrics across sections of your document. Abrupt shifts in sentence length, vocabulary diversity, or hedging density can indicate a change in voice — whether from co-authorship, AI assistance, or simply a shift in writing mode. Addressing inconsistencies improves cohesion and is good editorial practice regardless of how the text was produced."
    );

    let mut recommendations = Vec::new();
    if inconsistencies.is_empty() {
        recommendations.push("Your document shows consistent stylistic metrics across all sections. No action needed.".into());
    } else {
        recommendations.push(format!(
            "{} stylistic inconsistencies detected. Review the flagged sections for cohesion.",
            inconsistencies.len()
        ));
        let has_sentence = inconsistencies
            .iter()
            .any(|i| i.metric == "avg_sentence_length");
        if has_sentence {
            recommendations.push("Sentence-length shifts detected. Consider smoothing transitions between sections with different sentence rhythms.".into());
        }
        let has_ttr = inconsistencies
            .iter()
            .any(|i| i.metric == "type_token_ratio");
        if has_ttr {
            recommendations.push("Vocabulary diversity shifts detected. One section may use notably different vocabulary than the rest — verify this is intentional.".into());
        }
    }

    ConsistencyReport {
        passages: passage_metrics,
        inconsistencies,
        overall_consistency_score: consistency,
        explanation,
        recommendations,
    }
}

fn check_deviation(
    inconsistencies: &mut Vec<Inconsistency>,
    idx: usize,
    label: &str,
    metric: &str,
    value: f64,
    average: f64,
) {
    if average == 0.0 || !average.is_finite() {
        return;
    }
    let deviation = ((value - average) / average).abs() * 100.0;
    if deviation > 30.0 {
        let severity = if deviation > 60.0 { "high" } else { "medium" };
        let note = match metric {
            "avg_sentence_length" => format!(
                "Sentence length is {:.0}% {} than the document average.",
                deviation,
                if value > average { "longer" } else { "shorter" }
            ),
            "type_token_ratio" => format!(
                "Vocabulary diversity is {:.0}% {} than the document average.",
                deviation,
                if value > average { "higher" } else { "lower" }
            ),
            "hedge_density" => format!(
                "Hedging density is {:.0}% {} than the document average.",
                deviation,
                if value > average { "higher" } else { "lower" }
            ),
            "passive_ratio" => format!(
                "Passive voice usage is {:.0}% {} than the document average.",
                deviation,
                if value > average { "higher" } else { "lower" }
            ),
            "flesch_reading_ease" => format!(
                "Readability is {:.0}% {} than the document average.",
                deviation,
                if value > average { "easier" } else { "harder" }
            ),
            _ => format!("Deviation of {:.0}% from document average.", deviation),
        };
        inconsistencies.push(Inconsistency {
            passage_index: idx,
            passage_label: label.to_string(),
            metric: metric.to_string(),
            value,
            document_average: average,
            deviation_pct: (deviation * 10.0).round() / 10.0,
            severity: severity.to_string(),
            note,
        });
    }
}

fn split_into_sections(text: &str, target_words: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![text.to_string()];
    }
    let mut sections = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = (start + target_words).min(words.len());
        sections.push(words[start..end].join(" "));
        start = end;
    }
    if sections.is_empty() {
        sections.push(text.to_string());
    }
    sections
}
