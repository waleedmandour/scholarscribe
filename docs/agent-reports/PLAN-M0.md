# PLAN-M0: Audit milestone (no behavior changes)

- Date: 2026-10-13
- Base: `main` @ `37055bc` (v2.2.1)
- Branch: `agent/m0-audit`
- Maintainer override in force: the next releasable milestone (M1) is versioned **v2.2.1 r2 (pre-release)** instead of v2.2.2 as written in the work order. M0 itself makes no version changes.

## Tasks, files, order

| Order | Task | Files touched (planned) |
|---|---|---|
| 1 | A0.1 Baseline: `npm ci`, `npm run check`, `npm run build`, `cargo fmt --check`, `cargo check`, `cargo test`, `cargo clippy` in sandbox; record results | none (record only); `src-tauri/Cargo.lock` version-sync fix only |
| 2 | A0.2 Branch map + PR classification (read-only) | none (recorded in AUDIT.md) |
| 3 | A0.3 Network inventory (grep + read) | none (recorded in AUDIT.md) |
| 4 | A0.4 Claims table for README, USER_GUIDE, USER_MANUAL, SECURITY, ETHICS, PROVENANCE_SPEC | none (recorded in AUDIT.md) |
| 5 | A0.5 Provenance review | none (recorded in AUDIT.md) |
| 6 | A0.6 LLM layer review | none (recorded in AUDIT.md) |
| 7 | Write outputs | `docs/AUDIT.md`, `docs/DECISIONS.md`, `docs/agent-reports/OBSERVATIONS.md`, `docs/agent-reports/REPORT-M0.md`, this plan |
| 8 | Commit (small conventional commits referencing A0.x) and push branch. No tag. No release. | git only |

## Risks

1. **Sandbox has no root.** Tauri Linux system libraries (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`) are absent, so `cargo check/test/clippy` fail at the pkg-config stage. Mitigation attempted first: a user-level "sysroot" built from downloaded Debian dev packages plus `PKG_CONFIG_PATH`/`LD_LIBRARY_PATH`. If that works, the baseline is fully green; the workaround is documented in DECISIONS.md and is NOT part of the repo. If it does not work, affected gates are recorded as NOT RUN with the reason and the exact commands for the maintainer.
2. **npm 11 "allowScripts" gate** blocks esbuild's postinstall in `npm ci`; workaround is local-only (`npm rebuild esbuild` / `node node_modules/esbuild/install.js`). Recorded as an environment note, not a repo change.
3. **No GUI/Ollama runtime in sandbox.** Anything requiring a running Ollama daemon or a desktop session is NOT RUN by necessity; behavior claims are settled from code reading plus the test suite.

## Stop condition

After `docs/AUDIT.md` is pushed, M0 ends and the agent stops for maintainer review, per the work order. M1 proceeds only on the maintainer's instruction; its release name will be v2.2.1 r2 (pre-release) per the maintainer's instruction.
