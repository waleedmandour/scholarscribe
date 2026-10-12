<!--
Canonical release-notes template (T1.8). The release workflow's
tauri-action `releaseBody` in .github/workflows/release.yml is kept in sync
with this file by hand; when you edit one, edit the other. The
"What changed in this release" section MUST be filled from CHANGELOG.md at
release time, and the CHANGELOG must be linked. Release notes must never
contain claims that contradict README/SECURITY/USER_GUIDE/USER_MANUAL or
docs/NETWORK_SURFACE.json; the network-surface CI test enforces the host list.
-->
# ScholarScribe {TAG} (Pre-release)

## What changed in this release

<!-- FILL THIS FROM CHANGELOG.md's entry for {TAG}. Keep it short: 5-10 bullets. -->

-

## Install

1. Install [Ollama](https://ollama.com/download).
2. Download the installer for your platform below.
3. Open ScholarScribe, go to the Models tab, download a model.
4. See [USER_GUIDE.md](USER_GUIDE.md) for the 5-minute walkthrough. A polished PDF copy is attached below as `ScholarScribe-User-Guide.pdf`.

## Platforms

- **Windows**: `.msi` and `.exe` installers (x64).
- **macOS Apple Silicon**: `.dmg` and `.app.tar.gz` (aarch64).
- **macOS Intel**: `.dmg` and `.app.tar.gz` (x86_64).
- **Linux**: `.deb`, `.rpm`, and `.AppImage` (x64).

## Privacy

Zero telemetry. ScholarScribe's own process contacts `127.0.0.1:11434` (your local Ollama) always, plus Google's OAuth/Drive endpoints only if you opt in to the Google Docs import in the Provenance tab. Model downloads are fetched by the Ollama daemon itself (from `registry.ollama.ai` or `hf.co`) at your request and carry no text. All inference runs locally via Ollama. Full host list: [docs/NETWORK_SURFACE.json](docs/NETWORK_SURFACE.json) and [SECURITY.md](SECURITY.md).

## Ethical use

ScholarScribe does **not** include detection-evasion features. See [docs/ETHICS.md](docs/ETHICS.md).

## Verification

This release was gated on: `npm ci`, `npm run check`, `npm run build`, `cargo fmt --check`, `cargo check`, `cargo test` (all Rust tests including the Rust-vs-JS verifier cross-check fixture), `cargo clippy -- -D warnings`, the network-surface test (source hosts vs `docs/NETWORK_SURFACE.json` vs SECURITY.md), the references test (citations vs `src/lib/references.json`), and `npm audit --omit=dev`. Full history in [CHANGELOG.md](CHANGELOG.md). The `CHECKSUMS.txt` asset contains SHA-256 digests for every binary artifact and the User Guide PDF; when the maintainer's signing secret is configured in CI, a detached minisign signature `CHECKSUMS.txt.minisig` is attached as well.

## Provenance packet note

Writing Provenance export packets contain `manifest.json`, `disclosure.txt`, `citation_validation.json`, and `README.txt`. `style_analysis.json` is included only if you opt in at export time; it is informational and not evidence. Existing packets that contain the file still verify with the offline verifier. Risk Profile output is never exported.
