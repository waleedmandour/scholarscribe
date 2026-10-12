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
