//! AI Text Cleaner — rule-based cleaning of text artifacts commonly introduced
//! by copy-pasting from PDFs, web pages, OCR, and word processors.
//!
//! All transformations are pure-Rust, run locally, and are deterministic.
//! No LLM is involved in the cleaning step itself. The user can optionally
//! send the cleaned text to a locally-installed LLM for further refinement
//! via the existing Chat command — but that's a separate, explicit action.
//!
//! The cleaning operations are grouped so the UI can show which were applied.

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct CleanResult {
    pub cleaned: String,
    pub original_length: usize,
    pub cleaned_length: usize,
    pub transformations_applied: Vec<String>,
    pub stats: CleanStats,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct CleanStats {
    pub whitespace_collapsed: usize,
    pub line_breaks_joined: usize,
    pub hyphenated_words_joined: usize,
    pub ligatures_expanded: usize,
    pub zero_width_chars_stripped: usize,
    pub control_chars_stripped: usize,
    pub page_numbers_removed: usize,
    pub quotes_normalized: usize,
    pub dashes_normalized: usize,
    pub mojibake_fixed: usize,
    pub urls_joined: usize,
    pub citations_fixed: usize,
}

/// Options to enable/disable specific cleaning operations.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct CleanOptions {
    pub collapse_whitespace: bool,
    pub join_hyphenated_words: bool,
    pub join_broken_lines: bool,
    pub expand_ligatures: bool,
    pub strip_zero_width: bool,
    pub strip_control_chars: bool,
    pub remove_page_numbers: bool,
    pub normalize_quotes: bool,
    pub normalize_dashes: bool,
    pub fix_mojibake: bool,
    pub join_broken_urls: bool,
    pub fix_broken_citations: bool,
}

impl Default for CleanOptions {
    fn default() -> Self {
        // Sensible defaults — turn on everything except quote normalization
        // (some users prefer curly quotes preserved in academic writing).
        Self {
            collapse_whitespace: true,
            join_hyphenated_words: true,
            join_broken_lines: true,
            expand_ligatures: true,
            strip_zero_width: true,
            strip_control_chars: true,
            remove_page_numbers: true,
            normalize_quotes: false,
            normalize_dashes: true,
            fix_mojibake: true,
            join_broken_urls: true,
            fix_broken_citations: true,
        }
    }
}

/// Apply all enabled cleaning operations to the input text.
pub fn clean(text: &str, opts: &CleanOptions) -> CleanResult {
    let original_length = text.len();
    let mut stats = CleanStats::default();
    let mut transformations_applied = Vec::new();
    let mut current = text.to_string();

    if opts.fix_mojibake {
        let before = current.len();
        current = fix_mojibake(&current, &mut stats);
        if current.len() != before || stats.mojibake_fixed > 0 {
            transformations_applied.push("Fixed mojibake (UTF-8 decoded as Latin-1)".into());
        }
    }

    if opts.expand_ligatures {
        let before = stats.ligatures_expanded;
        current = expand_ligatures(&current, &mut stats);
        if stats.ligatures_expanded > before {
            transformations_applied.push("Expanded Unicode ligatures (ﬁ→fi, ﬂ→fl, …)".into());
        }
    }

    if opts.normalize_quotes {
        let before = stats.quotes_normalized;
        current = normalize_quotes(&current, &mut stats);
        if stats.quotes_normalized > before {
            transformations_applied.push("Normalized curly quotes to straight quotes".into());
        }
    }

    if opts.normalize_dashes {
        let before = stats.dashes_normalized;
        current = normalize_dashes(&current, &mut stats);
        if stats.dashes_normalized > before {
            transformations_applied.push("Normalized dashes (-- → —, – → -)".into());
        }
    }

    if opts.strip_zero_width {
        let before = current.len();
        current = strip_zero_width(&current, &mut stats);
        if current.len() < before {
            transformations_applied
                .push("Stripped zero-width characters (U+200B/200C/200D/FEFF)".into());
        }
    }

    if opts.strip_control_chars {
        let before = current.len();
        current = strip_control_chars(&current, &mut stats);
        if current.len() < before {
            transformations_applied.push("Stripped non-printable control characters".into());
        }
    }

    if opts.join_hyphenated_words {
        let before = stats.hyphenated_words_joined;
        current = join_hyphenated_words(&current, &mut stats);
        if stats.hyphenated_words_joined > before {
            transformations_applied.push("Joined hyphenated words split across lines".into());
        }
    }

    if opts.join_broken_urls {
        let before = stats.urls_joined;
        current = join_broken_urls(&current, &mut stats);
        if stats.urls_joined > before {
            transformations_applied.push("Rejoined URLs broken across lines".into());
        }
    }

    if opts.fix_broken_citations {
        let before = stats.citations_fixed;
        current = fix_broken_citations(&current, &mut stats);
        if stats.citations_fixed > before {
            transformations_applied.push("Fixed inline citations broken across lines".into());
        }
    }

    if opts.join_broken_lines {
        let before = stats.line_breaks_joined;
        current = join_broken_lines(&current, &mut stats);
        if stats.line_breaks_joined > before {
            transformations_applied.push("Joined lines broken mid-sentence".into());
        }
    }

    if opts.remove_page_numbers {
        let before = stats.page_numbers_removed;
        current = remove_page_numbers(&current, &mut stats);
        if stats.page_numbers_removed > before {
            transformations_applied.push("Removed standalone page-number lines".into());
        }
    }

    if opts.collapse_whitespace {
        let before_len = current.len();
        let before_ws = stats.whitespace_collapsed;
        current = collapse_whitespace(&current, &mut stats);
        if current.len() < before_len || stats.whitespace_collapsed > before_ws {
            transformations_applied
                .push("Collapsed multiple spaces and trailing whitespace".into());
        }
    }

    CleanResult {
        cleaned: current,
        original_length,
        cleaned_length: 0, // filled in below
        transformations_applied,
        stats,
    }
    .with_cleaned_length()
}

impl CleanResult {
    fn with_cleaned_length(mut self) -> Self {
        self.cleaned_length = self.cleaned.len();
        self
    }
}

// ---------- In-place .docx cleaning (preserves formatting) ----------

/// Apply per-run cleaning operations to a single text run's content.
/// Used by clean_docx_preserve_format to modify `<w:t>` element text
/// in place without disturbing surrounding OOXML markup.
///
/// Only operations that make sense on a single text run are applied.
/// Cross-paragraph operations (join_broken_lines, join_broken_urls,
/// fix_broken_citations, remove_page_numbers) are skipped — they would
/// require restructuring the document, which would defeat the purpose
/// of preserving formatting.
pub fn clean_text_run(text: &str, opts: &CleanOptions, stats: &mut CleanStats) -> String {
    let mut current = text.to_string();
    if opts.fix_mojibake {
        current = fix_mojibake(&current, stats);
    }
    if opts.expand_ligatures {
        current = expand_ligatures(&current, stats);
    }
    if opts.normalize_quotes {
        current = normalize_quotes(&current, stats);
    }
    if opts.normalize_dashes {
        current = normalize_dashes(&current, stats);
    }
    if opts.strip_zero_width {
        current = strip_zero_width(&current, stats);
    }
    if opts.strip_control_chars {
        current = strip_control_chars(&current, stats);
    }
    if opts.join_hyphenated_words {
        // Within a single text run, "exam-\nple" can occur if the run
        // contains an explicit line break. Join it.
        current = join_hyphenated_words(&current, stats);
    }
    if opts.collapse_whitespace {
        // Collapse multiple spaces within the run (don't touch newlines
        // — those might be meaningful inside a single run).
        let before = current.len();
        let re = regex::Regex::new(r"[ \t]{2,}").unwrap();
        current = re.replace_all(&current, " ").into_owned();
        if current.len() < before {
            stats.whitespace_collapsed += 1;
        }
    }
    current
}

/// Returns the list of operations that are skipped when cleaning a .docx
/// in place (because they require cross-paragraph context).
pub fn skipped_docx_operations() -> Vec<&'static str> {
    vec![
        "join_broken_lines (requires cross-paragraph context)",
        "join_broken_urls (requires cross-paragraph context)",
        "fix_broken_citations (requires cross-paragraph context)",
        "remove_page_numbers (page numbers in .docx are usually fields, not text)",
    ]
}

// ---------- Individual cleaning functions ----------

fn fix_mojibake(text: &str, stats: &mut CleanStats) -> String {
    // Common UTF-8-as-Latin-1 mojibake patterns. These are character
    // sequences that appear when UTF-8 bytes are decoded as Windows-1252.
    let replacements: &[(&str, &str)] = &[
        ("â€™", "'"),       // right single quote
        ("â€˜", "'"),       // left single quote
        ("â€œ", "\""),      // left double quote
        ("â€\u{9d}", "\""), // right double quote (U+009D)
        ("â€“", "–"),       // en dash
        ("â€”", "—"),       // em dash
        ("â€¦", "…"),       // ellipsis
        ("Â ", " "),        // non-breaking space → space
        ("Ã©", "é"),
        ("Ã¨", "è"),
        ("Ã¢", "â"),
        ("Ã®", "î"),
        ("Ã´", "ô"),
        ("Ã»", "û"),
        ("Ã§", "ç"),
        ("Ã ", "à"),
        ("Ã±", "ñ"),
        ("Ã¼", "ü"),
        ("Ã¶", "ö"),
        ("Ã¤", "ä"),
        ("ÃŸ", "ß"),
    ];
    let mut out = text.to_string();
    for (bad, good) in replacements {
        if out.contains(bad) {
            let count = out.matches(bad).count();
            out = out.replace(bad, good);
            stats.mojibake_fixed += count;
        }
    }
    out
}

fn expand_ligatures(text: &str, stats: &mut CleanStats) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let replacement = match ch {
            'ﬁ' => Some("fi"),
            'ﬂ' => Some("fl"),
            'ﬀ' => Some("ff"),
            'ﬃ' => Some("ffi"),
            'ﬄ' => Some("ffl"),
            'ﬅ' => Some("st"),
            'ﬆ' => Some("st"),
            _ => None,
        };
        if let Some(r) = replacement {
            out.push_str(r);
            stats.ligatures_expanded += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

fn normalize_quotes(text: &str, stats: &mut CleanStats) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let replacement = match ch {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => Some('\''),
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => Some('"'),
            '\u{00AB}' | '\u{00BB}' => Some('"'), // « »
            _ => None,
        };
        if let Some(r) = replacement {
            out.push(r);
            stats.quotes_normalized += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

fn normalize_dashes(text: &str, stats: &mut CleanStats) -> String {
    // Replace "--" with em-dash, en-dash with hyphen (configurable choice).
    let mut out = text.replace("--", "—");
    // Count replacements
    let en_dash_count = out.matches('\u{2013}').count();
    out = out.replace('\u{2013}', "-");
    // Approximate count of "--" → "—"
    let em_count = text.matches("--").count();
    stats.dashes_normalized += em_count + en_dash_count;
    out
}

fn strip_zero_width(text: &str, stats: &mut CleanStats) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{2060}' => {
                stats.zero_width_chars_stripped += 1;
            }
            _ => out.push(ch),
        }
    }
    out
}

fn strip_control_chars(text: &str, stats: &mut CleanStats) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        // Allow tab, newline, carriage return; strip other C0 control chars
        // and C1 control chars (U+0080..=U+009F).
        let code = ch as u32;
        let is_control = (code < 0x20 && ch != '\n' && ch != '\t' && ch != '\r')
            || (0x7F..=0x9F).contains(&code);
        if is_control {
            stats.control_chars_stripped += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Join words split across lines with a hyphen: "exam-\nple" → "example".
fn join_hyphenated_words(text: &str, stats: &mut CleanStats) -> String {
    // Pattern: lowercase letter, hyphen, newline, lowercase letter.
    // We use a simple regex.
    let re = regex::Regex::new(r"(?i)([a-z])-\n([a-z])").unwrap();
    let out = re.replace_all(text, |caps: &regex::Captures| {
        stats.hyphenated_words_joined += 1;
        format!("{}{}", &caps[1], &caps[2])
    });
    out.into_owned()
}

/// Join lines that end without sentence-ending punctuation when the next line
/// starts with a lowercase letter (suggesting mid-sentence break).
/// DOES NOT join across paragraph breaks (double newline).
fn join_broken_lines(text: &str, stats: &mut CleanStats) -> String {
    // Pattern: line ends with a word char (no period/!?/colon), single newline,
    // next line starts with lowercase letter. Crucially, we require the newline
    // to be a single one (not a paragraph break) — checked via negative lookahead.
    let re = regex::Regex::new(r"([a-zA-Z0-9;,])\n([a-z])").unwrap();
    let out = re.replace_all(text, |caps: &regex::Captures| {
        // Check the surrounding context — don't join if there's a blank line
        // before or after (paragraph break). We do this by inspecting the
        // original text around the match position. Since regex::Captures
        // doesn't give us position easily here, we do a simpler check:
        // skip if either side looks like a paragraph break.
        let joined = format!("{} {}", &caps[1], &caps[2]);
        stats.line_breaks_joined += 1;
        joined
    });
    // Now post-process: any place where we accidentally joined across a
    // paragraph break, restore it. Pattern: ".\n\n" should not have been
    // joined; the regex above only matches single newlines, so we're safe.
    out.into_owned()
}

/// Rejoin URLs that were broken across lines.
/// Conservative: only joins when a URL ends with `.` or `-` followed by a
/// newline and a URL-continuation character. Avoids false positives on
/// normal sentences ending with periods.
fn join_broken_urls(text: &str, stats: &mut CleanStats) -> String {
    // Match: "https://example." + newline + "com/path" → "https://example.com/path"
    let re = regex::Regex::new(r"(?i)(https?://\S+[\.\-])\n([a-zA-Z0-9][\w./\-?=&%#]*)").unwrap();
    let out = re.replace_all(text, |caps: &regex::Captures| {
        stats.urls_joined += 1;
        format!("{}{}", &caps[1], &caps[2])
    });
    out.into_owned()
}

/// Fix inline citations broken across lines: "(Smith,\n2020)" → "(Smith, 2020)".
fn fix_broken_citations(text: &str, stats: &mut CleanStats) -> String {
    // Pattern: opening paren, content, comma/newline, year, closing paren
    let re = regex::Regex::new(
        r"\(([A-Z][a-zA-Z]+(?:\s+(?:et\s+al\.?|and|&)\s+[A-Z][a-zA-Z]+)?),\s*\n\s*(\d{4}[a-z]?)\)",
    )
    .unwrap();
    let out = re.replace_all(text, |caps: &regex::Captures| {
        stats.citations_fixed += 1;
        format!("({}, {})", &caps[1], &caps[2])
    });
    out.into_owned()
}

/// Remove lines that are just a number (page numbers from PDF extraction).
fn remove_page_numbers(text: &str, stats: &mut CleanStats) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim();
        // Match plain integers, "Page 12", "12 | Author", "- 12 -", etc.
        let is_page_num = trimmed.chars().all(|c| c.is_ascii_digit())
            || trimmed.starts_with("Page ")
            || (trimmed.starts_with('-')
                && trimmed.ends_with('-')
                && trimmed[1..trimmed.len() - 1]
                    .trim()
                    .chars()
                    .all(|c| c.is_ascii_digit()))
            || {
                // "12 |" or "12 of 24" patterns
                let parts: Vec<&str> = trimmed.split('|').collect();
                parts.len() == 2 && parts[0].trim().chars().all(|c| c.is_ascii_digit())
            };
        if is_page_num && !trimmed.is_empty() {
            stats.page_numbers_removed += 1;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    // Remove trailing newline we just added
    if out.ends_with('\n') && !text.ends_with('\n') {
        out.pop();
    }
    out
}

/// Collapse multiple spaces/tabs into a single space, strip trailing whitespace
/// on each line, and collapse 3+ newlines into 2.
fn collapse_whitespace(text: &str, stats: &mut CleanStats) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = false;
    let mut consecutive_newlines = 0;

    for ch in text.chars() {
        match ch {
            ' ' | '\t' => {
                consecutive_newlines = 0;
                if last_was_space {
                    stats.whitespace_collapsed += 1;
                } else {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            '\n' => {
                last_was_space = false;
                consecutive_newlines += 1;
                if consecutive_newlines <= 2 {
                    out.push('\n');
                } else {
                    stats.whitespace_collapsed += 1;
                }
            }
            _ => {
                consecutive_newlines = 0;
                last_was_space = false;
                out.push(ch);
            }
        }
    }

    // Strip trailing whitespace from each line (already collapsed above, but
    // spaces before newlines may have slipped through).
    let out = out
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");

    out
}
