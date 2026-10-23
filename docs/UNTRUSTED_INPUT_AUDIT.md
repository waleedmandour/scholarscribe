# Untrusted-input audit — .docx / .zip / .xml / .bib surfaces

Status: PROPOSAL for a separate PR to `main` (owner's P3). No behaviour is changed
on the integration branch by this document.

- Snapshot: every `file:line` below is at integration tip `4d305f2` + P1
  (`84e8f65`). Line numbers may drift on later commits; the function names are
  the stable anchors.
- Premise (owner ruling, Gate 1): a `.docx` or `.bib` received from a
  collaborator, a journal, or any folder is **untrusted input**. "The user
  chose to open it" is not a trust statement.
- Threat: memory exhaustion (zip bombs), CPU exhaustion (quadratic XML
  attribute scan — RUSTSEC-2026-0194), pathological regex/parse behaviour.
  The Word Assist listener is a *different* untrusted channel and is already
  capped by H3 (handshake 5 s, header 30 s, body 30 s, headers 16 KiB, body
  64 KiB, 16 concurrent) — documented here only as the contrast case.

## 1. Every place that opens untrusted input

| # | Site (file:line) | What it opens | Into memory or disk? | Decompressed-size cap | Entry-count cap | Nesting-depth guard |
|---|---|---|---|---|---|---|
| 1 | `src/docx_reading.rs:35` (`read_document_xml`) → `ZipArchive::new` `:40` → `by_index` loop `:45` → `read_to_end` `:51` | whole `.docx` file, then `word/document.xml` | memory (whole file + decompressed part) | **none** | **none** (iterates `archive.len()`) | n/a (name-equality match; no nested zip opened) |
| 2 | `src/docx_to_latex.rs:64` → `ZipArchive::new` `:67` → `read_to_end` `:76` | whole `.docx`, `word/document.xml` | memory | **none** | **none** | n/a |
| 3 | `src/structure_analyzer.rs:59` (`analyze_docx`) → `ZipArchive::new` `:61` → `read_to_string` `:70` | whole `.docx`, `word/document.xml` | memory | **none** | **none** | n/a |
| 4 | `src/commands.rs:625` (`clean_docx_in_place`) → `ZipArchive::new` `:628` → `by_index` loop over **all** entries `:645` → `read_to_string` per matching part `:648`; reopens input at `:665` to copy | whole `.docx` **twice** (`fs::read` + `.clone()` at `:626`), every text part (document/header*/footer*/footnotes/endnotes) | memory (input ×2 + all rewritten parts held in a `HashMap` before the output zip is written) | **none** | **none** | n/a |
| 5 | `src/citation_manager.rs:503` (`read_bib_file`) → caller `src/commands.rs:926` | whole `.bib` as UTF-8 string | memory | **none** (`read_to_string`) | n/a | n/a |
| 6 | `src/commands.rs:144` (`read_file`, ext in txt/md/markdown/tex/rst/csv/json) and the `.docx` leg at `:163` | whole text file | memory | **none** | n/a | n/a |
| 7 | `src/provenance.rs:1351`, `:1368` | `.docx` fixtures | memory | test-only (`#[cfg(test)]`) — out of scope | | |

Other XML consumers: `quick_xml::Reader::from_str` at `src/docx_reading.rs:209`
and `src/docx_to_latex.rs:131` parse the already-decompressed part; the
exposure is therefore bounded by rows 1–4. `grep -rn "NsReader" src/` → no
matches (RUSTSEC-2026-0195's affected API is not used).

Extraction to disk: **none anywhere** — `grep -rn "\.extract(\|ZipStreamReader" src/`
→ no matches. All zip access is `by_index` + read into memory. This is why the
zip-crate symlink/permission advisories (RUSTSEC-2026-0367/-0368/-0369) are not
exercised (see docs/ADVISORIES_A6.md §4).

## 2. Worst case on a hostile file (current behaviour, INFERRED from code + crate semantics)

Let S = size of the on-disk file, D = size of the decompressed part.

- **Memory.** Row 1–3: ≥ 2S + D at peak (raw bytes + zip structures +
  decompressed part; deflate allows D ≫ S — a 10 MB archive can legally
  declare and deliver a multi-GB `word/document.xml`; nothing in our code or
  in the `zip` crate's streaming reader imposes an application-level ceiling,
  so `read_to_end`/`read_to_string` will attempt to allocate it). Row 4 is
  worse: `bytes.clone()` doubles S before parsing, and every rewritten part is
  buffered until the output zip is built (≈ 2S + Σ parts + output buffer).
  A crafted file can therefore OOM the desktop app (the OS kills the process;
  unsaved journal state is lost).
- **CPU.** `src/docx_reading.rs:299-307` and `src/docx_to_latex.rs:285`
  iterate `e.attributes()` with the default duplicate check on every start tag
  — O(N²) in attributes per tag (RUSTSEC-2026-0194: ~6 s at 80 k attributes,
  ~10 min at 800 k, one core). The I/O-free check cannot be interrupted by any
  timeout we install. `extract_docx_headings` then runs regex passes over the
  same attacker-sized string (linear, but on D).
- **Entry iteration.** Rows 1–4 iterate `0..archive.len()`; entry names are
  compared by equality — no path-traversal risk, but `archive.len()` is
  attacker-chosen (central directory) and each `by_index` costs a seek + local
  header parse (linear scan cost per iteration on damaged archives).
- **`.bib` (row 5).** Plain line parser, O(size); the only amplified costs are
  `String` growth per field. Memory = file size; a multi-GB "bib" is possible
  but requires the user to select it; no cap exists.

## 3. Proposed caps (for the follow-up PR to main)

All numbers are proposals, to be confirmed by the owner; they are chosen to be
far above legitimate manuscripts (the largest realistic `.docx` is a few
hundred MB with embedded media; the XML parts we read are KB–MB scale).

| Cap | Proposed value | Where it would live |
|---|---|---|
| Max input file size (`fs::read` guard) | 512 MiB | one helper used by rows 1–6 (`metadata()?.len()` before read) |
| Max per-entry decompressed size | 256 MiB | read via `entry.take(cap)` / `Read::take`, error if the limit is hit |
| Max total decompressed across parts (row 4) | 512 MiB running counter | `clean_docx_in_place` loop |
| Max entry count | 16 384 | `if archive.len() > CAP { err }` |
| Max compression ratio per entry | 200:1 (with a 1 MiB floor so small XML is never rejected) | checked after `take`-bounded read: `decompressed > 200 × compressed && decompressed > 1 MiB → err` |
| Max text-file size (row 6, `.bib`/txt/md/…) | 64 MiB | same helper as above |
| CPU: attribute scan | `with_checks(false)` on our two parsers **plus** an input-length pre-check | `docx_reading.rs:209`, `docx_to_latex.rs:131`; duplicate-attribute detection is not needed by our extractors (we read first-match) — but this alone is a behaviour change, hence PR-to-main, not P3 |

Notes:
- `with_checks(false)` disables quick-xml's duplicate-attribute error — our
  helpers return the first matching attribute, so semantics are unchanged for
  well-formed input; for malformed input we currently *error* where the
  relaxed mode would proceed. That trade-off must be an explicit owner
  decision (proposal: keep checks ON, rely on the size caps to bound N; the
  O(N²) term then costs at most ~(16k attrs)² fast byte compares ≈ well under
  a second per tag at the proposed caps).
- The caps are app-level, in our helper layer — no dependency change, no new
  crate. (`zip` has no built-in bomb switch in 2.4.2; upgrading to 9.x for
  RUSTSEC-2026-0367/-0368/-0369 is a separate owner decision, major bump.)

## 4. Test ideas (for the same PR)

1. **Zip-bomb rejection:** unit test with a nested-deflate archive whose
   `word/document.xml` decompresses > 256 MiB from a ~1 MB input; assert the
   helper errors with a size message and the process RSS stays bounded
   (spawn + measure, or simply assert the error before any large `Vec` is
   returned).
2. **Ratio guard:** entry with declared small size, huge actual stream →
   error mentions the ratio cap.
3. **Entry-count bomb:** archive with 100 k stub entries (accepted by the
   central directory) → error at the 16 384 cap.
4. **Quadratic-tag timeout:** XML start tag with 100 k attributes parsed by
   our two extractors completes in a bounded wall time (regression guard for
   whichever owner decision lands: caps vs `with_checks(false)`).
5. **Symlink / device entries:** archive containing a symlink entry named
   `word/document.xml` → assert it is skipped/errored, never followed (guards
   the in-memory path against future refactors that might add extraction).
6. **Huge `.bib`:** 64 MiB+1 byte file → clean error, no allocation beyond
   the cap.
7. **Legit files keep working:** golden .docx/.bib fixtures below every cap
   produce identical outputs to the pre-cap implementation (snapshot the
   current outputs first).

## 5. Non-goals

- No change to the Word Assist listener (H3 caps already bound that channel;
  its body limit is 64 KiB, verified by E2E tests).
- No behaviour change on `feature/word-assist-integration` — this document is
  the P3 deliverable; implementation is proposed for a separate PR to `main`.
- No new dependency.
