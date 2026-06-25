#![allow(unused_variables, unused_mut, unused_assignments, dead_code)]

//! Multi-Paper Style Fingerprint — aggregates style metrics across multiple
//! reference papers by the same author, producing a shareable (privacy-safe)
//! stylometric signature. Only aggregate metrics are exported — no raw text
//! ever leaves the device.
//!
//! The fingerprint can accompany a manuscript submission as supplementary
//! evidence of authorial consistency.

use crate::style;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StyleFingerprint {
    pub papers_analyzed: usize,
    pub total_words: usize,
    pub metrics: FingerprintMetrics,
    pub per_paper_profiles: Vec<PaperSummary>,
    pub export_json: String,
    pub export_markdown: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FingerprintMetrics {
    pub avg_sentence_length: f64,
    pub sentence_length_stdev: f64,
    pub type_token_ratio: f64,
    pub passive_ratio: f64,
    pub hedge_density: f64,
    pub connector_density: f64,
    pub first_person_singular_ratio: f64,
    pub first_person_plural_ratio: f64,
    pub citation_density: f64,
    pub flesch_reading_ease: f64,
    pub flesch_kincaid_grade: f64,
    pub gunning_fog: f64,
    pub avg_syllables_per_word: f64,
    pub complex_word_ratio: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct PaperSummary {
    pub label: String,
    pub word_count: usize,
    pub avg_sentence_length: f64,
    pub type_token_ratio: f64,
    pub flesch_reading_ease: f64,
}

/// Compute a style fingerprint from multiple text samples (the author's prior papers).
/// Each entry is (label, text).
pub fn compute_fingerprint(papers: Vec<(String, String)>) -> StyleFingerprint {
    let mut profiles: Vec<(String, style::StyleProfile)> = Vec::new();
    let mut total_words = 0usize;

    for (label, text) in &papers {
        let profile = style::analyze(text);
        total_words += profile.word_count;
        profiles.push((label.clone(), profile));
    }

    let n = profiles.len().max(1) as f64;

    // Compute averages across all papers
    let metrics = if profiles.is_empty() {
        FingerprintMetrics {
            avg_sentence_length: 0.0,
            sentence_length_stdev: 0.0,
            type_token_ratio: 0.0,
            passive_ratio: 0.0,
            hedge_density: 0.0,
            connector_density: 0.0,
            first_person_singular_ratio: 0.0,
            first_person_plural_ratio: 0.0,
            citation_density: 0.0,
            flesch_reading_ease: 0.0,
            flesch_kincaid_grade: 0.0,
            gunning_fog: 0.0,
            avg_syllables_per_word: 0.0,
            complex_word_ratio: 0.0,
        }
    } else {
        let avg = |f: fn(&style::StyleProfile) -> f64| -> f64 {
            profiles.iter().map(|(_, p)| f(p)).sum::<f64>() / n
        };
        FingerprintMetrics {
            avg_sentence_length: avg(|p| p.avg_sentence_length),
            sentence_length_stdev: avg(|p| p.sentence_length_stdev),
            type_token_ratio: avg(|p| p.type_token_ratio),
            passive_ratio: avg(|p| p.passive_ratio),
            hedge_density: avg(|p| p.hedge_density),
            connector_density: avg(|p| p.connector_density),
            first_person_singular_ratio: avg(|p| p.first_person_singular_ratio),
            first_person_plural_ratio: avg(|p| p.first_person_plural_ratio),
            citation_density: avg(|p| p.citation_density),
            flesch_reading_ease: avg(|p| p.flesch_reading_ease),
            flesch_kincaid_grade: avg(|p| p.flesch_kincaid_grade),
            gunning_fog: avg(|p| p.gunning_fog),
            avg_syllables_per_word: avg(|p| p.avg_syllables_per_word),
            complex_word_ratio: avg(|p| p.complex_word_ratio),
        }
    };

    let per_paper: Vec<PaperSummary> = profiles
        .iter()
        .map(|(label, p)| PaperSummary {
            label: label.clone(),
            word_count: p.word_count,
            avg_sentence_length: p.avg_sentence_length,
            type_token_ratio: p.type_token_ratio,
            flesch_reading_ease: p.flesch_reading_ease,
        })
        .collect();

    // Generate export formats (privacy-safe: aggregate metrics only, no raw text)
    let export_json = generate_json_export(&metrics, &per_paper, papers.len(), total_words);
    let export_markdown = generate_markdown_export(&metrics, &per_paper, papers.len(), total_words);

    StyleFingerprint {
        papers_analyzed: papers.len(),
        total_words,
        metrics,
        per_paper_profiles: per_paper,
        export_json,
        export_markdown,
    }
}

fn generate_json_export(
    metrics: &FingerprintMetrics,
    per_paper: &[PaperSummary],
    n_papers: usize,
    total_words: usize,
) -> String {
    let json = serde_json::json!({
        "fingerprint_type": "ScholarScribe Style Fingerprint v1.0",
        "generated": "2026",
        "privacy_notice": "This fingerprint contains aggregate stylometric metrics only. No raw text is included.",
        "papers_analyzed": n_papers,
        "total_words_analyzed": total_words,
        "aggregate_metrics": {
            "avg_sentence_length": (metrics.avg_sentence_length * 100.0).round() / 100.0,
            "sentence_length_stdev": (metrics.sentence_length_stdev * 100.0).round() / 100.0,
            "type_token_ratio": (metrics.type_token_ratio * 1000.0).round() / 1000.0,
            "passive_ratio": (metrics.passive_ratio * 1000.0).round() / 1000.0,
            "hedge_density": (metrics.hedge_density * 1000.0).round() / 1000.0,
            "connector_density": (metrics.connector_density * 1000.0).round() / 1000.0,
            "first_person_singular_ratio": (metrics.first_person_singular_ratio * 1000.0).round() / 1000.0,
            "first_person_plural_ratio": (metrics.first_person_plural_ratio * 1000.0).round() / 1000.0,
            "citation_density": (metrics.citation_density * 1000.0).round() / 1000.0,
            "flesch_reading_ease": (metrics.flesch_reading_ease * 100.0).round() / 100.0,
            "flesch_kincaid_grade": (metrics.flesch_kincaid_grade * 100.0).round() / 100.0,
            "gunning_fog": (metrics.gunning_fog * 100.0).round() / 100.0,
            "avg_syllables_per_word": (metrics.avg_syllables_per_word * 1000.0).round() / 1000.0,
            "complex_word_ratio": (metrics.complex_word_ratio * 1000.0).round() / 1000.0,
        },
        "per_paper_summaries": per_paper.iter().map(|p| {
            serde_json::json!({
                "label": p.label,
                "word_count": p.word_count,
                "avg_sentence_length": (p.avg_sentence_length * 100.0).round() / 100.0,
                "type_token_ratio": (p.type_token_ratio * 1000.0).round() / 1000.0,
                "flesch_reading_ease": (p.flesch_reading_ease * 100.0).round() / 100.0,
            })
        }).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&json).unwrap_or_default()
}

fn generate_markdown_export(
    metrics: &FingerprintMetrics,
    per_paper: &[PaperSummary],
    n_papers: usize,
    total_words: usize,
) -> String {
    let mut md = String::new();
    md.push_str("# ScholarScribe Style Fingerprint\n\n");
    md.push_str("**Privacy notice:** This fingerprint contains aggregate stylometric metrics only. No raw text is included.\n\n");
    md.push_str(&format!("**Papers analyzed:** {}\n", n_papers));
    md.push_str(&format!("**Total words analyzed:** {}\n\n", total_words));
    md.push_str("## Aggregate Metrics\n\n");
    md.push_str("| Metric | Value |\n|---|---|\n");
    md.push_str(&format!(
        "| Avg. sentence length | {:.2} words |\n",
        metrics.avg_sentence_length
    ));
    md.push_str(&format!(
        "| Sentence length σ | {:.2} |\n",
        metrics.sentence_length_stdev
    ));
    md.push_str(&format!(
        "| Vocabulary diversity (TTR) | {:.3} |\n",
        metrics.type_token_ratio
    ));
    md.push_str(&format!(
        "| Passive-voice density | {:.3} |\n",
        metrics.passive_ratio
    ));
    md.push_str(&format!(
        "| Hedging density | {:.3} |\n",
        metrics.hedge_density
    ));
    md.push_str(&format!(
        "| Connector density | {:.3} |\n",
        metrics.connector_density
    ));
    md.push_str(&format!(
        "| First-person singular | {:.3} |\n",
        metrics.first_person_singular_ratio
    ));
    md.push_str(&format!(
        "| First-person plural | {:.3} |\n",
        metrics.first_person_plural_ratio
    ));
    md.push_str(&format!(
        "| Citation density | {:.3} |\n",
        metrics.citation_density
    ));
    md.push_str(&format!(
        "| Flesch Reading Ease | {:.1} |\n",
        metrics.flesch_reading_ease
    ));
    md.push_str(&format!(
        "| Flesch-Kincaid Grade | {:.1} |\n",
        metrics.flesch_kincaid_grade
    ));
    md.push_str(&format!("| Gunning Fog | {:.1} |\n", metrics.gunning_fog));
    md.push_str(&format!(
        "| Avg. syllables/word | {:.3} |\n",
        metrics.avg_syllables_per_word
    ));
    md.push_str(&format!(
        "| Complex-word ratio | {:.3} |\n\n",
        metrics.complex_word_ratio
    ));
    md.push_str("## Per-Paper Summary\n\n");
    md.push_str("| Paper | Words | Avg. sent. len. | TTR | Flesch |\n|---|---|---|---|---|\n");
    for p in per_paper {
        md.push_str(&format!(
            "| {} | {} | {:.1} | {:.3} | {:.1} |\n",
            p.label, p.word_count, p.avg_sentence_length, p.type_token_ratio, p.flesch_reading_ease
        ));
    }
    md.push_str("\n---\n\n*Generated by ScholarScribe. This fingerprint may accompany a manuscript submission as supplementary evidence of authorial consistency.*\n");
    md
}
