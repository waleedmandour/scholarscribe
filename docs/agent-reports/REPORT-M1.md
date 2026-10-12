# REPORT-M1: Truthful claims, safe defaults, CI (v2.2.1 r2 pre-release)

- Date: 2026-10-13. Branch: `agent/m1-truthful-claims` (contains `agent/m0-audit`; both pushed).
- Release identity per maintainer override: **v2.2.1 r2 (pre-release)**. No tag created; the release body template is in place; publishing stays with the maintainer.

## 1. Done (task IDs and commit hashes)

| Task | What was done | Commits |
|---|---|---|
| T1.1 | SECURITY.md: TBD removed; GitHub Security tab named as the only channel (flagged below for maintainer confirmation); full host table incl. Google + hf.co with process attribution; "all outbound HTTP is in ollama.rs" corrected to name ollama.rs, abstract_generator.rs, google_docs_net.rs; verifier posture documented | see git log |
| T1.2 | `docs/NETWORK_SURFACE.json` created (23 hosts, classed: runtime-app / runtime-daemon / runtime-inbound / verifier-fallback / display-link / parse-only / namespace / dev-only); README, USER_GUIDE, USER_MANUAL privacy sections rewritten from it; "does not contact any third-party API" and "single outbound host" removed everywhere; version badge/install tables/feature text updated to v2.2.1 r2 | |
| T1.3 | Weber-Wulff corrected + verified (Crossref); Laban removed everywhere (searches documented); wrong DOI 100846 (a polymer-degradation paper) removed; "all 14 tools high FPR" rewritten from the abstract; Liang stats corroborated and one absolute weakened; `src/lib/references.json` single source + `scripts/check-references.mjs` CI test + Rust test `template_references_exist_in_references_json` | |
| T1.4 | Writing Journal: "verifiable" wording removed (README x3, USER_GUIDE, WritingJournal.svelte, module header); explicit full-text-storage warning added | |
| T1.5 | style_analysis.json opt-in (default off) with "informational, not evidence" checkbox; Risk Profile exclusion re-stated; PROVENANCE_SPEC + packet README.txt updated; v1 packets with the file still verify (test covers both shapes) | |
| T1.6 | Dash gate removed from release notes and CONTRIBUTING; the three "...in the Provenance tab. Google's ..." fragments fixed; no punctuation mass-replaced | |
| T1.7 | CI: `cargo test` blocking; clippy `-D warnings` blocking (all 26 warnings fixed); Rust-vs-JS cross-verifier fixture (deterministic, committed, verified by both languages, tamper test included); network-surface test; references test; cargo audit (report-only, annotated); npm audit (blocking); CycloneDX SBOMs; CHECKSUMS.txt minisign signing when the secret exists, visible skip notice otherwise; prerelease-by-default confirmed in release.yml | |
| T1.8 | `.github/RELEASE_NOTES_TEMPLATE.md`; honest backfills for v2.2.0-pre and v2.2.1 from `git log`; both GitHub pre-release bodies updated via API (release IDs 398984634, 399473213) | |
| T1.9 | CITATION.cff (maintainer's real ORCID from the About tab; Zenodo versioned + concept DOIs); docs/FOR_EDITORS.md; THIRD_PARTY_LICENSES.md (458 packages, zero license flags) with generator script | |

## 2. Tests: exact commands run and results (2026-10-13, M1 sandbox)

```
npm ci                                   -> OK
npm run check                            -> OK: 0 errors (warnings re-checked below)
npm run build                            -> OK
cargo fmt --check                        -> OK (cargo fmt applied to the new/edited code first)
cargo clippy -- -D warnings              -> OK (clean; was 26 warnings)
cargo test                               -> OK: 44 lib + 2 cross-language = 46 passed, 0 failed
npm test                                 -> OK: check-references, check-network-surface, cross-verify-fixture all pass
npm audit --omit=dev                     -> 0 vulnerabilities
cargo audit                              -> 4 unique advisories (quick-xml x2 DoS, rustls handshake, time DoS)
                                            + 7 unmaintained-crate warnings; all map to pending dependabot PRs
                                            (#16, #9, #12, #17). Report-only in CI until those merge.
node scripts/generate-third-party-licenses.mjs -> 458 packages, 0 flagged
```

NOT RUN items and reasons:

- **GitHub Actions run of the new ci.yml**: this sandbox cannot trigger workflow runs; the first CI run on the pushed branch is the verification. All gates were executed locally with the same commands first. Note: CI uses `sudo apt-get install` for the Tauri system libs (available on runners; the sandbox used the user-level sysroot from M0).
- **Release build of v2.2.1 r2 installers**: tagging and building is the maintainer's action (work order: tag nothing).
- **Crossref figure-level verification of the Liang 61.3%/97.8% numbers** against the paywalled/bot-blocked full text: corroborated via the publisher metadata and Stanford HAI summary only; the existing 2026-09-29 "Last verified" stamp for that figure was kept.

## 3. Decisions added to DECISIONS.md

1. Citation verification sources (T1.3) with URLs and dates.
2. Version representation: identity "v2.2.1 r2" in tag/name/docs; machine version fields stay numeric 2.2.1 (MSI constraint); Zenodo DOI note for the maintainer.
3. Packet export default: style_analysis opt-in rationale.
4. Clippy debt cleared before denying warnings; review approach for auto-fixes.
5. cargo audit report-only until dependabot bumps land; npm audit blocking.

## 4. Docs changed

`README.md`, `USER_GUIDE.md`, `USER_MANUAL.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md` (new 2.2.1-r2 entry), `docs/PROVENANCE_SPEC.md`, `docs/ETHICS.md`, plus new `docs/NETWORK_SURFACE.json`, `docs/FOR_EDITORS.md`, `docs/release-notes/{v2.2.0-pre,v2.2.1}.md`, `docs/agent-reports/{PLAN-M1,REPORT-M1}.md`, `CITATION.cff`, `THIRD_PARTY_LICENSES.md`, `.github/RELEASE_NOTES_TEMPLATE.md`.

## 5. Network changes

**None at runtime.** No new outbound host, CSP entry, or capability in the app. New dev-tooling network use (npm/cargo audit, Crossref metadata fetches during this milestone's citation verification, SBOM generation) is developer-side only and does not touch the app; `api.crossref.org` is recorded in NETWORK_SURFACE.json as dev-only for documentation purposes. The release workflow gains an optional minisign download/sign step (runner-side, only when the maintainer configures the secret).

## 6. Risks and open questions for the maintainer

1. **Confirm the GitHub Security tab is enabled** (Settings > Code security > Private vulnerability reporting). SECURITY.md now names it as the only channel.
2. **Dependabot sequencing**: merging #16 (quick-xml), #12 (time), #9 (reqwest/rustls), #17 (thiserror) would clear every open cargo-audit advisory; after that, flip cargo audit to blocking in ci.yml (one-line change, noted in the file's step name).
3. **Signing secret**: to get signed CHECKSUMS.txt, add `MINISIGN_SECRET_KEY` (+ `MINISIGN_PASSWORD` if the key is protected) as repo secrets and commit the matching public key to docs/. Until then releases ship unsigned checksums with a visible CI notice.
4. **Zenodo**: mint a new versioned DOI when publishing v2.2.1 r2 and update CITATION.cff's `version`/`date-released` at tag time (currently pinned to the v2.2.1 record).
5. **PR #27 (Proofread tab)** predates this work order and now overlaps M2 (LLM tab wiring) and M4 (AI-use ledger); decide whether to review/merge it before M2 starts.
6. The failed-pull-reports-success bug (ollama.rs pull stream ignores `{"error": ...}` lines) remains scheduled for M2 (LM-3); it is the top candidate for a standalone hotfix if you want it sooner.

## 7. Observations outside scope

See `docs/agent-reports/OBSERVATIONS.md` (M0 items; the "4 categories" tour text and the USER_MANUAL chunking claim listed there were picked up and fixed in M1; the remaining items stand).

## Handoff

Both branches (`agent/m0-audit`, `agent/m1-truthful-claims`) are pushed and reviewable. Suggested merge order: m0 then m1 (m1 already contains m0, so merging m1 after m0 is a fast-forward). Per the work order, M2 (local LLM health + model manager) waits for maintainer review of M0/M1.
