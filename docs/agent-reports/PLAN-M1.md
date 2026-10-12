# PLAN-M1: Truthful claims, safe defaults, CI (v2.2.1 r2 pre-release)

- Date: 2026-10-13. Base: `main` @ `37055bc`, branch `agent/m1-truthful-claims`, stacked on `agent/m0-audit` (merged so M1 docs can cite the audit).
- Maintainer override in force: release identity is **v2.2.1 r2 (pre-release)**, not v2.2.2.

## Tasks, files, order

| Order | Task | Files |
|---|---|---|
| 1 | T1.3 Citations: verify against Crossref/publisher; fix Weber-Wulff identity; remove Laban; fix wrong DOI 100846; add single source of references + tests | `src/lib/references.json` (new), `src/components/DetectorLiteracy.svelte`, `src-tauri/src/appeal_letter.rs`, `README.md`, `USER_MANUAL.md`, `docs/ETHICS.md`, `scripts/check-references.mjs` (new) |
| 2 | T1.1 SECURITY.md rewrite | `SECURITY.md` |
| 3 | T1.2 Network surface single source + doc sync + journal wording (T1.4) + stale claims | `docs/NETWORK_SURFACE.json` (new), `README.md`, `USER_GUIDE.md`, `USER_MANUAL.md`, `src/components/WritingJournal.svelte`, `src-tauri/src/writing_journal.rs`, `src/components/WelcomeTour.svelte` |
| 4 | T1.5 Packet defaults: style_analysis opt-in + spec + verifier compat + tests | `src-tauri/src/provenance.rs`, `src-tauri/src/provenance_commands.rs`, `src-tauri/src/google_docs_commands.rs`, `src/components/ProvenanceExport.svelte`, `src/lib/api.ts`, `docs/PROVENANCE_SPEC.md` |
| 5 | T1.6 Dash gate removal + sentence fixes | `.github/workflows/release.yml`, `CONTRIBUTING.md`, README/USER_GUIDE/USER_MANUAL fragments |
| 6 | T1.7 CI: clippy fixes then `-D warnings`; `cargo test`; cross-verifier fixture (example + integration test + Node script); network-surface test; cargo/npm audits; SBOM; CHECKSUMS signing; prerelease default confirmed | `src-tauri/examples/gen_fixture.rs` (new), `src-tauri/tests/cross_language.rs` (new), `src-tauri/fixtures/cross-check/*` (new, generated), `scripts/cross-verify-fixture.mjs` (new), `scripts/check-network-surface.mjs` (new), `scripts/generate-third-party-licenses.mjs` (new), `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `package.json`, 10 Rust files (clippy fixes), `src-tauri/src/lib.rs` (pub mod for tests) |
| 7 | T1.8 Release notes template + backfills + update the two GitHub pre-release bodies | `.github/RELEASE_NOTES_TEMPLATE.md` (new), `docs/release-notes/*.md` (new), GitHub API PATCH (both releases) |
| 8 | T1.9 CITATION.cff, FOR_EDITORS.md, THIRD_PARTY_LICENSES | `CITATION.cff` (new), `docs/FOR_EDITORS.md` (new), `THIRD_PARTY_LICENSES.md` (generated) |
| 9 | Version identity v2.2.1 r2 + CHANGELOG entry | `CHANGELOG.md`, `README.md` badge/install table, `USER_GUIDE.md`, `USER_MANUAL.md`, `src/App.svelte` (machine version fields intentionally stay 2.2.1; see DECISIONS.md) |
| 10 | Full gate re-run, `REPORT-M1.md`, commit and push. No tag. | `docs/agent-reports/` |

## Risks

1. **MSI versioning** constrains the machine version fields; resolved via the DECISIONS.md entry (identity in tag/name, numeric fields unchanged).
2. **Canonical payload drift** between Rust and JS is the main correctness risk of the cross-check; mitigated by pinning all three implementations to the committed `payload.txt` fixture and failing loudly on any difference.
3. **Clippy auto-fixes** can silently change semantics; every auto-applied hunk was reviewed, and two hand-merges were validated against the original UTF-8 boundary behavior (`utf8_char_len`).
4. **cargo audit findings exist today**; CI keeps it report-only until the pending dependabot PRs merge, otherwise main would go red immediately.
