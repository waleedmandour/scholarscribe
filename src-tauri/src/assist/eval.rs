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

/// One category row of the evaluation table. (Integration: also reused by
/// the Tier 1 harness report below, so it gained Default + Serialize.)
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
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

// =========================================================== Tier 1 harness
//
// A7 layer rebased onto the held-out scoring machinery above (integration
// resolution: ONE eval module, ONE Wilson implementation — the hardening
// round's `wilson_interval` / `CategoryScore` are the only ones; the
// harness items below were renamed where they collided: Stage A's
// `score_sentence` → `score_harness_sentence`, its `aggregate` →
// `aggregate_classes`, and its duplicate `CategoryScore` was dropped in
// favour of the one above (fields n/tp/fp/fn_count)).
//
// Mechanism only in v0.1. The owner has NOT supplied a held-out
// evaluation set (v0.1 posture): this machinery exists so the moment a
// set lands, one command produces the report. Nothing here writes
// accuracy figures into the UI or docs — the report goes to stdout for
// the owner's eyes, and the golden set in `tests/assist/golden.json` is
// a SMOKE TEST, never precision evidence (DECISIONS 2026-10).
//
// Frozen set (owner-supplied, `src-tauri/tests/eval/sentences.jsonl`):
// one JSON object per line, `EvalSentence` shape, annotated BEFORE any
// checker runs (docs/EVAL_PROTOCOL.md). The freeze marker is
// `docs/EVAL/FROZEN.txt`; `scripts/check-eval-freeze.mjs` is the single
// freeze checker (npm + CI) — Stage A's uncommitted duplicate
// `check-eval-frozen.mjs` was NOT resurrected.
//
// PRIVACY: the frozen set never contains manuscript text from real
// documents — sentences are owner-authored or license-cleared with the
// source recorded per sentence (annotation protocol).

/// z value for the 95% Wilson interval (two-sided); same constant the
/// survivor's `wilson_interval_95` inlines.
pub const WILSON_Z_95: f64 = 1.959_963_984_540_054;

/// One frozen, annotated sentence. `span` is the expected issue span in
/// Unicode scalar offsets relative to `text` (engine space), `None` for
/// a clean sentence (the harness scores a suggestion there as an FP).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvalSentence {
    pub id: String,
    pub text: String,
    /// Which category the annotation belongs to.
    pub category: String, // "grammar" | "spelling" | "clarity"
    /// true = the sentence contains the flagged issue; false = clean
    /// (a "clean" sentence exists to measure false positives).
    pub expect_issue: bool,
    /// Optional expected span (scalar offsets, inclusive start, exclusive end).
    #[serde(default)]
    pub span: Option<(usize, usize)>,
    /// Where the sentence came from (owner-authored, book, corpus name…).
    pub source: String,
    /// The license that allows its use here (annotation protocol).
    pub license: String,
}

/// One harness run over the frozen set with a live (or mocked) model.
/// `per_category` reuses the survivor's [`CategoryScore`] (n/tp/fp/fn_count).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct EvalReport {
    pub model: String,
    pub sentences: usize,
    pub per_category: Vec<CategoryScore>,
    /// Requests whose revision was rejected by the validator, by rule id.
    pub rejections_by_rule: std::collections::BTreeMap<u8, usize>,
    /// Requests that produced a usable revision.
    pub accepted: usize,
    /// Requests that failed before validation (unparseable JSON counts
    /// here: rule 1 rejections — kept separate for clarity).
    pub schema_rejections: usize,
    /// Latencies (ms) of all completed requests, sorted ascending.
    pub latencies_ms: Vec<u128>,
    /// Sentences where the model's revision changed protected content
    /// (validator rules 2/3: removed or ADDED protected spans).
    pub protected_span_changes: Vec<String>,
    /// Wilson 95% intervals per category over precision (TP / (TP+FP)),
    /// computed with the ONE Wilson implementation above.
    pub precision_wilson: std::collections::BTreeMap<String, (f64, f64)>,
}

/// Classify one annotated sentence against a (possible) revision verdict.
/// Returns (category, classification) where classification is
/// "tp" | "fp" | "fn" | "tn". (Harness-layer classifier; the label-vs-
/// issue scorer above is [`score_sentence`].)
pub fn score_harness_sentence(
    s: &EvalSentence,
    suggestion_applied: bool,
    overlap_with_span: bool,
) -> (&'static str, &'static str) {
    let class = if s.expect_issue {
        if suggestion_applied && overlap_with_span {
            "tp"
        } else if suggestion_applied {
            "fp" // changed something, not the annotated span
        } else {
            "fn" // missed the annotated issue
        }
    } else if suggestion_applied {
        "fp" // "fixing" a clean sentence
    } else {
        "tn"
    };
    (category_bucket(&s.category), class)
}

/// Bucket name for the report; unknown categories keep their name so a
/// frozen-set typo is VISIBLE in the report instead of silently dropped.
fn category_bucket(category: &str) -> &'static str {
    match category {
        "grammar" => "grammar",
        "spelling" => "spelling",
        "clarity" => "clarity",
        other => Box::leak(other.to_string().into_boxed_str()),
    }
}

/// Aggregate per-sentence classifications into report rows.
/// `rows` are (category, class) pairs from [`score_harness_sentence`].
pub fn aggregate_classes(rows: &[(&str, &str)]) -> Vec<CategoryScore> {
    let mut out: Vec<CategoryScore> = Vec::new();
    for (category, class) in rows {
        let entry = out.iter_mut().find(|c| c.category == *category);
        let entry = match entry {
            Some(e) => e,
            None => {
                out.push(CategoryScore {
                    category: (*category).to_string(),
                    ..Default::default()
                });
                out.last_mut().expect("just pushed")
            }
        };
        entry.n += 1;
        match *class {
            "tp" => entry.tp += 1,
            "fp" => entry.fp += 1,
            "fn" => entry.fn_count += 1,
            _ => {}
        }
    }
    out
}

/// Precision Wilson interval per category (TP over TP+FP), 95% level.
pub fn precision_intervals(
    scores: &[CategoryScore],
) -> std::collections::BTreeMap<String, (f64, f64)> {
    let mut out = std::collections::BTreeMap::new();
    for s in scores {
        let predicted_positive = s.tp + s.fp;
        out.insert(
            s.category.clone(),
            wilson_interval(s.tp, predicted_positive, WILSON_Z_95),
        );
    }
    out
}

/// p50 / p95 (nearest-rank) over sorted latencies in ms.
pub fn percentiles(sorted_ms: &[u128]) -> (Option<u128>, Option<u128>) {
    if sorted_ms.is_empty() {
        return (None, None);
    }
    let p = |q: f64| -> u128 {
        let idx = ((q * sorted_ms.len() as f64).ceil() as usize).clamp(1, sorted_ms.len());
        sorted_ms[idx - 1]
    };
    (Some(p(0.50)), Some(p(0.95)))
}

#[cfg(test)]
mod harness_tests {
    use super::*;

    #[test]
    fn harness_scoring_matrix() {
        let dirty = EvalSentence {
            id: "s1".into(),
            text: "The results indicates.".into(),
            category: "grammar".into(),
            expect_issue: true,
            span: Some((4, 12)),
            source: "owner-authored".into(),
            license: "CC0".into(),
        };
        let clean = EvalSentence {
            id: "s2".into(),
            text: "The result indicates a trend.".into(),
            category: "grammar".into(),
            expect_issue: false,
            span: None,
            source: "owner-authored".into(),
            license: "CC0".into(),
        };
        assert_eq!(score_harness_sentence(&dirty, true, true), ("grammar", "tp"));
        assert_eq!(score_harness_sentence(&dirty, true, false), ("grammar", "fp"));
        assert_eq!(score_harness_sentence(&dirty, false, false), ("grammar", "fn"));
        assert_eq!(score_harness_sentence(&clean, true, false), ("grammar", "fp"));
        assert_eq!(score_harness_sentence(&clean, false, false), ("grammar", "tn"));
    }

    #[test]
    fn harness_aggregate_and_intervals() {
        let rows = vec![
            ("grammar", "tp"),
            ("grammar", "tp"),
            ("grammar", "fp"),
            ("spelling", "fn"),
            ("spelling", "tp"),
        ];
        let scores = aggregate_classes(&rows);
        let g = scores.iter().find(|s| s.category == "grammar").unwrap();
        assert_eq!((g.n, g.tp, g.fp), (3, 2, 1));
        let intervals = precision_intervals(&scores);
        // grammar precision = 2/3; the interval must not be a point value.
        let (lo, hi) = intervals["grammar"];
        assert!(lo < 2.0 / 3.0 && hi > 2.0 / 3.0 && lo < hi);
    }

    #[test]
    fn percentiles_nearest_rank() {
        assert_eq!(percentiles(&[]), (None, None));
        let v: Vec<u128> = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let (p50, p95) = percentiles(&v);
        assert_eq!((p50, p95), (Some(50), Some(100)));
    }
}

// --------------------------------------------------- real-model run (A7)
//
// Lives here (crate-internal) rather than in an integration test so the
// private `ollama` module stays private. #[ignore]d: the owner runs it
// locally against a real Ollama model; CI never runs it. Its results are
// development measurements and never appear in docs (v0.1 posture).

#[cfg(test)]
mod real_model {
    use super::*;
    use crate::assist::engine::build_tier1_request;
    use crate::assist::validate::validate_revision;
    use crate::ollama::{base_url, chat_at, ChatMessage, ChatRequest, OllamaState};
    use std::time::Instant;

    const EVAL_DIR: &str = "tests/eval";
    const SET_FILE: &str = "sentences.jsonl";
    const MANIFEST_FILE: &str = "manifest.json";

    fn eval_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(EVAL_DIR)
            .join(name)
    }

    fn load_set() -> Result<Vec<EvalSentence>, String> {
        let raw = std::fs::read_to_string(eval_path(SET_FILE))
            .map_err(|e| format!("frozen set missing ({e}) — nothing to evaluate"))?;
        let mut out = Vec::new();
        for (i, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let s: EvalSentence = serde_json::from_str(line)
                .map_err(|e| format!("frozen set line {}: {e}", i + 1))?;
            out.push(s);
        }
        Ok(out)
    }

    #[tokio::test]
    #[ignore = "real-model eval: run locally with WORD_ASSIST_EVAL_MODEL set and a live Ollama; never run in CI"]
    async fn real_model_eval() {
        let model = std::env::var("WORD_ASSIST_EVAL_MODEL")
            .expect("set WORD_ASSIST_EVAL_MODEL=<model> for the real-model eval");
        let set = match load_set() {
            Ok(s) if !s.is_empty() => s,
            Ok(_) => {
                eprintln!("frozen set present but empty — nothing to evaluate (v0.1 posture)");
                return;
            }
            Err(e) => {
                eprintln!("{e}");
                eprintln!(
                    "v0.1 posture: no held-out set supplied yet; supply tests/eval/{SET_FILE} + {MANIFEST_FILE} per docs/EVAL_PROTOCOL.md"
                );
                return;
            }
        };
        let state = OllamaState::default();
        let url = base_url().to_string();
        let mut report = EvalReport {
            model: model.clone(),
            sentences: set.len(),
            ..Default::default()
        };
        let mut rows: Vec<(&str, &'static str)> = Vec::new();
        for s in &set {
            let started = Instant::now();
            let req = build_tier1_request(&model, &s.text);
            let outcome = chat_at(
                &state,
                &url,
                ChatRequest {
                    model: req.model,
                    messages: vec![
                        ChatMessage {
                            role: "system".into(),
                            content: req.system,
                        },
                        ChatMessage {
                            role: "user".into(),
                            content: req.user,
                        },
                    ],
                    stream: false,
                    temperature: req.temperature,
                    format: Some(req.format),
                    think: Some(req.think),
                },
            )
            .await;
            report.latencies_ms.push(started.elapsed().as_millis());
            let suggestion_applied = match outcome {
                Err(e) => {
                    eprintln!("[{}] ollama error: {e}", s.id);
                    false
                }
                Ok(msg) => match crate::assist::engine::parse_tier1_response(&msg.content) {
                    Err(_) => {
                        report.schema_rejections += 1;
                        false
                    }
                    Ok(resp) => {
                        let changed = resp.revised != s.text;
                        match validate_revision(&s.text, &resp.revised) {
                            Err(rejection) => {
                                *report.rejections_by_rule.entry(rejection.rule).or_insert(0) += 1;
                                if rejection.rule == 2 || rejection.rule == 3 {
                                    report.protected_span_changes.push(s.id.clone());
                                }
                                false
                            }
                            Ok(()) => changed, // identity revision = no suggestion
                        }
                    }
                },
            };
            let overlap = suggestion_applied && s.expect_issue;
            let (cat, class) = score_harness_sentence(s, suggestion_applied, overlap);
            rows.push((cat, class));
        }
        report.per_category = aggregate_classes(&rows);
        report.precision_wilson = precision_intervals(&report.per_category);
        println!("\n=== Word Assist Tier 1 evaluation (LOCAL, not for docs) ===");
        println!("{}", serde_json::to_string_pretty(&report).expect("report"));
        println!("p50/p95 = {:?}", percentiles(&report.latencies_ms));
        println!(
            "Protected-span-change cases: {:?}",
            report.protected_span_changes
        );
        println!("Development measurement only; v0.1 makes no accuracy claims from it.");
    }
}
