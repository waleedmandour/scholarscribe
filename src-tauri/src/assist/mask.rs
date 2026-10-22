//! Protected spans and masking.
//!
//! Scholarly sentences are full of content that a grammar engine must never
//! touch: citations, numbers, URLs, DOIs, inline LaTeX and quotations
//! (ruling 1 / recon §6 Q5). Strategy **S1** is the shipped behavior: lint
//! the *original* text, then drop any lint whose span intersects a
//! protected span. This keeps harper's grammatical context intact and costs
//! one post-filter. Strategy S2 (lint masked text) is kept available behind
//! `MaskMode` for the Phase 4 re-evaluation, because the golden set can
//! arbitrate both (D-4: decision pending golden-set evidence; E-1 keeps the
//! final defaults provisional until Phase 4).
//!
//! Protected span sources, in priority order:
//! 1. In-text citations via `citation_manager::extract_in_text_citations`
//!    (the single source of truth used by the Citations page — D-4's
//!    "reuse" requirement).
//! 2. URL / DOI literals (regex).
//! 3. Numbers: integers, decimals, thousands separators, percentages,
//!    ranges, years.
//! 4. Inline LaTeX: `$...$`, `\(...\)`, `\[...\]` fragments.
//! 5. Quoted spans: straight `"..."` and curly `“...”`.
//!
//! `mask`/`unmask` implement S2: replace protected spans with same-length
//! placeholder runs so offsets never move, and restore the originals
//! losslessly. The `unmask(mask(x)) == x` property is pinned by tests.

use crate::citation_manager;

/// One protected region of the input, scalar offsets, end exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectedSpan {
    pub start: usize,
    pub end: usize,
    /// Which detector produced the span (kept for tests and the spec).
    pub kind: ProtectedKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedKind {
    Citation,
    Url,
    Number,
    Latex,
    Quote,
}

/// Collect all protected spans in `text`, merged so that overlapping or
/// adjacent detections coalesce (a "(Smith, 2020)" citation must not leave
/// "2020" double-protected; merging is idempotent).
pub fn protected_spans(text: &str) -> Vec<ProtectedSpan> {
    let mut raw: Vec<ProtectedSpan> = Vec::new();

    // 1. Citations (author-year and numeric), scalar offsets out of the box.
    for c in citation_manager::extract_in_text_citations(text) {
        let len = c.raw.chars().count();
        raw.push(ProtectedSpan {
            start: c.position,
            end: c.position + len,
            kind: ProtectedKind::Citation,
        });
    }

    // 2. URLs and DOIs.
    let url_re = regex::Regex::new(
        r#"(?i)\b(?:https?://|www\.)[^\s"'<>()\[\]{}]+|\b(?:doi:\s*)?10\.\d{4,9}/[^\s"'<>()\[\]{}]+"#,
    )
    .expect("url regex");
    for m in url_re.find_iter(text) {
        // Trim a trailing dot/comma that belongs to the sentence, not the
        // URL ("See https://example.com/page." — the final dot is prose).
        let mut end = m.end();
        let url = m.as_str();
        if url.ends_with('.') || url.ends_with(',') {
            end -= 1;
        }
        let start = text[..m.start()].chars().count();
        let len = text[m.start()..end].chars().count();
        raw.push(ProtectedSpan {
            start,
            end: start + len,
            kind: ProtectedKind::Url,
        });
    }

    // 3. Numbers: 1,234.56 / 3.5 / 42% / 2020s / ranges "3-5".
    let num_re = regex::Regex::new(r"\b\d[\d.,]*\d(?:%|s)?\b|\b\d\b(?:\s*[-–]\s*\d+\b)?")
        .expect("number regex");
    for m in num_re.find_iter(text) {
        let start = text[..m.start()].chars().count();
        let len = m.as_str().chars().count();
        raw.push(ProtectedSpan {
            start,
            end: start + len,
            kind: ProtectedKind::Number,
        });
    }

    // 4. Inline LaTeX.
    let latex_re =
        regex::Regex::new(r"\$[^$\n]+\$|\\\([^)\n]+\\\)|\\\[[^\]\n]+\\\]").expect("latex regex");
    for m in latex_re.find_iter(text) {
        let start = text[..m.start()].chars().count();
        let len = m.as_str().chars().count();
        raw.push(ProtectedSpan {
            start,
            end: start + len,
            kind: ProtectedKind::Latex,
        });
    }

    // 5. Quoted spans (straight + curly). Straight quotes match paired
    //    quotes on one line; the regex crate has no look-around, so a
    //    stray unpaired quote is simply not protected (precision over
    //    recall: an unprotected quote can only mean a lint we show, never
    //    one Word applies).
    let quote_re = regex::Regex::new(r#"“[^”\n]{1,300}”|"[^"\n]{1,300}"#).expect("quote regex");
    for m in quote_re.find_iter(text) {
        let start = text[..m.start()].chars().count();
        let len = m.as_str().chars().count();
        raw.push(ProtectedSpan {
            start,
            end: start + len,
            kind: ProtectedKind::Quote,
        });
    }

    merge_spans(raw)
}

/// Specificity rank for merge kinds (lower wins): a DOI that also looks
/// like a number is still a DOI; a citation that contains a year is still
/// a citation.
fn kind_rank(kind: ProtectedKind) -> u8 {
    match kind {
        ProtectedKind::Citation => 0,
        ProtectedKind::Url => 1,
        ProtectedKind::Latex => 2,
        ProtectedKind::Quote => 3,
        ProtectedKind::Number => 4,
    }
}

/// Merge overlapping/adjacent spans; the most specific kind wins.
fn merge_spans(mut spans: Vec<ProtectedSpan>) -> Vec<ProtectedSpan> {
    if spans.is_empty() {
        return spans;
    }
    spans.sort_by_key(|s| (s.start, s.end));
    let mut out: Vec<ProtectedSpan> = Vec::with_capacity(spans.len());
    for s in spans {
        match out.last_mut() {
            // Overlapping or touching (adjacent protection reads as one
            // region to the filter).
            Some(last) if s.start <= last.end => {
                last.end = last.end.max(s.end);
                if kind_rank(s.kind) < kind_rank(last.kind) {
                    last.kind = s.kind;
                }
            }
            _ => out.push(s),
        }
    }
    out
}

/// True when a lint span [a, b) intersects any protected span.
pub fn intersects_protected(spans: &[ProtectedSpan], a: usize, b: usize) -> bool {
    spans.iter().any(|p| a < p.end && b > p.start)
}

/// Masking mode (S1 vs S2 arbitration, D-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskMode {
    /// Lint original text, drop lints intersecting protected spans (shipped).
    Filter,
    /// Lint masked text (placeholders), remap offsets 1:1 (same length).
    Mask,
}

/// Mask every protected span with `¤` (a BMP letter-ish placeholder that
/// harper treats as a word character) preserving scalar offsets exactly:
/// the output has the same number of characters as the input, and each
/// placeholder run starts where the protected span started.
pub fn mask(text: &str, spans: &[ProtectedSpan]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = chars.clone();
    for s in spans {
        for c in out.iter_mut().take(s.end.min(chars.len())).skip(s.start) {
            *c = '¤';
        }
    }
    out.into_iter().collect()
}

/// Restore placeholders from `masked` back to the original characters.
/// `unmask(mask(x, spans), spans) == x` for every x and spans.
pub fn unmask(masked: &str, original: &str) -> String {
    let m: Vec<char> = masked.chars().collect();
    let o: Vec<char> = original.chars().collect();
    if m.len() != o.len() {
        // Defensive: a masked text that changed length cannot be restored.
        // Callers treat this as "no suggestion" (precision over recall).
        return masked.to_string();
    }
    m.iter()
        .zip(o.iter())
        .map(|(mc, oc)| if *mc == '¤' { *oc } else { *mc })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{intersects_protected, mask, merge_spans, protected_spans, unmask, ProtectedKind};

    fn kinds(text: &str) -> Vec<ProtectedKind> {
        protected_spans(text).iter().map(|s| s.kind).collect()
    }

    #[test]
    fn citations_protected() {
        let text = "As shown by Smith (2020) and [12], the effect holds.";
        let spans = protected_spans(text);
        // Narrative citation: the span starts at "Smith" and covers the
        // year; numeric citation: the span covers "[12]".
        let chars: Vec<char> = text.chars().collect();
        let cite = spans
            .iter()
            .find(|s| s.kind == ProtectedKind::Citation)
            .expect("narrative citation span");
        let covered: String = chars[cite.start..cite.end.min(chars.len())]
            .iter()
            .collect();
        assert!(
            covered.contains("2020"),
            "span must cover the year: {covered:?}"
        );
        let numeric = spans
            .iter()
            .find(|s| s.kind == ProtectedKind::Citation && text[s.start..].starts_with('['))
            .expect("numeric citation span");
        assert_eq!(&text[numeric.start..numeric.end], "[12]");
    }

    #[test]
    fn urls_and_dois_protected() {
        assert!(kinds("See https://example.com/page for the data.").contains(&ProtectedKind::Url));
        assert!(
            kinds("The DOI is 10.1000/j.journal.2024.12345 in the text.")
                .contains(&ProtectedKind::Url)
        );
    }

    #[test]
    fn numbers_protected() {
        let spans = protected_spans("We measured 3.5 grams and 1,234 samples in 2024.");
        let n = spans
            .iter()
            .filter(|s| s.kind == ProtectedKind::Number)
            .count();
        assert!(n >= 3, "expected >=3 number spans, got {n}");
    }

    #[test]
    fn latex_protected() {
        let s = "The sum $\\sum_{i=1}^{10} x_i$ is finite.";
        assert!(kinds(s).contains(&ProtectedKind::Latex));
    }

    #[test]
    fn quotes_protected() {
        assert!(kinds(r#"He said "do not touch this" and left."#).contains(&ProtectedKind::Quote));
        assert!(kinds("He said “do not touch this” and left.").contains(&ProtectedKind::Quote));
    }

    #[test]
    fn citation_and_year_merge() {
        // "(Smith, 2020)" — the citation detector covers the year, so the
        // merged output must be ONE span covering the whole citation.
        let spans = protected_spans("As shown (Smith, 2020) elsewhere.");
        let inside = spans
            .iter()
            .filter(|s| s.start >= "As shown ".len() && s.end <= "As shown (Smith, 2020)".len())
            .count();
        assert_eq!(inside, 1);
    }

    #[test]
    fn mask_unmask_round_trip() {
        // Property: unmask(mask(x)) == x, on every edge-case corpus entry.
        const CORPUS: [&str; 8] = [
            "As shown by Smith (2020) and [12], the effect holds.",
            "See https://example.com/page?q=1 and doi:10.1000/abc.123.",
            "We measured 3.5 grams (±0.2) among 1,234 samples in 2024.",
            "The sum $\\sum_{i=1}^{10} x_i$ is finite, per Eq. 3.",
            "He said “do not touch this quote” and left.",
            "العربية مع (Smith, 2020) و رقم 42 هنا.",
            "Mixed: 学 in 2020 and “quotes” plus 🎓 emoji.",
            "No protected content here at all.",
        ];
        for text in CORPUS {
            let spans = protected_spans(text);
            let masked = mask(text, &spans);
            assert_eq!(masked.chars().count(), text.chars().count());
            assert_eq!(unmask(&masked, text), text);
            // Placeholder runs must start exactly at protected starts.
            let mchars: Vec<char> = masked.chars().collect();
            for s in &spans {
                assert_eq!(mchars[s.start], '¤', "span {s:?} of {text:?}");
            }
        }
    }

    #[test]
    fn intersection_check() {
        let text = "Cite (Smith, 2020) now.";
        let spans = protected_spans(text);
        // A lint overlapping the citation must be dropped.
        assert!(intersects_protected(
            &spans,
            "Cite ".len(),
            "Cite (Smith".len()
        ));
        // A lint outside protection must survive.
        assert!(!intersects_protected(
            &spans,
            "Cite (Smith, 2020) ".len(),
            text.len()
        ));
    }

    #[test]
    fn merge_is_idempotent_and_sorted() {
        let spans = merge_spans(vec![
            super::ProtectedSpan {
                start: 5,
                end: 8,
                kind: ProtectedKind::Number,
            },
            super::ProtectedSpan {
                start: 0,
                end: 3,
                kind: ProtectedKind::Citation,
            },
            super::ProtectedSpan {
                start: 7,
                end: 12,
                kind: ProtectedKind::Url,
            },
        ]);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[1].start, 5);
        assert_eq!(spans[1].end, 12); // merged 5..8 and 7..12
    }
}
