# Word Assist — assumptions register (Word behavior)

Status: owner review item 5 (2026-10-23). Phase 2 (add-in client) was
built before the Phase 0 spike run; every Word-facing row below is
therefore **INFERRED** (from Microsoft docs + the recon) or
**PROBED** (the client checks capability sets at runtime and degrades),
never **VERIFIED**. The spike (`word-addin/spike/OBSERVATIONS.md`,
experiments a–m) settles each row; "what changes if wrong" is the
blast radius. Engine-internal facts are listed at the bottom for
contrast — those are verified by tests, not by the spike.

The register rule from the owner's review, recorded verbatim:
**"Only the adapter needs adjusting" is INFERRED, not VERIFIED, until
the spike answers.**

| # | Assumption about Word | Where it lives | Settled by | If wrong, what changes |
|---|-----------------------|----------------|------------|------------------------|
| A1 | `Critique.start/length` are UTF-16 code units measured from the **paragraph** start (JS string semantics) | `taskpane.js:264-282` (`toCritique`, the declared ADAPTER); wire sends both spaces (`tier0.rs:44-57`); conversion (`offsets.rs`) | Spike **(a)**: annotate around astral emoji/Arabic and see where the underline lands | Only `toCritique` (and `applyTier1Edits`'s sentence-base math, `taskpane.js:430-443`) changes — IF the spike confirms this is the only divergence. Until then: INFERRED |
| A2 | For popup suggestions, **the add-in applies** the accepted text (Word only fires `onAnnotationPopupAction`) | `taskpane.js:311-330` (`attachPopupHandler` → `applyInlineEdit`); `:365-407` (`applyInlineEdit` rewrites the paragraph via `insertText`) | Spike **(b)/(b2)**: does the text change before/without our apply? Exact `critiqueSuggestion` payload | If Word auto-applies: every popup accept would double-apply — the `applyInlineEdit` call must be dropped for popup actions and kept for list mode; `/v1/accepted` reporting stays. Highest-corruption-risk row |
| A3 | The popup's `args.id` correlates with our issue id (we match `args.id.endsWith(issue.id)`) | `taskpane.js:318` — **and we never set an id**: the critique payload `:267-281` carries no id field | Spike **(j)**: record the actual id-correlation mechanism (`onAnnotationInserted` args) | Current code cannot work if Word assigns opaque ids: needs a side-table (Word id → our issue id) built from `onAnnotationInserted`, and the popup handler rewritten against it |
| A4 | Our own edits (`insertText`, `insertAnnotations`) fire `onParagraphChanged`, and the C4 watermark suppresses them | Watermark push `taskpane.js:372,443`; `isSelfEvent` `:503-507` — **never called (dead code)**; handler `:515-528` schedules unconditionally | Spike **(e)**: do our actions fire events at all, and with what args? | Two independent fixes, both gated on the spike: (1) wire `isSelfEvent(args)` into the handler or delete it; (2) add simulated-event tests for the suppression window (`SELF_EVENT_GRACE_MS=2000`, `:40`). Today only the server budget (`scheduler.rs:38`, 8 calls/paragraph) stops oscillation |
| A5 | Paragraph events exist at WordApi **1.6**, annotations at **1.7**, popups at **1.8**; older hosts degrade to list mode | Probe `taskpane.js:154-165`; fallbacks `:334-363`, `:509-514` | Spike **(f)**: the Environment block | If the sets differ, only the probe thresholds and mode selection change; no structural work |
| A6 | Track Changes: our `insertText("Replace")` is safe to run while TC is on (worst case: our edit becomes a tracked revision) | `taskpane.js:374-386, 445-456`; no TC-awareness anywhere | Spike **(m)** TC rows: annotate/apply with TC on, tracked deletions inside ranges | May need: detect TC state and either defer edits or apply through the TC-aware path; ledger's `paragraph_text_hash` correlation may need the tracked-text variant |
| A7 | `ctx.document.body.paragraphs` is the full checkable surface (tables/footnotes/headers/text boxes are out of scope for v1) | `taskpane.js:177-187` (`checkDocument`), same enumeration for annotation `:289-304` and apply `:374-386` | Spike **(m)** context rows | If body.paragraphs *includes* table cells, offset bases shift per context; if footnotes are editable but unchecked, scope must be stated in the UI. Either way: enumerate-or-document decision |
| A8 | Paragraph identity is stable as **"p" + enumeration index** between checks | `taskpane.js:191` (`const key = "p" + i`) with `lastKnownText` diffing | Spike **(d)** (drift) + an added observation: insert/delete a paragraph, re-check | If order shifts (insert/delete above), keys alias different paragraphs → stale `lastKnownText`, mis-annotated spans. Fix: key by content hash + position, or Word paragraph ids if exposed |
| A9 | `p.text` equals the text the engine checks (no hidden-text/platform surprises) | `taskpane.js:183-187`; getText options unprobed | Spike **(m)** (`getText({includeHiddenText,...})`) | If hidden text leaks into `p.text`, offsets and annotations drift; engine is agnostic (it checks what it is sent) |
| A10 | The WebView2/Chromium webview trusts our per-install self-signed cert for `https://localhost:<port>` after the user's manual Current User → Root install | `server.rs:702-757` (cert), `assist_commands.rs:384-411` (manual instructions) | Spike **(g)/(l)**: does the pane load over https without OS exemptions | If the webview refuses: Phase 3 fallbacks (port-agnostic `http://localhost` with the owner's explicit ruling, or a renewed cert story). Nothing auto-writes the trust store either way (D-5) |
| A11 | The only cross-origin fetch the pane makes is office.js from `appsforoffice.microsoft.com`, carrying no document text | `taskpane.html` (CDN script); declared D-6 | Spike **(h)**: request log/capture incl. the 1.7 popup "underlying service" | If popup actions phone Microsoft, that must be documented in SECURITY.md/NETWORK_SURFACE.json and may block the popup path by default |
| A12 | Annotation underlines re-anchor (or not) while typing — we re-render every check either way | `taskpane.js:284-309` (renderAnnotations per check) | Spike **(d)** | If Word leaves stale offsets, our re-render must first clear old annotations; if Word re-anchors, clearing must be selective to avoid flicker |

## Engine-internal assumptions (VERIFIED by tests, not by the spike)

| Fact | Where verified |
|------|----------------|
| Scalar↔UTF-16 round-trips losslessly (astral/Arabic/NBSP/CJK corpus) | `offsets.rs:110-199` (6 property tests) |
| Sentence splitter: abbreviation/decimal/URL/CJK/Arabic behavior | `sentences.rs` (9 tests) |
| S1/S2 protected-span handling and lossless mask/unmask | `mask.rs` (9 tests) |
| Six Tier 1 rejection rules + strictness profiles (E-1) | `validate.rs` (9 tests), golden harness (`tests/assist_golden.rs`, 8 tests) |
| Hash chain: append/verify/tamper-detection/link-commitment | `ledger.rs` (4 tests) |

## Register maintenance

When the owner's spike report lands: each row gets a **VERIFIED** mark
with the observed value, or its fix lands with a test. A1 ("only the
adapter changes") stays a hypothesis until then.
