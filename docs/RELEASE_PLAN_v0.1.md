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

## 4. Decisions: settled by the hardening rulings (2026-10-23)

The four decisions this plan originally queued were SETTLED by the owner's
hardening round (H0-H9, tip 0fbc012) and are implemented on this branch;
they are recorded here so the plan matches reality, not as open questions:

1. **Certificate model — SETTLED: H5 implemented.** The listener ships the
   ephemeral name-constrained CA (CA:TRUE, pathlen 0, keyCertSign only,
   nameConstraints DNS `localhost` + IP `127.0.0.1/32`, 3 years) signing one
   leaf (serverAuth only, SANs localhost + 127.0.0.1, 365 days). The CA key
   is used once and never persisted; the user trusts the CA, never the leaf;
   x509-parser tests assert the chain. The A4 trust instructions and the
   exported manifest refer to the CA (Windows `certutil -user -addstore
   Root` into the CURRENT USER store; macOS `security add-trusted-cert -r
   trustRoot`), with uninstall and revoke per step.
2. **Pairing lockout — SETTLED: H1 implemented.** 6-symbol OS-CSPRNG code;
   5 global wrong attempts burn the code; a new code needs an explicit user
   action behind a cooldown that doubles from 30 s to a 30-minute cap;
   TTL 300 s on an injectable clock; constant-time compares; success burns;
   `/v1/pair` route limiter 10/60 s on top of the global 120/60 s; audit
   events for success/failure/lockout never carry the code.
3. **Token lifetime — SETTLED: H2 implemented.** 32 CSPRNG bytes; only
   SHA-256(token) + created_at/last_used/label is persisted (keychain
   first, audited 0600-file fallback on unix, refused on Windows); the
   session survives restart; revoke and rotate in settings; rotate on
   feature disable; 30-day sliding expiry. The add-in keeps the plaintext
   token in localStorage — accepted and documented (threat model).
4. **TLS configuration — SETTLED: H3/H4 implemented.** Explicit limits
   (handshake 5 s, header 30 s, body 30 s, 16 concurrent, 32 headers /
   16 KiB, body 64 KiB, Tier 1 deadline 120 s) and rustls
   `builder_with_provider(ring)` with versions [TLS1.3, TLS1.2] and ALPN
   http/1.1 only. Do not tighten further without owner approval (spike
   (g)/(l) evidence first).

## 4b. What remains open (honest list)

- **Spike-dependent behaviour (Stage B, blocked on OBSERVATIONS.md a-r):**
  offset units (a); who applies a popup accept (b); annotation-id
  correlation (j); self-event suppression wiring (e); per-OS requirement
  sets (f)/(k); webview runtime + HTTPS behaviour (g)/(l); name-constraint
  support in real OS stores (q); network capture (h); tables/footnotes/
  headers/Track Changes (m); ::1 resolution (r); squiggle co-existence
  (o)/(p).
- **Mac is unvalidated.** The macOS build ships labelled "unvalidated";
  Mac QA rows stay NOT VERIFIED; no Mac behaviour is inferred from Windows.
- **No accuracy claims.** No held-out evaluation set exists for v0.1; the
  golden set is a smoke test; Tier 1 is off until the user picks a model
  and is labelled Experimental; spelling and clarity default off.
- **Advisories.** cargo audit is report-only in CI with known open
  advisories (see Gate 1 report A6 table for reachability and fix status);
  npm audit stays blocking.
- **Token in the add-in's localStorage is plaintext** (accepted by ruling;
  the pane's origin is the engine itself, so the token is readable only by
  code the engine served).

## 5. Owner actions (in order)

1. Push the hardening branch and this integration branch; send back CI
   results for the OS matrix (D1 evidence - nothing is CI-verified until
   then).
2. Run `workflow_dispatch` dry-run of `word-assist-release.yml` (A5) and
   confirm the artifact set.
3. Fill word-addin/spike/OBSERVATIONS.md items (a)-(r) (Stage B unblocks;
   several rows above say STOP on a negative result).
4. Execute docs/QA_v0.1.md on Windows (and Mac if available) and return the
   recorded results (Stage C unblocks).

