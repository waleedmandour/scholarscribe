# Word Assist — engineering spec

Status: **implemented through Phase 3; Phase 4 (final defaults + benchmark
harness) pending the owner's spike run and gate.** Branch
`feature/word-assist`. Everything below compiles behind the
`word-assist` Cargo feature; the default build is byte-for-byte the
pre-feature build.

Rulings referenced as D-n (Phase 0 gate) and E-n (Phase 1a gate) per
`docs/DECISIONS.md`.

---

## 1. What it is

Word Assist shows as-you-type grammar and clarity feedback inside
Microsoft Word through a task-pane add-in served by ScholarScribe itself.
Two tiers:

- **Tier 0 — deterministic, offline.** The existing `language_check`
  harper engine (single source of truth, ruling D-4) lints each sentence;
  the assist layer filters the result by strictness profile (E-1),
  protected spans, and the language gate, then maps spans into the
  offset space Word speaks.
- **Tier 1 — local model, opt-in on top of the feature opt-in.** One
  sentence at a time goes to the local Ollama daemon with a versioned
  prompt and structured output; the rewrite is validated by six
  rejection rules before Word ever sees it. Precision over recall: a
  failed validation is "no suggestion", never an error dialog.

No text leaves the machine. The listener binds `127.0.0.1` only; the
add-in's single external fetch is the office.js library from Microsoft's
CDN (never carries document text; ruling D-6's `addin-webview` class).

## 2. Module map

| Module | Responsibility |
|---|---|
| `assist/offsets.rs` | Scalar↔UTF-16 prefix-map; surrogate-safe span snapping; round-trip property tests |
| `assist/sentences.rs` | Script-agnostic splitter (Latin/CJK/RTL), abbreviation + decimal protection, script-dominance language gate |
| `assist/mask.rs` | Protected spans (citations via `citation_manager`, URLs/DOIs, numbers, LaTeX, quotes); S1 filter / S2 mask strategies; lossless mask/unmask |
| `assist/diff.rs` | Word-level LCS diff → word-aligned edits with scalar offsets; whitespace-correct apply |
| `assist/validate.rs` | The six rejection rules; strictness profiles (E-1) |
| `assist/cache.rs` | Memory-only LRU cache keyed by SHA-256 of sentence text; tier-0 issues + tier-1 verdicts |
| `assist/tier0.rs` | Adapter over `language_check::Engine`; filtering; wire DTOs (both offset spaces) |
| `assist/engine.rs` | Versioned Tier 1 prompt + structured-output schema + response parser |
| `assist/server.rs` | Loopback TLS listener (hyper 1 + tokio-rustls, ring provider); auth, Origin/Host pinning, body cap, rate limit; serves `/v1/*` and the add-in pages |
| `assist/scheduler.rs` | Single-flight Tier 1 (newest wins, stale discard, per-paragraph budget); pairing-code/token CSPRNG helpers |
| `assist/settings.rs` | Runtime state: resolved settings, pairing, rate limiter, assist audit log |
| `assist/ledger.rs` | HMAC-chained JSONL ledger (Phase 3); counts and hashes, never text |
| `assist_commands.rs` | Tauri command surface (always compiled; stubs without the feature) |
| `word-addin/dist/` | Production add-in: manifest, task pane client (probe + annotations + popup actions + list-mode fallback) |

## 3. Strictness (ruling E-1, PROVISIONAL until the Phase 4 gate)

The shipped default is `dev`: **Grammar on, Spelling off, Clarity off.**
Rationale per E-1: Grammar's supports were unanimous with a Wilson
lower bound judged sufficient for a dev default; Spelling's false-positive
share was far above the 15% re-evaluation bar (and Word already underlines
misspellings itself); Clarity was too context-dependent for as-you-type.
No precision figures from the golden set appear in the UI or README (E-1).

| Profile | Grammar | Spelling | Clarity |
|---|---|---|---|
| `dev` (shipped default) | on | off | off |
| `conservative` | on | off | off |
| `balanced` | on | on | off |
| `thorough` | on | on | on |

Per-rule overrides from the user's `language_check_rules` apply on top of
every profile (single source of truth, D-4). Final defaults are a Phase 4
decision and must not be flipped in code before that gate.

## 4. The six rejection rules (Tier 1)

A model rewrite that fails any rule is dropped entirely (the annotation
simply does not appear):

1. **Schema** — the reply must parse as `{"revised": string, "explanation"?: string}`; fenced or prose-wrapped JSON is tolerated, anything else is not.
2. **Presence** — non-empty, and word-level different from the original.
3. **Length** — word count may not grow beyond 1.5× the original.
4. **Protected content** — every citation, URL/DOI, number, LaTeX span and quote of the original survives verbatim, AND the revision introduces no protected content the original lacked (a rewrite that adds a citation or a number is a hallucination).
5. **Script** — the dominant script may not change (CJK↔Latin mixing is tolerated).
6. **Sentence count** — exactly one sentence in, one sentence out.

## 5. Listener security model

- Bind `127.0.0.1:<port>` only (default 38080, below the Windows
  ephemeral range, ruling D-3). Bind failure is a visible error; no
  silent port walking.
- `Host` must be exactly `localhost:<port>`; `Origin` must be exactly
  `https://localhost:<port>` on every `/v1` route except `/v1/health`.
  No CORS headers are ever emitted.
- Pairing: a 6-character one-time code (unambiguous alphabet, 5-minute
  TTL, burned on use) exchanges for a 32-byte bearer token; both are
  compared in constant time. localStorage is per-origin, so a port
  change forces re-pairing (stated in the UI, per ruling 5's note).
- 64 KiB body cap; sliding-window rate limit; TLS mandatory (the http
  experiment lives only in the Phase 0 spike).
- Certificate: per-install self-signed leaf, SANs `localhost` +
  `127.0.0.1`, `serverAuth` EKU, generated with rcgen (ruling D-2). The
  private key goes to the OS keychain (`scholarscribe` /
  `word-assist-tls-key-v1`) or, when unavailable, a 0600 file — both
  disclosed in the UI. **Nothing ever writes to the system trust store
  by itself** (ruling D-5 / 4c): the registration command only shows the
  certificate and the manual install steps; the trust decision is the
  user's, performed in the OS dialogs.

## 6. Ledger (Phase 3)

`<data>/word-assist/ledger.jsonl`, append-only, HMAC-SHA256-chained with
a per-install key (`word-assist-ledger-v1` in the keychain, 0600 file
fallback). Entries carry sequence, timestamp, engine version, kind
(accepted / rejected / failed / paired), paragraph-id hash, issue id,
rule id, category, span offsets, replacement length, and the add-in's
pre-edit paragraph hash — **hashes and counts, never text**
(PROVENANCE_SPEC §0.3). The chain is verified on every open; a broken
chain disables the feature rather than running unrecorded. The
Disclosure Assistant pre-fill (`word_assist_disclosure_prefill`) turns
the ledger into a counts-only sentence the user may paste into a
disclosure statement; it makes no quality claims and never mentions
detectors (E-1).

## 7. Wire API (loopback, all under `/v1`)

| Route | Auth | Purpose |
|---|---|---|
| `GET /v1/health` | none | engine version, paired state, strictness |
| `POST /v1/pair` | pairing code | one-time code → bearer token |
| `POST /v1/check` | bearer | paragraph text → tier0 issues + tier1 status per sentence |
| `POST /v1/accepted` | bearer | accepted-edit report → audit + ledger |

Tier 1 never blocks a check response: a pending sentence comes back as
`{"tier1": {"state": "pending"}}` and the add-in re-posts the same
paragraph (tier 0 is cached, so the re-poll is cheap) up to three times.

## 8. Add-in client

Plain JavaScript with `// @ts-check` + JSDoc (editor-time strictness
without adding a bundler to the repo — recorded in DECISIONS.md).
Capability probing is per requirement set: WordApi 1.6 (paragraph
events), 1.7 (annotations), 1.8 (popup actions). Anything missing
degrades to a list-mode panel with per-suggestion Apply buttons; nothing
hard-fails on older hosts. The offset ADAPTER (`toCritique`) is the
single place to adjust when the owner's spike run (experiment (a) in
`word-addin/spike/OBSERVATIONS.md`) pins down Word's exact critique
units. Self-edits are absorbed by a suppression watermark so the
accept→apply→event loop cannot oscillate (risk R3), plus the server-side
per-paragraph Tier 1 budget.

## 9. Testing

- Unit tests in every module (offsets round-trips, splitter edge cases,
  mask/unmask round-trip, diff whitespace handling, validator rules,
  cache LRU, scheduler single-flight/budget, ledger chain + tamper
  detection, PEM encoder).
- Golden set: `src-tauri/tests/assist/golden.json` (~50 scholarly
  sentences across citation/number/latex/url/quote/plain/Arabic/CJK/
  emoji/edge classes) driven by `tests/assist_golden.rs`: protected
  content is never flagged, the dev profile emits grammar only, Arabic
  and CJK gate correctly, ASCII offsets coincide in both spaces.
- CI: the `word-assist-gates` job runs fmt, clippy (`-D warnings`) and
  the full test suite **with the feature on**, plus a default-build
  check proving the feature stays opt-in (ruling D-1).

## 10. What Phase 4 still owes

1. The owner's spike run (a)–(m) — required before the add-in is
   enabled by default; the ADAPTER may need one adjustment.
2. Final strictness defaults (E-1's provisional `dev` stands until
   then), benchmarked against an enlarged golden set.
3. Trust-store consent: the Phase 3 registration flow stays manual until
   the owner explicitly approves an in-app write (ruling D-5).
4. Third-party license refresh on the release line
   (`scripts/generate-third-party-licenses.mjs` — new crates: rcgen,
   hmac and their X.509/HMAC stacks, all MIT OR Apache-2.0).
