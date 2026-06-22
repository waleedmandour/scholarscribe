//! Citation Manager — validates in-text citations against a .bib (BibTeX) file.
//!
//! Three checks:
//! 1. Undefined citations — in-text citations in the draft that don't match
//!    any .bib entry. These are the dangerous ones (likely fabricated).
//! 2. Unused references — .bib entries never cited in the draft.
//! 3. Citation count per reference — how many times each .bib entry is cited.
//!
//! All parsing is local. The .bib content never leaves the device.

use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct BibEntry {
    pub key: String,        // citation key, e.g. "smith2020"
    pub entry_type: String, // "article", "inproceedings", "book", etc.
    pub title: String,
    pub author: String,
    pub year: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InTextCitation {
    pub raw: String,            // "(Smith, 2020)" or "[12]"
    pub author: String,         // "Smith" or "Smith et al."
    pub year: String,           // "2020" (empty for numeric style)
    pub numeric: Option<usize>, // Some(12) for "[12]" style
    pub position: usize,        // character offset in the draft
}

#[derive(Debug, Serialize)]
pub struct CitationReport {
    pub bib_entries: Vec<BibEntry>,
    pub in_text_citations: Vec<InTextCitation>,
    pub undefined_citations: Vec<InTextCitation>,
    pub unused_references: Vec<BibEntry>,
    pub citation_counts: Vec<(BibEntry, usize)>, // (entry, times_cited)
    pub bib_parse_errors: Vec<String>,
    pub draft_path: Option<String>,
    pub bib_path: Option<String>,
}

/// Parse a .bib file into a list of entries. Returns (entries, parse_errors).
pub fn parse_bib(content: &str) -> (Vec<BibEntry>, Vec<String>) {
    let mut entries = Vec::new();
    let mut errors = Vec::new();

    // Match @type{key, ... } — non-greedy, balanced-brace-aware via simple state machine.
    let mut chars = content.chars().peekable();
    let mut pos = 0;
    let content_bytes = content.as_bytes();

    while pos < content_bytes.len() {
        // Find next '@'
        if content_bytes[pos] != b'@' {
            pos += 1;
            continue;
        }
        // Read entry type until '{'
        let type_start = pos + 1;
        let mut type_end = type_start;
        while type_end < content_bytes.len()
            && content_bytes[type_end] != b'{'
            && content_bytes[type_end] != b'('
        {
            type_end += 1;
        }
        if type_end >= content_bytes.len() {
            break;
        }
        let entry_type = content[type_start..type_end].trim().to_lowercase();
        if entry_type.is_empty() || entry_type == "comment" || entry_type == "string" {
            // Skip @comment and @string — they're not bibliography entries.
            // Find the matching close brace/paren and continue.
            pos = type_end + 1;
            continue;
        }
        let open = content_bytes[type_end];
        let close = if open == b'{' { b'}' } else { b')' };

        // Read the citation key (up to the first comma)
        let key_start = type_end + 1;
        let mut key_end = key_start;
        while key_end < content_bytes.len()
            && content_bytes[key_end] != b','
            && content_bytes[key_end] != close
        {
            key_end += 1;
        }
        if key_end >= content_bytes.len() {
            errors.push(format!(
                "@{} entry starting at offset {}: unterminated",
                entry_type, type_start
            ));
            break;
        }
        let key = content[key_start..key_end].trim().to_string();

        // Now scan fields until we find the matching close brace at depth 0.
        let mut depth = 1;
        let mut field_start = key_end + 1;
        let mut i = key_end + 1;
        let mut title = String::new();
        let mut author = String::new();
        let mut year = String::new();

        while i < content_bytes.len() && depth > 0 {
            match content_bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        // End of entry
                        break;
                    }
                }
                b'(' if open == b'(' => {}
                b')' if open == b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        if depth != 0 {
            errors.push(format!(
                "@{} entry '{}': braces unbalanced",
                entry_type, key
            ));
            pos = i;
            continue;
        }

        // Extract the body (between the comma after the key and the closing brace).
        let body = &content[key_end + 1..i];

        // Simple field extractor: look for fieldname = {value} or fieldname = "value".
        title = extract_field(body, "title").unwrap_or_default();
        author = extract_field(body, "author").unwrap_or_default();
        year = extract_field(body, "year").unwrap_or_default();

        entries.push(BibEntry {
            key,
            entry_type,
            title: clean_latex(&title),
            author: clean_latex(&author),
            year,
        });

        pos = i + 1;
    }

    (entries, errors)
}

/// Extract a field value from a .bib body. Looks for `fieldname = {value}` or
/// `fieldname = "value"`. Returns the value (without braces/quotes).
fn extract_field(body: &str, fieldname: &str) -> Option<String> {
    let lower = body.to_lowercase();
    let pattern = format!("{}\\s*=", fieldname.to_lowercase());
    let re = Regex::new(&pattern).ok()?;
    let mat = re.find(&lower)?;
    let after_eq = mat.end();
    let bytes = body.as_bytes();
    let mut i = after_eq;
    // Skip whitespace
    while i < bytes.len()
        && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r')
    {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let delim = bytes[i];
    if delim == b'{' {
        // Read until matching close brace
        let mut depth = 1;
        let mut j = i + 1;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                j += 1;
            }
        }
        if depth != 0 {
            return None;
        }
        Some(body[i + 1..j].to_string())
    } else if delim == b'"' {
        // Read until next unescaped quote
        let mut j = i + 1;
        while j < bytes.len() {
            if bytes[j] == b'\\' {
                j += 2;
                continue;
            }
            if bytes[j] == b'"' {
                return Some(body[i + 1..j].to_string());
            }
            j += 1;
        }
        None
    } else {
        // Bare value (e.g. year = 2020) — read until comma or end
        let mut j = i;
        while j < bytes.len() && bytes[j] != b',' && bytes[j] != b'\n' && bytes[j] != b'}' {
            j += 1;
        }
        Some(body[i..j].trim().to_string())
    }
}

/// Strip simple LaTeX commands from a string. {\i text} → text, \textit{text} → text, etc.
fn clean_latex(s: &str) -> String {
    let mut out = s.to_string();
    // Remove common formatting commands: {\i ...}, {\em ...}, \textit{...}, etc.
    let re = Regex::new(r"\\[a-zA-Z]+\{([^{}]*)\}").unwrap();
    out = re.replace_all(&out, "$1").into_owned();
    // Remove standalone braces
    let re2 = Regex::new(r"[{}]").unwrap();
    out = re2.replace_all(&out, "").into_owned();
    // Replace LaTeX escapes
    out = out.replace("\\&", "&");
    out = out.replace("\\%", "%");
    out = out.replace("\\$", "$");
    out = out.replace("\\#", "#");
    out = out.replace("\\_", "_");
    out.trim().to_string()
}

/// Extract in-text citations from a draft.
/// Recognizes:
/// - Author-year style: (Smith, 2020), (Smith et al., 2020), (Smith and Jones, 2020),
///   Smith (2020), Smith et al. (2020)
/// - Numeric style: [1], [2,3], [1-3]
pub fn extract_in_text_citations(text: &str) -> Vec<InTextCitation> {
    let mut out = Vec::new();

    // Author-year: parenthetical (Author, Year) or (Author et al., Year; Author2, Year2)
    let re_paren = Regex::new(r"\(([A-Z][a-zA-Z'\-]+(?:\s+(?:et\s+al\.?|and|&)\s+[A-Z][a-zA-Z'\-]+)?),?\s*(\d{4}[a-z]?)\)").unwrap();
    for cap in re_paren.captures_iter(text) {
        let m = cap.get(0).unwrap();
        out.push(InTextCitation {
            raw: m.as_str().to_string(),
            author: cap[1].to_string(),
            year: cap[2].to_string(),
            numeric: None,
            position: m.start(),
        });
    }

    // Author-year: narrative — Smith (2020), Smith et al. (2020), Smith and Jones (2020)
    let re_narrative = Regex::new(r"\b([A-Z][a-zA-Z'\-]+(?:(?:\s+et\s+al\.?)|(?:\s+and\s+[A-Z][a-zA-Z'\-]+)|(?:\s+&\s+[A-Z][a-zA-Z'\-]+))?)\s+\((\d{4}[a-z]?)\)").unwrap();
    for cap in re_narrative.captures_iter(text) {
        let m = cap.get(0).unwrap();
        // Avoid double-counting if this is inside a parenthetical we already captured
        if out.iter().any(|c| c.position == m.start()) {
            continue;
        }
        out.push(InTextCitation {
            raw: m.as_str().to_string(),
            author: cap[1].to_string(),
            year: cap[2].to_string(),
            numeric: None,
            position: m.start(),
        });
    }

    // Numeric: [1], [12], [1, 2, 3], [1-3]
    let re_numeric = Regex::new(r"\[([\d,\s\-]+)\]").unwrap();
    for cap in re_numeric.captures_iter(text) {
        let m = cap.get(0).unwrap();
        // Parse the numbers
        let nums_str = &cap[1];
        let mut nums = Vec::new();
        for part in nums_str.split(',') {
            let part = part.trim();
            if let Some((a, b)) = part.split_once('-') {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) {
                    for n in a..=b {
                        nums.push(n);
                    }
                }
            } else if let Ok(n) = part.parse::<usize>() {
                nums.push(n);
            }
        }
        for n in nums {
            out.push(InTextCitation {
                raw: m.as_str().to_string(),
                author: String::new(),
                year: String::new(),
                numeric: Some(n),
                position: m.start(),
            });
        }
    }

    out
}

/// Validate in-text citations against .bib entries. Produces a CitationReport.
pub fn validate(draft_text: &str, bib_content: &str) -> CitationReport {
    let (bib_entries, bib_parse_errors) = parse_bib(bib_content);
    let in_text_citations = extract_in_text_citations(draft_text);

    // Build a lookup: for each .bib entry, what (author_last_name, year) pairs
    // would match it? Most .bib entries have author = "Smith, John" — last name "Smith".
    let mut bib_lookup: HashMap<(String, String), &BibEntry> = HashMap::new();
    let mut bib_by_key: HashMap<String, &BibEntry> = HashMap::new();
    for entry in &bib_entries {
        bib_by_key.insert(entry.key.clone(), entry);
        // Extract last name from "Smith, John" or "John Smith"
        let last_name = extract_last_name(&entry.author);
        if !last_name.is_empty() && !entry.year.is_empty() {
            bib_lookup.insert((last_name.to_lowercase(), entry.year.clone()), entry);
        }
    }

    // For each in-text citation, check if it matches a .bib entry.
    let mut undefined_citations = Vec::new();
    let mut cited_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    for cite in &in_text_citations {
        if let Some(n) = cite.numeric {
            // Numeric style — match by position in .bib (1-indexed)
            if let Some(entry) = bib_entries.get(n.saturating_sub(1)) {
                cited_keys.insert(entry.key.clone());
            } else {
                undefined_citations.push(cite.clone());
            }
        } else {
            // Author-year style — match by (last_name, year)
            let last_name = extract_last_name(&cite.author);
            let key = (last_name.to_lowercase(), cite.year.clone());
            if let Some(entry) = bib_lookup.get(&key) {
                cited_keys.insert(entry.key.clone());
            } else {
                // Also try matching by citation key directly (some drafts use \cite{smith2020}
                // and the parser might pick up the key as "author")
                if bib_by_key.contains_key(&cite.author.to_lowercase()) {
                    cited_keys.insert(cite.author.to_lowercase());
                } else {
                    undefined_citations.push(cite.clone());
                }
            }
        }
    }

    // Unused references: .bib entries never cited.
    let unused_references: Vec<BibEntry> = bib_entries
        .iter()
        .filter(|e| !cited_keys.contains(&e.key))
        .cloned()
        .collect();

    // Citation counts per reference
    let mut counts: HashMap<String, usize> = HashMap::new();
    for cite in &in_text_citations {
        if let Some(n) = cite.numeric {
            if let Some(entry) = bib_entries.get(n.saturating_sub(1)) {
                *counts.entry(entry.key.clone()).or_insert(0) += 1;
            }
        } else {
            let last_name = extract_last_name(&cite.author);
            let key = (last_name.to_lowercase(), cite.year.clone());
            if let Some(entry) = bib_lookup.get(&key) {
                *counts.entry(entry.key.clone()).or_insert(0) += 1;
            }
        }
    }
    let citation_counts: Vec<(BibEntry, usize)> = bib_entries
        .iter()
        .map(|e| (e.clone(), *counts.get(&e.key).unwrap_or(&0)))
        .collect();

    CitationReport {
        bib_entries,
        in_text_citations,
        undefined_citations,
        unused_references,
        citation_counts,
        bib_parse_errors,
        draft_path: None,
        bib_path: None,
    }
}

/// Extract the last name from a BibTeX author string or in-text citation author.
/// "Smith, John" → "Smith", "John Smith" → "Smith", "Smith and Jones" → "Smith" (first author),
/// "Jones et al." → "Jones" (strip "et al." before extracting).
fn extract_last_name(author: &str) -> String {
    if author.is_empty() {
        return String::new();
    }
    // Strip "et al." (with or without period) — we want the first author only.
    let cleaned = {
        let re = Regex::new(r"(?i)\s+et\s+al\.?").unwrap();
        re.replace_all(author, "").to_string()
    };
    // Take the first author (before " and ")
    let first = cleaned.split(" and ").next().unwrap_or("").trim();
    if first.is_empty() {
        return String::new();
    }
    // "Smith, John" → "Smith"
    if let Some((last, _)) = first.split_once(',') {
        return last.trim().to_string();
    }
    // "John Smith" → "Smith" (last whitespace-separated token)
    let parts: Vec<&str> = first.split_whitespace().collect();
    if let Some(last) = parts.last() {
        return last.trim_end_matches('.').to_string();
    }
    String::new()
}

/// Read a .bib file from disk.
pub fn read_bib_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))
}

// ---------- v0.2.0: Citation Context Validator ----------

/// For each in-text citation, extract the surrounding context (the sentence
/// containing the citation) and check whether the citation's attributed
/// concept appears relevant to the cited work's title. This is a heuristic
/// check — it flags potential misattributions for human review.
///
/// Uses simple keyword overlap between the surrounding sentence and the
/// .bib entry's title/keywords. A local LLM could improve this, but the
/// heuristic version is fast and requires no model.
pub fn validate_citation_contexts(text: &str, bib_content: &str) -> Vec<CitationContextCheck> {
    let (bib_entries, _) = parse_bib(bib_content);
    let in_text = extract_in_text_citations(text);

    // Build lookup: (last_name, year) → bib entry
    let mut bib_lookup: std::collections::HashMap<(String, String), &BibEntry> =
        std::collections::HashMap::new();
    for entry in &bib_entries {
        let last_name = extract_last_name(&entry.author);
        if !last_name.is_empty() && !entry.year.is_empty() {
            bib_lookup.insert((last_name.to_lowercase(), entry.year.clone()), entry);
        }
    }

    let mut checks = Vec::new();

    for cite in &in_text {
        // Get the sentence containing this citation
        let sentence = extract_sentence_at(text, cite.position);

        // Find the matching bib entry
        let entry = if let Some(n) = cite.numeric {
            bib_entries.get(n.saturating_sub(1))
        } else {
            let last_name = extract_last_name(&cite.author);
            bib_lookup
                .get(&(last_name.to_lowercase(), cite.year.clone()))
                .copied()
        };

        if let Some(entry) = entry {
            // Compute keyword overlap between the sentence and the bib entry's title
            let sentence_words: std::collections::HashSet<String> = sentence
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3) // skip short words
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .collect();

            let title_words: std::collections::HashSet<String> = entry
                .title
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .collect();

            let overlap = sentence_words.intersection(&title_words).count();
            let max_possible = title_words.len().max(1);
            let overlap_pct = (overlap as f64 / max_possible as f64) * 100.0;

            let verdict = if overlap_pct > 30.0 {
                "strong".to_string()
            } else if overlap_pct > 10.0 {
                "moderate".to_string()
            } else {
                "weak".to_string()
            };

            let note = if overlap_pct < 10.0 {
                format!("Low keyword overlap between the citing sentence and the cited work's title (\"{}\"). Verify the citation is contextually appropriate.", entry.title)
            } else {
                format!(
                    "Citation appears contextually related to the cited work (\"{}\").",
                    entry.title
                )
            };

            checks.push(CitationContextCheck {
                citation_raw: cite.raw.clone(),
                bib_key: entry.key.clone(),
                bib_title: entry.title.clone(),
                sentence: sentence,
                keyword_overlap_pct: (overlap_pct * 10.0).round() / 10.0,
                verdict,
                note,
            });
        }
    }

    checks
}

#[derive(Debug, serde::Serialize)]
pub struct CitationContextCheck {
    pub citation_raw: String,
    pub bib_key: String,
    pub bib_title: String,
    pub sentence: String,
    pub keyword_overlap_pct: f64,
    pub verdict: String,
    pub note: String,
}

fn extract_sentence_at(text: &str, position: usize) -> String {
    let bytes = text.as_bytes();
    if position >= bytes.len() {
        return String::new();
    }
    // Find sentence start: walk backwards to find . ! ? or start of text
    let mut start = position;
    while start > 0 {
        let c = bytes[start - 1];
        if c == b'.' || c == b'!' || c == b'?' || c == b'\n' {
            break;
        }
        start -= 1;
    }
    // Find sentence end: walk forwards
    let mut end = position;
    while end < bytes.len() {
        let c = bytes[end];
        if c == b'.' || c == b'!' || c == b'?' {
            end += 1;
            break;
        }
        if c == b'\n' {
            break;
        }
        end += 1;
    }
    text[start..end.min(text.len())].trim().to_string()
}
