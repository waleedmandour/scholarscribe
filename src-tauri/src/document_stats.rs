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

// ---------- v0.2.0: Readability by Section ----------

#[derive(Debug, serde::Serialize)]
pub struct SectionReadability {
    pub section_name: String,
    pub word_count: usize,
    pub sentence_count: usize,
    pub avg_sentence_length: f64,
    pub flesch_reading_ease: f64,
    pub flesch_kincaid_grade: f64,
    pub gunning_fog: f64,
    pub interpretation: String,
}

#[derive(Debug, serde::Serialize)]
pub struct SectionReadabilityReport {
    pub sections: Vec<SectionReadability>,
    pub document_average: SectionReadability,
    pub explanation: String,
}

pub fn analyze_by_sections(text: &str) -> SectionReadabilityReport {
    let structure = crate::structure_analyzer::analyze_text(text);

    let sections: Vec<(String, String)> = if structure.headings.is_empty() {
        // No headings — split by paragraphs into ~300-word chunks
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut out = Vec::new();
        for (i, chunk) in words.chunks(300).enumerate() {
            out.push((format!("Section {}", i + 1), chunk.join(" ")));
        }
        out
    } else {
        // Extract text between headings
        let mut out = Vec::new();
        for (i, h) in structure.headings.iter().enumerate() {
            let start = if i == 0 {
                0
            } else {
                text.find(&h.text).unwrap_or(0)
            };
            let end = if i + 1 < structure.headings.len() {
                text.find(&structure.headings[i + 1].text)
                    .unwrap_or(text.len())
            } else {
                text.len()
            };
            let section_text = &text[start.min(end)..end];
            out.push((h.text.clone(), section_text.to_string()));
        }
        out
    };

    let mut section_results = Vec::new();
    let mut total_words = 0usize;
    let mut total_sentences = 0usize;
    let mut total_flesch = 0.0;
    let mut total_fk = 0.0;
    let mut total_fog = 0.0;
    let mut total_asl = 0.0;
    let mut count = 0usize;

    for (name, text) in &sections {
        if text.trim().is_empty() {
            continue;
        }
        let profile = crate::style::analyze(text);
        let interpretation = flesch_interpretation(profile.flesch_reading_ease);
        total_words += profile.word_count;
        total_sentences += profile.sentence_count;
        total_flesch += profile.flesch_reading_ease;
        total_fk += profile.flesch_kincaid_grade;
        total_fog += profile.gunning_fog;
        total_asl += profile.avg_sentence_length;
        count += 1;
        section_results.push(SectionReadability {
            section_name: name.clone(),
            word_count: profile.word_count,
            sentence_count: profile.sentence_count,
            avg_sentence_length: profile.avg_sentence_length,
            flesch_reading_ease: profile.flesch_reading_ease,
            flesch_kincaid_grade: profile.flesch_kincaid_grade,
            gunning_fog: profile.gunning_fog,
            interpretation,
        });
    }

    let doc_avg = SectionReadability {
        section_name: "Document average".into(),
        word_count: total_words,
        sentence_count: total_sentences,
        avg_sentence_length: if count > 0 {
            total_asl / count as f64
        } else {
            0.0
        },
        flesch_reading_ease: if count > 0 {
            total_flesch / count as f64
        } else {
            0.0
        },
        flesch_kincaid_grade: if count > 0 {
            total_fk / count as f64
        } else {
            0.0
        },
        gunning_fog: if count > 0 {
            total_fog / count as f64
        } else {
            0.0
        },
        interpretation: "Document-wide average".into(),
    };

    let explanation = format!(
        "Readability varies naturally across sections: Methods sections typically score harder on Flesch (which is appropriate — they describe technical procedures), while introductions and discussions should be more accessible. This section-aware view helps you calibrate your writing appropriately for each part of the manuscript."
    );

    SectionReadabilityReport {
        sections: section_results,
        document_average: doc_avg,
        explanation,
    }
}

fn flesch_interpretation(score: f64) -> String {
    if score >= 90 {
        "very easy (5th grade)".into()
    } else if score >= 70 {
        "easy (7th grade)".into()
    } else if score >= 60 {
        "standard (8th-9th grade)".into()
    } else if score >= 50 {
        "fairly hard (10th-12th grade)".into()
    } else if score >= 30 {
        "difficult (college)".into()
    } else {
        "very difficult (college graduate)".into()
    }
}
