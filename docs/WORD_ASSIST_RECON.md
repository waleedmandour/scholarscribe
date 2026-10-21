# Word Assist. Phase 0 Reconnaissance (feature/word-assist)

Branch: `feature/word-assist`, created from `main` @ `04b471975bdb73e11852487b1c24e5c4a16354f3` (recorded at branch creation).
Status at this document's commit: **Phase 0 complete. No feature code. No existing module touched. No dependency added. No Cargo.toml change.**

Tag legend used throughout, per the owner's protocol:

- **VERIFIED** — I reproduced it from repo files, the local Cargo registry cache, or official docs; evidence cited inline.
- **INFERRED** — a conclusion drawn from verified facts; the reasoning is stated.
- **NOT VERIFIED** — could not be established from here; needs the spike, a Windows machine, or docs I could not reach.
- **NEEDS HUMAN VERIFICATION** — requires Word/Microsoft 365 or an owner-run procedure; exact steps in `word-addin/spike/OBSERVATIONS.md`.

---

## 1. Governance check (ETHICS.md, PROVENANCE_SPEC.md)

- **VERIFIED** — ETHICS.md §4 "Features that fit" explicitly welcomes "Writing aids: paraphrasing the author's own sentences, suggesting alternative phrasings, critique, outlining", and §2's exclusions (evasion, detector-score loops, misrepresentation, citation fabrication) do not touch an accept-per-suggestion language checker. The Tier 2 rewrite idea stays within "paraphrasing the author's own sentences". No conflict found; nothing requires a policy change.
- **VERIFIED** — PROVENANCE_SPEC.md §0.3 and §11 set the pattern the assist ledger must follow: hashes and counts, never text; audit-log entries for anything that touches user content; opt-in persisted in `settings.json` (precedent keys: `provenance_enabled`, `persistence_enabled` in `src-tauri/src/persistence.rs:30-53`).
- **INFERRED** — the assist ledger can be an append-only JSONL file next to the app data (`data/` dir, `persistence.rs:23-25` conventions) with HMAC-SHA256 over each entry using a **separate** per-install keychain entry (pattern: `provenance_commands.rs:36-71`, keychain service `scholarscribe`, entry `provenance-signing-key-v1`). Reusing the provenance signing key would couple two features; a distinct `word-assist-hmac-key-v1` entry is proposed. This follows the "provenance key-handling pattern" the brief mandates; nothing in PROVENANCE_SPEC.md forbids a sibling ledger (§2/§6 are about the canonical manifest, which we do not touch).

## 2. Repo baseline (recorded exactly)

Environment note: this recon was produced in a sandbox without root; the Tauri Linux system libraries could not be installed (exact error recorded below). Frontend and fmt gates were run locally; the Rust compile gates were corroborated by the repo's own CI, as noted.

| Gate | Result | Evidence |
|---|---|---|
| `cargo fmt -- --check` | **PASS** (exit 0) | Run locally on the branch, Rust 1.99.1, clippy 0.1.99. |
| `cargo check` | **FAIL — environmental, pre-existing** (exit 101) | `gdk-sys` build script: "Package gdk-3.0 was not found in the pkg-config search path … The system library `gdk-3.0` required by crate `gdk-sys` was not found." Sandbox has no sudo; `apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev …` (the exact set CI installs, `.github/workflows/ci.yml:49-52`) is not possible here. Not a code regression. |
| `cargo clippy -- -D warnings`, `cargo test` | **NOT VERIFIED locally** — blocked by the same missing system libs | Corroboration **INFERRED**: GitHub Actions run #204 (CI workflow, dependabot/cargo/src-tauri/tauri-2571db8166, base = our SHA 04b4719, only the tauri crate bumped) concluded **success** on 2026-10-19, so the full matrix (fmt, clippy -D warnings, cargo test, cargo check, npm check/build/test, network surface) passes on code identical to `main` except one dependency patch. A separate crypto-bump PR (run #203) failed CI — unrelated to this branch, recorded as a pre-existing dependency-bump issue. |
| `npm run check` (svelte-check) | **PASS** — 0 errors, **11 pre-existing warnings** in 2 files | `src/components/WritingJournal.svelte:5` (1 warning) and `src/components/AppealLetter.svelte:50-64` (10 warnings). Pre-existing on `main`; not ours to fix. |
| `npm run build` | **PASS** (vite, 889 ms) | Local run. |
| `npm test` (5 JS checks) | **PASS** — references OK, network-surface OK ("24 distinct hosts found in source, all declared; 10 entries cross-checked against SECURITY.md"), cross-verify fixture OK, svelte-mount OK, tauri parity OK | Local run. |

- **VERIFIED** — versions in agreement: `Cargo.toml` version 2.2.1, `tauri.conf.json` 2.2.1, `package.json` 2.2.1, README badge "Status: v2.2.1 (stable)". The owner's brief asserted "README says v2.1.0; code is v2.2.1" — **I could not reproduce that staleness**: the only "v2.1.0" strings in README.md are the *correct* "(new in v2.1.0)" Provenance mentions. Two real (cosmetic) README defects found instead: broken harper links rendered as `arper](…)` at lines 237 and 558. Left untouched (no unrelated edits).
- **VERIFIED** — CI structure (`.github/workflows/ci.yml`): triggers only on push/PR to `main`. A push to `feature/word-assist` will not run CI. Consequence and proposal in §10/R1.
- **VERIFIED** — the network-surface gate (`scripts/check-network-surface.mjs`) scans host-literal URLs in `src/`, `src-tauri/src/`, `verifier/`, plus `index.html`/`tauri.conf.json`/capabilities, and fails CI on any undeclared host; `.xml` is not scanned. The spike under `word-addin/` therefore cannot trip it, but every Phase 1b file that names a host literal must be accompanied by a `docs/NETWORK_SURFACE.json` (+ SECURITY.md) update in the same commit.

## 3. Reusable in-repo APIs (real signatures, read from source)

All signatures below are quoted from the working tree of this branch.

### 3.1 `language_check.rs` — the Tier 0 engine (owner ruling 1: reuse, never re-implement)

- `pub const SUPPORTED_DIALECTS: [&str; 5] = ["American", "British", "Canadian", "Australian", "Indian"]` and `pub fn parse_dialect(name: &str) -> Result<Dialect, String>` — the dialect setting Word Assist must share (ruling 1: "expose the dialect setting you already have").
- `pub enum IssueCategory { Spelling, Grammar, Style }` (`from_lint_kind` maps every harper `LintKind`, with a documented catch-all into Grammar).
- `pub enum SuggestionDto { ReplaceWith{ text }, InsertAfter{ text }, Remove }` — serde-tagged, mirrors harper `Suggestion`; `Suggestion::ReplaceWith(Vec<char>)` in harper, converted via `text.chars().collect()` / `chars.iter().collect()`.
- `pub struct Issue { id: u32, category, lint_kind: String, rule_id: String, span_start: usize, span_end: usize, problem_text: String, message: String, description: Option<String>, suggestions: Vec<SuggestionDto>, priority: u8 }` — offsets documented in-source as "indices into the character sequence of the input (Unicode scalar values, end exclusive)".
- `pub fn build_engine(dialect: Dialect, user_words: &[String]) -> Engine` — builds `MergedDictionary` = `FstDictionary::curated()` + `MutableDictionary` of personal words, then `LintGroup::new_curated(dict, dialect)`; also captures `group.all_descriptions()`.
- `impl Engine { pub fn apply_rules(&mut self, rules: &RuleMap); pub fn check(&mut self, text: &str) -> CheckOutcome }` — `check` sorts lints by position/priority, drops overlaps keeping higher priority, assigns dense ids, returns `CheckOutcome { issues, counts, text_length }` where `text_length = text.chars().count()`.
- `pub type RuleMap = BTreeMap<String, Option<bool>>` and `pub fn curated_rule_map() -> RuleMap` (from harper's `FlatConfig::new_curated()`) — the persisted per-rule override format.
- `pub fn check_text(text: &str, rules: &RuleMap, engine: &mut Engine) -> CheckOutcome` and `pub fn apply_edits(text: &str, edits: &[SuggestedEdit]) -> Result<ApplyOutcome, String>` (rightmost-first application, validate-before-mutate).
- State layer: `LanguageCheckState` with an `EngineCache` (one cached engine for the most recent dialect, `Mutex`-guarded; dialect change rebuilds; personal dictionary loaded via `persistence::settings_get` / `language_check_dictionary_path`), and `#[tauri::command]`s `language_check_run/apply/dictionary_*/settings_*` registered in `lib.rs:83-91`.
- **Thread-safety today (owner ruling 1 asked for evidence, not assertion)**:
  - **VERIFIED** — the crate's own manifest (fetched registry copy, `~/.cargo/registry/src/index.crates.io-*/harper-core-2.11.0/Cargo.toml`) declares `license = "Apache-2.0"`, and `[features] concurrent = ["dep:boxcar"]`, `default = ["thesaurus", "concurrent"]`. Our `Cargo.toml:48` pins `harper-core = "=2.11.0"` with **no** `default-features = false`, so **`concurrent` is already enabled**. The owner's working assumption ("concurrent is off by default") does not hold for 2.11.0 — the feature is on by default and on in our build. **No Cargo.toml feature change is needed or proposed**; if a future version flips the default, the pin already forces a deliberate review.
  - **VERIFIED** — the shared-module pattern wraps the engine in a `Mutex` (`EngineCache` in `language_check.rs:354-427`, `lock_cache_state` helper): engine access is serialized today regardless of the `concurrent` feature. Word Assist's listener will reuse the same discipline (one engine cache, one lock, short critical sections), so lints are computed off the request path only where the `concurrent` feature allows shared reads.
- **Span unit (owner ruling 1: expected Unicode scalar, unverified — now verified)**: **VERIFIED** — harper `Document` stores `source: Lrc<[char]>` (`harper-core-2.11.0/src/document.rs:19,55,63`), and `Span<T> { pub start: usize, pub end: usize }` with doc comment "represents an exclusive range" (`span.rs:19-26`). Spans index **Unicode scalar values, end-exclusive**. This matches `language_check.rs`'s own doc comment and `text.chars().count()`. Direct consequence for Q3: harper offsets are **not** Word's expected offsets (JS `string` indices = UTF-16 code units); the conversion lives in the 1a offsets module, not in `language_check.rs`.

### 3.2 Settings, audit, keyring, disclosure, citations

- `persistence::Settings` (`persistence.rs:30-53`): `persistence_enabled`, `provenance_enabled`, `theme`, `language_check_dialect` (default "American"), `language_check_rules: BTreeMap<String, Option<bool>>`, `version`. New assist settings follow the same `#[serde(default …)]` additive pattern (see Q-plans). `data_dir(app)` = `app_data_dir()/data`; drafts dir sibling; dictionary persisted only when `persistence_enabled`.
- `audit::AuditLog::record(&self, kind: &str, target: &str, detail: &str, bytes_in: u64, bytes_out: u64)` (`audit.rs:30`); kinds in use: `"file_read" | "http_call" | "ollama_command"`. In-memory only, capped at 1000 entries. Word Assist adds kinds without touching this file: the detail strings carry counts only ("assist: paragraph read (N chars)"), never text — **VERIFIED** that `AuditEntry` has no content field that would tempt otherwise.
- `commands::ollama_chat` (`commands.rs`) shows the command pattern: `State<OllamaState>` + `State<AuditLog>`, audit first, then `ollama::chat(state.inner(), request).await`.
- `ollama::ChatRequest` (`ollama.rs:225-232`): today `model`, `messages`, `stream` (default false), `temperature` (default 0.7). `chat()` posts `{model, messages, stream:false, options:{temperature}}` to `format!("{}/api/chat", OLLAMA_BASE)` with a hard 300 s timeout; `OLLAMA_BASE` is a `const`, and `base_url()` (`#[allow(dead_code)]`) returns it. Owner ruling D3 (approved): additive `Option` fields + `skip_serializing_if`, and the smallest change making the base URL injectable — its own commit in 1b, existing callers untouched.
- `citation_manager::extract_in_text_citations(text: &str) -> Vec<InTextCitation>` (`citation_manager.rs:319`), `parse_bib`, `validate`, `validate_citation_contexts` — reusable for masking citation spans and for Tier 0's citation-format-consistency rule. **INFERRED**: the citation-format-consistency Tier 0 rule (APA/numeric consistency per paragraph) is a thin pass over `InTextCitation` spans + a style-classifier (author-year vs. bracket-numeric); details in 1a.
- `disclosure::generate(input: &DisclosureInput) -> Result<DisclosureOutput, String>` (`disclosure.rs:97`) — the Phase 3 pre-fill point ("tool: local LLM; task: language editing; count of accepted suggestions") extends `DisclosureInput` additively.
- Keyring pattern (`provenance_commands.rs:36-71`): `keyring::Entry::new(service, user)`, NoEntry → `getrandom::getrandom` 32 bytes → hex → `set_password`, seed wiped from stack. Reused verbatim for the assist HMAC key (separate entry name).
- `google_docs_commands.rs:118-200` is the **actual** loopback-listener precedent (not `google_docs_net.rs`): `TcpListener::bind("127.0.0.1:0")`, nonblocking accept loop with a deadline inside `tauri::async_runtime::spawn_blocking`, hand-written minimal HTTP response, `state`-checked redirect. **VERIFIED** — `google_docs_net.rs` is a Drive/OAuth *client* (URL builders + reqwest calls), with no listener code.

### 3.3 Frontend conventions

- **VERIFIED** — `src/App.svelte:30`: `type Tab = "models" | … | "about"` union; tabs are `{id, label, icon, group}` entries rendered from a `tabs` array; adding `word-assist` means one union member + one entry + one conditional component block. `src/lib/api.ts` (987 lines) is a typed `invoke` wrapper, one interface per backend DTO; assist additions follow it.
- **VERIFIED** — `src/components/LanguageCheck.svelte` exists as the settings-UX sibling (dialect selector, rule toggles, personal dictionary) for the `WordAssist.svelte` tab to reuse stylistically.
- **VERIFIED** — Tauri CSP (`tauri.conf.json:25`) `connect-src 'self' http://127.0.0.1:11434 http://localhost:11434`. The ScholarScribe webview does **not** need to talk to the assist listener (settings flow through Tauri commands), so no CSP change is anticipated for the app itself; the *add-in's* CSP is a separate document owned by Phase 2.

## 4. External API verification (Office.js / Ollama / packaging)

| # | Claim | Status | Evidence |
|---|---|---|---|
| V1 | `Paragraph.insertAnnotations`, `Word.Critique {start, length, colorScheme}` in **WordApi 1.7** | VERIFIED (docs) | WordApi 1.7 requirement set page lists `Annotation {critiqueAnnotation, delete(), id, state}`, `AnnotationSet.critiques`, `Critique` etc.; `Word.Paragraph` 1.7 notes carry the M365-subscription warning. |
| V2 | **Annotation APIs require a Microsoft 365 subscription** (an underlying service) | VERIFIED (docs) | Explicit "Important" note on the 1.7 requirement set and `Word.Paragraph` pages, referencing office-js issue #4953. Consequence: perpetual/LTSC Word never gets annotations → fallback mode is a first-class path, not an edge case. |
| V3 | Popup options in **WordApi 1.8**: `Critique.popupOptions`, `CritiquePopupOptions {suggestions, titleResourceId, subtitleResourceId, brandingTextResourceId}`, `Document.onAnnotationPopupAction`, `AnnotationPopupActionEventArgs {id, action, critiqueSuggestion}` | VERIFIED (docs) | WordApi 1.8 "What's new" page. Exact `AnnotationPopupAction` enum values: NOT VERIFIED (fetch did not include the enum; spike (b)/(j) records them at runtime). |
| V4 | `Word.Document.onParagraphChanged` exists in **WordApi 1.6** | VERIFIED (docs) | `Word.ParagraphChangedEventArgs`: "API set: WordApi 1.6 … Used by Word.Document: onParagraphChanged." The brief said 1.7; it is 1.6 — two independent floors must be probed (`isSetSupported('WordApi','1.6')` for events, `1.7` for annotations). |
| V5 | `Critique.start/length` units | NOT VERIFIED (docs deliberately silent) | Doc text: "Specifies the start index of the critique inside paragraph" — no unit given. JS-string semantics suggest UTF-16 code units; **spike (a) decides empirically**. Engine-internal representation is Unicode scalars (harper), so the 1a offsets module owns the scalar↔UTF-16 mapping either way. |
| V6 | Who applies a popup suggestion | INFERRED (high confidence) | No auto-apply method exists anywhere in 1.7/1.8; `onAnnotationPopupAction` hands the add-in `critiqueSuggestion` — the add-in applies it (annotation → range → text edit), then re-issues annotations for the paragraph. Spike (b) confirms. |
| V7 | `SourceLocation` must be HTTPS | VERIFIED (docs) | Manifest reference, SourceLocation element: "The source location must be an HTTPS address, not a file path." No localhost exception documented. Confirms owner correction 4a: **design for HTTPS**; http is a spike experiment only. |
| V8 | office.js distribution | VERIFIED (docs/npm) | Supported route is the Microsoft CDN (`https://appsforoffice.microsoft.com/lib/1/hosted/office.js`); the npm mirror `@microsoft/office-js` states "The NPM package associated with this repo is no longer officially supported". One outbound GET from the add-in webview at pane load; payload is the API library, never document text; Microsoft receives IP/UA as any HTTPS client does. Disclosed in the opt-in dialog + README (ruling 5, C6). |
| V9 | Ollama `/api/chat` supports `format` (JSON schema), `think` (bool), `keep_alive`, `options.{temperature,num_ctx,num_predict}`, `stream:false` | VERIFIED (docs) | docs.ollama.com/api/chat parameter reference. Caveat carried forward: for `think`, docs say "Use /api/show to discover the supported values and default for the selected model" — the 1b engine tests `think:false` against the daemon's Ollama version and degrades gracefully (omit the field on older daemons). |
| V10 | Word on Windows webview runtime + whether `localhost` is reachable from it without an OS loopback exemption | NOT VERIFIED | Requires a live Word install; recorded as spike (l) (owner ruling 4d). |
| V11 | Does Windows honor name-constrained self-signed certs in the CurrentUser Root store for WebView2? | NOT VERIFIED | No authoritative public doc found; treated as empirical. Design therefore does not *depend* on name constraints: a per-install **self-signed leaf with SAN `localhost` + `127.0.0.1`** and `serverAuth` EKU is scoped by construction (it never signs anything else). Name constraints, if Windows honors them, are defense-in-depth (owner ruling 4c). |
| V12 | `WordApiDesktop` sets (e.g. 1.2+) availability on perpetual Office | NOT VERIFIED | Affects the fallback's polling design; spike (k) records what the owner's Word actually reports. |

## 5. Discrepancies between the brief and the repo (superset of the earlier review; all now closed or carried)

1. **D1 (closed)** — `language_check.rs`/harper-core existed but was absent from the brief's file list; Q5 rewritten per ruling 1 (see §6).
2. **D2 (closed)** — the loopback-listener precedent is `google_docs_commands.rs`, not `google_docs_net.rs` (§3.2). The precedent binds `127.0.0.1:0` (ephemeral) — correct for OAuth (dynamic redirect URI), unusable for a static manifest; Q1's premise stands.
3. **D3 (closed)** — `ChatRequest` lacked `format`/`think`/`keep_alive`/`options` and the base URL is a `const`; ruling D3 approved the additive fix in 1b.
4. **D4 (closed)** — NETWORK_SURFACE.json + SECURITY.md sync belongs to the listener commit (ruling D4).
5. **New** — `onParagraphChanged` is WordApi **1.6**, not 1.7 (V4); the add-in probes two floors.
6. **New** — the M365-subscription requirement for annotations (V2) is stronger than the brief's "if annotations are unavailable (no Microsoft 365)" aside: it is a documented hard dependency of the entire annotation path.
7. **New** — harper's `concurrent` default contradicts the owner's working assumption (§3.1); net effect is positive (thread-safe already, no Cargo.toml change).
8. **New** — README "v2.1.0 staleness" does not reproduce (§2); actual README defects are two broken harper links.
9. **New** — `openssl`/`native-tls` enter the tree only through reqwest's default-TLS chain (`cargo tree -i openssl`); on Windows native-tls maps to schannel, so the `openssl` crate is **not** a usable cross-platform cert generator here. Feeds the dependency proposal (§7).

## 6. Open design questions — proposals (none decided unilaterally)

### Q1 — Port, discovery, pairing

**PROPOSAL (carried to the gate; owner ruling 5 keeps it undecided).**
- **Port:** fixed default **38080** (`http://localhost:38080`), user-configurable in the assist settings. 38080 is below Windows' ephemeral range (49152–65535, so the OS will not hand it out as an ephemeral port and collide with us — **INFERRED** from the documented default dynamic range), unassigned in the IANA register per a registry check to be re-verified in 1b (**NOT VERIFIED** until that check is quoted into the spec). Startup collision handling: try to bind; on `AddrInUse`, report a clear, actionable error (another assist instance or a foreign process) and offer the settings page — no silent port walking, because a silent move breaks the static manifest.
- **Discovery:** the add-in's SourceLocation is static (`https://localhost:38080/addin/taskpane.html`, served by the engine). Changing the port regenerates the manifest — coherent, because registration is already a consent-gated Phase 3 flow.
- **Pairing/token:** the engine serves the task pane, so the page can authenticate itself **only** by something the user mediates. Proposal: the ScholarScribe app shows a 6-character one-time pairing code while the pane shows a matching prompt; the pane exchanges the code (POST `/v1/pair`) for a bearer token bound to that session; the token lives in WebView2 `localStorage` and in the engine's memory. **Owner ruling 5 note, documented:** `localStorage` is per-origin *including port*, so a port change invalidates the token and forces re-pairing — this is acceptable and will be stated in the UI when a port change is saved. Constant-time compare everywhere (no new dependency: `subtle`-style constant-time equality is ~10 lines and not a cipher — but if the owner prefers a crate, see §7).
- **Origin/Host defence:** strict equality against the single allowed origin (`https://localhost:38080`); Host must be `localhost:38080`; no wildcard CORS because add-in page and API share the origin (ruling 4c).

### Q2 — HTTPS and certificate trust

**PROPOSAL.**
- Server: TLS is **not** hand-rolled; the server is built from crates already in Cargo.lock (§7, option A). Served: the add-in pages and the `/v1/*` API on the **same origin** (no CORS surface at all).
- Certificate: per-install **self-signed leaf**, SANs `localhost` + `127.0.0.1`, `serverAuth` EKU, generated at first opt-in via the proposed `rcgen` (§7 option 1). Private key: generated on-machine, stored in the OS keychain via the existing `keyring` pattern (`word-assist-tls-key-v1` entry), **or** as a 0600 file under the app data dir if keychain writes prove unreliable on some systems — decided in 1b, disclosed either way.
- Trust: the leaf is placed in the **Current-User** Root store (never LocalMachine, never registry edits beyond the standard `certmgr`-equivalent API calls) — and only inside the Phase 3 registration flow, behind an explicit consent dialog that names what is installed, where, and how to undo it. Documented removal: delete the certificate from "Manage User Certificates → Trusted Root Certification Authorities → ScholarScribe Word Assist (localhost)". **The owner's explicit confirmation remains a hard gate before any trust-store change on any machine (ruling 4c); nothing in Phases 0–1b touches the store.**
- Name constraints (V11): proposed as optional hardening only; the leaf is scoped by construction regardless.

### Q3 — Offset units (start/length in `Word.Critique`)

**PROPOSAL (spike (a) closes it).**
- Engine-internal canonical offsets: **Unicode scalar values** (harper's unit, §3.1) for Tier 0, and a lossless **scalar↔UTF-16 code-unit mapping** built once per paragraph text in the 1a offsets module (prefix-sum table; O(1) both directions). The wire format to/from the add-in is UTF-16 code units (JS semantics), pending spike (a) confirming that `Critique.start/length` are UTF-16 code units against JS string indices.
- The mapping is exercised by property tests on the exact edge cases the brief lists: astral-plane emoji and ZWJ sequences, Arabic + diacritics (combining marks), NBSP, curly quotes, mixed bidi (Arabic prose + Latin citation), CJK. Property test: `unmask(mask(x)) == x` for the masking module; `utf16→scalar→utf16` identity for the mapping; and a golden round-trip `scalar_offsets == utf16_offsets` on ASCII-only text (where the two coincide).

### Q4 — Who applies the accepted suggestion

**PROPOSAL (INFERRED, V6; spike (b) confirms).** The add-in applies it: on `onAnnotationPopupAction` carrying `critiqueSuggestion`, it locates the annotation's range, applies the replacement, posts `/v1/accepted` (ledger + audit), then re-checks the paragraph (the apply itself triggers `onParagraphChanged`; the suppression watermark of C4 absorbs the self-event). Design consequence adopted now: every accept must trigger a fresh hash-verified annotation pass, because any in-paragraph replacement invalidates all sibling critiques in that paragraph.

### Q5 — Tier 0 vs. the existing lint set (rewritten per ruling 1)

**PROPOSAL.**
- **Reuse, single source of truth:** Word Assist Tier 0 calls the existing `language_check::Engine` (build_engine/check/check_text) with the app's persisted `language_check_dialect` and `language_check_rules`; the personal dictionary is shared via `language_check_dictionary_path`. Word Assist owns **no** rules, no dictionary, no dialect list of its own. Changes to `language_check.rs` are additive only (a new pure function or two, e.g. exposing per-issue `rule_id` sets for filtering), in their own commit.
- **Strictness defaults (spelling allowed per ruling 1):**
  - *conservative* (default): categories `Spelling` + `Grammar` only; `Style` dropped. Rationale: harper's style lints (word choice, redundancy, readability) are the noisiest and the most context-dependent — wrong for as-you-type underlines.
  - *balanced*: + harper style lints whose `priority` is high (exact threshold picked against the golden set in 1a, recorded in the spec).
  - *thorough*: everything the curated `FlatConfig` emits, subject to the same overlap filter as Language Check.
  - Per-rule toggles: the user's existing `language_check_rules` overrides apply on top of strictness in all levels (single source of truth), and per-issue filtering uses `IssueCategory` + `rule_id`, both already on the `Issue` struct.
- **Protected spans (ruling 1: decide with golden-set evidence).** Two candidate strategies, both implementable purely in the Tier 0 adapter: (S1) lint the **original** text, then drop lints whose span intersects a protected span (citation/number/URL/DOI/LaTeX/quote/glossary) — keeps harper's grammatical context intact, costs one post-filter; (S2) lint **masked** text — protects spans by construction but degrades harper's context (placeholders are not English words, and harper's sentence parsers may mis-segment around them). **Recommendation: S1**, because citation_manager already returns exact spans (§3.2) and S2 risks new false positives *around* protected regions — but the decision is explicitly **PENDING golden-set evidence in 1a** (the golden set contains citation/number/equation sentences precisely to arbitrate S1 vs S2, per the brief's "must come back unchanged" requirement).
- **Language gate (ruling 2):** a Unicode-script heuristic runs before Tier 0/Tier 1 (harper has no language detector — **VERIFIED** by absence: `harper-core-2.11.0` ships English parsers (`new_plain_english`) plus `harper-brill`/`harper-pos-utils` for English POS; no detection module). Threshold and the "language not supported yet" status line are 1a/1b items; the "experimental: other languages" toggle (off by default, per-language, labelled unvalidated) and the graduation criteria (golden set + spike (i) pass + native-speaker review) are settings/schema items specified in 1a, wired in 1b.

## 7. Dependency proposals (nothing added without owner approval)

**Option A — server without new dependencies (RECOMMENDED for the listener).** Cargo.lock already contains, at exact transitive versions: `hyper 1.12.0`, `hyper-util 0.1.21`, `http-body-util 0.1.5`, `http 1.5.0`, `tokio-rustls 0.26.6`, `rustls 0.23.45`, `hyper-rustls 0.27.10`, `rustls-pki-types 1.15.1`, `tokio 1` (full). A minimal HTTPS/1.1 server (single origin, four routes, 64 KB body cap, constant-time bearer check) is ~150 lines over these crates: `tokio-rustls::TlsAcceptor` wrapping a `TcpListener` bound to `127.0.0.1`, hyper's `server::conn::http1` per connection. **VERIFIED** — all eight crate versions quoted from `Cargo.lock` on this branch. Rules honored: TLS by rustls (maintained, in-tree), HTTP parsing by hyper (maintained, in-tree); the owner's revised rule for this component ("don't hand-roll HTTP when a maintained library in the tree can do it") is satisfied exactly. License surface: zero new licenses.

**Option B (alternative, rejected for now)** — `axum 0.8`: nicer routing, but pulls `axum-core`, `matchit`, and a wider tower surface as *new* crates; not needed at four routes. Revisit only if route count grows in Phase 3.

**Certificate generation — one genuinely new dependency required:**
- **`rcgen` (proposed: `=0.14.x`, license MIT OR Apache-2.0, pure Rust, actively maintained).** Generates the self-signed leaf + key at runtime on the user's machine. Returns DER directly, which `rustls-pki-types` (already in-tree) consumes — no PEM parser needed.
- Alternative considered and rejected: the `openssl` crate (already *in the lock* via reqwest→native-tls) — but `cargo tree -i openssl` shows it enters only through native-tls, which on Windows is schannel; openssl-sys is not built on the Windows target, so it cannot generate the certificate on the platform this feature targets first (**VERIFIED** from the dependency graph shape; the Windows claim is INFERRED from native-tls's documented backend mapping and flagged for the 1b build check).
- Also proposed for Phase 3 (ledger), listed now for completeness: **`hmac = "0.12"` (MIT OR Apache-2.0)** for HMAC-SHA256 over the existing `sha2`. Hand-rolling HMAC is small but still crypto; the owner's "never hand-roll" spirit applies. `hmac` has no transitive deps beyond `digest`, already present via sha2's stack (**VERIFIED**: sha2 0.10 in Cargo.toml).

**Not needed:** no CORS crate (same-origin), no rate-limit crate (a token-bucket over a `Mutex<HashMap>` fits the four routes), no auth crate (one bearer token, constant-time compare).

**Known constraint on any new dep:** `Cargo.toml:35-39` documents that harper's zip stack holds the exclusive `links = "lzma"` slot; the app's `zip` is `default-features = false` because of it. Any proposed dependency that activates a competing lzma/zip provider breaks the build — `rcgen`/`hmac` do not touch zip (**VERIFIED** from their dependency lists at the proposed versions; will be re-verified against the exact fetched manifests in 1b before the commit).

## 8. Per-phase, file-level plan (revised per rulings; each sub-phase ends at its own gate)

**Phase 1a — pure engine, offline, no socket, no user-visible surface.** Feature flag `word-assist = []` introduced in its first commit; everything gated `#[cfg(feature = "word-assist")]`.
- New `src-tauri/src/assist/mod.rs` + `offsets.rs` (scalar↔UTF-16 map), `sentences.rs` (language-agnostic splitter; RTL/CJK tests), `mask.rs` (protected spans via citation_manager + regex/quote spans; round-trip property tests), `diff.rs` (word-level diff → edits with scalar offsets), `validate.rs` (the six rejection rules + strictness levels), `cache.rs` (memory-only sentence cache keyed by text_hash), `tier0.rs` (adapter over `language_check::Engine`; S1/S2 arbitration via golden set), `engine.rs` (Tier 1 prompt constant, versioned; response schema validation).
- Golden set `src-tauri/tests/assist/` (~50 scholarly sentences; citation/number/equation sentences must be returned unchanged).
- **No changes to existing modules except the one-line `[features]` entry.** Gate: fmt/clippy/test with feature on AND off (locally: frontend + fmt only, per the sandbox gap; the compile gates run on the owner's machine or an enabled CI — R1).
- README privacy-claim edit is *scheduled for 1b* but drafted here (ruling 3: "in or before the first commit that opens a socket" — 1a opens no socket).

**Phase 1b — external surface.**
- `assist/server.rs` (Option A stack; auth/Origin/Host; body cap; rate limit; routes `/v1/health`, `/v1/check`, `/v1/accepted`, `/v1/pair`), `assist/scheduler.rs` (one in-flight, stale-hash discard, cursor priority, abort), `assist/settings.rs` + audit events.
- Existing-module edits, each its own commit: `ollama.rs` (additive `ChatRequest` fields + injectable base URL; ruling D3), `persistence.rs` (additive `Settings` fields: `word_assist_enabled`, tier models, port, strictness, experimental-language map), `lib.rs` (feature-gated state + command registration), `Cargo.toml` (`rcgen`, `hmac`, behind the feature).
- `tauri` commands + `WordAssist.svelte` tab (opt-in dialog first run: disclosure text incl. office.js CDN fetch per ruling 5; language status line incl. "language not supported yet").
- Same commit as the listener: `docs/NETWORK_SURFACE.json` entries (loopback listener host; office.js CDN classified under a proposed new class `addin-webview`) + SECURITY.md sync; README privacy claims updated (in-app wording: "reads live Word text while Word Assist is on; runs a loopback listener on 127.0.0.1").
- Mock-Ollama integration tests for the server (auth rejection, Origin/Host rejection, oversize body, stale discard, abort); real-model tests `#[ignore]`.

**Phase 2 — add-in** (`word-addin/`): manifest (HTTPS SourceLocation), TypeScript client (strict), debounce, `isSetSupported` probing (1.6/1.7/1.8 separately), annotations lifecycle, popup actions, suppression watermark unit tests against simulated event sequences, fallback suggestion list. Add-in CSP: allow the office.js CDN only; "no external network except the office.js load, and nothing that carries document text".

**Phase 3 — ledger & registration.** `assist/ledger.rs` (JSONL, HMAC, per-install keychain entry), `language_assist.json` in the Integrity Packet export (canonical manifest untouched), Disclosure Assistant pre-fill via additive `disclosure.rs` fields, fallback polish, Windows registration (trust-store step only after the owner's explicit go; certificate per Q2).

**Phase 4 — benchmark harness, docs (README, SECURITY.md, USER_GUIDE, docs/WORD_ASSIST_SPEC.md), CI extension.**

## 9. Spike (word-addin/spike/) — what it does and what OBSERVATIONS.md covers

Served by `serve.mjs` (plain Node, 127.0.0.1:38080, **http on purpose** — the experiment of ruling 4a; OBSERVATIONS.md explains that a *negative* result here is the expected design input, not a failure). Task pane: loads office.js from the CDN, logs every event to an on-screen panel, and provides five one-click experiments (insert hardcoded critique; insert at user-chosen start/length into a planted emoji/Arabic/NBSP test paragraph; listen for paragraph changes incl. own-insert events; popup action with a suggestion; requirement-set report incl. `Office.context.platform` + webview UA string). OBSERVATIONS.md walks the owner through (a)–(m) with expected/observed columns: offset units (a), who applies on accept (b), multi-suggestion behaviour (c), annotation drift while typing (d), self-event fingerprint (e), requirement-set checks (f), http://localhost acceptance (g), network capture separating Word traffic / CDN fetch / annotation service (h), webview runtime + loopback reachability (l), table/footnote/header/textbox/content-control/Track-Changes behaviour (m), plus RTL offset probing (i) and annotation↔id correlation (j).

## 10. Risks (ranked)

1. **R1 — compile gates blocked in this sandbox** (no sudo, no webkit/gtk). Mitigation: owner runs the three Rust gates per phase on Windows (no webkit needed there), or enables CI on the feature branch (ci.yml `on.push.branches` addition — small CI edit, owner call, listed in §11).
2. **R2 — annotation platform quirks** (V2 M365 requirement; undocumented offset units V5; popup enum values unverified). Spike first; fallback mode already planned; no Phase 2 code before the spike report comes back.
3. **R3 — the accept→apply→event→recheck loop** can oscillate (own edits retriggering checks). Mitigation: suppression watermark (C4) + hash re-verification + a per-paragraph recheck budget in the scheduler.
4. **R4 — trust-store step is consent-gated forever**; nothing ships in Phases 0–1b. If the owner declines, the only impact is the Phase 3 registration UX (manual cert install documented).
5. **R5 — surface governance**: any host literal added before NETWORK_SURFACE.json breaks CI; plan pairs those edits in single commits (D4 ruling).
6. **R6 — dependency constraints**: the `links = "lzma"` exclusivity (§7) means every new dep gets a lockfile diff review; `rcgen`/`hmac` are inert to it.
7. **R7 — Ollama version variance** (`think` semantics, structured-output subset): engine probes `/api/version` at opt-in and omits unsupported fields; the JSON-schema validator treats schema violations as "no suggestion" (precision over recall).

## 11. Decisions needed from the owner

1. **D-1 (CI):** extend `ci.yml` triggers to `feature/word-assist` pushes (one line), or run the Rust gates locally per phase? (Affects every gate report after 1a.)
2. **D-2 (§7):** approve `rcgen` (cert generation) and, for Phase 3, `hmac` — the only two proposed new dependencies; everything else reuses the in-tree stack.
3. **D-3 (§6 Q1):** confirm default port **38080** and the pairing-code model (with the documented localStorage/port consequences).
4. **D-4 (§6 Q5):** confirm strictness defaults (conservative = Spelling+Grammar) and the protected-span recommendation (S1), with S1-vs-S2 arbitration by the 1a golden set as written.
5. **D-5 (§6 Q2):** approve the per-install self-signed leaf + Current-User Root store design **as a design only**; the actual trust-store change still requires your separate, explicit confirmation in Phase 3.
6. **D-6 (C6):** approve the `addin-webview` class for NETWORK_SURFACE.json (office.js CDN as the single declared add-in-webview host).
7. **D-7:** spike run — after you run `word-addin/spike` per OBSERVATIONS.md, paste the (a)–(m) results; Phase 1a can proceed without them, Phase 2 cannot.
