//! Structure Analyzer — extracts a document's heading tree and suggests
//! missing sections based on common academic structure.
//!
//! For .docx files, headings are detected via Word's built-in heading styles
//! (Heading1, Heading2, etc.) in the OOXML. For plain text, headings are
//! detected via markdown-style `#` prefixes or ALL-CAPS lines.
//!
//! All processing is local.

use regex::Regex;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: u8, // 1 = H1, 2 = H2, etc.
    pub text: String,
    pub word_count: usize, // words in the section body (until next heading)
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
    pub source_kind: String, // "docx" | "text"
}

/// Common academic sections that should appear in most manuscripts.
/// Used to suggest missing sections.
const EXPECTED_SECTIONS: &[(&str, &[&str])] = &[
    ("Introduction", &["introduction", "background", "overview"]),
    (
        "Methods",
        &[
            "method",
            "methodology",
            "materials and methods",
            "experimental",
            "study design",
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

/// Analyze the structure of a .docx file by extracting its heading tree.
/// Headings are detected via Word's heading styles in the OOXML.
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

/// Analyze the structure of plain text. Headings are detected via:
/// - Markdown-style `#`, `##`, `###` prefixes
/// - ALL-CAPS lines (length >= 4, mostly uppercase, no sentence punctuation)
pub fn analyze_text(text: &str) -> StructureReport {
    let headings = extract_text_headings(text);
    build_report(headings, None, "text")
}

/// Extract headings from OOXML by finding paragraphs with heading styles.
fn extract_docx_headings(xml: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    // A heading paragraph looks like:
    //   <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>...<w:t>Heading Text</w:t>...</w:p>
    // We split by </w:p> to get paragraphs, then check each for a heading style.
    let paragraphs: Vec<&str> = xml.split("</w:p>").collect();

    for para in paragraphs {
        // Check if this paragraph has a heading style
        let style_re = Regex::new(r#"w:pStyle\s+w:val="([^"]*)""#).unwrap();
        let style = style_re
            .captures(para)
            .map(|c| c[1].to_lowercase())
            .unwrap_or_default();

        let level = if style.starts_with("heading") {
            // "Heading1", "Heading 1", "heading1" → 1
            let digits: String = style.chars().filter(|c| c.is_ascii_digit()).collect();
            digits.parse::<u8>().unwrap_or(0)
        } else if style == "title" {
            0 // Title — treat as level 0 (above H1)
        } else {
            0 // Not a heading
        };

        if level == 0 && style != "title" {
            continue;
        }

        // Extract all <w:t> text from this paragraph
        let text_re = Regex::new(r"<w:t[^>]*>([^<]*)</w:t>").unwrap();
        let text: String = text_re
            .captures_iter(para)
            .map(|c| c[1].to_string())
            .collect::<Vec<_>>()
            .join("");

        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        headings.push(Heading {
            level: if style == "title" { 0 } else { level },
            text: text.to_string(),
            word_count: 0, // filled in below
        });
    }

    // We can't easily compute section word counts from the XML without parsing
    // the full body. For now, leave them as 0. (A future version could walk
    // paragraphs between headings and count <w:t> content.)
    headings
}

/// Extract headings from plain text.
fn extract_text_headings(text: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Markdown heading?
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
                // Count words until next heading or end
                let word_count = count_words_until_next_heading(&lines, i + 1);
                headings.push(Heading {
                    level,
                    text: heading_text.to_string(),
                    word_count,
                });
                continue;
            }
        }

        // ALL-CAPS heading? (length >= 4, >= 80% uppercase, no sentence punctuation)
        let alpha_chars: Vec<char> = trimmed.chars().filter(|c| c.is_alphabetic()).collect();
        if alpha_chars.len() >= 4 {
            let upper_count = alpha_chars.iter().filter(|c| c.is_uppercase()).count();
            let ratio = upper_count as f64 / alpha_chars.len() as f64;
            if ratio > 0.8
                && !trimmed.ends_with('.')
                && !trimmed.ends_with(';')
                && !trimmed.ends_with(',')
                && !trimmed.ends_with(':')
            {
                let word_count = count_words_until_next_heading(&lines, i + 1);
                headings.push(Heading {
                    level: 1, // ALL-CAPS treated as H1
                    text: trimmed.to_string(),
                    word_count,
                });
            }
        }
    }

    headings
}

fn count_words_until_next_heading(lines: &[&str], start: usize) -> usize {
    let mut count = 0;
    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Stop at next heading
        if trimmed.starts_with('#') {
            break;
        }
        let alpha_chars: Vec<char> = trimmed.chars().filter(|c| c.is_alphabetic()).collect();
        if alpha_chars.len() >= 4 {
            let upper_count = alpha_chars.iter().filter(|c| c.is_uppercase()).count();
            if upper_count as f64 / alpha_chars.len() as f64 > 0.8
                && !trimmed.ends_with('.')
                && !trimmed.ends_with(';')
            {
                break;
            }
        }
        count += trimmed.split_whitespace().count();
    }
    count
}

fn build_report(
    headings: Vec<Heading>,
    source_path: Option<String>,
    source_kind: &str,
) -> StructureReport {
    let total_sections = headings.len();
    let max_depth = headings.iter().map(|h| h.level).max().unwrap_or(0);

    // Find missing expected sections
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

    // Find short sections (< 100 words, excluding title-level headings)
    let short_sections: Vec<Heading> = headings
        .iter()
        .filter(|h| h.level >= 1 && h.word_count > 0 && h.word_count < 100)
        .cloned()
        .collect();

    // Build suggestions
    let mut suggestions = Vec::new();
    if !missing_sections.is_empty() {
        suggestions.push(format!(
            "Missing sections: {}. Consider adding these if appropriate for your venue.",
            missing_sections.join(", ")
        ));
    }
    if !short_sections.is_empty() {
        suggestions.push(format!(
            "Short sections (< 100 words): {}. These may need more content.",
            short_sections
                .iter()
                .map(|h| h.text.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if total_sections == 0 {
        suggestions.push("No headings detected. If this is a manuscript, consider adding section headings to improve structure.".into());
    }
    if total_sections > 0 && total_sections < 3 {
        suggestions.push("Only a few sections detected. Most academic manuscripts have 5-7 main sections (Introduction, Methods, Results, Discussion, Conclusion, References).".into());
    }
    if max_depth >= 4 {
        suggestions.push("Deep nesting detected (4+ heading levels). Consider flattening your structure — most journals prefer at most 3 levels.".into());
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
