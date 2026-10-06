# Grammar Spec — Proofread Feature (Tier 1 + Phase 2a/2b + Phase 3)

**Version:** v2.3.0 (Phase 1 + 2a + 2b + 3)
**Status:** Tier 1 MVP for .txt/.md; .docx lint-only + apply-fixes; Tier 2 local-LLM (gated).
**Date:** 2026-10-05
**Normative for:** `src-tauri/src/grammar.rs`, `src-tauri/src/grammar_commands.rs`, `src-tauri/src/text_masking.rs`, `src-tauri/src/tier2.rs`, `src-tauri/src/docx_reading.rs` (Phase 2a/2b additions), `src/components/Proofread.svelte`, `src/lib/api.ts` (grammar wrappers), `scripts/check-proofread-invariants.sh`.

---

## 1. Principles

### 1.1 Privacy
User text never leaves the device. Tier 1 (Harper) makes zero HTTP calls and zero file reads beyond the user-selected input. Tier 2 (local-LLM) reuses the existing Ollama client at `127.0.0.1:11434` — no new host, no new HTTP crate. The audited network surface does not grow.

### 1.2 Academic integrity
The author stays the author. The tool corrects; it does not write. Every suggestion is a minimal edit op, never a rewrite. Tier 2 returns the sentence unchanged when already grammatical (H4). No silent auto-apply (H3).

### 1.3 No detector coupling
No imports from `style`, `risk_profiler`, `style_fingerprint`, `voice_consistency`, `appeal_letter`. Verified by `scripts/check-proofread-invariants.sh` (H2).

### 1.4 Correction ledger is counts only
`LedgerFinding`, `LedgerDecision`, `LedgerSummary` contain counts and enumerated IDs only — no user text, no hashes. Verified by H5 static check and runtime probe test `grammar::tests::ledger_serializes_no_user_text`.

### 1.5 No provenance interference
Does not modify `provenance.rs`, `verifier/`, or `PROVENANCE_SPEC.md`. Out of scope.

---

## 2. Data flow

### 2.1 Text/Markdown mode (Phase 1)
```
grammar_check(text, format, dialect, ignored_rules, personal_words)
  -> audit.record("proofread_check", "local", "tier1 check, N chars", 0, 0)
  -> grammar::check() -> CheckResult
  -> ProofreadSession::record_findings()
  -> return CheckResult
```

### 2.2 .docx mode (Phase 2a/2b)
```
grammar_check_docx(path, dialect, ...)
  -> audit.record("file_read", path, "proofread .docx, N paragraphs, M chars", byte_count, 0)
  -> docx_reading::extract_text_with_runs(path) -> (text, ParagraphMap, Vec<RunSpan>)
  -> grammar::check_docx(text, paragraph_map, run_spans, dialect, ...)
       -> for each finding: can_apply_in_run(start, end, run_spans) -> (bool, blocker)
  -> return DocxCheckResult

grammar_apply_docx_fix(input_path, output_path, finding, replacement)
  -> re-extract run_spans
  -> read document.xml from input .docx
  -> grammar::apply_suggestion_to_docx_xml(xml, run_spans, finding, replacement)
  -> write new .docx to output_path (never overwrites original; H7)
  -> audit.record("file_write", output_path, "proofread apply-docx-fix", 0, byte_count)
  -> record decision in ledger
```

### 2.3 Tier 2 local-LLM (Phase 3, gated)
```
grammar_tier2_suggest(sentence, dialect, model)
  -> check ProofreadSession::tier2_enabled (off by default; H3 opt-in)
  -> audit.record("ollama_command", "{base}/api/chat", "tier2 minimal-edit, N chars, model=X", 0, 0)
  -> tier2::suggest_minimal_edit(client, base, request)
       -> POST {base}/api/chat with TIER2_SYSTEM_PROMPT, temperature 0, JSON-only
       -> parse response -> Tier2Result { fixes, was_clean }
  -> return Tier2Result (H4: if fixes empty, was_clean=true; caller drops finding)
```

---

## 3. Formats

### 3.1 Rule ID mapping (stable, append-only)
| rule_id | LintKind | RuleKind |
|---------|----------|----------|
| 1 | Spelling | Spelling |
| 2 | Typo | Spelling |
| 3 | Capitalization | Spelling |
| 10-17 | Grammar-class | Grammar |
| 20-21 | Punctuation-class | Punctuation |
| 30-34 | Style-class | Style |
| 40-41 | Regionalism/Nonstandard | Consistency |
| 99 | Miscellaneous | Misc |

### 3.2 DocxFinding (Phase 2a + 2b)
```json
{
  "start_char": 32, "end_char": 41,
  "start_utf16": 32, "end_utf16": 41,
  "tier": "harper", "rule_kind": "spelling", "rule_id": 1,
  "explanation": "...", "original_text": "mispelled",
  "suggestions": [...],
  "paragraph_index": 2, "paragraph_count": 12,
  "can_apply_in_place": true,
  "apply_blocker": ""
}
```

### 3.3 Tier2Result (Phase 3)
```json
{
  "fixes": [{ "kind": "spelling", "description": "replace 'mispelled' with 'misspelled'" }],
  "model": "gemma3:4b",
  "was_clean": false
}
```
`was_clean: true` when `fixes` is empty (H4).

---

## 4. Phase 2b — apply-fixes

### 4.1 RunSpan offset map
`docx_reading::extract_text_with_runs` returns `(text, ParagraphMap, Vec<RunSpan>)`. Each `RunSpan` maps a char range in the extracted text to the byte range of the run's text content in `document.xml` (between `<w:t...>` and `</w:t>`).

### 4.2 Three edit cases
1. **Span fits in one run** → `apply_suggestion_to_docx_xml` splices the replacement into the run's text. Writes a new `<name>-proofread.docx`.
2. **Span crosses N runs in one paragraph** → Phase 2b does NOT do run consolidation (safety). `can_apply_in_place = false`; UI shows "Cross-run: resolve in Word".
3. **Span crosses `</w:p>`** → same as case 2; flagged for manual review.

### 4.3 H7 (never overwrite)
`grammar_apply_docx_fix` rejects `output_path == input_path`. Always writes to a new file.

---

## 5. Phase 3 — Tier 2 local-LLM (gated)

### 5.1 Opt-in gate
`ProofreadSession::tier2_enabled: AtomicBool`, default `false`. Toggle via `grammar_tier2_enable` / `grammar_tier2_disable`. Status via `grammar_tier2_status`.

### 5.2 Dedicated system prompt
`tier2::TIER2_SYSTEM_PROMPT` — NOT inherited from Chat/WritingCoach (which forbid rewriting). Explicitly allows minimal-edit fixes; forbids rewriting for style; "if unsure, return {\"fixes\": []}".

### 5.3 H4 enforcement
H4 ("return unchanged when already grammatical") is enforced in CODE: the caller drops `fixes` when `was_clean == true`. The prompt's "if unsure, return empty" is defence in depth.

### 5.4 H1 (no new network paths)
Tier 2 reuses `ollama::OllamaState::client` (a `reqwest::Client`) and `ollama::base_url()` (`http://127.0.0.1:11434`). No new HTTP crate, no new host. The audit log records an `ollama_command` entry.

---

## 6. Decisions & assumptions
1. harper-core pinned to `=2.11.0`.
2. `concurrent` feature left at default (on).
3. Rust MSRV bumped 1.77 → 1.85 (edition 2024).
4. PlainEnglish parser, not IsolateEnglish (deviation).
5. Dialect default: American.
6. Personal dictionary + ignored rules session-scoped.
7. Ledger session-scoped, in-memory, cleared on app close.
8. Provenance integration out of scope.
9. Phase 2b: single-run apply only; cross-run edits flagged for manual review.
10. Phase 3: dedicated Tier 2 prompt; H4 enforced in code.
11. `zip` default features disabled (build fix for harper-core lzma conflict).

---

## 7. Change process
Any change to the ledger types, rule_id mapping, audit log entry format, mask span taxonomy, RunSpan structure, Tier 2 system prompt, CSP, or H2 forbidden-modules list must update this spec, the Rust implementation, the invariants script, and the CI workflow in the same commit.
