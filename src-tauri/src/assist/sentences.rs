//! Language-agnostic sentence splitting.
//!
//! Tier 0 and Tier 1 both work per sentence: Tier 0 lints whole paragraphs
//! but reports sentence-bounded spans, Tier 1 rewrites one sentence at a
//! time. The splitter is deliberately script-agnostic (harper has no
//! language detector — recon §6 Q5) so the language gate can run on the
//! split result for any input: Latin scripts use `.!?…`, CJK uses
//! `。！？…`, and decimal/thousand separators and known abbreviations are
//! protected from false splits.
//!
//! Design constraints:
//! - No dictionaries, no per-language models: a heuristic that errs toward
//!   *fewer* boundaries (merging is safer than splitting for Tier 1, since
//!   a merged pair just becomes one slightly longer rewrite unit).
//! - Offsets are scalar values into the original paragraph string.
//! - No allocation of substrings unless the caller asks for them.

/// A sentence boundary span: `[start, end)` in scalar offsets into the
/// paragraph, `end` exclusive and covering the terminator but not the
/// trailing whitespace. `text` is the slice itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// True when the code point could plausibly start a new sentence: an
/// uppercase Latin/Cyrillic/Greek letter, a CJK ideograph, kana, Hangul,
/// an Arabic letter, a digit, or an opening quote/bracket. Used after a
/// terminator to decide whether a boundary is real.
fn looks_like_sentence_start(c: char) -> bool {
    c.is_uppercase()
        || c.is_numeric()
        || matches!(c,
            '\u{0600}'..='\u{06FF}'   // Arabic
            | '\u{4E00}'..='\u{9FFF}' // CJK ideographs
            | '\u{3040}'..='\u{30FF}' // kana
            | '\u{AC00}'..='\u{D7AF}' // Hangul
            | '\u{201C}' | '\u{2018}' | '"' | '(' | '[' | '\u{00AB}' | '\u{00BF}' | '\u{00A1}')
}

/// Known abbreviations that must not terminate a sentence. Matched
/// case-sensitively against the word ending at the dot.
const ABBREVIATIONS: [&str; 15] = [
    "e.g", "i.e", "etc", "cf", "et al", "al", "vs", "Fig", "Eq", "Sec", "Tab", "No", "Dr", "Prof",
    "Sr",
];

/// Split a paragraph into sentences. Whitespace between sentences is not
/// part of any sentence. A paragraph without terminators is one sentence.
pub fn split_sentences(paragraph: &str) -> Vec<Sentence> {
    let chars: Vec<char> = paragraph.chars().collect();
    let mut out: Vec<Sentence> = Vec::new();
    let mut start = 0usize; // scalar offset of the current sentence
    let mut idx = 0usize; // current scalar index

    let is_terminator = |c: char| matches!(c, '.' | '!' | '?' | '…' | '。' | '！' | '？');

    while idx < chars.len() {
        let c = chars[idx];
        if is_terminator(c) {
            // Absorb runs like "!!!" / "?!" / "..." / "…。" as one boundary.
            let mut end = idx + 1;
            while end < chars.len() && is_terminator(chars[end]) {
                end += 1;
            }
            // A single '.' needs abbreviation and decimal protection.
            let real_boundary = if c == '.' && end - idx == 1 {
                // Decimal protection: digit on both sides of the dot.
                let digit_before = idx > 0 && chars[idx - 1].is_numeric();
                let digit_after = end < chars.len() && chars[end].is_numeric();
                if digit_before && digit_after {
                    idx += 1;
                    continue;
                }
                // Abbreviation: check the word that ends here.
                if let Some(word) = word_ending_at(&chars, idx) {
                    if ABBREVIATIONS.iter().any(|a| a.eq_ignore_ascii_case(&word))
                        || word.len() == 1
                    {
                        // Single-letter initial ("J. Smith") — not a boundary.
                        idx += 1;
                        continue;
                    }
                }
                true
            } else {
                true
            };

            if real_boundary {
                // Require whitespace + plausible starter, or end of text.
                let mut next = end;
                while next < chars.len() && chars[next].is_whitespace() {
                    next += 1;
                }
                let boundary = if next >= chars.len() {
                    true
                } else {
                    // The next character must look like a starter,
                    // otherwise the terminator is decorative (URLs,
                    // "3.x.y", code). No whitespace is required: CJK and
                    // tight typography place the next sentence directly
                    // after the terminator.
                    looks_like_sentence_start(chars[next])
                };
                if boundary {
                    // Trim trailing whitespace off the sentence itself.
                    let mut sent_end = end;
                    while sent_end > start && chars[sent_end - 1].is_whitespace() {
                        sent_end -= 1;
                    }
                    if sent_end > start {
                        out.push(Sentence {
                            start,
                            end: sent_end,
                            text: chars[start..sent_end].iter().collect(),
                        });
                    }
                    start = next;
                    idx = next;
                    continue;
                }
            }
            idx = end;
            continue;
        }
        idx += 1;
    }

    // Tail: whatever is left (a paragraph fragment without a terminator).
    let mut tail_end = chars.len();
    while tail_end > start && chars[tail_end - 1].is_whitespace() {
        tail_end -= 1;
    }
    if tail_end > start {
        out.push(Sentence {
            start,
            end: tail_end,
            text: chars[start..tail_end].iter().collect(),
        });
    }
    out
}

/// The word (letters only) ending right before `dot_idx`.
fn word_ending_at(chars: &[char], dot_idx: usize) -> Option<String> {
    let mut start = dot_idx;
    while start > 0 && (chars[start - 1].is_alphabetic() || chars[start - 1] == '.') {
        start -= 1;
    }
    if start == dot_idx {
        return None;
    }
    Some(chars[start..dot_idx].iter().collect())
}

/// Script-share language gate (recon §6 Q5, ruling 2): classifies the
/// paragraph's letters by script and returns the dominant script plus its
/// share of all letters. The engine uses this to produce the
/// "language not supported yet" status instead of linting garbage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Latin,
    Arabic,
    Cjk,
    Cyrillic,
    Greek,
    Other,
}

pub fn dominant_script(text: &str) -> Option<(Script, f64)> {
    let mut counts = [0usize; 6];
    let mut total = 0usize;
    for c in text.chars() {
        if !c.is_alphabetic() {
            continue;
        }
        let s = if c.is_ascii() || ('\u{00C0}'..='\u{024F}').contains(&c) {
            Script::Latin
        } else if ('\u{0600}'..='\u{06FF}').contains(&c) {
            Script::Arabic
        } else if ('\u{4E00}'..='\u{9FFF}').contains(&c)
            || ('\u{3040}'..='\u{30FF}').contains(&c)
            || ('\u{AC00}'..='\u{D7AF}').contains(&c)
        {
            Script::Cjk
        } else if ('\u{0400}'..='\u{04FF}').contains(&c) {
            Script::Cyrillic
        } else if ('\u{0370}'..='\u{03FF}').contains(&c) {
            Script::Greek
        } else {
            Script::Other
        };
        counts[match s {
            Script::Latin => 0,
            Script::Arabic => 1,
            Script::Cjk => 2,
            Script::Cyrillic => 3,
            Script::Greek => 4,
            Script::Other => 5,
        }] += 1;
        total += 1;
    }
    if total == 0 {
        return None;
    }
    let names = [
        Script::Latin,
        Script::Arabic,
        Script::Cjk,
        Script::Cyrillic,
        Script::Greek,
        Script::Other,
    ];
    let (best, n) = counts
        .iter()
        .zip(names.iter())
        .max_by_key(|(n, _)| **n)
        .map(|(n, s)| (*s, *n))?;
    Some((best, n as f64 / total as f64))
}

#[cfg(test)]
mod tests {
    use super::{dominant_script, split_sentences, Script};

    #[test]
    fn english_basic() {
        let s = split_sentences("The cat sat. The dog barked! What next?");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].text, "The cat sat.");
        assert_eq!(s[2].text, "What next?");
        // Offsets must slice back to the sentence text.
        for sent in &s {
            // start offset points at the sentence's first character
            let all: Vec<char> = "The cat sat. The dog barked! What next?".chars().collect();
            assert_eq!(all[sent.start], sent.text.chars().next().unwrap());
        }
    }

    #[test]
    fn decimals_and_thousands_not_split() {
        let s = split_sentences(
            "The sample measured 3.5 grams. Costs rose to 1.234.567 units in 2024.",
        );
        assert_eq!(s.len(), 2);
        assert!(s[0].text.contains("3.5 grams."));
        assert!(s[1].text.contains("1.234.567 units"));
    }

    #[test]
    fn abbreviations_not_split() {
        let s =
            split_sentences("We used e.g. three corpora (Smith et al. 2020). Dr. Lee reviewed it.");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn arabic_sentence() {
        let s = split_sentences("هذه جملة أولى. وهذه جملة ثانية.");
        assert_eq!(s.len(), 2);
        assert!(s[0].text.ends_with('.'));
    }

    #[test]
    fn cjk_terminators() {
        let s = split_sentences("学术写作很重要。它能帮助作者！你同意吗？");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].text, "学术写作很重要。");
    }

    #[test]
    fn url_dot_not_split() {
        let s = split_sentences("See example.com for details. Then continue here.");
        assert_eq!(s.len(), 2);
        assert!(s[0].text.contains("example.com for details."));
    }

    #[test]
    fn no_terminator_is_one_sentence() {
        let s = split_sentences("a fragment without any terminator");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "a fragment without any terminator");
    }

    #[test]
    fn emoji_offsets_survive() {
        let s = split_sentences("🎓 Nice. Second one 🚀!");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].text, "🎓 Nice.");
        assert_eq!(s[1].text, "Second one 🚀!");
    }

    #[test]
    fn script_gate() {
        assert!(matches!(
            dominant_script("Hello world, this is English."),
            Some((Script::Latin, share)) if share > 0.99
        ));
        assert!(matches!(
            dominant_script("العربية نص عربي"),
            Some((Script::Arabic, share)) if share > 0.99
        ));
        assert!(matches!(
            dominant_script("学术写作"),
            Some((Script::Cjk, _))
        ));
        assert!(dominant_script("3.14 + 2.71 = 5.85").is_none());
    }
}
