# Stage B pre-plan — spike items → outcomes → code, tests, user-visible behaviour

Status: **PLAN ONLY** (owner's Gate-1 item P8). No Word-dependent logic is
touched by this document. It exists so that when the owner pastes a filled
`word-addin/spike/OBSERVATIONS.md` (items a–r), every implementation branch is
already scoped: files, functions, tests, and what the user would see.

Baseline for all file references: integration tip at the time of writing
(`66b1bf6`, 2026-10-24). The assumptions register (`docs/WORD_ASSIST_ASSUMPTIONS.md`)
is updated in EVERY branch: each row relevant to an item flips VERIFIED (or is
downgraded with the documented fallback, per Definition-of-Done D4).

Per work order §8: "Where it says STOP, stop and ask." "No Mac results means
Mac rows stay NOT VERIFIED."

---

## (a) Offset units — UTF-16 code units vs code points

Outcome A1: Word annotates with **UTF-16 code-unit** indexing (`string.length` semantics).
- Code: `src/assist/offsets.rs` — keep the UTF-16 mapper as the single truth;
  confirm `src/assist/sentences.rs` and `src/assist/mask.rs` emit UTF-16
  indices; the add-in client passes indices through unchanged.
- Tests: extend the existing offset tests with parametrised cases (astral
  emoji 👍🏽/🎉, Arabic, combining marks, NBSP) asserting index == UTF-16 unit;
  add-in-side tests stay as-is.
- User-visible: none (correctness holds as already designed).

Outcome A2: Word annotates with **code-point** indexing.
- Code: `src/assist/offsets.rs` — add the code-point branch of the adapter
  (the mapper is designed for exactly this switch); `word-addin` client gains
  the conversion step per the adapter's decision; `src/assist/scheduler.rs`
  payload builder switches on the adapter flag.
- Tests: same parametrised suites, now asserting the conversion (emoji shifts
  each subsequent index); E2E: one mocked-critique round-trip with astral
  characters.
- User-visible: underlines land on the intended word for emoji/Arabic text
  instead of drifting by one position per astral character.

## (b) Who applies a popup accept

Outcome B1: **Word applies the replacement itself.**
- Code: `word-addin` popup-action handler (taskpane client): on
  `onAnnotationPopupAction`, do NOT edit the document; only delete/clear the
  annotation id, update session state, and notify the ledger. Any
  "apply-then-delete" double-write path is deleted (dead code removal is
  explicitly required by work order §8 (e) pattern).
- Tests: add-in tests asserting exactly one side effect per accept; simulated
  response tests (the existing pairing/session test suite) gain an
  "apply-popup" case.
- User-visible: clicking a suggestion replaces the text exactly once (no
  double replacement, no leftover underline).

Outcome B2: **Word only fires the event; the add-in must apply.**
- Code: add-in applies the replacement via `document.body.insertText`-style
  range edit bounded by the (a) adapter's units, exactly once, then clears the
  annotation; `src/assist/diff.rs` unchanged (word-diff already produces the
  replacement).
- Tests: add-in apply-once test (idempotence: a second event for the same id
  is a no-op); E2E: mocked popup-action → text change → ledger entry.
- User-visible: same as B1 (single replacement), plus a brief "applied" state
  in the pane history.

## (j) Annotation↔id correlation

Outcome J1: a mapping mechanism exists (Word-assigned id reachable from our
insert + events).
- Code: `word-addin` session state stores critique-id → Word-annotation-id;
  `src/assist/ledger.rs` records the correlation on accept/reject.
- Tests: correlation unit tests in the add-in suite; ledger round-trip test.
- User-visible: the right suggestion card highlights when its underline is
  clicked; "applied/rejected" history lines up with what the user clicked.

Outcome J2: **no mechanism exists → STOP and ask** (work order §8 (j)).
- Code: none; report includes the exact API behaviours observed.
- User-visible: unchanged; the owner decides between a redesign (e.g.
  selection-based anchoring) or deferral.

## (e) Self-events (do our own actions fire ParagraphChanged/AnnotationInserted?)

Outcome E1: our own writes fire events.
- Code: suppression watermark in the add-in client — a short-lived
  (monotonic, per-connection) watermark that ignores events caused by our own
  apply; dead code from any alternative suppression attempt deleted.
- Tests: add-in tests: apply → next ParagraphChanged ignored → subsequent
  user edits processed; watermark expiry test.
- User-visible: applying a suggestion does not spawn a duplicate suggestion
  or an echo loop.

Outcome E2: our writes do NOT fire events (or events are distinguishable).
- Code: no watermark; document the guarantee; keep the (still simpler)
  suppression path out of the client.
- Tests: regression test asserting no suppression logic is needed (documents
  the observation).
- User-visible: none (simplest behaviour confirmed).

## (f)/(k) Requirement sets per OS

Outcome FK1: annotations supported on the tested OS(es) (WordApi 1.7+ present).
- Code: none beyond docs; `docs/QA_v0.1.md` and the install notes state the
  supported matrix.
- Tests: none new.
- User-visible: full underline + popup experience on supported Word builds.

Outcome FK2: annotations unsupported (requirement-set check fails on an OS).
- Code: add-in falls back to **list-mode only** (suggestions listed in the
  pane, no annotations) behind the existing capability check; the pane shows
  a one-line banner "This Word build doesn't support inline suggestions —
  showing a list instead"; `docs/WORD_ASSIST_SPEC.md` and install notes state
  which OS is list-mode only (work order: "stated in UI and docs").
- Tests: add-in capability-detection tests (mocked `Office.context.requirements`);
  E2E banner path.
- User-visible: read-only list of suggestions on unsupported builds; no
  broken annotations.

## (g)/(l) Plain HTTP acceptance + webview runtime

Outcome GL1: HTTPS+trusted CA loads in the pane (expected for the shipped
listener) — RECORD ONLY.
- Code: none (work order: "record. Do not tighten the TLS floor without owner
  approval"). `docs/WORD_ASSIST_ASSUMPTIONS.md` webview row → VERIFIED;
  findings appended to OBSERVATIONS + EVAL notes.
- User-visible: none.

Outcome GL2: the pane fails even with a trusted CA (webview refuses
name-constrained roots, etc.).
- Code: none in this branch; **STOP and ask** is not automatic here — the
  work order only says record — but the report must propose options (e.g.
  explicit trust dialog copy change, or an owner-approved TLS-floor change)
  before any change.
- User-visible: unchanged pending the owner's decision.

## (h) Network capture (does document text leave the machine via annotations?)

Outcome H1: only Word's known traffic + the office.js CDN fetch appear.
- Code: none. Docs: `docs/RELEASE_NOTES_v0.1_DRAFT.md` privacy section gains
  the precise statement (the current text already anticipates this: "anything
  else only once spike item (h) is in"); `docs/NETWORK_SURFACE.json` gains
  the observed hosts with class + trigger; `docs/SECURITY.md` updated to
  match (its CI cross-check enforces this — `scripts/check-network-surface.mjs`).
- Tests: network-surface test keeps CI green after the JSON update (the check
  requires each new host to be documented).
- User-visible: none in the app; the published privacy statement becomes
  capture-backed (D6).

Outcome H2: **document text leaves the machine because of our annotations →
STOP** (work order §8 (h)).
- Code: none; report the exact host, payload evidence, and the decision
  needed (e.g. disable annotation popups by default or add an explicit
  user consent gate).
- User-visible: unchanged until the owner decides.

## (m) Contexts: tables, footnotes, headers, text boxes, Track Changes

Outcome M1: annotations work in body + tables; some contexts reject/misplace.
- Code: `src/assist/mask.rs` + `src/assist/offsets.rs` learn the per-context
  offsets the spike measured (footnote/header ranges arrive with different
  bases); the add-in only inserts annotations in contexts marked supported;
  `src/assist/scheduler.rs` filters critiques for unsupported contexts.
- Tests: mask/offset unit tests per context fixture (table cell, footnote,
  header, content control); Track Changes ON with a tracked deletion inside
  the range (offset vs `getText` options — ties to item (m) getText support);
  E2E: a table-cell round-trip.
- User-visible: suggestions appear where they are reliable; contexts that are
  not covered are silently skipped in v0.1 and **documented** in
  `docs/WORD_ASSIST_SPEC.md` + the pane's small "coverage" note.

Outcome M2: annotations only work in the body.
- Code: scheduler restricts to body ranges (mask/offset work already scoped);
  docs state the v0.1 coverage explicitly.
- Tests: E2E asserts non-body critiques are filtered out.
- User-visible: only body text is checked; the pane says so.

## (o)/(p) Colours and docs tuning

- Code: add-in styles (popup contrast in dark/light Word themes, per
  observation); `docs/USER_GUIDE`/QA matrix wording.
- Tests: existing add-in dist-integrity test re-run (npm test) after asset
  changes.
- User-visible: readable suggestion cards in both themes; documentation
  matches what the user actually sees.

## (q) Name-constrained CA in Word's pane and Edge (+ q2 negative test)

Outcome Q1: both runtimes honour the constrained CA (expected candidates say
"load WITHOUT warnings").
- Code: none. Docs: `docs/WORD_ASSIST_P6_REVIEW.md` §"spike (q) untested"
  caveat resolved; install notes drop the corresponding caution line.
- Tests: the existing x509-parser constraint tests stay as the regression
  net; owner's OBSERVATIONS entry is the human evidence.
- User-visible: none (pane loads without warnings — already the design).

Outcome Q2: **a target OS ignores the constraints → STOP and report the
blast radius** (work order §8 (q)).
- Code: none; the report must quantify it: the trusted CA could mint certs
  for names outside localhost on that OS, and `leaf-other-name.pem` (q2)
  demonstrates it; owner picks between (i) shipping with the OS-specific
  warning, (ii) per-OS manifest using `127.0.0.1` literals, or (iii) not
  shipping Word Assist on that OS.
- User-visible: unchanged until decided.

## (r) Does the webview resolve localhost to ::1 first?

Outcome R1: IPv4 only (expected; Host/Origin pins unaffected).
- Code: none; `docs/WORD_ASSIST_ASSUMPTIONS.md` loopback row → VERIFIED.
- User-visible: none.

Outcome R2: ::1 is attempted first and fails.
- Code (either, owner-approved): (i) the manifest/taskpane URLs switch to
  `https://127.0.0.1:38080` (Host/Origin pins already compare the literal
  name — they would be updated in `src/assist/server.rs` Host/Origin checks
  and the E2E), or (ii) the listener binds dual-stack and the pin set accepts
  both names. Existing tests: `tests/word_assist_e2e.rs` Host/Origin pinning
  cases updated accordingly; `src/assist/server.rs:322-324` port-pin logic.
- User-visible: the pane loads on hosts where `localhost` is ::1-first; docs
  note which address form the manifest uses.

---

## Cross-cutting (every Stage B branch)

- `docs/WORD_ASSIST_ASSUMPTIONS.md`: rows touched per above; nothing stays
  "assumed" once an observation lands (D4).
- Tests: no test may be weakened or skipped for a Stage B outcome; parametrised
  offset suites grow with the observed real-world cases (emoji, Arabic,
  combining marks) regardless of outcome A1/A2.
- Stage B ends with a Gate 2 report (summary, labelled commits, acceptance
  against work order §11, discrepancies, owner actions, risks) and STOP.
