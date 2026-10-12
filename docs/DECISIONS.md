# ScholarScribe Agent Decisions Log

Running log per the work order: date, decision, alternatives considered, sources. Append-only.

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

(M1 entries follow below as milestone work proceeds.)
