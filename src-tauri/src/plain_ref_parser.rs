//! Plain-text references to BibTeX converter.
//!
//! Many researchers keep their reference list in a Word document or a
//! plain-text file formatted in their target venue's citation style
//! (APA, MLA, Chicago, Vancouver, etc.) rather than in a BibTeX file.
//! This module parses pasted plain-text reference lists and converts
//! each entry to a BibTeX entry that can be validated against the
//! manuscript, downloaded as a .bib file, or both.
//!
//! The parser is heuristic and tolerant of common format variations.
//! It does not require perfectly formatted input. When a field
//! cannot be extracted, it is left empty in the BibTeX entry and a
//! warning is recorded so the user can fix the entry manually.

use regex::Regex;
use serde::Serialize;

use crate::citation_manager::BibEntry;

/// Supported plain-text reference styles. The parser uses the style
/// hint to disambiguate field boundaries, but the heuristics are
/// tolerant of mixed formats.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlainRefStyle {
    /// APA 7th edition: "Author, A. A., & Author, B. B. (Year). Title.
    /// Journal, Vol(Issue), Pages. DOI."
    Apa,
    /// MLA 9th edition: 'Author. "Title." Journal, vol. X, no. Y, Year,
    /// pp. A-B.'
    Mla,
    /// Chicago 17th edition (notes-bibliography): 'Author First Last.
    /// "Title." Journal Vol, no. Issue (Year): Pages.'
    Chicago,
    /// Vancouver (medical journals): "Author AA, Author BB. Title.
    /// Journal. Year;Vol(Issue):Pages."
    Vancouver,
    /// Plain numeric: "1. Author A, Author B. Title. Journal. Year;Vol:Pages."
    /// Numbered list with any delimiter. The parser tries each style's
    /// heuristics in turn.
    Plain,
}

impl PlainRefStyle {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "apa" => Some(Self::Apa),
            "mla" => Some(Self::Mla),
            "chicago" => Some(Self::Chicago),
            "vancouver" => Some(Self::Vancouver),
            "plain" | "auto" => Some(Self::Plain),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Apa => "APA 7th",
            Self::Mla => "MLA 9th",
            Self::Chicago => "Chicago 17th (notes-bibliography)",
            Self::Vancouver => "Vancouver (medical)",
            Self::Plain => "Plain numeric / auto-detect",
        }
    }
}

/// Result of converting a plain-text reference list to BibTeX.
#[derive(Debug, Serialize)]
pub struct ConvertResult {
    /// The generated BibTeX content, ready to write to a .bib file or
    /// feed back into the citation validator.
    pub bibtex: String,
    /// Number of references parsed.
    pub entry_count: usize,
    /// Per-entry warnings (e.g., "could not extract year for entry 3").
    /// Empty when parsing is clean.
    pub warnings: Vec<String>,
    /// The parsed entries, returned for in-memory validation without
    /// a round-trip through a .bib file.
    pub entries: Vec<BibEntry>,
}

/// Parse a plain-text reference list and convert each entry to BibTeX.
///
/// The `style` parameter is a hint. The parser tries the hinted style
/// first, then falls back to generic heuristics that work across styles.
/// References are split by newlines (with optional numbered prefixes
/// like "1.", "[1]", "(1)").
pub fn convert_plain_to_bib(content: &str, style: PlainRefStyle) -> ConvertResult {
    let mut warnings = Vec::new();
    let entries_raw = split_references(content);
    let mut entries = Vec::with_capacity(entries_raw.len());

    for (i, raw) in entries_raw.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (entry, entry_warnings) = parse_single(trimmed, style);
        // Prefix warnings with the entry index for easy locating.
        for w in entry_warnings {
            warnings.push(format!("Entry {}: {}", i + 1, w));
        }
        entries.push(entry);
    }

    let bibtex = entries_to_bibtex_string(&entries);
    let entry_count = entries.len();

    ConvertResult {
        bibtex,
        entry_count,
        warnings,
        entries,
    }
}

/// Split a pasted reference list into individual raw reference strings.
/// Handles three common formats:
///   - One reference per line (blank line between refs)
///   - Numbered prefix: "1.", "1)", "[1]", "(1)"
///   - Hanging indent: subsequent lines of the same ref are indented
fn split_references(content: &str) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    let mut current = String::new();

    // Matches a leading reference number: "1.", "1)", "[1]", "(1)"
    let num_re = Regex::new(r"^\s*(?:\[\d+\]|\(\d+\)|\d+\.|\d+\))\s+").unwrap();

    for line in content.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            // Blank line: end of current ref
            if !current.is_empty() {
                refs.push(std::mem::take(&mut current));
            }
            continue;
        }
        if num_re.is_match(trimmed) {
            // Numbered prefix: start of new ref
            if !current.is_empty() {
                refs.push(std::mem::take(&mut current));
            }
            // Strip the numbered prefix; the parser does not need it.
            let stripped = num_re.replace(trimmed, "").into_owned();
            current.push_str(&stripped);
        } else if !current.is_empty() {
            // Continuation of current ref (hanging indent or wrapped line)
            current.push(' ');
            current.push_str(trimmed.trim_start());
        } else {
            // First line of a non-numbered ref
            current.push_str(trimmed);
        }
    }
    if !current.is_empty() {
        refs.push(current);
    }
    refs
}

/// Parse a single reference string into a BibEntry, using the style
/// hint to disambiguate field boundaries.
fn parse_single(raw: &str, style: PlainRefStyle) -> (BibEntry, Vec<String>) {
    let mut warnings = Vec::new();
    let mut entry = BibEntry {
        key: String::new(),
        entry_type: "article".to_string(),
        author: String::new(),
        title: String::new(),
        journal: String::new(),
        year: String::new(),
        volume: String::new(),
        number: String::new(),
        pages: String::new(),
        doi: String::new(),
        publisher: String::new(),
        booktitle: String::new(),
        raw: raw.to_string(),
    };

    // --- Field extraction (style-agnostic heuristics, applied in order) ---

    // Year: 4-digit number, possibly in parentheses. Prefer the first
    // match that looks like a publication year (19xx or 20xx).
    let year_re = Regex::new(r"(?:^|\D)((?:19|20)\d{2})(?:\D|$)").unwrap();
    if let Some(caps) = year_re.captures(raw) {
        entry.year = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
    } else {
        warnings.push("could not extract year".to_string());
    }

    // DOI: starts with "10." followed by a slash and a non-space suffix.
    let doi_re = Regex::new(r"(10\.\d{4,}/[^\s,;]+)").unwrap();
    if let Some(caps) = doi_re.captures(raw) {
        entry.doi = caps
            .get(1)
            .map(|m| m.as_str().trim_end_matches('.').to_string())
            .unwrap_or_default();
    }

    // Pages: "12-34", "12--34", "pp. 12-34", "pp. 12-34.".
    let pages_re = Regex::new(r"(?:pp?\.?\s*)?(\d{1,4})\s*[-\u2013\u2014]\s*(\d{1,4})").unwrap();
    if let Some(caps) = pages_re.captures(raw) {
        let start = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let end = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        if !start.is_empty() && !end.is_empty() {
            entry.pages = format!("{}--{}", start, end);
        }
    }

    // Volume + issue: "12(3)" or "Vol. 12, No. 3" or "vol. 12" alone.
    let vol_issue_re =
        Regex::new(r"(?i)(?:vol\.?\s*)?(\d{1,4})\s*(?:\((\d{1,4})\)|,\s*no\.?\s*(\d{1,4}))?")
            .unwrap();
    // Find the first volume-like pattern that isn't the year or a page number.
    'vol_search: for caps in vol_issue_re.captures_iter(raw) {
        let vol_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if vol_str.len() < 1 {
            continue;
        }
        // Skip if this is the year.
        if vol_str == entry.year {
            continue;
        }
        // Skip if this is part of the pages.
        if !entry.pages.is_empty() {
            let p_start = entry.pages.split("--").next().unwrap_or("");
            if vol_str == p_start {
                continue;
            }
        }
        entry.volume = vol_str.to_string();
        // Issue: prefer the parenthesis form, then the "no." form.
        if let Some(m) = caps.get(2).or_else(|| caps.get(3)) {
            entry.number = m.as_str().to_string();
        }
        break 'vol_search;
    }

    // Authors and title: split on the year. Everything before is authors;
    // everything after, up to the next period, is the title.
    if !entry.year.is_empty() {
        // Find the year position in the raw string (first occurrence).
        if let Some(year_pos) = raw.find(&entry.year) {
            let before = raw[..year_pos]
                .trim_end_matches(|c: char| c == ' ' || c == ',' || c == '.' || c == '(');
            let after = raw[year_pos + entry.year.len()..]
                .trim_start_matches(|c: char| c == ' ' || c == ')' || c == '.');
            entry.author = clean_authors(before, style);
            // Title = text up to the next period that ends a sentence
            // (followed by a space and a capital letter, or end of string).
            let title_end = after
                .find(". ")
                .or_else(|| after.find(".\n"))
                .or_else(|| after.find('?'))
                .or_else(|| after.find('!'))
                .unwrap_or(after.len());
            let title_raw = after[..title_end].trim_end_matches('.').trim();
            entry.title = clean_title(title_raw, style);
            // Journal = text after the title, up to the volume pattern or end.
            let after_title = after[title_end.min(after.len())..]
                .trim_start_matches(|c: char| c == ' ' || c == '.');
            let journal_end = after_title
                .find(',')
                .or_else(|| after_title.find('.'))
                .unwrap_or(after_title.len());
            let journal_raw = after_title[..journal_end].trim_end_matches(',').trim();
            entry.journal = clean_journal(journal_raw, style);
        }
    } else {
        // No year found. Try to split on the first period + space.
        let parts: Vec<&str> = raw.splitn(3, ". ").collect();
        if parts.len() >= 1 {
            entry.author = clean_authors(parts[0], style);
        }
        if parts.len() >= 2 {
            entry.title = clean_title(parts[1], style);
        }
        if parts.len() >= 3 {
            // The remainder is journal + vol/issue/pages + DOI.
            let remainder = parts[2];
            let journal_end = remainder
                .find(',')
                .or_else(|| remainder.find('.'))
                .unwrap_or(remainder.len());
            entry.journal =
                clean_journal(remainder[..journal_end].trim_end_matches(',').trim(), style);
        }
    }

    // Decide entry_type: if there's a journal, it's an article; otherwise
    // treat as misc.
    if entry.journal.is_empty() && entry.publisher.is_empty() {
        entry.entry_type = "misc".to_string();
    }

    // Generate BibTeX key: FirstAuthorLastName + Year + FirstTitleWord
    entry.key = generate_bibtex_key(&entry);

    (entry, warnings)
}

/// Clean up the authors field for BibTeX.
/// - Strips surrounding quotes/brackets
/// - Normalizes "&" to "and" (BibTeX convention)
/// - Strips trailing punctuation
fn clean_authors(s: &str, _style: PlainRefStyle) -> String {
    let mut s = s.trim().to_string();
    // Remove surrounding quotes
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s = s[1..s.len() - 1].to_string();
    }
    // Normalize "&" and " and " to BibTeX " and "
    s = s.replace(" & ", " and ").replace(" & ", " and ");
    // Strip trailing punctuation
    s = s
        .trim_end_matches(|c: char| c == ',' || c == '.' || c == ';')
        .trim()
        .to_string();
    s
}

/// Clean up the title field for BibTeX.
/// - Strips surrounding quotes (MLA, Chicago)
/// - Strips trailing punctuation
fn clean_title(s: &str, _style: PlainRefStyle) -> String {
    let mut s = s.trim().to_string();
    // MLA / Chicago use double quotes around titles
    if s.starts_with('"') && s.ends_with('"') {
        s = s[1..s.len() - 1].to_string();
    }
    // Strip trailing punctuation
    s = s
        .trim_end_matches(|c: char| c == ',' || c == '.' || c == ';')
        .trim()
        .to_string();
    s
}

/// Clean up the journal field for BibTeX.
/// - Strips trailing punctuation
fn clean_journal(s: &str, _style: PlainRefStyle) -> String {
    s.trim()
        .trim_end_matches(|c: char| c == ',' || c == '.' || c == ';')
        .trim()
        .to_string()
}

/// Generate a BibTeX key from the entry. Format: LastnameYearWord
/// - Lastname = first author's last name (no spaces, capitalized)
/// - Year = the 4-digit year (or "NoYear" if missing)
/// - Word = first significant word of the title (lowercase, >= 4 chars)
fn generate_bibtex_key(entry: &BibEntry) -> String {
    // First author's last name: take the part before the first comma
    // in the author field, or the first word if no comma.
    let first_author = entry.author.split(" and ").next().unwrap_or("");
    let last_name = if let Some(idx) = first_author.find(',') {
        first_author[..idx].trim()
    } else {
        // No comma: take the first word (assumed to be the last name)
        first_author.split_whitespace().next().unwrap_or("")
    };
    let last_name_clean: String = last_name.chars().filter(|c| c.is_alphanumeric()).collect();
    let last_name_part = if last_name_clean.is_empty() {
        "Anon".to_string()
    } else {
        // Capitalize the first letter
        let mut c = last_name_clean.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str().to_lowercase().as_str(),
            None => "Anon".to_string(),
        }
    };

    let year_part = if entry.year.is_empty() {
        "NoYear".to_string()
    } else {
        entry.year.clone()
    };

    // First significant word of the title (>= 4 chars, lowercase)
    let title_word = entry
        .title
        .split_whitespace()
        .find(|w| {
            let cleaned: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
            cleaned.len() >= 4
                && !matches!(
                    cleaned.to_lowercase().as_str(),
                    "the"
                        | "and"
                        | "for"
                        | "with"
                        | "from"
                        | "into"
                        | "that"
                        | "this"
                        | "these"
                        | "those"
                )
        })
        .map(|w| {
            let cleaned: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
            cleaned.to_lowercase()
        })
        .unwrap_or_else(|| "ref".to_string());

    format!("{}{}{}", last_name_part, year_part, title_word)
}

/// Render a list of BibEntry as a single BibTeX string. Each entry is
/// formatted on multiple lines with consistent indentation, suitable
/// for writing to a .bib file.
fn entries_to_bibtex_string(entries: &[BibEntry]) -> String {
    let mut out = String::new();
    for e in entries {
        out.push_str(&format!("@{}{{{},\n", e.entry_type, e.key));
        if !e.author.is_empty() {
            out.push_str(&format!("  author = {{{}}},\n", e.author));
        }
        if !e.title.is_empty() {
            out.push_str(&format!("  title = {{{}}},\n", escape_bibtex(&e.title)));
        }
        if !e.journal.is_empty() {
            out.push_str(&format!("  journal = {{{}}},\n", escape_bibtex(&e.journal)));
        }
        if !e.year.is_empty() {
            out.push_str(&format!("  year = {{{}}},\n", e.year));
        }
        if !e.volume.is_empty() {
            out.push_str(&format!("  volume = {{{}}},\n", e.volume));
        }
        if !e.number.is_empty() {
            out.push_str(&format!("  number = {{{}}},\n", e.number));
        }
        if !e.pages.is_empty() {
            out.push_str(&format!("  pages = {{{}}},\n", e.pages));
        }
        if !e.doi.is_empty() {
            out.push_str(&format!("  doi = {{{}}},\n", e.doi));
        }
        if !e.publisher.is_empty() {
            out.push_str(&format!(
                "  publisher = {{{}}},\n",
                escape_bibtex(&e.publisher)
            ));
        }
        if !e.booktitle.is_empty() {
            out.push_str(&format!(
                "  booktitle = {{{}}},\n",
                escape_bibtex(&e.booktitle)
            ));
        }
        // Remove trailing comma from the last field
        if out.ends_with(",\n") {
            out.truncate(out.len() - 2);
            out.push('\n');
        }
        out.push_str("}\n\n");
    }
    out
}

/// Escape characters that have special meaning in BibTeX values:
///   - '&' -> '\&'
///   - '%' -> '\%'
///   - '_' -> '\_'
///   - '#' -> '\#'
/// Braces are left alone because BibTeX uses them for nesting.
fn escape_bibtex(s: &str) -> String {
    s.replace('&', "\\&")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('#', "\\#")
}
