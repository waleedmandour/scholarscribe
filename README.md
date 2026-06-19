# ScholarScribe

> A privacy-first, local-LLM writing companion for researchers. Runs entirely on your device — no telemetry, no cloud calls, no paid APIs.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: Pre-release](https://img.shields.io/badge/Status-Pre--release-orange.svg)]()

ScholarScribe helps researchers who are writing their own manuscripts to:

- Run open LLMs (Gemma 3, Qwen 3, Phi-4, DeepSeek R1, Llama 3.3, and more) **fully offline** on Windows.
- **Import local `.gguf` files** — pick a model file you already downloaded (e.g. from HuggingFace); ScholarScribe checks whether your device has enough RAM, then registers it with Ollama.
- Analyze whether a draft sounds like **their own** prior writing, with standard readability metrics (Flesch, Flesch-Kincaid, Gunning Fog).
- Generate **venue-compliant AI-use disclosure statements** (ICMJE, Nature, IEEE, Elsevier, ACL).
- Understand **how AI detectors actually work** — and where they fail.
- Verify the app's own privacy claims via an in-app **Privacy Audit log** of every file read and outbound HTTP call.
- **Light, dark, and auto themes** — click the icon in the sidebar to cycle.

---

## Ethical Use — please read

ScholarScribe is designed for researchers who have genuinely written or substantially contributed to a manuscript and wish to work with AI assistance transparently and on their own device.

### What ScholarScribe does

- Helps you draft, paraphrase, and refine **your own** writing with a local LLM.
- Compares a draft's stylistic profile to a sample of **your own prior writing** so you can decide whether the draft still sounds like you.
- Generates **disclosure statements** so you can comply with journal and conference AI-use policies.
- Educates you about how AI-detection tools work and what their known limitations are.

### What ScholarScribe explicitly does NOT do

- **Does not target or attempt to lower AI-detection scores.** No "marker-targeting" engine. No Turnitin/GPTZero/Originality score-reduction pipeline. No stealth modes.
- **Does not help misrepresent AI-generated text as original human work.** The chat module's system prompt explicitly refuses requests to evade detectors or submit AI output as one's own.
- **Does not contact any third-party API.** The only network call is to `registry.ollama.ai` when you choose to download a model — and that call carries no text, no prompts, no usage data.

If you are looking for a tool to "humanize" AI text to bypass Turnitin, this is not that tool. The author of this project believes detection-evasion tools cause net harm to research integrity — and disproportionately harm honest researchers, especially non-native English writers, who are most likely to be falsely accused of AI use.

### Your responsibility

Users are responsible for compliance with their institution's AI-use policies and the policies of any venue to which they submit. When in doubt, disclose AI assistance. When still in doubt, ask your editor.

---

## Installation

### 1. Install Ollama (the local LLM runtime)

ScholarScribe uses [Ollama](https://ollama.com) as its backend — a free, open-source local LLM runner.

1. Download Ollama from <https://ollama.com/download> (Windows installer, ~150 MB).
2. Run the installer. Ollama starts automatically as a background service.
3. Look for the Ollama icon (a llama) in your system tray.

That's it — you don't need to use Ollama directly. ScholarScribe talks to it on `http://127.0.0.1:11434`.

### 2. Install ScholarScribe

Two options:

**Option A — download the pre-built installer (recommended for most users)**

Grab the latest `.msi` or `.exe` from the [Releases page](https://github.com/waleedmandour/scholarscribe/releases). Double-click to install. ScholarScribe will appear in your Start menu.

**Option B — build from source**

Requires Rust 1.77+, Node.js 18+, and the Tauri prerequisites (Microsoft Visual Studio C++ Build Tools, WebView2 — see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)).

```powershell
git clone https://github.com/waleedmandour/scholarscribe.git
cd scholarscribe
npm install
npm run tauri build
```

The installer appears in `src-tauri/target/release/bundle/`.

**Option C — let a bootstrap script do it**

If you're on a fresh Windows machine, `scripts/build-windows.ps1` will check for / install all prerequisites (Rust, Node, MSVC, WebView2, Ollama), run `npm install` and `cargo check`, then build the .msi. Run it from PowerShell:

```powershell
.\scripts\build-windows.ps1
```

**Option D — build AND push to GitHub in one command**

If you want to build the installer and publish a pre-release to GitHub in a single step, use `scripts/release.ps1`. It requires the GitHub CLI (`winget install --id GitHub.cli` then `gh auth login`):

```powershell
.\scripts\release.ps1 -RepoName scholarscribe -Public
```

This builds the `.msi`, creates the GitHub repo, commits and pushes, tags `v0.1.0-pre`, creates a pre-release, and attaches the installer — all in one go. See the script header for details.

> **Security note:** never pass a GitHub token as a command-line argument. `gh auth login` stores credentials in the OS credential manager, which is the only safe place for them. If you've previously shared a token in chat or a screenshot, revoke it at <https://github.com/settings/tokens> before doing anything else.

---

## Usage

See [USER_MANUAL.md](USER_MANUAL.md) for the full walkthrough. Quick start:

1. Open ScholarScribe. The sidebar shows whether Ollama is running.
2. Go to the **Models** tab. Download a model that fits your RAM (Gemma 2 2B for 8 GB RAM machines, Gemma 2 9B or Qwen 2.5 7B for 16 GB+).
3. Go to the **Chat** tab to talk to the model about your draft.
4. Go to the **Style Analysis** tab to compare your draft against your own prior writing.
5. Go to the **Disclosure** tab to generate a venue-compliant AI-use statement.
6. Read the **Detector Literacy** tab to understand how AI detectors work and where they fail.

---

## Privacy

| Property | Status |
|---|---|
| Telemetry | **None.** No analytics, no crash reporting, no usage tracking. |
| Network calls | One outbound call, to `registry.ollama.ai`, only when you click "Download" on a model. Carries no text or usage data. |
| User text | **Never leaves your device.** Drafts, reference samples, chat messages — all stay in memory or local files. |
| Third-party APIs | **None.** No OpenAI, Anthropic, Google, or any other cloud LLM API. |
| Crash reports | None collected. Errors are written to a local log file only. |

The CSP in `tauri.conf.json` explicitly restricts outbound connections from the UI to `127.0.0.1:11434` (your local Ollama). The Rust backend only contacts `ollama.com` for model downloads and nothing else.

If you want to verify this yourself, audit `src-tauri/src/ollama.rs` — every outbound HTTP call is in that file.

---

## Project structure

```
scholarscribe/
├── src-tauri/                  Rust backend (Tauri 2)
│   ├── src/
│   │   ├── main.rs             Entry point; command registration
│   │   ├── commands.rs         Tauri command handlers
│   │   ├── ollama.rs           Ollama HTTP client (the ONLY outbound network code)
│   │   ├── style.rs            Style analysis (descriptive statistics)
│   │   └── disclosure.rs       Disclosure-statement generator
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                        Svelte frontend
│   ├── App.svelte              Shell + sidebar
│   ├── lib/api.ts              Typed Tauri invoke wrappers
│   └── components/
│       ├── Models.svelte       Download & manage local LLMs
│       ├── Chat.svelte         Local chat interface
│       ├── StyleAnalysis.svelte
│       ├── Disclosure.svelte
│       └── DetectorLiteracy.svelte
├── docs/
│   └── ETHICS.md               Full ethical-use policy
├── USER_MANUAL.md
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE
└── README.md (this file)
```

---

## Roadmap

- **v0.1.0** — Models, Chat, Style Analysis, Disclosure, Detector Literacy.
- **v0.1.1** — Fixed console-window-on-launch bug. Added GGUF import with compatibility check. Added reading-level metrics. Added Privacy Audit log. Added About/Credits.
- **v0.1.2** (this release) — Fixed GGUF import HTTP 400 ("neither 'from' or 'files' was specified"). Added manual dark/light/auto theme toggle. Expanded models catalog with latest academic-focused open LLMs (Gemma 3, Qwen 3, Phi-4, DeepSeek R1, Llama 3.3).
- **v0.2** — `.docx` and `.pdf` file reading (planned via `docx-rs` and `pdf-extract` crates).
- **v0.3** — Bundled llama.cpp option, so users who can't install Ollama separately still get a working app.
- **v0.4** — Multi-reference style profile (analyze against a folder of your papers rather than one).
- **v0.5** — Citation-aware chat (model can read your `.bib` file and avoid fabricating references).

---

## Credits

ScholarScribe v0.1.2 — © 2026 **Dr. Waleed Mandour**, released under the MIT License.

Designed and directed by Dr. Waleed Mandour, 2026, with engineering support from **GLM 5.2** (Z.ai).

Built on top of outstanding open-source work, including:

- [Tauri](https://tauri.app) — the cross-platform desktop framework that keeps the installer tiny.
- [Ollama](https://ollama.com) — the local LLM runtime that does the heavy lifting of model management.
- [Svelte](https://svelte.dev) — the frontend framework.
- The open LLM authors: Google (Gemma), Alibaba (Qwen), Meta (Llama), Microsoft (Phi).
- The detector-evaluation research community, especially Liang et al. (2023), Weber-Wulff et al. (2023), and Laban et al. (2024), whose work the Detector Literacy module is built on.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The short version: bug reports and feature requests that align with the ethical-use policy are very welcome; pull requests attempting to add detection-evasion features will be closed without merge.

---

## License

MIT © Waleed Mandour. See [LICENSE](LICENSE).

---

## Acknowledgments

- [Ollama](https://ollama.com) for making local LLM running genuinely easy.
- The authors of the detector-evaluation literature cited in the app — Liang et al. (2023), Weber-Wulff et al. (2023), Laban et al. (2024) — whose work the Detector Literacy module is built on.
