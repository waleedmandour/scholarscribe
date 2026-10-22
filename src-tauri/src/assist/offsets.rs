//! Scalar↔UTF-16 offset mapping.
//!
//! The engine's canonical offsets are Unicode scalar values (harper's unit;
//! see recon §6 Q3). The add-in boundary speaks UTF-16 code units because
//! that is what JavaScript string indices and the Word object model use.
//! The two coincide on ASCII and BMP-only text and diverge exactly on
//! astral-plane content (emoji, some CJK ideographs, historic scripts).
//!
//! The map is built once per text and answers both directions in
//! O(log n) via a prefix table + binary search. It is lossless by
//! construction and pinned by round-trip property tests on the exact edge
//! cases the brief lists: astral-plane emoji and ZWJ sequences, Arabic with
//! combining diacritics, NBSP, curly quotes, mixed bidi, CJK.
//!
//! Spike note: the wire format stays UTF-16 unless the owner's spike run
//! (experiment (a) in word-addin/spike/OBSERVATIONS.md) proves
//! `Critique.start/length` are not UTF-16 code units, in which case only
//! this module's two conversion helpers at the server boundary change.

use super::{ScalarOffset, Utf16Offset};

/// Prefix-sum table over the character sequence of a text:
/// `prefix[i]` is the UTF-16 length of the first `i` characters.
#[derive(Debug, Clone)]
pub struct OffsetMap {
    prefix: Vec<usize>,
}

impl OffsetMap {
    /// Build the map for `text`. Cost: one pass over the characters.
    pub fn new(text: &str) -> Self {
        let mut prefix = Vec::with_capacity(text.chars().count() + 1);
        prefix.push(0usize);
        let mut acc = 0usize;
        for c in text.chars() {
            acc += c.len_utf16();
            prefix.push(acc);
        }
        Self { prefix }
    }

    /// Number of characters (scalar values) the text has.
    pub fn scalar_len(&self) -> usize {
        self.prefix.len() - 1
    }

    /// UTF-16 length of the whole text.
    pub fn utf16_len(&self) -> usize {
        self.prefix[self.prefix.len() - 1]
    }

    /// Scalar → UTF-16. Identity on BMP-only texts. Out-of-range scalars
    /// clamp to the end (callers validate spans before mapping).
    pub fn scalar_to_utf16(&self, scalar: usize) -> Utf16Offset {
        let idx = scalar.min(self.scalar_len());
        self.prefix[idx]
    }

    /// UTF-16 → scalar. A UTF-16 offset in the middle of a surrogate pair
    /// snaps to the scalar that *owns* it (the pair's start), so a span
    /// never begins or ends inside an astral character. Out-of-range
    /// offsets clamp to the end.
    pub fn utf16_to_scalar(&self, utf16: Utf16Offset) -> ScalarOffset {
        let want = utf16.min(self.utf16_len());
        // prefix is non-decreasing; find the last index whose prefix is
        // <= want. That index is the scalar whose UTF-16 offset owns `want`:
        // for a BMP scalar the prefix equals its offset; for the first half
        // of a surrogate pair the offset points at the pair's start.
        // prefix is non-decreasing; the index below is the scalar whose
        // UTF-16 offset owns `want`: for a BMP scalar the prefix equals its
        // offset; for the first half of a surrogate pair the offset points
        // at the pair's start.
        self.prefix
            .partition_point(|&p| p <= want)
            .saturating_sub(1)
    }

    /// Map a scalar span (start, end) to a UTF-16 span.
    pub fn span_scalar_to_utf16(
        &self,
        start: ScalarOffset,
        end: ScalarOffset,
    ) -> (Utf16Offset, Utf16Offset) {
        (self.scalar_to_utf16(start), self.scalar_to_utf16(end))
    }

    /// Map a UTF-16 span (start, end) to a scalar span, snapping both ends
    /// out of surrogate-pair interiors.
    pub fn span_utf16_to_scalar(
        &self,
        start: Utf16Offset,
        end: Utf16Offset,
    ) -> (ScalarOffset, ScalarOffset) {
        let s = self.utf16_to_scalar(start);
        // The end boundary snaps forward to the next scalar start when it
        // lands inside a pair, so the span covers the whole astral char.
        let e = self.utf16_to_scalar(end);
        let e = if self.prefix.get(e).copied().unwrap_or(0) < end.min(self.utf16_len())
            && self.scalar_len() > e
            && self.prefix[e + 1] <= end
        {
            e + 1
        } else {
            e
        };
        (s, e.max(s))
    }
}

#[cfg(test)]
mod tests {
    use super::OffsetMap;

    /// The edge-case corpus from the brief: astral emoji, ZWJ sequences,
    /// Arabic + diacritics (combining marks), NBSP, curly quotes, mixed
    /// bidi, CJK.
    const EDGE_CASES: [&str; 8] = [
        "Plain ASCII sentence.",
        "Emoji: 🎓🚀 and ZWJ family 👨‍👩‍👧‍👦 in text.",
        "العربية مع التشكيل: مَدْرَسَةٌ وَكِتَابٌ.",
        "Non-breaking\u{00A0}space and “curly quotes” — plus an em dash.",
        "Mixed bidi: العربية text with (Smith, 2020) citation [12].",
        "CJK: 学术写作与句号。还有感叹号！问号？",
        "Math: x² + y³ over 3.5 ± 0.2 and ∑ n=1..10.",
        "Combining: é vs e\u{0301} and å\u{030A} normalization pairs.",
    ];

    #[test]
    fn utf16_round_trip_identity() {
        for text in EDGE_CASES {
            let map = OffsetMap::new(text);
            // scalar → utf16 → scalar must be the identity for every
            // scalar boundary.
            for s in 0..=map.scalar_len() {
                let u = map.scalar_to_utf16(s);
                assert_eq!(map.utf16_to_scalar(u), s, "text {text:?} scalar {s}");
            }
        }
    }

    #[test]
    fn utf16_length_matches_javascript() {
        // Spot checks against known UTF-16 lengths (JS `.length`).
        assert_eq!(OffsetMap::new("a🎓b").utf16_len(), 4); // surrogate pair = 2
        assert_eq!(OffsetMap::new("👨‍👩‍👧‍👦").utf16_len(), 11); // 8 + 3 ZWJ
        assert_eq!(OffsetMap::new("مَدْ").utf16_len(), 4); // base + combining
        assert_eq!(OffsetMap::new("学。").utf16_len(), 2);
    }

    #[test]
    fn ascii_golden_scalar_equals_utf16() {
        // On ASCII-only text both offset spaces coincide exactly.
        let text = "The cat sat on the mat, and the dog barked (2020).";
        let map = OffsetMap::new(text);
        assert_eq!(map.scalar_len(), text.chars().count());
        assert_eq!(map.utf16_len(), text.len());
        for s in 0..=map.scalar_len() {
            assert_eq!(map.scalar_to_utf16(s), s);
            assert_eq!(map.utf16_to_scalar(s), s);
        }
    }

    #[test]
    fn span_mapping_round_trips() {
        for text in EDGE_CASES {
            let map = OffsetMap::new(text);
            let chars: Vec<char> = text.chars().collect();
            for start in 0..=chars.len() {
                for end in start..=chars.len() {
                    let (u0, u1) = map.span_scalar_to_utf16(start, end);
                    let (s0, s1) = map.span_utf16_to_scalar(u0, u1);
                    // Span boundaries that were on scalar boundaries stay
                    // there; a UTF-16 span snapped out of a pair interior
                    // may grow to cover the astral char, never shrink below
                    // the original boundaries.
                    assert!(s0 <= start, "start regressed: {text:?} {start}->{s0}");
                    assert!(s1 >= end, "end regressed: {text:?} {end}->{s1}");
                    assert!(s1 <= chars.len());
                }
            }
        }
    }

    #[test]
    fn surrogate_interior_snaps_to_owner() {
        let text = "a🎓b"; // scalar: a=0, 🎓=1, b=2; utf16: a=0, pair=1..3, b=3
        let map = OffsetMap::new(text);
        assert_eq!(map.utf16_to_scalar(2), 1); // inside the pair -> owner
        let (s, e) = map.span_utf16_to_scalar(2, 4);
        assert_eq!((s, e), (1, 3)); // covers the whole emoji + b
    }

    #[test]
    fn out_of_range_clamps() {
        let map = OffsetMap::new("abc");
        assert_eq!(map.scalar_to_utf16(99), 3);
        assert_eq!(map.utf16_to_scalar(99), 3);
    }
}
