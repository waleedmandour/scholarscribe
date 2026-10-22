//! Held-out evaluation scoring machinery (owner directive, hardening
//! round: "build the machinery now, not the data").
//!
//! This module scores a checker run against a LABELLED sentence set and
//! produces the per-category n/TP/FP/FN table with Wilson score
//! intervals. It contains NO data: the owner supplies or approves the
//! sentences (docs/EVAL/LABELLING_PROTOCOL.md), and public GEC corpora
//! are never vendored into the repo.
//!
//! LABELS COME FIRST (protocol rule 1): the labelling protocol exists so
//! the labels are written before any checker run — this module only
//! ever CONSUMES a finished (sentence, label, run) triple.
//!
//! MATCHING POLICY (documented, deliberately simple): a checker issue is
//! a TRUE POSITIVE when its span overlaps a labelled error span of the
//! same category; otherwise it is a FALSE POSITIVE. A labelled span no
//! checker issue overlaps (of that category) is a FALSE NEGATIVE. Span
//! overlap is inclusive (start..=end, scalar offsets, the engine's
//! canonical space). Precision = TP/(TP+FP), recall = TP/(TP+FN), each
//! reported with a Wilson interval — never a bare percentage.

/// One category row of the evaluation table.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryScore {
    pub category: String,
    /// labelled error spans scored in this category.
    pub n: usize,
    /// labelled errors the checker found.
    pub tp: usize,
    /// checker issues that matched no labelled error.
    pub fp: usize,
    /// labelled errors the checker missed.
    pub fn_count: usize,
}

/// One labelled error in the source data (scalar span + category).
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSpan {
    pub start: usize,
    pub end: usize,
    pub category: String,
}

/// One checker issue in the run output (scalar span + category).
#[derive(Debug, Clone, PartialEq)]
pub struct CheckerIssue {
    pub start: usize,
    pub end: usize,
    pub category: String,
}

fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

/// Score one sentence's labels against one run's issues. Returns the
/// per-category increments; the caller folds them into [`CategoryScore`].
pub fn score_sentence(
    category: &str,
    labels: &[LabelSpan],
    issues: &[CheckerIssue],
) -> Vec<(String, usize, usize, usize, usize)> {
    // (category, n, tp, fp, fn)
    let mut out: std::collections::BTreeMap<String, (usize, usize, usize, usize)> =
        std::collections::BTreeMap::new();
    let bump = |map: &mut std::collections::BTreeMap<String, (usize, usize, usize, usize)>,
                cat: &str,
                idx: usize| {
        let e = map.entry(cat.to_string()).or_insert((0, 0, 0, 0));
        match idx {
            0 => e.0 += 1,
            1 => e.1 += 1,
            2 => e.2 += 1,
            _ => e.3 += 1,
        }
    };

    // Labels of THIS sentence partition into matched (TP) and missed
    // (FN). Every label contributes n += 1 in its own category; a
    // matched label also contributes tp += 1, a missed one fn += 1.
    // Every UNMATCHED checker issue contributes fp += 1 in its category
    // (a matched issue is CONSUMED by its label — one label, one TP, no
    // double counting).
    let mut matched_labels = vec![false; labels.len()];
    let mut matched_issues = vec![false; issues.len()];
    for (li, label) in labels.iter().enumerate() {
        for (ii, issue) in issues.iter().enumerate() {
            if !matched_issues[ii]
                && issue.category == label.category
                && overlaps((label.start, label.end), (issue.start, issue.end))
            {
                matched_labels[li] = true;
                matched_issues[ii] = true;
                break;
            }
        }
    }
    for (li, label) in labels.iter().enumerate() {
        bump(&mut out, &label.category, 0); // n counts labelled errors
        bump(
            &mut out,
            &label.category,
            if matched_labels[li] { 1 } else { 3 }, // tp or fn
        );
    }
    for (ii, issue) in issues.iter().enumerate() {
        if !matched_issues[ii] {
            bump(&mut out, &issue.category, 2); // fp
        }
        let _ = ii;
    }
    let _ = category;
    out.into_iter()
        .map(|(cat, (n, tp, fp, fnn))| (cat, n, tp, fp, fnn))
        .collect()
}

/// Fold per-sentence increments into the full report, sorted by category.
pub fn aggregate(rows: Vec<(String, usize, usize, usize, usize)>) -> Vec<CategoryScore> {
    let mut map: std::collections::BTreeMap<String, CategoryScore> =
        std::collections::BTreeMap::new();
    for (cat, n, tp, fp, fn_count) in rows {
        let e = map.entry(cat.clone()).or_insert_with(|| CategoryScore {
            category: cat,
            n: 0,
            tp: 0,
            fp: 0,
            fn_count: 0,
        });
        e.n += n;
        e.tp += tp;
        e.fp += fp;
        e.fn_count += fn_count;
    }
    map.into_values().collect()
}

/// Wilson score interval for a binomial proportion (z = 1.96 for 95%).
/// Returns (lower, upper), clamped to [0, 1]. `total == 0` yields
/// (0.0, 0.0) — "no data", never "0%".
pub fn wilson_interval(successes: usize, total: usize, z: f64) -> (f64, f64) {
    if total == 0 {
        return (0.0, 0.0);
    }
    let n = total as f64;
    let p = successes as f64 / n;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denom;
    let half = (z / denom) * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    (
        (center - half).clamp(0.0, 1.0),
        (center + half).clamp(0.0, 1.0),
    )
}

/// 95% Wilson interval (the only level the report uses).
pub fn wilson_interval_95(successes: usize, total: usize) -> (f64, f64) {
    wilson_interval(successes, total, 1.959_963_984_540_054)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    /// Known-value check: Wilson(8, 10, z=1.96) ≈ (0.490, 0.943).
    #[test]
    fn wilson_known_value() {
        let (lo, hi) = wilson_interval(8, 10, 1.96);
        assert!(close(lo, 0.490), "lo = {lo}");
        assert!(close(hi, 0.943), "hi = {hi}");
    }

    /// Degenerate inputs are defined, never NaN.
    #[test]
    fn wilson_degenerate_inputs() {
        assert_eq!(wilson_interval_95(0, 0), (0.0, 0.0));
        let (lo, hi) = wilson_interval_95(0, 10);
        assert_eq!(lo, 0.0);
        assert!(close(hi, 0.278), "0/10 upper bound ≈ 0.278, got {hi}");
        let (lo, hi) = wilson_interval_95(10, 10);
        assert!(close(lo, 0.722), "10/10 lower bound ≈ 0.722, got {lo}");
        assert!((hi - 1.0).abs() < 1e-9);
    }

    /// More successes never lower the interval.
    #[test]
    fn wilson_is_monotone_in_successes() {
        let mut prev_lo = -1.0;
        for k in 0..=20 {
            let (lo, _hi) = wilson_interval_95(k, 20);
            assert!(lo >= prev_lo, "lower bound must not decrease at k={k}");
            prev_lo = lo;
        }
    }

    /// Small samples give wide intervals — the whole point of reporting
    /// Wilson instead of a bare percentage.
    #[test]
    fn wilson_widens_for_small_samples() {
        let (lo1, hi1) = wilson_interval_95(4, 5);
        let (lo2, hi2) = wilson_interval_95(40, 50);
        assert!(hi1 - lo1 > hi2 - lo2, "n=5 must be wider than n=50");
    }

    #[test]
    fn overlap_is_inclusive_and_order_free() {
        assert!(overlaps((0, 5), (5, 10))); // touching ends count
        assert!(overlaps((5, 10), (0, 5)));
        assert!(!overlaps((0, 4), (5, 10)));
    }

    /// The matching policy: same category + span overlap = TP; wrong
    /// category = FP (checker) + FN (label); disjoint = FP + FN.
    #[test]
    fn score_sentence_policy() {
        let labels = vec![
            LabelSpan {
                start: 0,
                end: 5,
                category: "grammar".into(),
            },
            LabelSpan {
                start: 20,
                end: 25,
                category: "spelling".into(),
            },
        ];
        // checker finds the grammar error (TP), invents a clarity issue
        // (FP), and misses the spelling error (FN).
        let issues = vec![
            CheckerIssue {
                start: 3,
                end: 6,
                category: "grammar".into(),
            },
            CheckerIssue {
                start: 30,
                end: 33,
                category: "clarity".into(),
            },
        ];
        let rows = score_sentence("grammar", &labels, &issues);
        let agg = aggregate(rows);
        let grammar = agg.iter().find(|c| c.category == "grammar").unwrap();
        assert_eq!((grammar.tp, grammar.fp, grammar.fn_count), (1, 0, 0));
        let spelling = agg.iter().find(|c| c.category == "spelling").unwrap();
        assert_eq!((spelling.tp, spelling.fp, spelling.fn_count), (0, 0, 1));
        let clarity = agg.iter().find(|c| c.category == "clarity").unwrap();
        assert_eq!((clarity.tp, clarity.fp, clarity.fn_count), (0, 1, 0));
        // n = labelled errors in the category = 1 for each labelled one.
        assert_eq!(grammar.n, 1);
        assert_eq!(spelling.n, 1);
        // clarity has no labels: n stays 0, fp is the only signal.
        assert_eq!(clarity.n, 0);
    }

    /// One labelled span matched by TWO checker issues consumes only one
    /// label; the second issue is an FP (no double counting).
    #[test]
    fn double_detection_is_one_tp_one_fp() {
        let labels = vec![LabelSpan {
            start: 0,
            end: 4,
            category: "grammar".into(),
        }];
        let issues = vec![
            CheckerIssue {
                start: 0,
                end: 2,
                category: "grammar".into(),
            },
            CheckerIssue {
                start: 3,
                end: 4,
                category: "grammar".into(),
            },
        ];
        let agg = aggregate(score_sentence("grammar", &labels, &issues));
        let g = &agg[0];
        assert_eq!((g.tp, g.fp, g.fn_count), (1, 1, 0));
    }

    /// End-to-end shape: several sentences fold into one table row.
    #[test]
    fn aggregate_folds_sentences() {
        let mut rows = Vec::new();
        for _ in 0..3 {
            rows.extend(score_sentence(
                "grammar",
                &[LabelSpan {
                    start: 0,
                    end: 3,
                    category: "grammar".into(),
                }],
                &[CheckerIssue {
                    start: 0,
                    end: 3,
                    category: "grammar".into(),
                }],
            ));
        }
        let agg = aggregate(rows);
        assert_eq!(agg.len(), 1);
        assert_eq!(
            (agg[0].n, agg[0].tp, agg[0].fp, agg[0].fn_count),
            (3, 3, 0, 0)
        );
        let (lo, hi) = wilson_interval_95(agg[0].tp, agg[0].tp + agg[0].fp);
        assert!((hi - 1.0).abs() < 1e-9 && lo > 0.3);
    }
}
