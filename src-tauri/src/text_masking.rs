//! Text masking for the Proofread tab (v2.3.0).
//! Masks citations, DOIs, URLs, inline math, code spans before Harper lints.
//! H8: regexes produce Vec<MaskSpan> (char offsets) only; never store text.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskSpan {
    pub start: usize,
    pub end: usize,
}

impl MaskSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub static RE_PAREN_CITATION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\(([A-Z][a-zA-Z'\-]+(?:\s+(?:et\s+al\.?|and|&)\s+[A-Z][a-zA-Z'\-]+)?),?\s*(\d{4}[a-z]?)\)").unwrap()
});
pub static RE_NARRATIVE_CITATION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b([A-Z][a-zA-Z'\-]+(?:(?:\s+et\s+al\.?)|(?:\s+and\s+[A-Z][a-zA-Z'\-]+)|(?:\s+&\s+[A-Z][a-zA-Z'\-]+))?)\s+\((\d{4}[a-z]?)\)").unwrap()
});
pub static RE_NUMERIC_CITATION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\[([\d,\s\-]+)\]").unwrap());
pub static RE_DOI: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\b10\.\d{4,9}/[^\s,;"'<>)\]]+[^\s,;"'<>)\].,:!?]"#).unwrap());
pub static RE_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\bhttps?://[^\s<>\]\)]+|\bftp://[^\s<>\]\)]+").unwrap());
pub static RE_INLINE_MATH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$[^$]*\$|\\\([^)]*\\\)|\\\[[^\]]*\\\]").unwrap());
pub static RE_CODE_SPAN: Lazy<Regex> = Lazy::new(|| Regex::new(r"`[^`]+`").unwrap());

pub fn find_mask_spans(text: &str) -> Vec<MaskSpan> {
    let mut all: Vec<MaskSpan> = Vec::new();
    let byte_to_char: Vec<usize> = build_byte_to_char_map(text);
    let mut push = |re: &Regex| {
        for m in re.find_iter(text) {
            all.push(MaskSpan::new(
                byte_to_char[m.start()],
                byte_to_char[m.end()],
            ));
        }
    };
    push(&RE_URL);
    push(&RE_DOI);
    push(&RE_INLINE_MATH);
    push(&RE_CODE_SPAN);
    push(&RE_NUMERIC_CITATION);
    push(&RE_PAREN_CITATION);
    push(&RE_NARRATIVE_CITATION);
    all.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| b.end.cmp(&a.end)));
    let mut out: Vec<MaskSpan> = Vec::with_capacity(all.len());
    for span in all {
        if let Some(last) = out.last() {
            if span.start < last.end {
                continue;
            }
        }
        out.push(span);
    }
    out
}

fn build_byte_to_char_map(text: &str) -> Vec<usize> {
    let mut map = Vec::with_capacity(text.len() + 1);
    let mut char_idx: usize = 0;
    for (byte_idx, _) in text.char_indices() {
        while map.len() < byte_idx {
            map.push(char_idx.saturating_sub(1));
        }
        map.push(char_idx);
        char_idx += 1;
    }
    while map.len() <= text.len() {
        map.push(char_idx);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spans_eq(a: &[MaskSpan], e: &[(usize, usize)]) -> bool {
        a.len() == e.len()
            && a.iter()
                .zip(e.iter())
                .all(|(x, y)| x.start == y.0 && x.end == y.1)
    }
    #[test]
    fn finds_paren_citation() {
        assert!(spans_eq(
            &find_mask_spans("See Smith (2020) for details."),
            &[(4, 16)]
        ));
    }
    #[test]
    fn finds_numeric_citation() {
        assert!(spans_eq(
            &find_mask_spans("Previous work [12, 13] showed this."),
            &[(14, 22)]
        ));
    }
    #[test]
    fn finds_doi() {
        let text = "The dataset is at 10.5281/zenodo.1234567.";
        let s = find_mask_spans(text);
        let m: String = text
            .chars()
            .skip(s[0].start)
            .take(s[0].end - s[0].start)
            .collect();
        assert_eq!(m, "10.5281/zenodo.1234567");
    }
    #[test]
    fn finds_url() {
        let text = "Visit https://example.com/path for more.";
        let s = find_mask_spans(text);
        let m: String = text
            .chars()
            .skip(s[0].start)
            .take(s[0].end - s[0].start)
            .collect();
        assert_eq!(m, "https://example.com/path");
    }
    #[test]
    fn finds_inline_math() {
        let text = "The formula $x^2 + y^2 = z^2$ is Pythagoras.";
        let s = find_mask_spans(text);
        let m: String = text
            .chars()
            .skip(s[0].start)
            .take(s[0].end - s[0].start)
            .collect();
        assert_eq!(m, "$x^2 + y^2 = z^2$");
    }
    #[test]
    fn finds_code_span() {
        let text = "Use `print(\"hello\")` to display.";
        let s = find_mask_spans(text);
        let m: String = text
            .chars()
            .skip(s[0].start)
            .take(s[0].end - s[0].start)
            .collect();
        assert_eq!(m, "`print(\"hello\")`");
    }
    #[test]
    fn url_takes_priority_over_doi() {
        let text = "See https://doi.org/10.1234/abc.5678 for the paper.";
        let s = find_mask_spans(text);
        assert_eq!(s.len(), 1);
        let m: String = text
            .chars()
            .skip(s[0].start)
            .take(s[0].end - s[0].start)
            .collect();
        assert_eq!(m, "https://doi.org/10.1234/abc.5678");
    }
    #[test]
    fn handles_multibyte_text() {
        let text = "The café ☕ is nice. (Smith, 2020) is a citation.";
        let target = "(Smith, 2020)";
        let start = text.find(target).unwrap();
        let start_char = text[..start].chars().count();
        let end_char = start_char + target.chars().count();
        assert!(spans_eq(&find_mask_spans(text), &[(start_char, end_char)]));
    }
    #[test]
    fn handles_empty_text() {
        assert!(find_mask_spans("").is_empty());
    }
    #[test]
    fn handles_text_with_no_masks() {
        assert!(find_mask_spans("This is just plain English.").is_empty());
    }
    #[test]
    fn multiple_masks_sorted_non_overlapping() {
        let s = find_mask_spans("See [12] and (Smith, 2020) and $x=1$ and `code`.");
        for w in s.windows(2) {
            assert!(w[0].end <= w[1].start);
        }
        assert!(s.len() >= 4);
    }
}
