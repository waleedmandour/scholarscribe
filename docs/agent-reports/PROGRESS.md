# Word Assist — Agent Progress Report

Updated: 2026-10-26, owner-reported enable bug + app-versioned pre-release. Author: agent (Z.ai CLI).
Work order: `docs/WORD_ASSIST_WORK_ORDER.md` (verbatim copy, committed on the integration branch).

## Current state

Branch `feature/word-assist`, tip = this commit. CI fully green through
`acdbc55` (the 2026-10-24 CI campaign: 5 C1 classes fixed, e2e credential
isolation `6adbbbb`, parallel e2e restored) and the `word-assist-v0.1.0`
preview released 2026-10-25 from the Word Assist Release pipeline.

2026-10-26 round (owner input: "enabling the plugin in the app fails with
'Word Assist is not opted in.'"; rebuild + release as an APP pre-release
carrying the main app version, with the add-in artifacts included; do not
mix the app and the add-in identities):

1. Root cause: `word_assist_enabled` was NEVER written by any code path.
   `word_assist_enable` (assist_commands.rs:130) refuses with the exact
   owner-reported error when the persisted flag is false; persistence.rs
   documents "the disclosure dialog ... is the only code path that sets it
   to true" — but the frontend disclosure flow (`persistSettings`) wrote
   strictness/tier1/port/languages and forgot the flag. Fix: the disclosure
   acceptance (`acceptDisclosure`) calls `persistSettings(true)`, which
   writes `s.word_assist_enabled = true` and nothing else does (a plain
   "Save settings" stays consent-free). Regression gate
   `scripts/check-wa-consent.mjs` (dependency-free, npm test chain) pins
   the three consent-path properties; positive AND negative tested locally.
2. Version de-mixing: `src-tauri/tauri.wordassist.conf.json` no longer
   overrides the app version with the add-in's 0.1.0 (overlay now `{}`);
   app version bumped 2.2.1 -> 2.3.0 across tauri.conf.json, package.json,
   package-lock.json (root entries only), Cargo.toml, Cargo.lock,
   CITATION.cff.
3. Release pipeline: `word-assist-release.yml` triggers on the EXACT tag
   `v2.3.0-pre` (plus the legacy `word-assist-v*`), names installers with
   the app identity (`ScholarScribe-<tag>-<os>-<arch>`), and stages the
   Word Assist add-in artifacts (manifest template + dist zip with
   WORD_ASSIST_INSTALL.md) into the same release; `release.yml`
   skip-guards `v2.3.0-pre` (job-level `if`) so exactly one pipeline
   publishes it. Pre-release notes: fix explanation, asset map, install
   steps, unsigned/QA-pending caveats.

## Next

Owner actions pending (unchanged): push/verify CI, push tag `v2.3.0-pre`
(publishes the app-versioned pre-release), fill
`word-addin/spike/OBSERVATIONS.md` (a)-(r) for CARD B0, then Stage C QA
(`docs/QA_v0.1.md` — the enable step A2 must now pass).
