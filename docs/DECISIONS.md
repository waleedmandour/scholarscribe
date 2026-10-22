## 2026-10-23 | word-assist | A7: held-out set mechanism shipped empty; golden v1 reclassified as SMOKE TEST; add-in client moves to TypeScript

- **Decision (golden set):** `src-tauri/tests/assist/golden.json` (8 test
  cases) is recorded as a **SMOKE TEST** — it pins engine behaviour
  (determinism, protected-span handling, offset round-trips) and is never
  precision evidence. v0.1's held-out evaluation set is NOT supplied
  (owner default); `src-tauri/tests/eval/` ships the frozen-set mechanism
  empty (schema in `docs/EVAL_PROTOCOL.md`, sha256 manifest + freeze
  checks in `npm test` and `frozen_set_integrity`), the scoring tool
  (per-category n/TP/FP/FN + Wilson 95% intervals, unit-tested in
  `assist/eval.rs`), and the `#[ignore]` real-model run
  (`WORD_ASSIST_EVAL_MODEL=… cargo test --features word-assist --test
  eval_harness -- --ignored --nocapture`) whose stdout report must never
  reach UI or docs without an owner decision. No third-party corpus is
  vendored; sentences are owner-authored/approved with per-sentence
  source and license fields.
- **Decision (A2 supersession):** the add-in client is now TypeScript
  strict compiled by the repo's existing `typescript` devDependency —
  replacing the earlier "plain JS with @ts-check + JSDoc" choice —
  because Node's built-in test runner against a hand-written Office.js
  mock needs real modules, and `tsc` emits the dist with NO bundler and
  NO new dependency (ES2020 modules served per-file by the listener;
  ESM is supported by every webview the add-in runs in: Edge WebView2
  and Safari WKWebView). The dist-integrity gate
  (`scripts/check-addin-dist.mjs`) rebuilds and byte-compares the dist,
  and the Rust listener fails its build if an embedded module file is
  missing. The offset ADAPTER remains the single spike-(a) seam, now the
  `OFFSET_UNIT_MODE` constant in `word-addin/src/core.ts`.
- **Decision (A3):** the ledger export (`language_assist.json`) is signed
  through the existing provenance ed25519 path (`sign_payload`), carries
  counts + the HMAC chain head (which commits to every prior entry), and
  states inside the file that the bundled HTML verifier does not check
  it. No change to `verify.js`.
- **Alternatives:** bundling the add-in with esbuild (rejected: new
  dependency for one file); vendoring a public GEC corpus (rejected:
  non-commercial terms unverified); shipping a held-out set authored by
  the agent (rejected: sentences are owner-supplied by the work order).
- **Consequence:** Stage B still owns the spike-dependent strategies
  ((a) offset units, (b)/(j)/(k) popup semantics); the QA matrix
  (`docs/QA_v0.1.md`) records actual observed behaviour per row; the
  certificate model question (self-signed leaf vs the D-5 temporary CA)
  remains an owner decision queued in `docs/RELEASE_PLAN_v0.1.md` §4.

---

## 2026-10-22 | word-assist | Phases 1a-4 implemented on feature/word-assist (E-rulings applied)

- **Decision:** Word Assist (feature `word-assist`, default-off) is implemented through Phase 4's documentation pass on `feature/word-assist`, applying the Phase 1a gate rulings: (E-1) the shipped strictness default is `dev` = Grammar on, Spelling off, Clarity off, marked provisional until the Phase 4 gate, and no golden-set precision figures appear in the UI or README; (D-4) Tier 0 reuses `language_check::Engine` via `build_engine`/`check_text` with the user's `language_check_rules` on top, protected spans use strategy S1 (lint original, drop intersecting lints) with S2 kept behind `MaskMode` for the Phase 4 arbitration; (D-3) port 38080 with one-time pairing codes (6 chars, unambiguous alphabet, 5-minute TTL, constant-time compares); (D-2) new dependencies are rcgen 0.14 (certificate) and hmac 0.12 (ledger chain) only, everything else reuses the in-tree hyper/tokio-rustls/rustls stack; (D-5/4c) the trust store is never written by the app - the registration command only shows the certificate and manual install steps; (D-6) `appsforoffice.microsoft.com` declared as the single `addin-webview` host; (D-1) CI gains a `word-assist-gates` job (fmt, clippy `-D warnings`, tests with the feature on, plus a default-build check) and the push trigger now includes `feature/word-assist`.
- **Implementation decisions taken under the rulings:** (1) the Tauri command surface lives in a always-compiled `assist_commands.rs` with per-cfg duplicate definitions because tauri's command macro generates argument glue from the token stream and cannot handle `#[cfg]`-stripped parameters; without the feature every command is a stub that reports the feature is absent. (2) The Tier 1 verdict reaches the add-in through a re-poll protocol (pending → ready/rejected) instead of long-polling, keeping `/v1/check` non-blocking while the sentence cache makes re-polls cheap; the scheduler keeps one in-flight call, newest wins, with a per-paragraph session budget of 8 calls against oscillation (risk R3). (3) Validator rule 4 works in both directions: protected content of the original must survive, and the revision may not introduce protected content the original lacked (a rewrite that adds a citation is a hallucination). (4) The assist ledger fails closed: if the HMAC chain or the key cannot be read, enable() fails rather than running unrecorded. (5) The assist surface keeps its own in-memory audit log (same shape as the app's) instead of modifying the existing audit surface; a Phase 4 gate can merge them. (6) The add-in client is plain JavaScript with `// @ts-check` + JSDoc instead of a TypeScript build: strict editor-time checking without adding a bundler/packaging step to the repo; the spike-verified annotation shape is isolated in one ADAPTER function awaiting the owner's spike report. (7) The Tier 1 model is configured by name in settings; an empty name surfaces a "no model configured" failure per sentence instead of silently skipping Tier 1.
- **CI repair note:** while wiring the D-1 trigger extension, the push/PR trigger block was suspected corrupted (`branches: ain]` as displayed through shell tools); a byte-level check showed the file correctly contains `branches: [main]` and the "corruption" was a terminal-output artifact. Only the D-1 extension (feature branch trigger + gates job) was applied.
- **Third-party licenses:** new crates are rcgen + its X.509 stack (yasna, pem, x509-parser, der-parser, asn1-rs, oid-registry) and hmac + digest - all MIT OR Apache-2.0, no lzma/zip provider conflicts (the `links = "lzma"` exclusivity is untouched). THIRD_PARTY_LICENSES.md is a release-time artifact and should be regenerated on the release machine (`scripts/generate-third-party-licenses.mjs`); regenerating in this sandbox would have under-reported the crate list because the local registry cache is partial.
- **Consequence:** the owner's spike run (experiments (a)-(m) in `word-addin/spike/OBSERVATIONS.md`) is still required before the add-in can be enabled by default; the offset ADAPTER in `word-addin/dist/taskpane.js` is the only file expected to change if Word's critique units differ from UTF-16 code units. Final strictness defaults remain a Phase 4 gate decision (E-1).

## 2026-10-23 | word-assist | E-4 applied (HMAC removed), golden re-frozen, assumptions register (owner review round)

- **E-4 / ledger:** the HMAC-SHA256 ledger chain introduced after Phase 1a is **reverted to a plain SHA-256 hash chain** (each entry's canonical form includes its predecessor's hash; genesis zeros; `open()` fails closed on a broken chain). The threat model (`docs/LEDGER_THREAT_MODEL.md`) names no threat that a locally stored MAC key stops which the chain plus an export-time signature does not: same-user malware can read the key and re-chain; the file's owner can regenerate regardless (no local scheme prevents owner deletion — stated verbatim in the threat model). The compensating control is the owner-approved design: chain head signed at export via the existing provenance Ed25519 signing path; the export surface itself is a Phase 4 gate item and **nothing signs at rest today**. **The current verifier does not check the ledger file** (it validates provenance packages only). `hmac` was removed from `Cargo.toml` and `Cargo.lock` in a dedicated commit; THIRD_PARTY_LICENSES.md still lists it and **must be regenerated from a complete registry before release (release blocker)** — new crates after removal: rcgen + X.509 stack only, all MIT OR Apache-2.0.
- **Golden re-freeze:** the Phase 1a freeze record was lost in the sandbox reset. Re-frozen now from the pushed branch, unchanged: `src-tauri/tests/assist/golden.json`, 51 sentences (plain 11, citation 6, number 5, clean 5, edge 5, latex 3, url 3, arabic 3, quote 2, mixed_protected 2, cjk 2, emoji 2, tier1_candidate 2), SHA-256 `717819165f3ad4704f724359c5516a964b1cc8d37e99621ddcb34020ea4b94fb`.
- **Assumptions register:** `docs/WORD_ASSIST_ASSUMPTIONS.md` — 12 Word-facing rows (offset unit, who applies a popup suggestion, annotation-id correlation, self-edit event suppression, requirement-set floors, Track Changes, tables/footnotes, paragraph identity, `p.text` fidelity, webview cert trust, CDN isolation, annotation re-anchoring), each with file:line, the spike experiment that settles it, and the blast radius. All rows INFERRED/PROBED until the owner's spike run; two code-level consequences already visible: the C4 watermark (`isSelfEvent`) exists but is not wired into the event handler, and the popup handler's id correlation assumes `args.id.endsWith(issue.id)` although no id is ever set in the critique payload.
- **Two listener defects fixed (found during this review):** (1) `/v1/pair` demanded a bearer token, which an unpaired client cannot have — first-time pairing was impossible; the pairing code is now the authenticator for that route (Origin pin retained; health unchanged). (2) a successful pair recorded the `paired` ledger event twice. Route classification extracted into a tested predicate (`server.rs::route_requires_bearer`).
- **Gate status at the 1a/1b bars (owner review item 6):** golden v1 re-frozen with recorded hash (above) — DONE; held-out set of ≥150 sentences with labels written before any checker run and licenses recorded — **NOT DONE** (does not exist on the branch; the reset also destroyed the Phase 1a per-category evaluation, so the n/TP/FP/FN Wilson table must be re-derived and is NOT DONE); Tier 1 evaluation harness with its own frozen set runnable locally against real models — **NOT DONE** (SPEC §10 item 2); hash/ledger and Tier 1 tests by name — listed in the owner report and present in the tree.
- **Certs (D-5 evidence, owner review item 3):** `server.rs:702-757` — one self-signed leaf (rcgen 0.14.10, SANs `localhost`+`127.0.0.1`, serverAuth EKU, rcgen default validity 1975-01-01→4096-01-01); **there is no separate CA and no CA key at rest** — the leaf's own TLS key IS the signing key and must be retained (it terminates TLS per handshake), so D-5's "destroy the CA key after issuing the leaf" model does not map 1:1; the residual risk (key holder can mint other localhost certs where the user manually trusted this one) and the Phase 3 alternative (ephemeral CA, destroyed) are recorded for the owner's gate. Key: OS keychain `scholarscribe`/`word-assist-tls-key-v1`, else 0600 file (unix) / profile-ACL file (Windows). **Nothing writes to any trust store or registry** — the registration command only displays the certificate and manual steps (`assist_commands.rs:384-411`).

## 2026-10-23 | word-assist | Hardening round H0-H9 (owner-authorized bounded work)

- **H5 / D-8 (ephemeral CA model — REPLACES the self-signed leaf):** `server.rs::ensure_certificate_chain` now generates an ephemeral CA (CA:TRUE, pathlen 0, KeyUsage = keyCertSign ONLY, nameConstraints permitting DNS `localhost` + IP 127.0.0.1/32, validity 3 years) and a leaf signed ONCE by it (serverAuth EKU only, SANs `localhost`+`127.0.0.1`, validity 365 days, strictly inside the CA's window). The CA key is never serialized, never written to disk or keychain, and is dropped immediately after issuance (ring zeroizes its own secret allocations on drop; we hold no copy — honest caveat: rcgen does not expose a zeroize hook for its internal key buffers, so "destroyed" is best-effort in-memory, and absolute). The user trusts the CA, never the leaf; `word_assist_registration` now returns the CA cert + renewed instructions. Leaf key: keychain `scholarscribe`/`word-assist-tls-key-v1`; 0600-file fallback ONLY on unix and ONLY with an `assist_tls_key_fallback` audit event; on Windows the fallback is **REFUSED** (the stated choice from "owner-only ACL or refuse": refusing adds no ACL dependency and Windows Credential Manager is always present). Renewal = expired/missing material regenerates CA+leaf and the user re-trusts the NEW CA (consent re-asked). x509 assertions (`generated_chain_matches_the_d5_model`) parse the generated certs and assert CA flag, pathlen, keyUsage, nameConstraints, EKU, SANs and validity bounds; x509-parser 0.18.1 (already in Cargo.lock as rcgen's dep) was declared as an OPTIONAL dependency wired into `word-assist` only — the default build still resolves neither rcgen nor x509-parser (CI step added). Spike observation **(q)** added to `word-addin/spike/OBSERVATIONS.md` with a TEST-ONLY generator (`cargo test --features word-assist generate_owner_test_ca -- --ignored --nocapture`, writes to `target/word-assist-test-ca/`, never committed; trust-store changes stay manual and owner-run). If name constraints turn out unsupported in Word's webview runtime, the blast radius is: any locally-trusted-CA holder can mint certs for OTHER names — an owner decision, reported after spike (q).
- **H1 (pairing, D-3 as ruled):** CSPRNG code+token generation (`getrandom`; `scheduler.rs::rand_compat`); rejection-sampled digit mapping with an exact-uniformity test (`256 % 31 == 8` → bytes ≥ 248 redrawn; `pairing_code_mapping_has_no_modulo_bias`); code length stays 6 over a 31-symbol alphabet (≈ 2^29.7 > 2^19.9 for 6 decimal digits); GLOBAL 5-failure budget (not per-IP — everything arrives from loopback), 5th failure burns the code; new codes only via explicit user action, gated by a 30 s doubling cooldown capped at 30 min; TTL 300 s behind an injectable clock (`PairingState` + `Clock`/`FakeClock`, so the expiry path is unit-tested); constant-time compares on fixed-length values throughout; a route-specific `/v1/pair` limiter (10 req/60 s) on top of the global 120/60 s; audit events for success / failure / lockout that never contain the code value.
- **H2 (token persistence — resolves the D-3 memory-only conflict):** 32 random bytes; the engine persists ONLY `SHA-256(token)` + created_at / last_used / label (keychain `word-assist-client-token-v1` first, 0600 file fallback on unix, audited; none on Windows). Survives app restart (restored at enable). Revoke = wipe memory+keychain+file (`word_assist_unpair`); rotate = revoke + fresh pairing code (`word_assist_rotate_token`, because the token is never pushed and only reaches the add-in through pairing); disable rotates (revokes). Sliding expiry 30 days since last use, checked on restore and on every bearer match. Plaintext token in add-in localStorage is **ACCEPTED** by the owner (same-user trust boundary) and documented in `docs/LEDGER_THREAT_MODEL.md` §5.
- **H3 (resource limits, `server.rs::NetLimits::production`):** TLS handshake 5 s; header read 30 s — hyper 1.x has no separate keep-alive idle timeout, so `header_read_timeout` bounds BOTH slow headers AND dormant keep-alive sockets (one value, two properties); body read 30 s (64 KiB over loopback otherwise arrives in ms); max concurrent connections 16 with excess closed immediately; max 32 headers; max 16 KiB of header bytes. Rationale: Word's pane uses ≤ 6 sockets per host; every value is ≥ 2 orders of magnitude above legitimate traffic and still caps slowloris-style abuse. Tier 1 handlers keep their own 120 s scheduler deadline, independent of the socket limits. Tests drive tighter limits through `start_with_limits` so every limit is observable without waiting on production durations.
- **H4 (TLS floor):** the ServerConfig is built with `builder_with_provider(ring)` (explicit, not process-default) and `with_protocol_versions([TLS1.3, TLS1.2])` explicitly; ALPN pinned to `http/1.1` only. 1.3-only tightening waits for spike item (l) per the owner. E2E tests cover: ALPN selected = http/1.1, TLS 1.3 negotiates, TLS 1.2 still connects (owner floor). What is NOT testable: rejecting TLS 1.0/1.1 — rustls 0.23 cannot express those versions at all (structurally impossible, stated rather than tested).
- **H7:** one step added to the `word-assist-gates` job asserting `cargo tree -i rcgen` and `cargo tree -i x509-parser` resolve NOTHING in the default config. Per the owner's note, hyper/rustls are deliberately NOT checked: they enter the default tree through reqwest/tauri.
- **Golden v1 relabel (H directive):** golden v1 (`src-tauri/tests/assist/golden.json`, SHA-256 `717819165f3ad4704f724359c5516a964b1cc8d37e99621ddcb34020ea4b94fb`, 51 sentences) is labelled a **SMOKE TEST, not evidence of precision**. It guards against engine regressions only; precision claims must come from the frozen held-out set with per-category n/TP/FP/FN + Wilson intervals (machinery built this round; the sentences themselves are owner-supplied).
- **Release blocker kept:** THIRD_PARTY_LICENSES.md must be regenerated from a complete registry on the release machine before release (unchanged from the E-4 round; the x509-parser direct declaration adds no new crate — it was already locked as rcgen's dep).

---

# ScholarScribe Agent Decisions Log

Running log per the work order: date, decision, alternatives considered, sources. Append-only.

---

## 2026-10-13 | r3-fix | Tauri JS packages pinned to the Rust crates' minors

- **Decision:** The v2.2.1-r2 lockfile regeneration (PR #35) floated
  `@tauri-apps/api` to 2.12.3, `@tauri-apps/plugin-dialog` to 2.8.2 and
  `@tauri-apps/plugin-shell` to 2.5.0 (they sit on `^2.0.0` in package.json),
  while the Rust crates stayed at tauri 2.11.5 / dialog 2.7.3 / shell 2.3.6
  (the Rust-side dependabot group PR #26 is still unmerged). The tauri CLI
  hard-errors on this parity break ("Found version mismatched Tauri
  packages"), which failed all three non-queued release legs on the first
  v2.2.1-r3 tag push. Fix: pin the three JS packages to the exact versions
  matching the Rust minors (`@tauri-apps/api 2.11.1`,
  `@tauri-apps/plugin-dialog 2.7.3`, `@tauri-apps/plugin-shell 2.3.6`) and
  regenerate the lockfile. `@tauri-apps/cli` stays on `^2.0.0`.
- **Alternatives:** merge PR #26 (Rust 2.12.2/dialog 2.8.1/shell 2.4.1), rejected: shell would then mismatch JS 2.5.0 anyway, and a same-day Rust minor cascade is more risk than a JS pin; or wait for dependabot to pair both sides, rejected: blocks the r3 release.
- **Consequence:** when the maintainer later bumps these, BOTH sides must move together (Rust crates in Cargo.toml AND the JS pins here). Dependabot will propose JS bumps; do not merge a JS-only tauri bump while the Rust side lags. Same fix must also restore the cross-compile matrix in release.yml (lost from #37 by a local `git reset --hard` before commit; re-applied in the same PR).

---

## 2026-10-13 | r3 | Maintainer's four usability changes shipped as v2.2.1 r3

- **Decision:** The maintainer requested (1) About directly under Get started, (2) Ollama connectivity detection displayed below the PC specs on the Models page, (3) a functional Model search, and (4) docx→LaTeX conversion "in its right section", then a rebuild as r3 pre-release. Placement decisions: About joins the Get started group (Models, About) rather than staying in Privacy and app, because the welcome flow is where version/credits questions arise; the Ollama status is a card below the specs card (machine + runtime in one glance) driven by a new `ollama_probe` command (`/api/version` + `/api/tags`, loopback only, no new hosts) instead of reusing the boolean `ollama_status`, so the card can show version and model count; the search filters installed + recommended lists live (no backend change, no network); the LaTeX converter lives on the Text Cleaner page because that is where .docx import already lives, avoiding a 21st tab that would ripple through the "20 tools" claims across README/USER_GUIDE/Quick Guide.
- **Alternatives:** a new "LaTeX" tab (rejected: tool-count claims in docs would all change); a sidebar-wide "search all pages" (rejected: the request was model search); showing Ollama status only via the existing sidebar pill (rejected: the maintainer asked for the Models page specifically, below the PC specs).
- **Version identity:** tag `v2.2.1-r3`, display "v2.2.1 r3 (Pre-release)"; machine fields stay numeric 2.2.1 (same MSI rationale as r2, DECISIONS 2026-10-13 M1).
- **Release engineering note:** the Intel-macOS leg cross-compiles on Apple Silicon (`--target x86_64-apple-darwin`) because the macos-13 queue never picks up jobs for this repo (evidence: v2.2.1 job queued exactly 24h then auto-cancelled; v2.2.1 shipped no Intel assets). `release-intel-mac.yml` backfills Intel assets onto already-cut tags; its build/upload path must go through tauri-action because tauri-action (not the bundler) packages `.app` into `.app.tar.gz`.

---

## 2026-10-13 | M0 | User-level sysroot for baseline gates in the audit sandbox

- **Decision:** To run `cargo check/test/clippy` in a rootless sandbox missing `libwebkit2gtk-4.1-dev`/`libgtk-3-dev`/`libsoup-3.0-dev`, the audit downloaded the Debian 13 dev packages and extracted them to a user directory (`/home/z/sysroot`), resolving them via `PKG_CONFIG_PATH` and `LD_LIBRARY_PATH`. This is a sandbox-only workaround; nothing in the repo or CI depends on it.
- **Alternatives:** (a) mark cargo gates NOT RUN, losing baseline verification for 43 tests; (b) request a GitHub Actions run, which the agent cannot trigger without touching CI config in M0. (a) was the fallback if extraction failed; it did not fail.
- **Source:** Debian package search for trixie; packages listed in `docs/agent-reports/REPORT-M0.md`.

## 2026-10-13 | M0 | Cargo.lock one-line version sync kept in the M0 branch

- **Decision:** The committed `src-tauri/Cargo.lock` contained `scholarscribe 2.1.0` while `Cargo.toml` declares `2.2.1` (drift from the 2.2.1 bump). Regenerating the lock as part of the baseline is recorded as the only repo change in M0, committed as `fix(build)`.
- **Alternatives:** revert the lock and leave it to the maintainer; rejected because a baseline audit that ships with a known-inconsistent lockfile is worse than a one-line, zero-behavior sync. No dependency versions changed, only the self-version entry.

## 2026-10-13 | M0 | Release naming override recorded

- **Decision:** Per the maintainer's instruction, the M1 milestone release is versioned **v2.2.1 r2 (pre-release)** (string `2.2.1-r2`) instead of v2.2.2 as written in the work order. M0 itself changes no versions. No tag is created; releases stay pre-releases awaiting maintainer approval.
- **Alternatives:** none (explicit maintainer override).

---

## 2026-10-13 | M1 | Citation verification sources (T1.3)

- **Decision:** Weber-Wulff et al. (2023) is "Testing of detection tools for AI-generated text", *International Journal for Educational Integrity* 19(1), 26, DOI 10.1007/s40979-023-00146-z (verified 2026-10-13 via https://api.crossref.org/works/10.1007/s40979-023-00146-z; 8 authors Weber-Wulff, Anohina-Naumeca, Bjelobaba, Foltynek, Guerrero-Dib, Popoola, Sigut, Waddington). Summaries in the app were rewritten from the Crossref abstract: the tools are "neither accurate nor reliable" with "a main bias towards classifying the output as human-written"; obfuscation worsens performance. The old "all 14 tools had high false-positive rates" claim is gone.
- **Liang et al. (2023)** confirmed as "GPT detectors are biased against non-native English writers", *Patterns* 4(7), 100779, DOI 10.1016/j.patter.2023.100779 (https://api.crossref.org/works/10.1016/j.patter.2023.100779). The 61.3%/91-essays/97.8% figures were corroborated qualitatively against https://hai.stanford.edu/news/ai-detectors-biased-against-non-native-english-writers (fetched 2026-10-13); the unverifiable absolute "native essays misclassified at near-zero rates" was weakened to "far less often". The full-text figure check against the Cell Press page was bot-blocked; the existing "Last verified 2026-09-29" stamp is retained for the numeric figure.
- **Laban et al. (2024)**: no paper titled "Determining the believability and harm of AI-generated outputs" could be found (Crossref title and author queries, 2026-10-13). The closest real Laban et al. 2024 work is "Can LLM-Generated Misinformation Be Detected?" (ICLR 2024), which studies a different question and does not support the detector-literacy claims it was cited for. Removed everywhere per T1.3; tombstone in `src/lib/references.json` under `removed_references`.
- **DOI 10.1016/j.patter.2023.100846** (attributed in v2.2.1 to "Weber-Wulff, Patterns"): Crossref shows it belongs to "Revealing factors influencing polymer degradation with rank-based machine learning" (Yuan et al.). Removed; replaced by the verified IJEI citation.

## 2026-10-13 | M1 | Version representation for the maintainer's "v2.2.1 r2" override

- **Decision:** The release identity is **v2.2.1 r2 (pre-release)** (git tag `v2.2.1-r2` when the maintainer cuts it; release display name "{tag} (Pre-release)"; docs/badges/footer say "v2.2.1 r2"). The machine version fields (`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`) stay at numeric `2.2.1` because Windows MSI `ProductVersion` must be purely numeric and a semver pre-release suffix in `tauri.conf.json` risks breaking the bundle step; the r2 identity travels with the tag and release name instead.
- **Alternatives:** bump machine versions to `2.2.2` (rejected: the maintainer explicitly overrode the work order's v2.2.2 with v2.2.1 r2) or set `2.2.1-r2` everywhere (rejected: MSI product-version parsing).
- **Consequence:** the installer filename stays `ScholarScribe_2.2.1_x64_*`; USER_MANUAL was updated accordingly. A fresh Zenodo versioned DOI should be minted when v2.2.1 r2 is published (CITATION.cff currently pins the v2.2.1 record DOI 10.5281/zenodo.23052647).

## 2026-10-13 | M1 | Packet export default: style_analysis.json opt-in (T1.5)

- **Decision:** `finalize_export`/`provenance_export_zip`/`google_export_zip` take `include_style_analysis: Option<bool>` (default false); the export dialog checkbox labels the file "informational, not evidence". The verifier never required the file, so v1 packets with or without it verify (N5 held; test `export_zip_contains_all_entries` covers both shapes).
- **Alternatives:** removing the metrics entirely (rejected: some authors legitimately want the descriptive statistics for self-review) or shipping them by default (rejected by D6: uncalibrated numbers read as evidence harm non-native writers).

## 2026-10-13 | M1 | Clippy debt cleared before denying warnings (T1.7)

- **Decision:** All 26 informational clippy warnings fixed (18 via `cargo clippy --fix`, 8 by hand, each reviewed). Two lint-motivated refactors were checked against original semantics: `utf8_char_len` (commands.rs) had two identical `1`-branches merged with the boundary preserved (`< 0xC0`), and BOM handling switched to `strip_prefix`. `finalize_export` keeps its `#[allow(clippy::too_many_arguments)]` and `check_deviation` gained one with a written reason rather than restructuring signatures mid-milestone.
- **Alternatives:** bulk `--fix` without review (rejected), leaving clippy advisory (rejected by T1.7).

## 2026-10-13 | M1 | cargo audit report-only in CI for now

- **Decision:** `cargo audit` runs in CI with `continue-on-error: true` and a visible annotation, because 4 unique advisories (quick-xml x2, rustls, time) and 7 unmaintained-crate warnings are open today and all map to pending dependabot PRs (#16, #9, #12, #17). `npm audit --omit=dev` is blocking (currently 0 vulnerabilities). Once the dependabot bumps merge, flip cargo audit to blocking.
- **Alternatives:** ignoring the findings (N1/N3 violation), pinning forks (out of scope).

## 2026-10-13 | M2 | Root component must mount via Svelte 5 `mount()` (r4 white screen fix)

- **Decision:** `src/main.ts` instantiates the app with `mount(App, { target })` from "svelte". The Svelte 4 style `new App({ target })` compiles fine under svelte 5.57.x but throws `effect_orphan` at runtime before first paint, which shipped in the r3 installers as a white screen. A new npm gate (`scripts/check-svelte-mount.mjs`, wired into `npm test`) fails CI if `new App(` or `new Component(` appears in code (comments excluded).
- **Why it slipped:** the r2 dependency bump (svelte ^5.57.2) passed svelte-check and `vite build`, neither of which exercises runtime mounting; no smoke test loaded the built bundle until now. The r4 verification added a headless Chromium sweep that loads the production `build/` output, mocks the Tauri bridge, and clicks through all 20 tabs.
- **Consequence:** pages keep the legacy `$:` reactive statements and `on:click` syntax, which remain supported for non-runes components; migration to runes stays optional and out of scope for r4.

## 2026-10-13 | M2 | One inline SVG icon set, no icon fonts, no emoji glyphs

- **Decision:** sidebar tabs, page headers (new shared `PageHeader.svelte`), the theme toggle, and the welcome tour use `Icon.svelte` + `src/lib/icons.ts`: inline SVG paths, 24 viewBox, 1.7 stroke, currentColor. The name registry lives in a plain TS module because `export type` from a Svelte 5 instance script is rejected by svelte-check.
- **Alternatives:** an icon font (rejected: extra asset, worse CSP story), emoji glyphs (rejected: platform-dependent rendering, unprofessional variance), external SVG files via `img` (rejected: extra requests and no currentColor inheritance).
- **Consequence:** docs/NETWORK_SURFACE.json gained one `namespace` class entry for `www.w3.org` (the SVG xmlns identifier in `Icon.svelte`); it is a namespace label, never fetched. All prior CSS class names are preserved; only tokens were retuned.

## 2026-10-13 | M3 | Language Check: embed harper-core in the Rust process (r5)

- **Decision:** The Language Check feature embeds `harper-core = "=2.11.0"` (Apache 2.0, Automattic) in `src-tauri/src/language_check.rs` behind a small command surface (run, apply, dictionary add/remove/list, settings get/set, file import, txt export). Approved plan defaults: a dedicated page in "Prepare the draft" directly after Text Cleaner; the optional local model style pass is deferred to a later release; American English is the default dialect with the choice remembered; the vehicle is the v2.2.1 r5 pre-release with machine version fields staying numeric 2.2.1. harper is pinned to an exact version so upstream upgrades are deliberate events reviewed against their changelog.
- **Alternatives:** LanguageTool (rejected: needs a bundled Java runtime and a large memory footprint), nlprule (rejected: unmaintained since April 2021), Hunspell alone (rejected: spelling only), a cloud API (rejected on privacy grounds outright).
- **Consequence:** no new network hosts (network-surface test unchanged and green); ten Rust fixture tests pin engine behavior; corrections are applied server side from the rightmost span leftward so offsets never drift; the app's `zip` dependency now enables only `deflate` because harper's zip stack needs the exclusive `links = "lzma"` slot; CHECKSUMS refresh and minisign signing moved to a final `checksums` job gated on all four build legs (closes the r4 race observed on the r4 release).
