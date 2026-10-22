# Held-out evaluation protocol (Word Assist Tier 1) — A7 mechanism

Purpose: define HOW the held-out evaluation set is built so that, when it
exists, one local command produces a defensible report. v0.1 ships NO set
(owner default: not supplied) and therefore makes NO accuracy claim. The
golden set in `src-tauri/tests/assist/golden.json` is a **SMOKE TEST**
(engine-behaviour pinning), recorded as such in `docs/DECISIONS.md` — it
is never cited as precision evidence.

## Ordering rule (hard)

**Annotation precedes any checker.** A sentence enters the set only with
its annotation already fixed on paper (category + span + clean/dirty);
the checker then faces the sentence blind. Re-annotating after seeing a
model's output is forbidden: it converts an evaluation into a calibration
exercise.

## Where the sentences come from

- Sentences are **owner-authored or owner-approved**, one `source` field
  per sentence recording its origin.
- **No third-party corpus is vendored into the repository.** Public GEC /
  grammar corpora frequently carry non-commercial or research-only terms;
  any import from such a source happens through a LOCAL script the owner
  runs, after the license has been read and recorded in the `license`
  field, and the sentences land only on the owner's machine — never in a
  commit. When in doubt: write sentences by hand instead.
- Real manuscript text must not be committed either (the set is frozen
  in git); sentences derived from real drafts are paraphrased to
  non-identifying form first.

## Schema (`src-tauri/tests/eval/sentences.jsonl`, one JSON object per line)

```json
{
  "id": "s001",
  "text": "The results indicates a strong trend.",
  "category": "grammar",
  "expect_issue": true,
  "span": [4, 12],
  "source": "owner-authored",
  "license": "CC0"
}
```

- `category`: `grammar` | `spelling` | `clarity` (the shipped flags; other
  values appear in the report under their own name — never silently
  dropped).
- `span`: the expected issue span in **Unicode scalar offsets** relative
  to `text` (engine space), `[start, end)`; `null`/omitted for clean
  sentences.
- `expect_issue: false` marks a clean sentence: a validated model
  suggestion on it scores as a false positive.
- `source` + `license`: mandatory on every line; the freeze check rejects
  lines missing them.

## Freezing and the CI gate

1. Annotate the sentences (per the ordering rule).
2. Compute `sha256sum src-tauri/tests/eval/sentences.jsonl` (and any
   companion file) into `src-tauri/tests/eval/manifest.json`:

   ```json
   { "files": { "sentences.jsonl": "<sha256 hex>" } }
   ```

3. Commit. From then on, `npm test` runs
   `scripts/check-eval-freeze.mjs` (the single freeze checker, shared
   with the hardening round's CI gate — Stage A's separate
   `check-eval-frozen.mjs` was retired during integration), and CI
   additionally runs
   `cargo test --features word-assist --test eval_harness` whose
   `frozen_set_integrity` test fails when (a) any listed file's hash
   changed, or (b) a file appears in `tests/eval/` that the manifest does
   not list. To change a sentence: re-annotate deliberately, regenerate
   the manifest, and say so in the commit message.
4. The absence of `manifest.json` is reported, not failed: it is the v0.1
   PREVIEW posture.

## Running the evaluation

```sh
cd src-tauri
WORD_ASSIST_EVAL_MODEL=<model> cargo test --features word-assist \
  --test eval_harness -- --ignored --nocapture
```

The report (stdout only) contains: per-category n / TP / FP / FN with
Wilson 95% intervals on precision, validator rejection counts by rule,
p50/p95 latency per request, and the sentence ids whose revision changed
protected spans. Span-overlap classification in v0.1 is conservative: a
validated non-identity revision of a dirty sentence counts as touching
the annotated span; the report footer says so. **Nothing from the report
enters the UI or the docs without an explicit owner decision** — v0.1
carries no accuracy claims.
