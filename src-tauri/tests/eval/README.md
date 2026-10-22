# tests/eval — frozen held-out evaluation set (A7)

EMPTY in v0.1 (PREVIEW posture: the owner has not supplied a held-out
set, so v0.1 carries no accuracy claims). When sentences are annotated,
they land here as `sentences.jsonl` + `manifest.json` per
`docs/EVAL_PROTOCOL.md`; the freeze check (npm test + the Rust
`frozen_set_integrity` test) fails on any drift after annotation.

The golden set in `tests/assist/golden.json` is a SMOKE TEST, not
precision evidence (docs/DECISIONS.md).
