//! Word-level diff between an original sentence and a proposed revision.
//!
//! Tier 1 returns a revised sentence; the add-in can only apply word-level
//! edits (the annotation model works on ranges), so the engine computes the
//! edit list itself. The algorithm is a classic LCS over word tokens
//! (whitespace-split, but keeping punctuation attached — a comma belongs to
//! the token it follows), then equal runs are collapsed so that edits are
//! as large as the LCS allows and always word-aligned.
//!
//! All offsets are scalar values into the ORIGINAL sentence. Replacement
//! text carries the revised tokens joined the way they appear in the
//! revision. Whitespace runs normalize to a single space when they change
//! (Word's object model can't express "change whitespace width" as an edit
//! anyway) — with one exception: nothing here touches a token that is only
//! whitespace.

/// One word-level edit against the original sentence, scalar offsets,
/// end exclusive. `replacement` replaces `original[start..end]`; an empty
/// replacement is a deletion, an empty range an insertion.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WordEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
    start: usize,
    end: usize,
}

/// Tokenize into words with attached trailing punctuation; whitespace
/// between words is implicit (recoverable from the original).
fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        out.push(Token {
            text: chars[start..i].iter().collect(),
            start,
            end: i,
        });
    }
    out
}

/// Normalize a token for comparison: case-sensitive (a capitalization fix
/// IS an edit worth reporting) but whitespace-free by construction.
fn norm(t: &str) -> String {
    t.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Compute word-level edits turning `original` into `revised`.
/// Returns edits in left-to-right order, non-overlapping, each expressed
/// against the original's scalar offsets. An empty result means the texts
/// are word-identical.
pub fn word_diff(original: &str, revised: &str) -> Vec<WordEdit> {
    let a = tokenize(original);
    let b = tokenize(revised);

    // LCS table over normalized tokens.
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if norm(&a[i].text) == norm(&b[j].text) {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    // Walk the table collecting op runs: Equal / Delete / Insert.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Op {
        Equal(usize, usize),
        Del(usize),
        Ins(usize),
    }
    let mut ops: Vec<Op> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if norm(&a[i].text) == norm(&b[j].text) {
            ops.push(Op::Equal(i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            ops.push(Op::Del(i));
            i += 1;
        } else {
            ops.push(Op::Ins(j));
            j += 1;
        }
    }
    while i < n {
        ops.push(Op::Del(i));
        i += 1;
    }
    while j < m {
        ops.push(Op::Ins(j));
        j += 1;
    }

    // Collapse adjacent Del/Ins runs into single edits. An edit's original
    // range spans the deleted tokens (or an insertion point between them);
    // its replacement is the inserted tokens joined with single spaces.
    let mut edits: Vec<WordEdit> = Vec::new();
    let mut last_equal_end: Option<usize> = None; // original offset after the last Equal token
    let mut k = 0usize;
    while k < ops.len() {
        if let Op::Equal(ai, _) = ops[k] {
            last_equal_end = Some(a[ai].end);
            k += 1;
            continue;
        }
        let mut dels: Vec<usize> = Vec::new();
        let mut ins: Vec<usize> = Vec::new();
        while k < ops.len() && !matches!(ops[k], Op::Equal(..)) {
            match &ops[k] {
                Op::Del(d) => dels.push(*d),
                Op::Ins(t) => ins.push(*t),
                Op::Equal(..) => unreachable!(),
            }
            k += 1;
        }
        // Edit range: the deleted tokens' span; a pure insertion anchors at
        // the previous Equal token's end (or the start of the text).
        let start = dels
            .first()
            .map(|d| a[*d].start)
            .or(last_equal_end)
            .unwrap_or(0);
        let mut end = dels.last().map(|d| a[*d].end).unwrap_or(start);
        let replacement = if ins.is_empty() {
            String::new()
        } else {
            ins.iter()
                .map(|t| b[*t].text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let (replacement, end) = if dels.is_empty() {
            // Pure insertion: Word needs the joining whitespace to travel
            // with the replacement (ranges carry no whitespace of their
            // own). Prefix a single space unless the previous character is
            // already whitespace or the insertion opens the text.
            let needs_prefix = start > 0
                && original
                    .chars()
                    .nth(start.wrapping_sub(1))
                    .map(|c| !c.is_whitespace())
                    .unwrap_or(false);
            let repl = if needs_prefix && !replacement.is_empty() {
                format!(" {replacement}")
            } else {
                replacement
            };
            (repl, end)
        } else if ins.is_empty() {
            // Pure deletion: swallow one trailing whitespace character when
            // both sides would otherwise leave a double space.
            let swallow = original
                .chars()
                .nth(end)
                .map(|c| c.is_whitespace())
                .unwrap_or(false)
                && start > 0
                && original
                    .chars()
                    .nth(start - 1)
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false);
            if swallow {
                end += 1;
            }
            (String::new(), end)
        } else {
            (replacement, end)
        };
        // Skip no-op edits (identical replacement at the identical range).
        let original_slice: String = chars_at(original, start, end);
        if replacement != original_slice {
            edits.push(WordEdit {
                start,
                end,
                replacement,
            });
        }
    }
    edits
}

fn chars_at(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

/// Apply edits left-to-right is impossible on offsets; this helper applies
/// them right-to-left (like language_check::apply_edits) and returns the
/// resulting text. Used by tests and by Tier 1's local preview.
pub fn apply_word_edits(original: &str, edits: &[WordEdit]) -> String {
    let chars: Vec<char> = original.chars().collect();
    let mut sorted: Vec<&WordEdit> = edits.iter().collect();
    sorted.sort_by(|x, y| y.start.cmp(&x.start).then(y.end.cmp(&x.end)));
    let mut out = chars;
    for e in sorted {
        if e.start > e.end || e.end > out.len() {
            continue; // defensive; validated upstream
        }
        let repl: Vec<char> = e.replacement.chars().collect();
        out.splice(e.start..e.end, repl);
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::{apply_word_edits, word_diff};

    #[test]
    fn identical_text_no_edits() {
        assert!(word_diff("The cat sat.", "The cat sat.").is_empty());
    }

    #[test]
    fn single_word_replace() {
        let edits = word_diff("The cat sat.", "The dog sat.");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start, "The ".len());
        assert_eq!(edits[0].end, "The cat".len());
        assert_eq!(edits[0].replacement, "dog");
        assert_eq!(apply_word_edits("The cat sat.", &edits), "The dog sat.");
    }

    #[test]
    fn insertion_and_deletion() {
        let o = "The results were clear.";
        let r = "The results were not clear.";
        let edits = word_diff(o, r);
        assert_eq!(apply_word_edits(o, &edits), r);

        let o2 = "The results were very much clear.";
        let r2 = "The results were clear.";
        let edits2 = word_diff(o2, r2);
        assert_eq!(apply_word_edits(o2, &edits2), r2);
    }

    #[test]
    fn multi_word_block() {
        let o = "We show that the method works.";
        let r = "We demonstrate that the approach works.";
        let edits = word_diff(o, r);
        assert_eq!(apply_word_edits(o, &edits), r);
        // Two separate edits: "show"→"demonstrate", "method"→"approach".
        assert_eq!(edits.len(), 2);
    }

    #[test]
    fn punctuation_is_part_of_token() {
        let o = "However, the model failed.";
        let r = "However the model failed.";
        let edits = word_diff(o, r);
        // Removing the comma means replacing "However," with "However".
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].replacement, "However");
        assert_eq!(apply_word_edits(o, &edits), r);
    }

    #[test]
    fn emoji_offsets_survive() {
        let o = "🎓 The results hold (2020) even here.";
        let r = "🎓 The results hold even here.";
        let edits = word_diff(o, r);
        assert_eq!(apply_word_edits(o, &edits), r);
        // The removed token starts at a scalar offset, not a UTF-16 one.
        assert!(edits[0].start >= "🎓 The results hold ".chars().count() - 1);
    }

    #[test]
    fn case_change_is_an_edit() {
        let edits = word_diff("the Cat sat.", "the cat sat.");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].replacement, "cat");
    }

    #[test]
    fn whole_sentence_replace() {
        let edits = word_diff("Alpha beta gamma.", "Delta epsilon.");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start, 0);
        assert_eq!(
            apply_word_edits("Alpha beta gamma.", &edits),
            "Delta epsilon."
        );
    }
}
