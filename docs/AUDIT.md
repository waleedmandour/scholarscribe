# ScholarScribe M0 Audit

- Date: 2026-10-13. Auditor: release-engineering agent (M0).
- Base commit: `main` @ `37055bc` ("docs: update Zenodo DOI to v2.2.1 versioned DOI (#25)").
- Branch: `agent/m0-audit`. No behavior changes are made in this milestone.
- Honesty rule in force: every gate below was actually executed in the audit sandbox unless explicitly marked NOT RUN, with the reason. Where a claim could not be settled from code, it is marked UNVERIFIABLE-FROM-CODE.

## 1. A0.1 Baseline gates (all executed 2026-10-13)

| Gate | Result | Evidence / notes |
|---|---|---|
| `npm ci` | OK | npm 11.19.0, Node 24.21.0. Environment note: npm's `allowScripts` gate held back esbuild's postinstall; the esbuild binary was installed locally to run the gates (`node node_modules/esbuild/install.js`). This is a sandbox workaround, not a repo change. |
| `npm run check` (svelte-check) | OK: **0 errors, 11 warnings in 2 files** | Matches the v2.2.1 CHANGELOG claim exactly. |
| `npm run build` (vite build) | OK | `build/index.html` 1.42 kB, JS bundle 304.06 kB, built in 2.32 s. |
| `cargo fmt --check` | OK (clean) | rustfmt 1.10.0-stable. |
| `cargo check` | OK | cargo 1.99.0. Finishes with 1 `dead_code` warning (an unused helper in the lib). |
| `cargo test` | OK: **43 passed, 0 failed** | All in `src-tauri/src` inline modules: provenance.rs (22), docx_reading.rs (9), google_docs.rs (10), google_docs_net.rs (2). See section 6. |
| `cargo clippy` | OK (exit 0), **26 warnings, advisory only** | Breakdown: 7 manual char comparison, 3 useless `format!`, 3 collapsible `if`, 2 identical `if` blocks, 2 length comparison to one, 2 `sort_by_key`, 1 each: map-over-inspect, consecutive `str::replace`, 9-arg function, manual prefix strip, `let`-then-return, dead method, doc list indentation. 18 are auto-fixable (`cargo clippy --fix`). The work order's "about 21 warnings" estimate is close; the exact current count is 26. |

Baseline finding (only repo change in M0): committed `src-tauri/Cargo.lock` still contained `name = "scholarscribe" version = "2.1.0"` while `src-tauri/Cargo.toml` declares `version = "2.2.1"`. The lockfile had not been regenerated after the 2.2.1 bump. Fixed in this branch with a one-line sync; nothing else changed.

### Environment limitations (transparency)

- Sandbox has no root. Tauri Linux system libraries were made available through a user-level "sysroot" (Debian 13 dev packages extracted to `/home/z/sysroot`, resolved via `PKG_CONFIG_PATH`/`LD_LIBRARY_PATH`). This affects only the audit sandbox; the repo and CI are untouched. Recorded in DECISIONS.md.
- No Ollama daemon and no desktop session exist in the sandbox. Everything that requires a running daemon (pull behavior, real model inference, GUI flows) is settled from code reading and the automated tests, and is marked as such. M2's LM-11 manual matrix remains NOT RUN for that reason.

## 2. A0.2 Branch map and PR classification (read-only)

### Tag containment

- `v2.2.1`, `v2.2.0-pre` and `v2.1.0` are all **on `main`** (verified with `git branch -r --contains <tag>`; each includes `origin/main`).
- Default branch `main`, HEAD `37055bc`, last push 2026-10-06.
- Long-lived side branch: `origin/feature/improvement-sprint-v2.2.0` (not merged; PR-less).

### The 17 open PRs, classified (no PR merged or closed)

| PR | Title (abridged) | Class | Risk note |
|---|---|---|---|
| #27 | feat: Proofread tab, Tier 1 + Phase 2a/2b + Phase 3 (gated) | **Feature** (by maintainer) | Large feature branch `feat/proofread-tier1-2b-3`; not part of this work order; needs maintainer review. Interacts with future M2/M4 scope (LLM tabs, ledger). |
| #26 | bump tauri group (4 updates, cargo) | Dependency bot, same-major group | Low risk; re-run gates before merge. |
| #20 | bump tauri-js group (4 updates, npm) | Dependency bot, same-major group | Low risk. |
| #17 | thiserror 1.0.69 → 2.0.20 | Dependency bot, **major bump** | Requires code review (derive API changes). |
| #16 | quick-xml 0.31 → 0.38 | Dependency bot, minor-range jump on 0.x | 0.x minor = breaking by convention; check `docx_reading.rs` usage. |
| #15 | zip 2.3.0 → 7.2.0 | Dependency bot, **major bump** | High-risk: zip I/O underpins .docx parsing and packet export; needs explicit compat review. |
| #14 | sysinfo 0.32.1 → 0.39.6 | Dependency bot, 0.x jump | Used for RAM detection in GGUF preflight; verify API. |
| #13 | bump build-tooling group (npm dev) | Dependency bot (dev) | Low risk. |
| #12 | time 0.3.36 → 0.3.55 | Dependency bot, same-major | Low risk. |
| #11 | uuid 1.26.0 → 1.26.1 | Dependency bot, patch | Trivial. |
| #9 | reqwest 0.12.28 → 0.13.4 | Dependency bot, **major bump** | Touches the app's only two HTTP clients (ollama.rs, google_docs_net.rs); needs review. Note: Cargo.lock already contains BOTH 0.12.28 and 0.13.4 (duplication), so this PR would also de-duplicate. |
| #8 | bump crypto group (3 updates, cargo) | Dependency bot, group | Contains ed25519-dalek/ring-family updates; re-run the 43 tests before merge. |
| #7 | bump svelte group (npm dev) | Dependency bot (dev) | Low risk. |
| #5 | actions/cache 4 → 6 | CI bot, **major bump** | Trivial workflow change. |
| #4 | actions/checkout 4 → 7 | CI bot, **major bump** | Trivial workflow change. |
| #3 | actions/setup-node 4 → 7 | CI bot, **major bump** | Trivial workflow change. |
| #2 | tauri-apps/tauri-action 0 → 1 | CI bot, **major bump** | Affects release.yml build+publish path; review before next release. |

Summary: 1 feature PR (maintainer's own), 12 dependency-bot PRs (4 with major/0.x jumps needing review: #17, #15, #9, #14, plus #16), 4 CI-action PRs. None are stale in the sense of superseded; all dependabot branches rebase automatically. Recommendation (maintainer decision, not acted on): merge the two group bumps (#26/#20) and CI bumps first, then the single-crate same-major ones, and schedule the major bumps (#15 zip, #9 reqwest, #17 thiserror) for a reviewed pass with the full gate suite.

## 3. A0.3 Network inventory

Method: exhaustive grep over `src-tauri/src/*.rs`, `src/**/*.{ts,svelte}`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/*.json`, `index.html`, `verifier/`, `scripts/` for HTTP client construction, URL literals, CSP entries and Tauri capabilities; every hit cross-checked against SECURITY.md, README.md, USER_GUIDE.md, USER_MANUAL.md.

### 3.1 Runtime network surface of the shipped app

| # | Host | Initiating process | Trigger | Opt-in? | Code evidence | Documented? |
|---|---|---|---|---|---|---|
| N1 | `http://127.0.0.1:11434` (+ `http://localhost:11434` in CSP only) | ScholarScribe process (reqwest) to the local Ollama daemon | Every LLM feature: status poll, model list, pull, delete, chat, GGUF import, abstract, section commentary, writing coach | Default-on for status; user-triggered for the rest | `ollama.rs:11`; `abstract_generator.rs:8`; CSP `tauri.conf.json:25` | Yes (README:83, SECURITY:22, USER_MANUAL:238, PrivacyAudit tab) |
| N2 | `registry.ollama.ai` | **Ollama daemon** (never the app) | Model download from the library | Opt-in (user clicks Download) | App only POSTs `127.0.0.1:11434/api/pull` (`ollama.rs:92`); no URL literal for the registry exists in app code | Yes |
| N3 | `hf.co` → `huggingface.co` | **Ollama daemon** | Pull of `hf.co/sciver/sciver-6b:Q4_K_M` (in the hardcoded catalog, `ollama.rs:359`) or any user-typed `hf.co/...` name (`Models.svelte:115-121`) | Opt-in | Name string passed verbatim to `/api/pull` | **No. Zero documentation hits for hf.co in any doc.** |
| N4 | `https://accounts.google.com/o/oauth2/v2/auth` | System browser (opened via `shell()`; webview deliberately unused) | Google Docs connect (OAuth, PKCE + loopback) | Opt-in (provenance mode + user-supplied client_id) | `google_docs_net.rs:14`, opened at `google_docs_commands.rs:137-139` | Partial ("Google's OAuth endpoints" generically; exact host never named) |
| N5 | `https://oauth2.googleapis.com/token` | ScholarScribe process (reqwest POST) | OAuth code exchange + refresh | Opt-in (same) | `google_docs_net.rs:15`, `:64-93` | Partial (generic) |
| N6 | `https://www.googleapis.com/drive/v3/files/{id}/revisions[?alt=media]` | ScholarScribe process (reqwest GET, bearer) | List/download Docs revisions | Opt-in (same) | `google_docs_net.rs:16`, `:100-156` | Partial (generic) |
| N7 | `https://docs.google.com/feeds/download/documents/export/Export` | ScholarScribe process | Fallback plain-text revision export | Opt-in, fallback only | `google_docs_net.rs:19`, `:158-166` | Named in PROVENANCE_SPEC.md:89 only |
| N8 | `http://127.0.0.1:{random port}` | ScholarScribe process, **inbound** loopback listener | OAuth redirect capture | Opt-in (same) | `google_docs_commands.rs:123-126` | Yes (PROVENANCE_SPEC.md:83) |

OAuth scope requested: exactly `https://www.googleapis.com/auth/drive.readonly` (`google_docs.rs:28`, asserted by test). Revision text is diffed in memory and dropped; never written to disk or exported (`google_docs_commands.rs:8-9`).

### 3.2 Non-app network (not part of the app's runtime surface, listed for completeness)

- `verifier/index.html:17-23`: CDN fallback to `cdn.jsdelivr.net` for jszip/js-sha256/tweetnacl **only if the vendored copies in `verifier/vendor/` are missing**. Vendored by default, so the verifier page is offline-capable as shipped; the fallback is undocumented in SECURITY.md.
- `scripts/*.ps1` (developer machine only): win.rustup.rs, nodejs.org, visualstudio.microsoft.com, git-scm.com, cli.github.com, github.com.
- Build-time: registry.npmjs.org, crates.io; `$schema` fetches are editor-side (schema.tauri.app).
- Display-only strings never fetched by the app (system-browser links on click): ollama.com, orcid.org, github.com/waleedmandour, doi.org, tauri.app, svelte.dev, icmje.org, nature.com, elsevier.com, aclweb.org, zenodo.org.

### 3.3 CSP and capabilities

- CSP (`tauri.conf.json:25`): `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'self' http://127.0.0.1:11434 http://localhost:11434`. `connect-src` correctly confines the webview to loopback Ollama only. `'unsafe-inline'` for scripts/styles is required by the theme boot script (`index.html:8-30`, documented in a code comment).
- Capabilities (`src-tauri/capabilities/default.json`): `core:default`, `dialog:default`, `shell:default` only. No `http:*`, no `fs:*`. `shell:default` allows opening any `https://` URL in the system browser (used for OAuth consent and About-page links).
- The frontend contains zero `fetch`/`XMLHttpRequest`/`EventSource`/`WebSocket` calls; all networking is Rust-side (verified by grep).
- No telemetry, analytics, updater, font-fetching or crash-report code exists anywhere in `src/` or `src-tauri/` (grep zero hits; Cargo.toml has no updater plugin).

### 3.4 Documentation gaps found by the inventory

1. `hf.co/huggingface.co` (N3) is a first-class pull path (it is in the shipped catalog) and is documented **nowhere**.
2. SECURITY.md:39 says "All outbound HTTP is in `src-tauri/src/ollama.rs`" — false since v2.1.0; the Google client lives in `google_docs_net.rs` (hosts N4-N7).
3. SECURITY.md:22 scope omits the four Google hosts.
4. The exact Google hostnames are never named outside `google_docs_net.rs` and PROVENANCE_SPEC.md:89.
5. `OLLAMA_HOST` is not honored: the base URL is a hardcoded const (`ollama.rs:11`, duplicated in `abstract_generator.rs:8`). Users with a non-default daemon bind cannot connect. (Also an M2 preflight input.)

## 4. A0.4 Claims table (checkable public claims vs code)

Verdicts: TRUE / FALSE / MISLEADING / UNVERIFIABLE-FROM-CODE (with reason). Line numbers refer to the base commit.

| Claim (abridged, exact wording quoted where load-bearing) | Where | Verdict | Evidence |
|---|---|---|---|
| "Does not contact any third-party API. The only network call is to `registry.ollama.ai`..." | README.md:58 | **FALSE** | Google hosts N4-N7 exist and are app-initiated (opt-in). README itself contradicts this at :384. |
| "One outbound call, to `registry.ollama.ai`, only when you click Download" | README.md:377 | **MISLEADING** | True for that one action, but the doc implies the total surface; omits loopback Ollama traffic and the Google bridge. |
| "Zero telemetry. Single outbound host (registry.ollama.ai, only on model download)." | release.yml:117 (release body template) | **FALSE** | Same as above; shipped in v2.2.0-pre/v2.2.1 release notes. |
| "All outbound HTTP is in `src-tauri/src/ollama.rs`" | SECURITY.md:39 | **FALSE** | `google_docs_net.rs` has three app-initiated HTTPS clients. |
| Security scope allows only `127.0.0.1:11434` + `registry.ollama.ai` | SECURITY.md:22 | **INCOMPLETE** | Omits Google hosts and hf.co daemon-side pulls. |
| "The only outbound hosts are `registry.ollama.ai` (model downloads, no text) and, only if you explicitly connect a Google Doc in the Provenance tab. Google's Docs API." | USER_GUIDE.md:144 | **TRUE but broken sentence** | Content is correct; "tab. Google's Docs API." is a sentence fragment (dash-sweep collateral). Same fragment pattern at README.md:384 and USER_MANUAL.md:238. |
| "No telemetry, ever" / "Does not collect telemetry, analytics, or crash reports" | README.md:3,59; SECURITY.md:36 | **TRUE** | No telemetry/updater/analytics code found; no relevant crates in Cargo.toml. |
| "User text never leaves your device" (drafts, samples, chats, .bib) | README.md:378; USER_GUIDE.md:132 | **TRUE with the documented exception** | Text only reaches local Ollama; Google path sends document text to Google only when the user explicitly connects a Doc (inherent to the feature, disclosed in USER_MANUAL.md:188). Docs that state the exception correctly are fine; absolute phrasings ("Nothing you write... ever leaves", USER_GUIDE.md:132) are over-broad. |
| "ScholarScribe talks to it on `http://127.0.0.1:11434`" | README.md:83 | **TRUE** | `ollama.rs:11`. |
| Writing Journal creates "a verifiable process record" / "verifiable evidence" | README.md:25,50,260; WritingJournal.svelte:96-97; writing_journal.rs:2-3 | **FALSE** | Journal is plain JSON on disk: no hash, no signature, no chain linkage, no verifier support (grep zero matches for "journal" in provenance.rs / verify.js). See 6.6. |
| "16-model catalog" | README.md:144; USER_MANUAL.md:79 | **TRUE** | Exactly 16 entries in `ollama.rs:236-360`. |
| "20 tabs" | USER_GUIDE.md:31 | **TRUE** | 20 tab entries in `App.svelte:83-111`. WelcomeTour.svelte:267 still says "4 categories" (stale; actual 7 groups). |
| "7 phases/groups" | README.md:12,134 | **TRUE** | 7 distinct `group` values in App.svelte. |
| "6-step welcome tour" | README.md:12,134,452 | **TRUE** | 6 steps in WelcomeTour.svelte:12-36. |
| "24 rule-based transformations (12 default + 11 strict)" | README.md:16 | **INCONSISTENT** | 12+11=23; README:163 says "Strict (24 ops)". One of the numbers is wrong (settled by counting text_cleaner.rs ops; left to M1 fix). |
| v2.2.1 CHANGELOG: "`npm run check`: 0 errors, 11 pre-existing warnings; cargo fmt clean; em dash grep zero matches" | CHANGELOG.md:112-115 | **TRUE** (reproduced) | Baseline reproduced all three results on 2026-10-13. |
| "Provenance sessions above 1,000 revisions are processed in chunks with a progress indicator" | USER_MANUAL.md:225 | **FALSE** | No chunking exists in `src-tauri/src` (grep zero hits). Google import caps at 500 revisions (`google_docs_commands.rs:31`). |
| "Errors are written to a local log file only" | README.md:380 | **MISLEADING** | Logging is `env_logger` to stderr (`lib.rs:37-39`); no log file is written. |
| "byte-equality with the Rust builder is covered by an automated cross-language test (fixture + Node script)" | PROVENANCE_SPEC.md:177-179 | **FALSE** | No cross-language test, fixture, or Node script exists anywhere in the repo; CI never runs `cargo test` either. |
| Risk Profile "Four proxy signals"; ~200-word passage; thresholds high>0.65 / medium>0.45 | README.md:239-250 | **TRUE** | `risk_profiler.rs:52-53,276-289`. |
| Style Analysis "12 metrics" | README.md:286-300 | **MOSTLY TRUE** | `provenance.rs:704-717` defines exactly 12 STYLE_METRICS; but README's bullet list includes "Complex-word ratio", which is not one of them (it lives in style_fingerprint.rs). Wording needs a fix in M1. |
| Citations: Weber-Wulff et al. 2023 | ETHICS.md:64; DetectorLiteracy.svelte:69,99 vs appeal_letter.rs:76,103,129 | **CONTRADICTORY (two identities)** | Two different titles/venues/DOIs circulate: "Testing of detection **signals**..." Patterns 2023 DOI 10.1016/j.patter.2023.100846 vs "Testing of detection **tools**..." Int. J. Educational Integrity 19:26, DOI 10.1007/s40979-023-00146-z. The work order says the IJEI one is real; verified against the publisher in M1 (T1.3). |
| "all 14 tools had high false-positive rates" (attributed to Weber-Wulff) | README.md:22,248,508; ETHICS.md:27; CHANGELOG.md:168-170 | **OVERSTATED** | The paper's abstract (fetched in M1) concludes detectors are neither accurate nor reliable and lean toward labelling text human; both false positives and false negatives occurred. "High FPR for all 14" is not what the paper says. |
| "Laban et al. (2024)" as detector literature | README.md:272,331; USER_MANUAL.md:168; DetectorLiteracy.svelte:37,100 | **UNVERIFIED / FALSE AS STATED** | The letter template does **not** cite Laban at all (contradicting README:272). The DetectorLiteracy entry ("Determining the believability and harm of AI-generated outputs", ACL 2024) has no DOI and needs external verification in M1. |
| Liang et al. 2023 (Patterns 4(7):100779) + 61.3%/91 essays/97.8% figures | DetectorLiteracy.svelte:61-65; RiskProfiler.svelte:112-117; ETHICS.md:63 | **PLAUSIBLE, verify in M1** | DOI used consistently; statistics to be re-verified against the paper in T1.3. |
| Security contact "TBD ... before v1.0 release" | SECURITY.md:8 | **STALE + DEFECT D1** | App is at v2.2.1; TBD persists. |
| "Built for v2.2.1" (install table), version badges | README.md:6,93-97,476; USER_GUIDE.md:4 | **TRUE for v2.2.1** | All version strings say 2.2.1 except USER_MANUAL.md:3,56,71 (**stale v2.2.0**), CHANGELOG.md:75-80 claims it was bumped (false), and Cargo.lock (was 2.1.0, fixed in this branch). |
| "A grep for em (U+2014) and en (U+2013) dashes across user-facing files returns zero matches" (v2.2.1 release gate) | release.yml:125; CHANGELOG.md:124-127; CONTRIBUTING.md:48-51 | **MISLEADING** | The gate is manual, not automated, and its documented scope (`src/ src-tauri/src/ public/ docs/ *.md *.html verifier/`) excludes `scripts/`, where 15 em dashes exist in the .ps1 files. The claim as scoped is true; the gate as a quality bar is weaker than it sounds. T1.6 handles it. |
| Privacy Audit tab "Expected hosts: registry.ollama.ai" hint | PrivacyAudit.svelte:100 | **MISLEADING** | That host can never appear in the audit log: the app never connects to it (the daemon does), and the log's host extraction only reads `http_call` entries. |
| Verifier "works offline" | verifier/README.md:8-10 | **TRUE with caveat, honestly documented** | Vendored libs by default; CDN fallback if `vendor/` is missing (documented in the same file). |

## 5. A0.3/A0.4 cross-check: the work order's defects D1-D8

| ID | Work order says | M0 verdict |
|---|---|---|
| D1 | SECURITY.md has TBD contact; scope omits Google; "all outbound HTTP is in ollama.rs" false | **Confirmed** (SECURITY.md:8, :22, :39). |
| D2 | v2.2.0/v2.2.1 notes say "Single outbound host"; README says "no third-party API" | **Confirmed** (release.yml:117 shipped in both release notes; README.md:58). |
| D3 | Weber-Wulff citation wrong; "all 14 tools high FPR" overstates | **Confirmed** (two contradictory identities in-repo; abstract fetch and rewrite scheduled for M1 T1.3). |
| D4 | "Laban et al. (2024)" unverified | **Confirmed, plus in-repo contradiction** (README:272 claims the appeal letter draws on it; appeal_letter.rs never mentions Laban). |
| D5 | Writing Journal called "verifiable" but is unsigned editable JSON | **Confirmed** (README.md:25,50,260; zero crypto or verifier linkage). |
| D6 | style_analysis.json in default export; uncalibrated style distance + Risk Profile risk | **Partially confirmed**: style_analysis.json IS in the default export (always written, `provenance_commands.rs:463-477`, zip writer `provenance.rs:823-851`). Risk Profile output is NOT exported anywhere (grep zero "risk" in provenance.rs/provenance_commands.rs/verify.js) — the packet README should keep saying so explicitly. |
| D7 | Dash gate listed as a verification gate; guide section 6 fragment | **Confirmed** (release.yml:125; fragment at USER_GUIDE.md:144, also README.md:384, USER_MANUAL.md:238). |
| D8 | No cargo test gate; cross-test unknown | **Confirmed**: CI has no `cargo test` step; the cross-language test claimed by the spec does not exist. |

## 6. A0.5 Provenance review

### 6.1 Timestamps

- `.docx` path: session/revision times come **from the document's own metadata** — the `w:date` attribute of `w:ins`/`w:del` elements, parsed RFC 3339, zoneless treated as UTC (`docx_reading.rs:225,325-335`). Unparsable dates are skipped and counted (`skipped_unparsable_dates`). Google path: Drive revision `modifiedTime` (`google_docs.rs:245-259`).
- Future timestamps (> now + 1 day) are flagged as anomalies (`provenance.rs:162-178`).
- Manifest generation time and session ids use the system clock / UUIDv4 (`provenance_commands.rs:561-566`, `provenance.rs:396`).
- The honesty caveats exist in four places (PROVENANCE_SPEC.md:219-220; provenance.rs:903-905 DISCLOSURE_TXT; verifier/README.md:50-54; packet README.txt provenance_commands.rs:548-553). The spec's admission (section 10) matches the code: the signature proves package integrity and producer, not timestamp truth.

### 6.2 Key storage

- Ed25519 via `ed25519-dalek` 2.x with `rand_core`; 32-byte seed from `getrandom`, stored as hex in the **OS keychain** via the `keyring` crate 3.x (service `scholarscribe`, user `provenance-signing-key-v1`; Linux fallback message points at gnome-keyring/KWallet) (`provenance_commands.rs:28-68`).
- Fingerprint = `ed25519:` + SHA-256 hex of the public key (`provenance.rs:580-582`); public key export format `ed25519-pub:{hex}` JSON (`provenance_commands.rs:164-186`). The private seed never leaves the keychain; the in-memory copy is zeroed after load.
- Google OAuth refresh token stored separately (`scholarscribe` / `google-refresh-token-v1`).

### 6.3 Canonical payload and cross-implementation consistency

- Payload label `SCHOLARSCRIBE-MANIFEST-v1`; manifest `version: "1.0"` (`provenance.rs:43,495`).
- Canonical string (Rust, `provenance.rs:493-535`): header line, then version, document_hash, author_key_fingerprint, generated_at, generator, revision_count, one line per session with 10 pipe-separated fields (session_id, author, start, end, snapshot_hash, chars_added, chars_removed, largest_insertion, prev_record_hash, record_hash), then distance_score formatted `%.4f` or literal `null`, baseline_source, metrics_compared.
- The JS verifier (`verifier/verify.js:60-81`) reproduces this exactly, including the trailing-newline quirk and a comment pinning the float format to Rust's. Record canonical strings (`scholarscribe-record-v1`, 9 fields) also match (`verify.js:38-51` vs `provenance.rs:335-349`).
- Residual risk: double rounding (Rust rounds f32 then formats; JS calls `toFixed(4)` on f64) could diverge on exact-half boundary values. No boundary-value test exists. Low likelihood, worth a fixture in M3.
- **Spec vs reality:** PROVENANCE_SPEC.md:177-179 claims an automated Rust-to-JS cross-language test. It does not exist (D8).

### 6.4 Hash chain

- Per-author chains (`build_chain`, `provenance.rs:387-423`): each record's `prev_record_hash` is that author's previous `record_hash`, else genesis `sha256:` + 64 zeros (mirrored in verify.js:6).
- Record hash = SHA-256 over the canonical record string; verified by `verify_chain` including per-author start-time monotonicity (`provenance.rs:588-638`).
- Snapshot hashes are **not recomputable by the verifier** (revision text is never published) — the verifier can only re-derive record hashes and the signature. This is inherent to the privacy design and should be stated more explicitly in the verifier UI copy (M3 candidate, not changed now).

### 6.5 Export packet

- Exactly five entries, all written unconditionally: `manifest.json`, `disclosure.txt`, `style_analysis.json`, `citation_validation.json`, `README.txt` (`provenance_commands.rs:481-488`, `provenance.rs:823-851`).
- **style_analysis.json is in the default export with no opt-out** (D6, first half confirmed). Its content carries a caveat ("not an AI-detection score") but the uncalibrated style-distance number and interpretation bands ship by default.
- Risk Profile output is never exported (grep zero matches) — the second half of D6 is already true in code and must stay true.
- Export aborts if the manifest fails its own verification (`provenance_commands.rs:449-459`).

### 6.6 Writing Journal

- `writing_journal.rs` (291 lines): full snapshot text as plain JSON under `app_data_dir/data/journals/`, gated by `persistence_enabled`. No hashing, no signing, no chain, no manifest field, no verifier support.
- Yet README.md:25,50,260, WritingJournal.svelte:96-97 and the module header call it "verifiable" evidence (D5 confirmed). M1 T1.4 re-words this; M3 decides merge-vs-retire.

### 6.7 Verifier

- Offline HTML page + `verify.js`; steps: load zip, verify chain (recompute record hashes, genesis, per-author linkage and ordering), verify signature (only if a public key is supplied: fingerprint match then Ed25519 detached verify via vendored tweetnacl), optional document binding (hash `word/document.xml` for .docx, raw bytes otherwise).
- It never validates `manifest.version` (displays only) and cannot recompute snapshot hashes (by design).
- v1 support only, consistent with today's packet label. M3's v2 label must be added without breaking this path (N5).

### 6.8 Tests present (43 total, all inline Rust)

- provenance.rs (22): session grouping (30-min gap, 7-day anomaly, cross-author isolation), deletions, future-timestamp flag, chain build/link/tamper, snapshot determinism, sign/verify roundtrip, tampered manifest, wrong key, no-document-text-in-manifest, style distance, canonical payload stability, zip contents, hex roundtrip.
- docx_reading.rs (9): real in-memory .docx fixtures, insertions/deletions, corrupt inputs, hash stability.
- google_docs.rs (10): URL parsing, diffs, revision-to-session mapping, replacement handling, unknown-author labeling, fixtures, token response shapes, docx-pipeline parity.
- google_docs_net.rs (2): URL encoding, consent URL (PKCE + scope).
- **No JS tests exist for the verifier; no Rust-to-JS cross-check exists; CI runs neither `cargo test` nor any verifier test.**

### 6.9 Google Docs bridge summary

Pulls revision metadata + per-revision text via Drive API (cap 500 revisions), diffs in memory, builds sessions with the same pipeline as .docx, hashes final text as document_hash, never persists text, audit-logs every outbound hop. OAuth: installed-app flow, PKCE S256 + state, one-shot loopback listener, system browser, 300 s deadline, single read-only scope.

## 7. A0.6 LLM layer review

### 7.1 Model-name validation: none

`ollama_pull_model` passes the raw string to `/api/pull`; `/api/chat` passes `request.model` verbatim; GGUF import only requires a non-empty name (`commands.rs:36-56,75-88,329-332`; `ollama.rs:84-99`). No regex, allowlist or charset check. Injection risk is bounded by JSON encoding (no shell, no URL interpolation), but arbitrary registry names, `hf.co/...` refs and junk strings all reach the daemon. M2's LM-2 parser is the right fix.

### 7.2 Pull mechanics

- POST `/api/pull` `{name, stream:true}`, 3600 s timeout, NDJSON byte stream re-emitted as global Tauri events `ollama://pull-progress|pull-start|pull-end` (`ollama.rs:89-138`, `commands.rs:49-54`).
- **No cancellation** (no abort flag/command; grep zero). No explicit resume, though Ollama resumes partial blobs daemon-side when the same name is re-pulled.
- **Bug: failed pulls report success.** `{"error": ...}` lines in the pull stream are ignored (parser reads only status/completed/total, `ollama.rs:121-136`), so a mid-stream failure yields `pull-end {ok:true}` and the UI shows success with no model. Confirmed by code reading; fix scheduled with M2 (LM-3), flagged to maintainer now as a candidate hotfix.

### 7.3 Generation options

- Every options object sets **temperature only** (`ollama.rs:201` default 0.7; coach 0.5; abstract 0.4; commentary 0.3). **`num_ctx` is never set anywhere** (grep zero) — long inputs are silently truncated at the daemon's default context window. This is the correctness bug the work order calls out (LM-5). The abstract generator sends up to 12,000 chars of draft (`abstract_generator.rs:65-69`), so the risk is real for its intended use.
- `num_predict` is never set; `max_words` is prompt-enforced only.

### 7.4 Streaming, cancellation, timeouts

- Inference is never streamed to the UI (`"stream": false` everywhere; one-shot responses). Pull progress uses global events; no Tauri channels anywhere.
- No cancellation path for any LLM call; a 300 s chat blocks that feature with no stop control.
- Timeouts: status 3 s, list 10 s, pull 3600 s, delete 30 s, chat 300 s, GGUF create 600 s, abstract 5 s/300 s. **Google clients use bare `Client::new()` with no timeouts at all** (`google_docs_net.rs:39,104,129`) and honor system proxy env vars.

### 7.5 Thinking models: not handled

No `<think>` stripping or `thinking`-field handling exists (grep only marketing copy in catalog descriptions). Qwen3/GPT-OSS/DeepSeek-R1 reasoning content flows verbatim into chat bubbles and into the section-commentary parser, which naively splits on the first colon (`abstract_generator.rs:266-285`). M2 LM-4.2 scope; flagged now.

### 7.6 Error mapping

- Every reqwest send error is mapped to `OllamaError::NotRunning` via `map_err(|_| ...)` (`ollama.rs:65-67,97-99,158-160,210-212`) — so DNS failures, TLS errors **and the 300 s chat timeout** all tell the user "Ollama is not running". Misleading.
- HTTP errors pass through raw: `HTTP {status}: {body}`. 404 (model not found) and 500 (OOM) are user-visible but undifferentiated.

### 7.7 GGUF import

`/api/create` with both the modern `{name, from}` body and legacy `{modelfile}` fallback on HTTP 400; 600 s timeout; `stream:false` so **no progress events** (bare spinner in UI). Path validation is existence + `.gguf` extension; Windows `\\?\` prefixes stripped. Pre-flight RAM check is the crude size x 1.5 rule (`commands.rs:236-291`).

### 7.8 Catalog

Hardcoded 16-entry list in `recommended_models()` (`ollama.rs:233-367`) with name/label/size_gb/min_ram_gb/tags/description. Includes `hf.co/sciver/sciver-6b:Q4_K_M` — "SciGLM" style academic models are **not** present; the work order's example of a possibly-wrong catalog entry will be settled by M2's `scripts/verify-catalog.mjs` against the live registry (not runnable in this sandbox; NOT RUN).

### 7.9 Version check and privacy audit

- No `/api/version`, `/api/show`, `/api/ps` calls exist. Health/per-GPU info is absent (M2 LM-4 scope).
- PrivacyAudit.svelte is instrumentation-by-convention: the in-memory `AuditLog` (audit.rs, capped 1000, cleared on restart) records entries appended at each call site. Hosts are extracted only from `kind=="http_call"` entries; consequences: (a) the tab's "Expected hosts: registry.ollama.ai" hint can never match (the app never contacts it); (b) `bytes_in/out` are hardcoded 0 for every entry; (c) Ollama traffic is recorded as `ollama_command` with target loopback and is excluded from host extraction, so the "ScholarScribe vs Ollama daemon" distinction the work order wants (LM-8) does not exist yet.

## 8. Ranked findings (input to milestone planning)

1. **Truthful-claims cluster (M1):** "no third-party API"/"single outbound host"/"all outbound HTTP in ollama.rs"/Writing-Journal-"verifiable"/cross-language-test claim — all false today, all documented above.
2. **Citation integrity (M1 T1.3):** two contradictory Weber-Wulff identities; "all 14 tools high FPR" overstatement; Laban unverified and falsely attributed to the appeal-letter template.
3. **Failed-pull-looks-successful bug** (ollama.rs:121-136): candidate hotfix; formally scheduled for M2 LM-3 but trivially separable if the maintainer wants it earlier.
4. **num_ctx never set** (LM-5): silent truncation correctness bug; M2.
5. **No cancellation anywhere**; pull and chat cannot be stopped; M2.
6. **Error mapping catch-all** ("Ollama is not running" for everything); M2.
7. **Google clients have no timeouts** and honor system proxies silently; M2/M1 hardening candidate (a one-line client builder change; behavior-preserving; flagged for M2 to keep M0 diff-free).
8. **Audit tab cosmetic gaps** (registry hint that can never match; 0-byte traffic columns); M2 LM-8.
9. **CI gaps:** no `cargo test`, no clippy denial, no cross-verifier test, no network-surface test, no SBOM; M1 T1.7.
10. **Lockfile hygiene:** Cargo.lock version drift (fixed in this branch); duplicate reqwest 0.12.28+0.13.4 in the lock (resolves via PR #9 or a maintainer `cargo update`).
11. **USER_MANUAL chunking claim unimplemented**; "4 categories" stale in WelcomeTour; "12+11=24" arithmetic slip; "errors written to a local log file" overstates stderr logging. All M1 wording fixes.
12. **style_fingerprint.rs:137 hardcodes `"generated": "2026"`** in exported JSON — will silently become stale each year. Logged to OBSERVATIONS.md (outside M1's claims scope, minimal-diff rule).

## 9. What M0 did NOT do (on purpose)

- No behavior changes beyond the one-line Cargo.lock version sync.
- No PRs merged, closed, or commented on. No tags, no releases, no release-name changes.
- No external citation verification (M1 T1.3 does it against the publishers and records sources in DECISIONS.md).
- No catalog verification against the live Ollama registry (needs network + daemon; M2 LM-9 developer script).

