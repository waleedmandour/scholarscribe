//! Plain-text references to BibTeX converter.
//!
//! Many researchers keep their reference list in a Word document or a
//! plain-text file formatted in their target venue's citation style
//! (APA, MLA, Chicago, Vancouver, AMA, IEEE, etc.) rather than in a
//! BibTeX file. This module parses pasted plain-text reference lists and
//! converts each entry to a BibTeX entry that can be validated against
//! the manuscript, downloaded as a .bib file, or both.
//!
//! The parser is heuristic and tolerant of common format variations.
//! It does not require perfectly formatted input. When a field
//! cannot be extracted, it is left empty in the BibTeX entry and a
//! warning is recorded so the user can fix the entry manually.
//!
//! v2.2.1 r7: added AMA (American Medical Association) and IEEE
//! (Institute of Electrical and Electronics Engineers) numeric styles,
//! with full style-specific heuristics. Both are common in medical and
//! engineering journals respectively and were previously lumped into
//! the generic Plain/auto-detect path, which does not split IEEE's
//! bracketed [1] prefix or quoted title correctly without the new
//! dedicated parsers.

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
    /// AMA (American Medical Association, used by JAMA and many medical
    /// journals). Very close to Vancouver but with subtle differences
    /// (e.g., semicolons between authors, "et al." rules at 6+ authors).
    /// Format: "Author AA, Author BB. Title. Journal. Year;Vol(Issue):Pages."
    /// v2.2.1 r7.
    Ama,
    /// IEEE (Institute of Electrical and Electronics Engineers, used by
    /// all IEEE journals and conferences). Numbered with a bracketed [N]
    /// prefix, initials before last name, quoted title, and labeled
    /// vol./no./pp. fields.
    /// Format: "[1] A. Author, B. Coauthor, "Title," Journal, vol. X,
    /// no. Y, pp. A-B, Year." v2.2.1 r7.
    Ieee,
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
            "ama" => Some(Self::Ama),
            "ieee" => Some(Self::Ieee),
            "plain" | "auto" => Some(Self::Plain),
            _ => None,
        }
    }

    /// Human-readable style label. Surfaced in the UI dropdown (v2.2.1 r7
    /// onwards; previously only documented the enum for future work).
    pub fn label(self) -> &'static str {
        match self {
            Self::Apa => "APA 7th",
            Self::Mla => "MLA 9th",
            Self::Chicago => "Chicago 17th (notes-bibliography)",
            Self::Vancouver => "Vancouver (medical)",
            Self::Ama => "AMA (American Medical Association)",
            Self::Ieee => "IEEE (engineering)",
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
    // v2.2.1 r7: dedicated AMA and IEEE parsers handle the style-specific
    // patterns the generic parser can't (AMA's Year;Vol(Issue):Pages
    // signature and IEEE's quoted-title + labeled vol./no./pp. fields).
    // For Plain (auto-detect), sniff the reference's signature and
    // dispatch to the right parser; this is per-reference so a mixed-
    // format list still parses correctly.
    match style {
        PlainRefStyle::Ama => return parse_ama(raw),
        PlainRefStyle::Ieee => return parse_ieee(raw),
        PlainRefStyle::Plain => {
            if let Some(sniffed) = sniff_style(raw) {
                return match sniffed {
                    PlainRefStyle::Ieee => parse_ieee(raw),
                    PlainRefStyle::Ama => parse_ama(raw),
                    _ => parse_default(raw, style),
                };
            }
        }
        _ => {}
    }
    parse_default(raw, style)
}

/// Sniff a single reference's signature to guess its style. Returns
/// `Some(Ieee)` when the reference has IEEE's labeled vol./no./pp. fields
/// (or a quoted title); `Some(Ama)` when it has the Year;Vol(Issue):Pages
/// pattern; `None` otherwise (caller falls back to the default parser).
/// Used by Plain (auto-detect) mode.
fn sniff_style(raw: &str) -> Option<PlainRefStyle> {
    // IEEE: labeled fields vol./no./pp. (at least two of the three).
    let ieee_labels = ["vol.", "no.", "pp."]
        .iter()
        .filter(|l| raw.to_lowercase().contains(*l))
        .count();
    if ieee_labels >= 2 || raw.contains('"') {
        return Some(PlainRefStyle::Ieee);
    }
    // AMA: Year;Vol(Issue):Pages pattern.
    let ama_re = Regex::new(r"(?:19|20)\d{2};\d{1,4}(?:\(\d{1,4}\))?:\d{1,4}").unwrap();
    if ama_re.is_match(raw) {
        return Some(PlainRefStyle::Ama);
    }
    None
}

/// The default, style-agnostic parser. This is the original `parse_single`
/// body, kept intact so APA/MLA/Chicago/Vancouver references continue to
/// parse exactly as before. AMA and IEEE references that don't match their
/// dedicated parsers' patterns also fall through to this.
fn parse_default(raw: &str, style: PlainRefStyle) -> (BibEntry, Vec<String>) {
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
        if vol_str.is_empty() {
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
            let before = raw[..year_pos].trim_end_matches([' ', ',', '.', '(']);
            let after = raw[year_pos + entry.year.len()..].trim_start_matches([' ', ')', '.']);
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
            let after_title = after[title_end.min(after.len())..].trim_start_matches([' ', '.']);
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
        if !parts.is_empty() {
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

/// Parse a single AMA (American Medical Association) reference into a
/// BibEntry. AMA format:
///   "Author AA, Author BB. Title. Journal. Year;Vol(Issue):Pages."
/// Example:
///   "Smith J, Jones K, Brown L. A great paper on medicine. Journal of
///    Medical Things. 2020;15(3):45-67."
///
/// AMA-specific notes (v2.2.1 r7):
/// - Authors are comma-separated, no "&" or "and" in the source. The
///   parser joins them with BibTeX's " and " separator.
/// - Authors use "Lastname II" format (no comma between last name and
///   initials). The BibTeX key generator extracts the first word as
///   the last name, which works for AMA.
/// - The 6+ authors "et al." truncation rule is NOT applied here;
///   the full list is preserved and the BibTeX renderer handles
///   truncation downstream.
/// - The Year;Vol(Issue):Pages pattern at the end is the AMA signature
///   and is extracted with a dedicated regex.
fn parse_ama(raw: &str) -> (BibEntry, Vec<String>) {
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

    // DOI (same as default).
    let doi_re = Regex::new(r"(10\.\d{4,}/[^\s,;]+)").unwrap();
    if let Some(caps) = doi_re.captures(raw) {
        entry.doi = caps
            .get(1)
            .map(|m| m.as_str().trim_end_matches('.').to_string())
            .unwrap_or_default();
    }

    // AMA signature: "Year;Vol(Issue):Pages" (with optional issue in parens
    // and optional page range). Examples: "2020;15(3):45-67", "2020;15:45-67",
    // "2020;15(3):45-67." (with trailing period).
    let yvp_re = Regex::new(
        r"((?:19|20)\d{2});(\d{1,4})(?:\((\d{1,4})\))?:(\d{1,4})\s*[-\u2013\u2014]\s*(\d{1,4})",
    )
    .unwrap();
    if let Some(caps) = yvp_re.captures(raw) {
        entry.year = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        entry.volume = caps
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        if let Some(m) = caps.get(3) {
            entry.number = m.as_str().to_string();
        }
        let p_start = caps.get(4).map(|m| m.as_str()).unwrap_or("");
        let p_end = caps.get(5).map(|m| m.as_str()).unwrap_or("");
        if !p_start.is_empty() && !p_end.is_empty() {
            entry.pages = format!("{p_start}--{p_end}");
        }
    } else {
        // Fall back to the generic year + pages + volume extractors.
        let year_re = Regex::new(r"(?:^|\D)((?:19|20)\d{2})(?:\D|$)").unwrap();
        if let Some(caps) = year_re.captures(raw) {
            entry.year = caps
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
        } else {
            warnings.push("could not extract year".to_string());
        }
        let pages_re =
            Regex::new(r"(?:pp?\.?\s*)?(\d{1,4})\s*[-\u2013\u2014]\s*(\d{1,4})").unwrap();
        if let Some(caps) = pages_re.captures(raw) {
            let start = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let end = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if !start.is_empty() && !end.is_empty() {
                entry.pages = format!("{start}--{end}");
            }
        }
        let vol_re =
            Regex::new(r"(?i)(?:vol\.?\s*)?(\d{1,4})(?:\s*\((\d{1,4})\))?").unwrap();
        for caps in vol_re.captures_iter(raw) {
            let vol_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if vol_str.is_empty() || vol_str == entry.year {
                continue;
            }
            if !entry.pages.is_empty() {
                let p_start = entry.pages.split("--").next().unwrap_or("");
                if vol_str == p_start {
                    continue;
                }
            }
            entry.volume = vol_str.to_string();
            if let Some(m) = caps.get(2) {
                entry.number = m.as_str().to_string();
            }
            break;
        }
    }

    // Split authors / title / journal on the AMA ". " separator. The
    // segments before the Year;Vol... pattern are: authors, title, journal.
    let split_point = if !entry.year.is_empty() {
        raw.find(&entry.year).unwrap_or(raw.len())
    } else {
        raw.len()
    };
    let before_year = raw[..split_point].trim_end().trim_end_matches('.').trim();
    let segments: Vec<&str> = before_year.split(". ").collect();

    if !segments.is_empty() {
        // Authors: comma-separated, no "&" or "and" in source. Join with " and ".
        let authors_raw = segments[0].trim();
        let authors_bib = authors_raw
            .split(',')
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
            .collect::<Vec<_>>()
            .join(" and ");
        entry.author = authors_bib;
    }
    if segments.len() >= 2 {
        entry.title = segments[1].trim().trim_end_matches('.').trim().to_string();
    }
    if segments.len() >= 3 {
        // The journal may itself contain ". " (e.g., "J. Med. Things"), so
        // rejoin everything after segment 2.
        let journal_raw = segments[2..].join(". ");
        entry.journal = journal_raw.trim_end_matches([',', '.']).trim().to_string();
    }

    if entry.journal.is_empty() && entry.publisher.is_empty() {
        entry.entry_type = "misc".to_string();
    }
    entry.key = generate_bibtex_key(&entry);
    (entry, warnings)
}

/// Parse a single IEEE (Institute of Electrical and Electronics Engineers)
/// reference into a BibEntry. IEEE format (after the bracketed [N] prefix
/// is stripped by `split_references`):
///   `A. Author, B. Coauthor, "Title," Journal, vol. X, no. Y, pp. A-B, Year.`
/// Example:
///   `J. Smith, K. Jones, "A great paper on circuits," IEEE Transactions
///    on Circuits, vol. 15, no. 3, pp. 45-67, 2020.`
///
/// IEEE-specific notes (v2.2.1 r7):
/// - Authors in "A. Author" format (initials before last name). The
///   parser converts them to BibTeX "Lastname, I." format so the key
///   generator extracts the actual last name, not an initial.
/// - Title is in double quotes, ending with a comma inside or outside
///   the closing quote.
/// - vol./no./pp. labels are explicit and labeled with periods.
/// - Year is at the end, sometimes followed by a period.
fn parse_ieee(raw: &str) -> (BibEntry, Vec<String>) {
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

    // DOI.
    let doi_re = Regex::new(r"(10\.\d{4,}/[^\s,;]+)").unwrap();
    if let Some(caps) = doi_re.captures(raw) {
        entry.doi = caps
            .get(1)
            .map(|m| m.as_str().trim_end_matches('.').to_string())
            .unwrap_or_default();
    }

    // Title: text inside double quotes. IEEE titles end with a comma
    // (inside or outside the closing quote).
    let title_re = Regex::new(r#""([^"]+)"\s*,?"#).unwrap();
    // Position of the opening quote (= where authors end and title begins).
    // Using match.start() rather than match.end() so `before_title` does
    // NOT include the title text itself.
    let title_start_pos: usize;
    // Position right after the full match (= where the journal begins).
    let after_title_start: usize;
    if let Some(caps) = title_re.captures(raw) {
        // Strip trailing comma/period — IEEE puts the comma INSIDE the
        // quotes ("Title,") so the captured group includes it.
        entry.title = caps
            .get(1)
            .map(|m| m.as_str().trim_end_matches([',', '.']).trim().to_string())
            .unwrap_or_default();
        title_start_pos = caps.get(0).map(|m| m.start()).unwrap_or(0);
        after_title_start = caps.get(0).map(|m| m.end()).unwrap_or(0);
    } else {
        title_start_pos = 0;
        after_title_start = 0;
        warnings.push(
            "could not find quoted title (IEEE style expects \"Title,\")".to_string(),
        );
    }

    // Authors = text before the title's opening quote. Convert "A. Author, B. Coauthor"
    // to BibTeX "Author, A. and Coauthor, B." so the key generator picks
    // up the actual last name.
    if title_start_pos > 0 {
        let before_title = raw[..title_start_pos].trim_end_matches([',', ' ']).trim();
        entry.author = convert_ieee_authors_to_bibtex(before_title);
    }

    // After the title: "Journal, vol. X, no. Y, pp. A-B, Year."
    let after_title = if after_title_start > 0 {
        raw[after_title_start..].trim()
    } else {
        raw
    };

    // Volume: "vol. X" (case-insensitive).
    let vol_re = Regex::new(r"(?i)vol\.?\s*(\d{1,4})").unwrap();
    if let Some(caps) = vol_re.captures(after_title) {
        entry.volume = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
    }

    // Issue: "no. Y".
    let no_re = Regex::new(r"(?i)no\.?\s*(\d{1,4})").unwrap();
    if let Some(caps) = no_re.captures(after_title) {
        entry.number = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
    }

    // Pages: "pp. A-B".
    let pp_re =
        Regex::new(r"(?i)pp?\.?\s*(\d{1,4})\s*[-\u2013\u2014]\s*(\d{1,4})").unwrap();
    if let Some(caps) = pp_re.captures(after_title) {
        let start = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let end = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        if !start.is_empty() && !end.is_empty() {
            entry.pages = format!("{start}--{end}");
        }
    }

    // Year: any 4-digit 19xx or 20xx, typically at the end.
    let year_re = Regex::new(r"((?:19|20)\d{2})").unwrap();
    if let Some(caps) = year_re.captures(after_title) {
        entry.year = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
    } else {
        warnings.push("could not extract year".to_string());
    }

    // Journal: text after the title up to the first labeled field
    // (vol./no./pp.) — whichever comes first. This is more reliable than
    // the default parser's "split on comma" approach because IEEE
    // journal names can contain commas.
    let label_re = Regex::new(r"(?i)\b(?:vol|no|pp?)\.").unwrap();
    let journal_end = label_re
        .find(after_title)
        .map(|m| m.start())
        .unwrap_or(after_title.len());
    // Trim whitespace first (the journal usually ends with ", vol. ..." so
    // there's a trailing ", " before the label), then strip trailing
    // punctuation, then trim again.
    let journal_raw = after_title[..journal_end]
        .trim_end()
        .trim_end_matches([',', '.'])
        .trim();
    entry.journal = journal_raw.to_string();

    if entry.journal.is_empty() && entry.publisher.is_empty() {
        entry.entry_type = "misc".to_string();
    }
    entry.key = generate_bibtex_key(&entry);
    (entry, warnings)
}

/// Convert IEEE author format ("A. Author, B. Coauthor") to BibTeX format
/// ("Author, A. and Coauthor, B."). IEEE puts initials before the last
/// name; BibTeX puts the last name first, then initials. The conversion
/// is needed so `generate_bibtex_key` extracts the actual last name as
/// the key prefix, not an initial.
///
/// Handles multi-initial tokens like "A.B." (concatenated initials) and
/// "A. B." (space-separated initials) — both produce BibTeX "A. B.".
fn convert_ieee_authors_to_bibtex(s: &str) -> String {
    let s = s.trim().trim_end_matches('.');
    let authors: Vec<&str> = s
        .split(',')
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .collect();
    let mut bib_authors: Vec<String> = Vec::new();
    for author in &authors {
        let words: Vec<&str> = author.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        // Initials: one or more tokens at the start that consist of 1-4
        // uppercase letters, optionally with periods between them.
        // Examples: "A.", "A.B.", "A.B.C.", "AB." (concatenated) and
        // "A.", "B.", "C." (separate tokens). A token like "Smith" or
        // "JOHN" (no period) is NOT an initial — the period is required
        // to disambiguate from all-caps surnames.
        let mut initials_end = 0;
        for w in &words {
            let cleaned: String = w.chars().filter(|c| c.is_alphabetic()).collect();
            if cleaned.is_empty()
                || cleaned.len() > 4
                || !cleaned.chars().all(|c| c.is_uppercase())
                || !w.contains('.')
            {
                break;
            }
            initials_end += 1;
        }
        if initials_end == 0 {
            // No initials detected; assume the whole token is a last name
            // (rare for IEEE but defensive).
            bib_authors.push((*author).to_string());
        } else if initials_end < words.len() {
            let last_name = words[initials_end..].join(" ");
            // Each initial token's letters are split into separate "X."
            // entries, joined with spaces. "A.B." -> "A. B."; "A." -> "A.";
            // "A. B." -> "A. B." (preserved).
            let initials: Vec<String> = words[..initials_end]
                .iter()
                .map(|w| {
                    let cleaned: String = w.chars().filter(|x| x.is_alphabetic()).collect();
                    cleaned
                        .chars()
                        .map(|c| format!("{c}."))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect();
            bib_authors.push(format!("{last_name}, {}", initials.join(" ")));
        } else {
            // All initials (unusual); pass through unchanged.
            bib_authors.push((*author).to_string());
        }
    }
    bib_authors.join(" and ")
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
    s = s.trim_end_matches([',', '.', ';']).trim().to_string();
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
    s = s.trim_end_matches([',', '.', ';']).trim().to_string();
    s
}

/// Clean up the journal field for BibTeX.
/// - Strips trailing punctuation
fn clean_journal(s: &str, _style: PlainRefStyle) -> String {
    s.trim()
        .trim_end_matches([',', '.', ';'])
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
///   - `&` becomes `\&`
///   - `%` becomes `\%`
///   - `_` becomes `\_`
///   - `#` becomes `\#`
///
/// Braces are left alone because BibTeX uses them for nesting.
fn escape_bibtex(s: &str) -> String {
    s.replace('&', "\\&")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('#', "\\#")
}

// ---------------------------------------------------------------------------
// Tests (v2.2.1 r7)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a representative AMA reference list (3 entries).
    /// Real AMA manuscripts number their references (1., 2., 3.) like
    /// IEEE's bracketed form, so split_references strips the prefix
    /// before parse_single is called.
    fn ama_sample() -> String {
        // AMA: "1. Author AA, Author BB. Title. Journal. Year;Vol(Issue):Pages."
        [
            "1. Smith J, Jones K, Brown L. A great paper on medicine. Journal of Medical Things. 2020;15(3):45-67.",
            "2. Lee S, Park H. Neural correlates of memory. Brain Research. 2021;200:1-12.",
            "3. Garcia M, Kim D. Another AMA reference with a longer title. New England Journal of Medicine. 2019;381(7):624-631.",
        ]
        .join("\n")
    }

    /// Build a representative IEEE reference list (3 entries with [N] prefix).
    fn ieee_sample() -> String {
        // IEEE: "[1] A. Author, B. Coauthor, \"Title,\" Journal, vol. X, no. Y, pp. A-B, Year."
        // The bracketed [N] prefix is stripped by split_references before parse_single.
        [
            "[1] J. Smith, K. Jones, \"A great paper on circuits,\" IEEE Transactions on Circuits, vol. 15, no. 3, pp. 45-67, 2020.",
            "[2] S. Lee, H. Park, \"Neural correlates of memory,\" Brain Research, vol. 200, pp. 1-12, 2021.",
            "[3] M. Garcia, D. Kim, \"A third IEEE reference,\" Proceedings of the IEEE, vol. 109, no. 11, pp. 2000-2015, 2022.",
        ]
        .join("\n")
    }

    #[test]
    fn parses_ama_references_with_year_vol_issue_pages_pattern() {
        let result = convert_plain_to_bib(&ama_sample(), PlainRefStyle::Ama);
        assert_eq!(
            result.entry_count, 3,
            "expected 3 AMA entries, got {}: {:?}",
            result.entry_count, result.entries
        );
        let e = &result.entries[0];
        // The Year;Vol(Issue):Pages signature should be parsed cleanly.
        assert_eq!(e.year, "2020", "year mismatch: {}", e.year);
        assert_eq!(e.volume, "15", "volume mismatch: {}", e.volume);
        assert_eq!(e.number, "3", "issue mismatch: {}", e.number);
        assert_eq!(e.pages, "45--67", "pages mismatch: {}", e.pages);
        // Authors should be "Smith J and Jones K and Brown L" (AMA's
        // comma-separated list joined with BibTeX's " and ").
        assert!(
            e.author.contains("Smith") && e.author.contains("Jones") && e.author.contains("Brown"),
            "author mismatch: {}", e.author
        );
        // Title should be the text between the first and second ". ".
        assert!(
            e.title.contains("great paper on medicine"),
            "title mismatch: {}", e.title
        );
        // Journal should be the text between the second and third ". ",
        // i.e., "Journal of Medical Things".
        assert!(
            e.journal.contains("Journal of Medical Things"),
            "journal mismatch: {}", e.journal
        );
        // BibTeX key should start with the first author's last name + year.
        assert!(
            e.key.starts_with("Smith2020"),
            "key should start with 'Smith2020...', got: {}", e.key
        );
    }

    #[test]
    fn parses_ieee_references_with_quoted_title_and_labeled_fields() {
        let result = convert_plain_to_bib(&ieee_sample(), PlainRefStyle::Ieee);
        assert_eq!(
            result.entry_count, 3,
            "expected 3 IEEE entries, got {}: {:?}",
            result.entry_count, result.entries
        );
        let e = &result.entries[0];
        // Authors should be converted from "A. Author" to "Author, A." format
        // so the key generator extracts the actual last name.
        assert!(
            e.author.contains("Smith, J.") && e.author.contains("Jones, K."),
            "author mismatch (expected 'Smith, J. and Jones, K.'): {}",
            e.author
        );
        // Title should be the text inside the double quotes.
        assert!(
            e.title.contains("A great paper on circuits"),
            "title mismatch: {}", e.title
        );
        // Journal: text after the title up to the first vol./no./pp. label.
        assert!(
            e.journal.contains("IEEE Transactions on Circuits"),
            "journal mismatch: {}", e.journal
        );
        // Labeled fields.
        assert_eq!(e.volume, "15", "volume mismatch: {}", e.volume);
        assert_eq!(e.number, "3", "issue mismatch: {}", e.number);
        assert_eq!(e.pages, "45--67", "pages mismatch: {}", e.pages);
        assert_eq!(e.year, "2020", "year mismatch: {}", e.year);
        // BibTeX key should start with the first author's actual last name
        // (not the initial). This is the regression for the bug where the
        // generic parser would pick "J." as the key prefix.
        assert!(
            e.key.starts_with("Smith2020"),
            "key should start with 'Smith2020...', got: {}", e.key
        );
    }

    #[test]
    fn ieee_author_conversion_handles_multi_initials() {
        // "A.B. Lastname" should become "Lastname, A. B." in BibTeX.
        let converted = convert_ieee_authors_to_bibtex("A.B. Smith, C. Jones");
        assert!(
            converted.contains("Smith, A. B.") && converted.contains("Jones, C."),
            "multi-initial conversion mismatch: {}", converted
        );
    }

    #[test]
    fn plain_auto_detect_sniffs_ama_and_ieee_per_reference() {
        // A mixed-format list: first reference is AMA, second is IEEE.
        // Plain (auto-detect) should sniff each per-reference and dispatch
        // to the right parser.
        let mixed = format!("{}\n{}", ama_sample(), ieee_sample());
        let result = convert_plain_to_bib(&mixed, PlainRefStyle::Plain);
        // Should get 6 entries total (3 AMA + 3 IEEE).
        assert_eq!(
            result.entry_count, 6,
            "expected 6 entries from mixed-format list, got {}: {:?}",
            result.entry_count, result.entries
        );
        // The first entry (AMA) should have the Year;Vol(Issue):Pages
        // pattern parsed correctly.
        let ama_e = &result.entries[0];
        assert_eq!(ama_e.year, "2020", "AMA sniff failed: year");
        assert_eq!(ama_e.volume, "15", "AMA sniff failed: volume");
        assert_eq!(ama_e.pages, "45--67", "AMA sniff failed: pages");
        // The fourth entry (IEEE, first in ieee_sample) should have the
        // quoted title and labeled fields parsed correctly.
        let ieee_e = &result.entries[3];
        assert!(
            ieee_e.title.contains("A great paper on circuits"),
            "IEEE sniff failed: title: {}", ieee_e.title
        );
        assert_eq!(ieee_e.volume, "15", "IEEE sniff failed: volume");
        assert_eq!(ieee_e.number, "3", "IEEE sniff failed: number");
        assert_eq!(ieee_e.pages, "45--67", "IEEE sniff failed: pages");
        assert_eq!(ieee_e.year, "2020", "IEEE sniff failed: year");
    }

    #[test]
    fn ama_without_year_vol_pages_signature_falls_back_gracefully() {
        // An AMA reference that doesn't have the Year;Vol(Issue):Pages
        // signature (e.g., a book chapter) should still parse without
        // panicking — just with empty fields where the signature was
        // expected.
        let result = convert_plain_to_bib(
            "Smith J. A book chapter title. In: Book Title. Publisher; 2018:45-60.",
            PlainRefStyle::Ama,
        );
        assert_eq!(result.entry_count, 1);
        assert_eq!(result.entries[0].year, "2018");
        // The book title "Book Title" should appear somewhere (it might
        // end up in journal or publisher depending on parsing).
        let combined = format!(
            "{} | {} | {}",
            result.entries[0].journal, result.entries[0].publisher, result.entries[0].title
        );
        assert!(
            combined.contains("Book Title") || combined.contains("book chapter"),
            "expected 'Book Title' or 'book chapter' somewhere, got: {}",
            combined
        );
    }

    #[test]
    fn ieee_reference_without_quoted_title_records_warning() {
        // An IEEE reference that's missing the quoted title should record
        // a warning rather than panic.
        let result = convert_plain_to_bib(
            "J. Smith, K. Jones, IEEE Transactions, vol. 15, no. 3, pp. 45-67, 2020.",
            PlainRefStyle::Ieee,
        );
        assert_eq!(result.entry_count, 1);
        // Should still extract vol/no/pp/year correctly.
        assert_eq!(result.entries[0].volume, "15");
        assert_eq!(result.entries[0].number, "3");
        assert_eq!(result.entries[0].pages, "45--67");
        assert_eq!(result.entries[0].year, "2020");
        // Should record a warning about the missing quoted title.
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("could not find quoted title")),
            "expected a 'could not find quoted title' warning, got: {:?}",
            result.warnings
        );
    }
}
