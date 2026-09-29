//! Structure Analyzer, extracts a document's heading tree and suggests
//! missing sections based on common academic structure.

use regex::Regex;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub word_count: usize,
    /// First ~15 words of the section's body content (the text that
    /// follows this heading, up to the next heading). Lets the user
    /// preview what each section contains without scrolling. Empty
    /// string when the section has no body or the body could not be
    /// extracted.
    pub excerpt: String,
}

#[derive(Debug, Serialize)]
pub struct StructureReport {
    pub headings: Vec<Heading>,
    pub total_sections: usize,
    pub max_depth: u8,
    pub missing_sections: Vec<String>,
    pub short_sections: Vec<Heading>,
    pub suggestions: Vec<String>,
    pub source_path: Option<String>,
    pub source_kind: String,
}

const EXPECTED_SECTIONS: &[(&str, &[&str])] = &[
    ("Introduction", &["introduction", "background", "overview"]),
    (
        "Methods",
        &[
            "method",
            "methodology",
            "materials and methods",
            "experimental",
        ],
    ),
    ("Results", &["result", "finding", "outcome"]),
    ("Discussion", &["discussion", "interpretation"]),
    (
        "Conclusion",
        &["conclusion", "concluding remarks", "summary"],
    ),
    ("References", &["reference", "bibliography", "works cited"]),
    ("Abstract", &["abstract", "summary"]),
    (
        "Limitations",
        &["limitation", "study limitation", "constraints"],
    ),
];

pub fn analyze_docx(path: &Path) -> Result<StructureReport, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("unzip .docx: {}", e))?;
    let mut document_xml = String::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {}: {}", i, e))?;
        if entry.name() == "word/document.xml" {
            use std::io::Read;
            entry
                .read_to_string(&mut document_xml)
                .map_err(|e| format!("read document.xml: {}", e))?;
            break;
        }
    }
    if document_xml.is_empty() {
        return Err("word/document.xml not found in .docx".into());
    }
    let headings = extract_docx_headings(&document_xml);
    Ok(build_report(
        headings,
        Some(path.to_string_lossy().into_owned()),
        "docx",
    ))
}

pub fn analyze_text(text: &str) -> StructureReport {
    let headings = extract_text_headings(text);
    build_report(headings, None, "text")
}

fn extract_docx_headings(xml: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let paragraphs: Vec<&str> = xml.split("</w:p>").collect();
    let style_re = Regex::new(r#"w:pStyle\s+w:val="([^"]*)""#).unwrap();
    let text_re = Regex::new(r"<w:t[^>]*>([^<]*)</w:t>").unwrap();

    // Pre-compute (is_heading, level, body_text) for each paragraph so we
    // can walk forward to extract the section body without re-parsing.
    #[derive(Clone)]
    struct Para {
        is_heading: bool,
        level: u8,
        text: String,
    }
    let mut paras: Vec<Para> = Vec::with_capacity(paragraphs.len());
    for para in &paragraphs {
        let style = style_re
            .captures(para)
            .map(|c| c[1].to_lowercase())
            .unwrap_or_default();
        let level = if style.starts_with("heading") {
            let digits: String = style.chars().filter(|c| c.is_ascii_digit()).collect();
            digits.parse::<u8>().unwrap_or(0)
        } else if style == "title" {
            0
        } else {
            0
        };
        let is_heading = level != 0 || style == "title";
        let text: String = text_re
            .captures_iter(para)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>()
            .join("");
        paras.push(Para {
            is_heading,
            level: if style == "title" { 0 } else { level },
            text,
        });
    }

    for (i, p) in paras.iter().enumerate() {
        if !p.is_heading || p.text.trim().is_empty() {
            continue;
        }
        // Walk forward from i+1 until the next heading, collecting body
        // paragraph text.
        let mut body_words: Vec<&str> = Vec::new();
        for next_para in &paras[i + 1..] {
            if next_para.is_heading {
                break;
            }
            let t = next_para.text.trim();
            if t.is_empty() {
                continue;
            }
            for w in t.split_whitespace() {
                body_words.push(w);
                if body_words.len() >= 15 {
                    break;
                }
            }
            if body_words.len() >= 15 {
                break;
            }
        }
        let mut excerpt = body_words
            .iter()
            .take(15)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        // Compute the full section word count by walking the same range
        // without the 15-word cap. The first walk only collected up to
        // 15 words for the excerpt; this walk counts every word in the
        // section body.
        let mut full_count = 0;
        for next_para in &paras[i + 1..] {
            if next_para.is_heading {
                break;
            }
            full_count += next_para.text.split_whitespace().count();
        }
        if full_count > 15 {
            excerpt.push_str("...");
        }
        headings.push(Heading {
            level: p.level,
            text: p.text.trim().to_string(),
            word_count: full_count,
            excerpt,
        });
    }
    headings
}

fn extract_text_headings(text: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let md_level = if trimmed.starts_with("######") {
            Some(6)
        } else if trimmed.starts_with("#####") {
            Some(5)
        } else if trimmed.starts_with("####") {
            Some(4)
        } else if trimmed.starts_with("###") {
            Some(3)
        } else if trimmed.starts_with("##") {
            Some(2)
        } else if trimmed.starts_with("#") {
            Some(1)
        } else {
            None
        };
        if let Some(level) = md_level {
            let heading_text = trimmed.trim_start_matches('#').trim();
            if !heading_text.is_empty() {
                let (word_count, excerpt) = section_body_and_excerpt(&lines, i + 1);
                headings.push(Heading {
                    level,
                    text: heading_text.to_string(),
                    word_count,
                    excerpt,
                });
                continue;
            }
        }
        let alpha_chars: Vec<char> = trimmed.chars().filter(|c| c.is_alphabetic()).collect();
        if alpha_chars.len() >= 4 {
            let upper_count = alpha_chars.iter().filter(|c| c.is_uppercase()).count();
            if upper_count as f64 / alpha_chars.len() as f64 > 0.8
                && !trimmed.ends_with('.')
                && !trimmed.ends_with(';')
                && !trimmed.ends_with(',')
            {
                let (word_count, excerpt) = section_body_and_excerpt(&lines, i + 1);
                headings.push(Heading {
                    level: 1,
                    text: trimmed.to_string(),
                    word_count,
                    excerpt,
                });
            }
        }
    }
    headings
}

/// Walk forward from `start` until the next heading, returning
/// (total_word_count, first_15_words_excerpt). The excerpt ends with
/// "..." when the section body is longer than 15 words.
fn section_body_and_excerpt(lines: &[&str], start: usize) -> (usize, String) {
    let mut word_count = 0;
    let mut excerpt_words: Vec<&str> = Vec::new();
    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Markdown heading stop
        if trimmed.starts_with('#') {
            break;
        }
        // ALL-CAPS line (likely a section heading) stop
        let alpha_chars: Vec<char> = trimmed.chars().filter(|c| c.is_alphabetic()).collect();
        if alpha_chars.len() >= 4 {
            let upper_count = alpha_chars.iter().filter(|c| c.is_uppercase()).count();
            if upper_count as f64 / alpha_chars.len() as f64 > 0.8 && !trimmed.ends_with('.') {
                break;
            }
        }
        let line_words: Vec<&str> = trimmed.split_whitespace().collect();
        word_count += line_words.len();
        if excerpt_words.len() < 15 {
            for w in line_words {
                excerpt_words.push(w);
                if excerpt_words.len() >= 15 {
                    break;
                }
            }
        }
    }
    let mut excerpt = excerpt_words.join(" ");
    if word_count > 15 {
        excerpt.push_str("...");
    }
    (word_count, excerpt)
}

fn build_report(
    headings: Vec<Heading>,
    source_path: Option<String>,
    source_kind: &str,
) -> StructureReport {
    let total_sections = headings.len();
    let max_depth = headings.iter().map(|h| h.level).max().unwrap_or(0);
    let all_heading_text: String = headings
        .iter()
        .map(|h| h.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" | ");
    let mut missing_sections = Vec::new();
    for (canonical, aliases) in EXPECTED_SECTIONS {
        let found = aliases.iter().any(|alias| all_heading_text.contains(alias));
        if !found {
            missing_sections.push(canonical.to_string());
        }
    }
    let short_sections: Vec<Heading> = headings
        .iter()
        .filter(|h| h.level >= 1 && h.word_count > 0 && h.word_count < 100)
        .cloned()
        .collect();
    let mut suggestions = Vec::new();
    if !missing_sections.is_empty() {
        suggestions.push(format!(
            "Missing sections: {}.",
            missing_sections.join(", ")
        ));
    }
    if !short_sections.is_empty() {
        suggestions.push(format!(
            "Short sections (< 100 words): {}.",
            short_sections
                .iter()
                .map(|h| h.text.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if total_sections == 0 {
        suggestions.push("No headings detected. Consider adding section headings.".into());
    }
    if total_sections > 0 && total_sections < 3 {
        suggestions
            .push("Only a few sections detected. Most manuscripts have 5-7 main sections.".into());
    }
    if max_depth >= 4 {
        suggestions.push("Deep nesting detected (4+ levels). Consider flattening.".into());
    }
    StructureReport {
        headings,
        total_sections,
        max_depth,
        missing_sections,
        short_sections,
        suggestions,
        source_path,
        source_kind: source_kind.to_string(),
    }
}
