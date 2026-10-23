# Word Assist v0.1 (Preview) — Release Notes — **DRAFT**

> **DRAFT — owner-only publishing.** Nothing in this file is published by the
> agent. The owner reviews, edits, and (only when Stage C opens) publishes.
> Every factual claim carries an evidence tag: VERIFIED (evidence in-repo),
> NOT VERIFIED (no evidence yet), or NEEDS HUMAN VERIFICATION (requires a run
> or a QA pass the agent cannot perform).

## What this is

ScholarScribe Word Assist is a Microsoft Word add-in, served by the
ScholarScribe desktop app on your own computer. It checks grammar and (when
enabled) suggests clearer phrasing inside Word. Everything runs on your
machine: the add-in pages are served by the app on `localhost`, and no
ScholarScribe component sends your document text anywhere.

**This is a PREVIEW release.** It is feature-incomplete by design: the
grammar checker is provisional, the AI-powered tier ships disabled, and the
quality of suggestions has not been formally evaluated (no held-out
evaluation has been run yet — NOT VERIFIED on purpose, see "No accuracy
claims" below). Do not rely on it for graded or submitted work without
independent checking.

## Requirements

- **Microsoft 365 Word** (desktop) is required. The add-in uses the
  annotations API (WordApi 1.7+) which Microsoft documents as
  Microsoft-365-only; it does not work in perpetual-license Word 2019/2021
  or in Word for the web (VERIFIED against Microsoft's requirement-set
  documentation during Phase 0 recon; see docs/WORD_ASSIST_RECON.md).
- **Windows**: Windows 10/11 x64 (validation status below).
- **macOS**: Apple Silicon build exists but is **unvalidated** (below).
- **ScholarScribe desktop app** running locally, with the Word Assist
  feature enabled (the app generates a local HTTPS certificate and shows
  pairing + trust instructions in its Word Assist tab).
- No administrator rights are required at any point (VERIFIED — see
  docs/WORD_ASSIST_P6_REVIEW.md §1 for the per-user trust-store evidence).

## Platform validation status

| Platform | Build | Validation |
|---|---|---|
| Windows x64 + Microsoft 365 | ships in this release | **Pending owner QA** (NEEDS HUMAN VERIFICATION — the owner's Windows host test, work-order D7) |
| macOS Apple Silicon (arm64) | ships in this release | **Unvalidated** (owner decision: no Mac testing in v0.1; label kept, no claims made) |
| macOS Intel (x64) | ships in this release | **Unvalidated** (same) |

Nothing in these notes should be read as "works on macOS": the macOS builds
are provided for the owner's later QA only.

## Privacy facts

- **Document text**: your document text never leaves your device through
  ScholarScribe. The Word Assist listener binds `127.0.0.1` only, requires
  TLS 1.2+ with an app-generated local certificate, a pairing code, and a
  per-session token; no request is accepted without them (VERIFIED,
  docs/SECURITY.md and the H1–H5 hardening tests).
- **Local AI**: grammar/clarity processing runs in the local Ollama daemon
  on your machine (`127.0.0.1:11434`). Text sent there stays on your device
  (VERIFIED, docs/NETWORK_SURFACE.json).
- **Office.js CDN**: when Word loads the add-in pane, Word's webview fetches
  Microsoft's Office.js library from Microsoft's CDN (`appsforoffice.microsoft.com`).
  That is Microsoft's add-in runtime loading its own library; ScholarScribe
  code does not send document text to it (VERIFIED — manifest SourceLocation
  and office.js loading pattern from Phase 0 recon).
- **Other network activity**: the desktop app itself talks only to your
  local Ollama daemon, and — when you explicitly use the Google Docs
  features — to Google's OAuth/Drive endpoints in the system browser. No
  telemetry, no crash reporting, no update phone-home (VERIFIED,
  docs/NETWORK_SURFACE.json, CI-enforced by `check-network-surface.mjs`).
  Any additional network surface would be added **only after spike item (h)
  is in and the owner approves it**; none is present in this release.

## Known limitations (v0.1 Preview)

- **English only** (v1 scope decision).
- **Grammar checking is provisional**: enabled by default, based on the
  local harper engine; its recall/precision has not been measured.
- **Spelling and clarity checks ship disabled** (v1 default posture;
  enablement comes with evaluation).
- **AI suggestions ship disabled** (Tier 1 OFF by default). When enabled in
  a future preview, suggestions run through the local Ollama daemon with
  JSON-schema validation, protected-span masking, and word-diff review.
- **No formal evaluation has been run.** The golden set exists as a frozen
  smoke test only; the held-out set is Stage B work.
- **CA name constraints against real OS stores are untested** until spike
  item (q) (NOT VERIFIED by design; the trust step asks for your consent).
- **Unsigned binaries**: see install notes below; SmartScreen/Gatekeeper
  warnings are expected and are the OS working as intended.
- **The add-in must be re-trusted after certificate renewal** (the local CA
  regenerates when it expires; the app asks for consent again).

## No accuracy claims

This release makes **no accuracy, precision, recall, or correctness claims
of any kind** for grammar, spelling, clarity, or AI suggestions. The frozen
golden set is a smoke test for regressions, not a quality benchmark. A
held-out evaluation is Stage B work; until it is run and reviewed by the
owner, any number you see elsewhere about Word Assist accuracy did not come
from this project.

## Installing the unsigned build

### Windows (SmartScreen) — NOT VERIFIED

The installer is unsigned, so Windows SmartScreen will show **"Windows
protected your PC"** when you first run it. The commonly documented flow is:
in the SmartScreen dialog, click **More info**, then **Run anyway**.
**NOT VERIFIED against live Microsoft documentation on 2026-10-24** (the
Microsoft Defender SmartScreen pages were not publicly fetchable at
preparation time — behind an authorization wall). The owner should confirm
the exact wording against
https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/
before publishing these notes.

### macOS (Gatekeeper) — VERIFIED

The app is unsigned, so macOS Gatekeeper will block the first launch. Apple's
documented override ("Safely open apps on your Mac",
https://support.apple.com/en-us/102445, fetched 2026-10-24, VERIFIED):

1. Double-click the app once; macOS reports it cannot be opened.
2. Open **System Settings → Privacy & Security** and click **Open Anyway**.
3. The warning reappears; click **Open** to confirm.
4. The app is then saved as an exception and opens normally afterwards.

Apple's page also warns that running unsigned, non-notarized software carries
risk — use it only if you trust the source (i.e. you built this release from
the reviewed sources or obtained it from the project's own release page).

## Where the full instructions live

Step-by-step install, pairing, certificate trust, and uninstall/revoke:
`docs/WORD_ASSIST_INSTALL.md`. Security model and guarantees:
`docs/SECURITY.md`. Evaluation protocol (Stage B, not yet run):
`docs/EVAL_PROTOCOL.md`.
