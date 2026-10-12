# REPORT-M0: Audit milestone

- Date: 2026-10-13. Branch: `agent/m0-audit`. Base: `main` @ `37055bc`.
- Status: **complete, awaiting maintainer review** per the work order ("Output: docs/AUDIT.md. Then stop and wait for maintainer review").

## 1. Done

| Task | What was produced | Commit |
|---|---|---|
| A0.1 Baseline | All seven gates executed; results in `docs/AUDIT.md` section 1; one-line Cargo.lock version sync (`fix(build)`) | see git log |
| A0.2 Branch map + PR classification | Tag containment proven (v2.2.1, v2.2.0-pre, v2.1.0 all on `main`); all 17 PRs classified in `docs/AUDIT.md` section 2 | this report's commit |
| A0.3 Network inventory | Full host table with process attribution, opt-in status, code evidence and documentation gaps (`docs/AUDIT.md` section 3) | this report's commit |
| A0.4 Claims table | ~30 checkable claims from README/USER_GUIDE/USER_MANUAL/SECURITY/ETHICS/PROVENANCE_SPEC/CHANGELOG with TRUE/FALSE/MISLEADING verdicts and code evidence (`docs/AUDIT.md` section 4) | this report's commit |
| A0.5 Provenance review | Timestamps, key storage, canonical payload (Rust vs JS), hash chain, packet contents, Writing Journal, verifier, tests, Google bridge (`docs/AUDIT.md` section 6) | this report's commit |
| A0.6 LLM layer review | Model-name validation (none), pull mechanics + failed-pull bug, options (num_ctx never set), streaming/cancellation/timeouts, thinking models, error mapping, GGUF import, catalog, privacy audit mechanics (`docs/AUDIT.md` section 7) | this report's commit |
| D1-D8 confirmation | All eight work-order defects checked against code; D1-D5, D7, D8 confirmed; D6 confirmed for style_analysis.json (Risk Profile was already never exported) | `docs/AUDIT.md` section 5 |

## 2. Tests: exact commands run and results

Executed 2026-10-13 in the audit sandbox (Debian 13, Node 24.21.0, npm 11.19.0, cargo 1.99.0 stable):

```
npm ci                          -> OK (esbuild postinstall required manual approval; sandbox-only workaround)
npm run check                   -> OK: 0 errors, 11 warnings in 2 files
npm run build                   -> OK (vite build, 2.32 s)
cargo fmt --check               -> OK (clean)
cargo check                     -> OK (1 dead_code warning)
cargo test                      -> OK: 43 passed, 0 failed
cargo clippy                    -> OK (exit 0): 26 warnings, advisory
```

NOT RUN items and reasons:

- **Ollama-dependent behavior** (pull, inference, health): no daemon exists in the sandbox. Settled from code reading; runtime verification belongs to M2 (LM-11 manual matrix) on the maintainer's machine.
- **Catalog verification against the live Ollama registry** (LM-9 script): M2 scope; needs network + daemon.
- **GUI flows** (installer, tabs, welcome tour): no desktop session. Settled from code + svelte-check.
- Citation external verification (DOI/publisher fetches): deliberately NOT RUN in M0 because it precedes the M1 T1.3 edits; the fetches, sources and rewrites happen in M1 and are recorded in DECISIONS.md then.

## 3. Decisions added to DECISIONS.md

1. User-level sysroot workaround for rootless cargo gates (sandbox-only).
2. Cargo.lock one-line version sync kept in the M0 branch (2.1.0 -> 2.2.1 self-version entry).
3. Maintainer override recorded: M1 ships as **v2.2.1 r2 (pre-release)** instead of v2.2.2.

## 4. Docs changed

- `docs/AUDIT.md` (new, the M0 output)
- `docs/DECISIONS.md` (new)
- `docs/agent-reports/PLAN-M0.md`, `REPORT-M0.md` (this file), `OBSERVATIONS.md` (new)
- `src-tauri/Cargo.lock` (one line: self-version 2.1.0 -> 2.2.1)

## 5. Network changes

**None.** No code path, host, CSP entry, or capability was added or changed. The only outbound hosts touched by the milestone work itself were the audit's own fetches (GitHub API for PR metadata; Debian mirrors for the sysroot), which are sandbox tooling, not the app.

## 6. Risks and open questions for the maintainer

1. **Failed pulls report success** (`ollama.rs:121-136` ignores `{"error": ...}` stream lines; `pull-end {ok:true}` on failure). Scheduled for M2 LM-3; say the word if you want it as a standalone hotfix in v2.2.1 r2.
2. **USER_GUIDE.md:144 / README.md:384 / USER_MANUAL.md:238 all contain the "…in the Provenance tab. Google's …" sentence fragment** (dash-sweep collateral). Fixed in M1 T1.6 per the work order.
3. **Clippy warning count is 26 today, not "about 21"**; M1 T1.7 plans to fix them and deny warnings. 18 are auto-fixable; the fixes will be reviewed one by one, not bulk-applied, and gated by the full test suite.
4. **PR sequencing question** (no action taken): merge order for #26/#20 (group bumps) and the major bumps #15 (zip), #9 (reqwest), #17 (thiserror). The zip major bump deserves its own pass because .docx parsing and packet export depend on it.
5. **Whether GitHub Security tab ("Report a vulnerability") is enabled** - T1.1 will name it as THE channel; please confirm it is switched on in Settings before v2.2.1 r2 ships.
6. The work order's example "check whether SciGLM is actually an Ollama-library model": SciGLM does not appear in the current catalog at all (the only HF entry is `hf.co/sciver/sciver-6b:Q4_K_M`). The live-registry check still happens in M2 (LM-9).

## 7. Observations outside scope

Logged in `docs/agent-reports/OBSERVATIONS.md` (10 items), including the hardcoded `"generated": "2026"` export field, the 23-vs-24 cleaner-ops arithmetic slip, duplicate reqwest versions in Cargo.lock, and the `read_text_file` path-scope note for SECURITY.md.

## Handoff

M0 ends here. M1 (truthful claims, safe defaults, CI; released as **v2.2.1 r2 pre-release** per the maintainer's override) starts on the maintainer's go-ahead, on a fresh `agent/m1-*` branch from `main`.
