//! Document Statistics — quick health-check panel for a draft.
//!
//! Reports word count, sentence count, paragraph count, section count
//! (extracted from headings in plain text), citation count, reading time,
//! readability scores, and a comparison panel with common journal targets.
//!
//! All processing is local.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DocStats {
    pub word_count: usize,
    pub sentence_count: usize,
    pub paragraph_count: usize,
    pub section_count: usize,
    pub citation_count: usize,
    pub figure_count: usize,
    pub table_count: usize,
    pub avg_sentence_length: f64,
    pub type_token_ratio: f64,
    pub complex_word_ratio: f64,
    pub estimated_reading_time_minutes: usize,
    pub flesch_reading_ease: f64,
    pub flesch_kincaid_grade: f64,
    pub gunning_fog: f64,
    pub journal_comparison: Vec<JournalComparison>,
}

#[derive(Debug, Serialize, Clone)]
pub struct JournalComparison {
    pub venue: String,
    pub typical_word_count: usize,
    pub status: String, // "under", "near", "over"
    pub delta: i64,     // difference from typical
}

pub fn analyze(text: &str) -> DocStats {
    use crate::style;

    let profile = style::analyze(text);

    // Section count: count markdown-style headings (# Foo, ## Bar) and
    // ALL-CAPS lines that look like section headers (INTRODUCTION, METHODS, etc.)
    let section_count = count_sections(text);

    // Citation count: reuse the citation regex from style.rs (it counts
    // both author-year and numeric citations).
    let citation_count = profile
        .citation_density
        .mul_add(profile.sentence_count as f64, 0.0) as usize;

    // Figure/Table count: search for "Figure N", "Fig. N", "Table N"
    let figure_count = count_matches(text, &["Figure ", "Fig. ", "Figure&nbsp;"]);
    let table_count = count_matches(text, &["Table ", "Tab. "]);

    // Estimated reading time at 200 wpm
    let reading_time = (profile.word_count as f64 / 200.0).ceil() as usize;

    // Journal comparison
    let journal_comparison = compare_to_journals(profile.word_count);

    DocStats {
        word_count: profile.word_count,
        sentence_count: profile.sentence_count,
        paragraph_count: text.split('\n').filter(|l| !l.trim().is_empty()).count(),
        section_count,
        citation_count,
        figure_count,
        table_count,
        avg_sentence_length: profile.avg_sentence_length,
        type_token_ratio: profile.type_token_ratio,
        complex_word_ratio: profile.complex_word_ratio,
        estimated_reading_time_minutes: reading_time.max(1),
        flesch_reading_ease: profile.flesch_reading_ease,
        flesch_kincaid_grade: profile.flesch_kincaid_grade,
        gunning_fog: profile.gunning_fog,
        journal_comparison,
    }
}

fn count_sections(text: &str) -> usize {
    let mut count = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Markdown-style headings
        if trimmed.starts_with('#') {
            count += 1;
            continue;
        }
        // ALL-CAPS lines (length > 3, mostly uppercase, no sentence-ending punctuation)
        let alpha_chars: Vec<char> = trimmed.chars().filter(|c| c.is_alphabetic()).collect();
        if alpha_chars.len() >= 4 {
            let upper_count = alpha_chars.iter().filter(|c| c.is_uppercase()).count();
            if upper_count as f64 / alpha_chars.len() as f64 > 0.8
                && !trimmed.ends_with('.')
                && !trimmed.ends_with(';')
                && !trimmed.ends_with(',')
            {
                count += 1;
            }
        }
    }
    count
}

fn count_matches(text: &str, prefixes: &[&str]) -> usize {
    let mut count = 0;
    let lower = text.to_lowercase();
    for prefix in prefixes {
        let prefix_lower = prefix.to_lowercase();
        // Count word-boundary matches
        let pattern = format!(r"(?i)\b{}\d+", regex::escape(prefix_lower.trim_end()));
        if let Ok(re) = regex::Regex::new(&pattern) {
            count += re.find_iter(&lower).count();
        }
    }
    count
}

fn compare_to_journals(word_count: usize) -> Vec<JournalComparison> {
    let targets = [
        ("Nature (articles)", 5000),
        ("Science (research articles)", 2500),
        ("ICMJE medical journals (JAMA, NEJM, Lancet)", 3500),
        ("IEEE conference papers", 6000),
        ("IEEE transactions (full paper)", 8000),
        ("ACM SIGCHI (long papers)", 7000),
        ("ACL/EMNLP (long papers)", 8000),
        ("PLOS ONE", 8000),
        ("Most university theses (per chapter)", 6000),
    ];

    targets
        .iter()
        .map(|(venue, target)| {
            let target = *target as i64;
            let actual = word_count as i64;
            let delta = actual - target;
            let status = if delta.abs() < target / 10 {
                "near".to_string()
            } else if delta < 0 {
                "under".to_string()
            } else {
                "over".to_string()
            };
            JournalComparison {
                venue: venue.to_string(),
                typical_word_count: target as usize,
                status,
                delta,
            }
        })
        .collect()
}
