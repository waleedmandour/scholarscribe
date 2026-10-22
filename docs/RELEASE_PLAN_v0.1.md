# Word Assist v0.1 Release Plan (Stage 0 output)

Status: PREVIEW release plan prepared at the Stage 0 gate of the v0.1 work order.
Every factual claim carries an evidence tag (VERIFIED / INFERRED / NOT VERIFIED /
NEEDS HUMAN VERIFICATION). Access dates for web sources: 2026-10-23.

## 0. Reconciliation findings (Stage 0)

### 0.1 Test counts

Measured today on `feature/word-assist` tip `d55bc00` (VERIFIED, local run):

| Config | lib unittests | assist_golden | cross_language | main | Total |
|---|---|---|---|---|---|
| default (no feature) | 61 | 0 (binary compiles empty) | 2 | 0 | **63** |
| `--features word-assist` | 137 | 8 | 2 | 0 | **147** |

Suite enumeration (`cargo test -- --list`, both configs) is the baseline for
every later gate. Feature-only tests: 84 (assist engine/mask/offsets/scheduler/
ledger/server/tier0 units + golden set + disclosure note tests).

The 61/139 figures from the earlier session do not match any tree available
locally: the referenced tip `0fbc012` is not on the pushed branch (NOT
VERIFIED — the commit is unknown here). The default-config lib binary today
reports exactly 61, which suggests the earlier "61" counted only the lib
unittest binary, not the full run (63 including cross_language). The feature
lib today is 137; 139 cannot be reproduced from any commit on the pushed
branch. Today's numbers above are the reconciliation baseline.

### 0.2 Certificate leaf validity

VERIFIED from code + rcgen 0.14 source: `assist/server.rs::ensure_certificate`
generates a **self-signed leaf** (no CA exists) with `rcgen::CertificateParams`
defaults. rcgen's default validity is `not_before = 1975-01-01`,
`not_after = 4096-01-01` (rcgen-0.14.x `certificate.rs`, CertificateParams
Default impl) — a 2,121-year certificate. "No longer than the CA's" is
vacuously satisfied because the leaf is its own issuer; there is no CA to
compare against. The D-5 temporary-CA model (CA with pathlen 0 +
nameConstraints, CA key zeroized after one leaf signing) was ruled but is NOT
implemented in the pushed tree. This is a hardening gap carried into the v0.1
risk register below; changing the certificate model also changes the CA-trust
instructions in A4, so it needs an explicit owner decision before Stage C
(see §4).

### 0.3 How releases are built today

VERIFIED from `.github/workflows/`:

- `ci.yml`: push to `main` **and `feature/word-assist`** (D-1 ruling already
  applied) + PRs to main. Jobs: `check-frontend` (npm ci/check/build/test),
  `check-rust` (fmt, clippy `-D warnings`, test, check, cargo audit
  report-only), `security-and-sbom` (npm audit blocking, CycloneDX SBOM for
  npm + cargo as artifacts), `word-assist-gates` (feature-build fmt/clippy/
  test + default `cargo check`). All Linux only.
- `release.yml`: tag `v*` or workflow_dispatch; matrix = windows-latest,
  macos-latest (arm64 native), macos-latest with `--target x86_64-apple-darwin`
  (Intel cross-compile — the macos-13 Intel runner queue was unavailable for
  this repo), ubuntu-22.04. Builds via `tauri-apps/tauri-action@v0`, publishes
  the GitHub Release, attaches installers, then a `checksums` job recomputes
  CHECKSUMS.txt over all assets and minisign-signs it when
  `MINISIGN_SECRET_KEY` exists. Workflow-level `permissions: contents: write`.
- `release-intel-mac.yml`: backfill workflow for Intel macOS assets.
- `tauri.conf.json`: version 2.2.1, identifier `com.scholarscribe.app`,
  bundle targets "all", macOS `signingIdentity: null` (unsigned), Windows
  webviewInstallMode downloadBootstrapper, no updater, CSP allows connect-src
  127.0.0.1:11434 (Ollama) only.

What adding `--features word-assist` to a release build changes (VERIFIED/
INFERRED from code): the feature adds the assist module, the loopback TLS
listener, the rcgen dependency, and the always-compiled `assist_commands.rs`
stub surface. The default build is unchanged (feature stays opt-in; the
gates job checks `cargo check` without the feature). A release built with the
feature must pass `--features word-assist` to `tauri build` (tauri-action
`args`), and the Word Assist tab appears. No updater config exists, so the
word-assist release cannot self-update; the normal app release channel is
untouched because the word-assist tag will not match `v*` (§2).

### 0.4 Platform matrix (Microsoft docs; accessed 2026-10-23)

Sources: Word API requirement sets
(https://learn.microsoft.com/en-us/javascript/api/requirement-sets/word/word-api-requirement-sets?view=word-js-preview),
WordApi 1.6 / 1.7 pages
(https://learn.microsoft.com/en-us/javascript/api/requirement-sets/word/word-api-1-6-requirement-set?view=word-js-preview,
.../word-api-1-7-requirement-set?view=word-js-preview), browsers/webview page
(https://learn.microsoft.com/en-us/office/dev/add-ins/concepts/browsers-used-by-office-web-add-ins),
sideload pages
(https://learn.microsoft.com/en-us/office/dev/add-ins/testing/sideload-an-office-add-in-on-mac,
https://learn.microsoft.com/en-us/office/dev/add-ins/testing/create-a-network-shared-folder-catalog-for-task-pane-and-content-add-ins),
GitHub hosted runners (https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

| Requirement set | Word on web | Word on Windows (M365) | Word on Windows (Office 2024 perpetual) | Word on Mac | Word on iPad |
|---|---|---|---|---|---|
| WordApi 1.6 (paragraph events, uniqueLocalId) | Supported | 2308 (16731.20234) | 2308 | 16.76 | 16.76 |
| WordApi 1.7 (annotations; M365 subscription REQUIRED for annotation APIs) | Supported | 2311 (17029.20068) | 2311 | 16.79 | 16.79 |
| WordApi 1.8 (popup actions) | Supported | 2405 (17628.20110) | 2405 | 16.85 | 16.85 |

- Annotation APIs carry an explicit Microsoft 365 subscription warning
  ("Annotation APIs require a Microsoft 365 subscription in order to work
  properly because of an underlying service's requirement", WordApi 1.7 page,
  GitHub issue 4953). VERIFIED (docs). Consequence: volume-licensed/perpetual
  Office without M365 must degrade to list mode — the add-in already probes
  with `isSetSupported` and degrades (VERIFIED in taskpane code; behaviour on
  a perpetual license is NEEDS HUMAN VERIFICATION in the QA matrix).
- Webviews: Windows = Microsoft Edge WebView2 (installed with Office for
  supported versions); Mac = Safari WKWebView; web = the host browser.
  VERIFIED (browsers page, updated 2026-08-20).
- Mac sideload: copy manifest to
  `/Users/<username>/Library/Containers/com.microsoft.Word/Data/Documents/wef`
  (create if absent), restart Word, Home > Add-ins; remove by clearing the
  Office cache. VERIFIED (sideload-on-Mac page, updated 2026-05-11).
  The add-in-only XML manifest is required there; unified-manifest sideload
  on Mac is broken per community reports — this repo uses the XML manifest,
  so unaffected (INFERRED).
- Windows sideload: trusted network-share catalog via Trust Center
  (network-share deployment is Windows-only and not for production).
  VERIFIED (network-share page).
- GitHub hosted runners (public repos): `macos-latest`/`macos-14`/`macos-15`/
  `macos-26` = arm64; Intel images exist as `macos-15-intel` and
  `macos-26-intel` (the table accessed 2026-10-23 no longer lists macos-13).
  VERIFIED. A1's matrix uses arm64 + `macos-15-intel` so both architectures
  compile without relying on cross-compilation for tests.
- WordApi 1.6/1.7/1.8 on Word on the web: "Supported" per the requirement
  sets table; Word on the web is NOT a v0.1 target (the listener is
  loopback-only and web Word runs in a cloud sandbox — the add-in's
  localhost origin would not resolve there). Documented as out of scope
  (INFERRED from architecture).

Gaps marked NOT VERIFIED: none outstanding in the table above; the
platform-dependent runtime behaviours (popup flow on Mac, WKWebView TLS trust
of a locally generated CA) are queued behind the owner's spike items and the
QA matrix.

### 0.5 H-series hardening status (carried into this plan)

The earlier authorized H0–H9 hardening round is NOT present in the pushed
tree (VERIFIED by code inspection: no pairing failure lockout, token is
in-memory only — `assist/settings.rs`, TLS config uses rustls defaults rather
than an explicit version floor, self-signed leaf rather than the D-5 CA
model, no E2E listener suite). The v0.1 work order defines its own Stage A
scope; these gaps are recorded as risks and QA-matrix rows rather than
silently re-scoped. None of them is hidden: A9's QA matrix tests actual
behaviour, and §4 lists the owner decisions required before Stage C.

## 1. Release identity and posture (owner defaults applied)

- Tag: `word-assist-v0.1.0` (does NOT match release.yml's `v*` trigger, so
  the normal app release channel cannot fire accidentally — VERIFIED from
  the workflow trigger).
- Display: "Word Assist v0.1.0 (Preview)". Machine version: a tauri config
  overlay (`src-tauri/tauri.wordassist.conf.json`) sets `version "0.1.0"` and
  `productName "ScholarScribe Word Assist"` for the add-in build only; MSI
  ProductVersion stays numeric (0.1.0), and the app's own 2.2.1 line is not
  touched. Proposed here; formalized at Stage C.
- Signed: NO (owner default: unsigned pre-release; no secrets handled by the
  agent). Windows SmartScreen and macOS Gatekeeper warnings are expected and
  documented in A8/A9.
- Posture: PREVIEW. No held-out evaluation set supplied (owner default), so:
  Tier 0 grammar only by default (strictness `dev`: grammar on, spelling
  off, clarity off — VERIFIED as the shipped default, E-1), Tier 1 disabled
  until the user selects a model and labelled Experimental in the UI, and no
  accuracy claims anywhere (checked in A8).
- Platforms: Windows validated by the owner (default YES). macOS build ships
  labelled "unvalidated" and is excluded from the "supported" claim (owner
  default: no Mac available).

## 2. Definition of Done (D1–D8) — plan of record

- **D1** CI green on ubuntu, windows, macOS (both architectures):
  A1 adds a `word-assist-ci.yml` matrix (ubuntu-latest, windows-latest,
  macos-latest arm64, macos-15-intel x86_64 — runner labels verified
  2026-10-23) running fmt, clippy `-D warnings`, tests, and the default
  build, with and without the feature. Existing ci.yml stays for the app;
  the feature branch gains the matrix job.
- **D2** tag-triggered release workflow (dry-run via workflow_dispatch):
  A5 drafts `word-assist-release.yml` — Windows + macOS (arm64 and x86_64)
  builds with `--features word-assist`, asset names with OS+arch, SHA256SUMS,
  SBOM, pre-release flag, dry-run mode, read-only permissions except
  `contents: write` on the publish job.
- **D3** add-in client: A2 converts the client to TypeScript strict compiled
  with the repo's existing typescript devDependency (no bundler; single-file
  output preserved via `tsc --module none --outFile`, TS namespaces), tests
  with Node's built-in test runner against a hand-written Office.js mock,
  reproducible build, "dist matches source" CI check, manifest validated by
  the server-side registration command + schema checks in CI.
- **D4** spike results ingested: Stage B (blocked on owner OBSERVATIONS.md).
  Until then every spike-dependent assumption stays NOT VERIFIED in the
  register and the feature's UI/docs say Preview.
- **D5** QA matrix docs/QA_v0.1.md (A9) executed by the owner on Windows;
  Mac rows stay NOT VERIFIED if no Mac exists.
- **D6** docs/privacy statement match the network capture and real behaviour
  (A8): office.js CDN fetch declared; no document text leaves the device
  through ScholarScribe; Word's own traffic documented as observed by the
  spike capture.
- **D7** listener self-review table: no NOT TESTED/NOT IMPLEMENTED rows at
  the Stage C gate; rows proven by named tests.
- **D8** dependency audit + THIRD_PARTY_LICENSES.md regenerated from a
  complete registry (A6, network available).

## 3. Stage A delivery order (this work order)

A1 cross-platform CI → A2 add-in TypeScript conversion → A4 manifest/install
path → A3 ledger export → A5 release workflow draft → A6 supply chain →
A8 docs → A9 QA matrix → A7 Tier 1 evaluation harness. Gate report, then
STOP (Stage B waits on the owner's spike results; Stage C on the RC build).

## 4. Risks and owner decisions required

1. **Certificate model (decision needed before Stage C).** The pushed tree
   still ships a self-signed leaf with rcgen's default 1975→4096 validity.
   The D-5 temporary-CA ruling (CA pathlen 0 + nameConstraints, zeroized CA
   key, trust the CA not the leaf) is unimplemented. Either (a) implement the
   D-5 model as a Stage C fix-up commit (changes cert generation, the
   registration UI text, and the A4 trust instructions), or (b) ship v0.1
   Preview with the self-signed leaf and document it, doing D-5 in v0.2.
   Default if unanswered: (a) — the trust instructions in A4 are written
   against the CA model so the docs do not churn twice.
2. **Pairing hardening gaps.** No failed-attempt lockout exists (an
   unpaired client can guess 6-char codes within the 120 req/60s global
   limiter; codes expire in 300s and burn on success). Options: accept for
   Preview (loopback-only, code never leaves the machine, audit logs record
   attempts) or add the 5-failure burn rule. Default if unanswered: accept
   for Preview, document the residual risk; the QA matrix rows describe
   actual behaviour.
3. **Token lifetime.** The paired token is in-memory only: restarting
   ScholarScribe (or disabling/re-enabling Word Assist) invalidates the
   add-in's stored token and requires re-pairing. Acceptable for Preview;
   QA matrix covers it. Persistent hashed tokens stay deferred.
4. **TLS configuration.** rustls defaults (TLS 1.2+1.3) are used; no
   explicit version floor or handshake/read timeouts are configured. The
   webview evidence (spike (g)/(l)) must land before tightening; recorded as
   Stage B work, not Stage A.
5. **Mac unvalidated.** The Mac build ships "unvalidated" or is omitted;
   default is to ship it labelled (owner default posture). Mac QA rows stay
   NOT VERIFIED and no Mac behaviour is inferred from Windows.
6. **Advisories.** cargo audit is report-only in CI with known open
   advisories (quick-xml, rustls, time, thiserror — pending dependabot PRs);
   A6 reports each with ship/don't-ship reasoning. npm audit stays blocking.
7. **Spike dependence.** Offset units (spike (a)), popup apply semantics
   ((b)/(j)/(k)), name constraints ((q)), and webview TLS behaviour ((g)/(l))
   remain NOT VERIFIED until the owner's OBSERVATIONS.md lands; the add-in
   isolates the offset adapter and the strategy interfaces, and the QA
   matrix verifies each observed behaviour on real Word.

## 5. Owner actions (in order)

1. Push the Stage A branch/bundle commits; send back CI results for the new
   matrix (D1 evidence).
2. Run `workflow_dispatch` dry-run of `word-assist-release.yml` (A5) and
   confirm the artifact set.
3. Answer §4 decisions 1–3 (certificate model, lockout, token lifetime).
4. Fill word-addin/spike/OBSERVATIONS.md items (a)–(r) (Stage B unblocks).
5. Execute docs/QA_v0.1.md on Windows (and Mac if available) and return the
   recorded results (Stage C unblocks).
