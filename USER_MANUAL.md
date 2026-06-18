# ScholarScribe — User Manual

*v0.1.0 · Pre-release*

This manual walks you through installing ScholarScribe, downloading a model, and using each of the four modules.

---

## Table of contents

1. [System requirements](#1-system-requirements)
2. [Installing Ollama](#2-installing-ollama)
3. [Installing ScholarScribe](#3-installing-scholarscribe)
4. [Downloading your first model](#4-downloading-your-first-model)
5. [Module: Chat](#5-module-chat)
6. [Module: Style Analysis](#6-module-style-analysis)
7. [Module: Disclosure Assistant](#7-module-disclosure-assistant)
8. [Module: Detector Literacy](#8-module-detector-literacy)
9. [Troubleshooting](#9-troubleshooting)
10. [Privacy audit](#10-privacy-audit)

---

## 1. System requirements

| Component | Minimum | Recommended |
|---|---|---|
| OS | Windows 10 64-bit (1809+) | Windows 11 |
| RAM | 8 GB | 16 GB or more |
| Disk | 3 GB free (app + one small model) | 20 GB+ (multiple models) |
| WebView2 | Pre-installed on Windows 11; on Windows 10 the installer will fetch it | — |

Models have their own RAM requirements shown in the Models tab. A 9B-parameter model needs ~16 GB RAM; a 2B model runs on 8 GB.

---

## 2. Installing Ollama

ScholarScribe does not include the LLM engine — it uses [Ollama](https://ollama.com) as a separate, free, open-source runtime. Keeping them separate means you can use the same models across Ollama, ScholarScribe, and any other tool that speaks the Ollama API.

1. Go to <https://ollama.com/download>.
2. Click **Download for Windows**.
3. Run `OllamaSetup.exe`. It installs to `%LOCALAPPDATA%\Programs\Ollama` and starts a background service.
4. You should see a llama icon in your system tray. If you don't, open the Start menu, type `Ollama`, and click the Ollama app.

To verify Ollama is running, open a browser and visit <http://localhost:11434>. You should see the message "Ollama is running".

---

## 3. Installing ScholarScribe

### Option A — pre-built installer (recommended)

1. Go to <https://github.com/waleedmandour/scholarscribe/releases>.
2. Under the latest release, download `ScholarScribe_0.1.0_x64.msi` (or the `.exe` if you prefer NSIS installers).
3. Double-click the file. Windows SmartScreen may warn you — click **More info → Run anyway** (the installer is unsigned in this pre-release; v1.0 will be code-signed).
4. The installer adds ScholarScribe to your Start menu. Launch it from there.

### Option B — build from source

Prerequisites: Rust 1.77+, Node.js 18+, Microsoft Visual Studio C++ Build Tools, WebView2 runtime.

```powershell
git clone https://github.com/waleedmandour/scholarscribe.git
cd scholarscribe
npm install
npm run tauri build
```

Output installer appears at `src-tauri\target\release\bundle\msi\ScholarScribe_0.1.0_x64_en-US.msi`.

---

## 4. Downloading your first model

1. Launch ScholarScribe. The sidebar shows an "Ollama backend" status pill. It should be green ("running"). If it's red, see [Troubleshooting](#9-troubleshooting).
2. Click the **Models** tab in the sidebar.
3. The Recommended Catalog lists six pre-vetted models. Pick one that fits your RAM:
   - **8 GB RAM** → Gemma 2 2B or Qwen 2.5 3B or Phi-3 Mini
   - **16 GB RAM** → Gemma 2 9B or Qwen 2.5 7B or Llama 3.1 8B
4. Click **Download**. A progress bar appears. The first download can take 5-30 minutes depending on your connection and the model size.
5. Once complete, the model appears in the "Installed on this device" table at the top of the tab.

To install a model not in the catalog (e.g. `mistral:7b`, `qwen2.5:14b`, `command-r`), click **Install by name…** and type the Ollama model identifier.

To remove a model and reclaim disk space, click **Delete** next to it in the installed list.

---

## 5. Module: Chat

A simple, local-only chat interface to your installed models.

1. Go to the **Chat** tab.
2. Select a model from the dropdown at the top.
3. Adjust the temperature slider if you want (0 = deterministic, 1 = more creative; 0.7 is a good default for writing help).
4. Type a message in the box at the bottom. Press **Ctrl+Enter** (or click **Send**).
5. The model responds. Continue the conversation as long as you like. Click **Clear** to start over.

**Good prompts for academic writing:**

- "I'm writing the methods section of a paper on X. Here's my draft: [paste]. Suggest three ways to make the procedure description more reproducible without adding length."
- "Critique this paragraph for logical flow. Don't rewrite it — just point out where the argument jumps."
- "What are three alternative ways to phrase 'this study demonstrates that...'?"

**What the chat module will refuse:**

The system prompt instructs the model to decline requests to evade AI detectors, submit AI text as original work, or fabricate citations. If you find a model complying with such requests, please [open an issue](https://github.com/waleedmandour/scholarscribe/issues) — this guardrail is part of the project's ethical commitments.

---

## 6. Module: Style Analysis

Compares a draft's stylistic profile to a sample of your own prior writing.

1. Go to the **Style Analysis** tab.
2. In the **Draft** panel, paste your current draft (or click **Open file…** to load a `.txt`, `.md`, `.tex`, etc.).
3. In the **Reference** panel, paste a sample of your own published or finished writing — something that sounds like "you". Aim for 1,000+ words for a reliable profile.
4. Click **Analyze & compare**.

The output shows:

- **Overall distance** — a single number; lower = more similar to your reference. Typical within-author distance is under 0.5.
- **Summary notes** — plain-English interpretation, including which features stand out.
- **Feature-by-feature comparison** — for each metric (avg sentence length, vocabulary diversity, passive-voice density, hedging, connector use, first-person usage, citation density), the draft value, reference value, percent difference, and a one-word interpretation (very close / minor / notable / substantial difference).

**How to read the results:**

Differences aren't inherently bad. A methods section legitimately reads differently from a discussion. The tool is most useful for catching unintentional drift: "I thought this sounded like me, but I'm using way more 'however' than I usually do — let me check if a co-author (or an AI tool) inserted those."

**What this module does NOT do:**

It does not predict whether your text will be flagged by an AI detector. It does not "humanize" text. It compares your draft to your own writing — that's all.

---

## 7. Module: Disclosure Assistant

Generates a venue-compliant AI-use disclosure statement.

1. Go to the **Disclosure** tab.
2. Pick your target venue from the dropdown (ICMJE for most medical journals; Nature Portfolio; IEEE; Elsevier; ACL; or "Generic" if your venue isn't listed).
3. Fill in:
   - **Tool used** — e.g. "ChatGPT", "Gemini", "ScholarScribe with Gemma 2 9B"
   - **Model (optional)** — e.g. "GPT-4o", "gemma2:9b"
   - **Your name (optional)** — signs the statement at the end
   - **What did you use the tool for?** — be specific: "improve language and readability", "generate an outline for the introduction", "suggest alternative phrasings for the abstract"
4. Click **Generate disclosure**.
5. The generated statement appears in the result card. Click **Copy** to copy it to your clipboard.
6. The result also tells you where to include the statement (manuscript, cover letter, or both) and links to the venue's official AI-use policy.

**Important:** Always verify against the venue's current policy. Policies are still evolving. The link in the result card takes you to the authoritative source.

---

## 8. Module: Detector Literacy

A short, plain-English explainer of how AI-detection tools work and where they fail. No interactive features — just reading material.

The four cards cover:

1. **Perplexity and burstiness** — the two main signals most detectors use.
2. **Where detectors fail** — false-positive bias against non-native English writers, unreliability on short passages, sensitivity to editing, and adversarial fragility. Each point links to peer-reviewed evaluations.
3. **What this means for you** — practical guidance depending on whether you wrote the draft yourself, used AI assistance, or are an instructor/reviewer.
4. **Further reading** — Liang et al. (2023), Weber-Wulff et al. (2023), Laban et al. (2024), and university statements on detector reliability.

---

## 9. Troubleshooting

**"Ollama backend: not running" in the sidebar.**

- Check the system tray for the Ollama icon. If it's missing, launch Ollama from the Start menu.
- Open <http://localhost:11434> in a browser. If it doesn't say "Ollama is running", the service didn't start. Try `ollama serve` from a Command Prompt to see the error.
- If you use a corporate VPN, it may block localhost loopback in some configurations. Disconnect and try again.

**Download stuck at 0%.**

- Ollama downloads from `registry.ollama.ai`. Some corporate networks block it. Try a different network.
- Very large models on slow connections can take 30+ minutes. The progress bar will move; be patient.
- Check `%LOCALAPPDATA%\Programs\Ollama\server.log` for backend errors.

**Chat returns "Ollama returned HTTP 404".**

- The selected model isn't actually installed. Go back to the Models tab and verify.
- Some models need to be loaded into memory on first use, which can take 30+ seconds. Wait and try again.

**The app is slow / hangs.**

- You probably picked a model too large for your RAM. Use a smaller model (e.g. Gemma 2 2B instead of 9B).
- Close other memory-hungry apps (browsers with many tabs, other LLM tools).
- On machines with integrated graphics, Ollama falls back to CPU inference which is much slower.

**.docx files can't be opened.**

- v0.1 supports `.txt`, `.md`, `.tex`, `.rst`, `.csv`, `.json`. For Word documents, use **File → Save As → Plain Text** in Word, or paste the content directly into the text area.
- `.docx` support is planned for v0.2 via the `docx-rs` crate.

**The app crashes.**

- Open an issue at <https://github.com/waleedmandour/scholarscribe/issues> with: ScholarScribe version, Windows version, Ollama version, model used, and what you were doing when it crashed.
- ScholarScribe does not collect crash reports. You're in control of what gets shared.

---

## 10. Privacy audit

If you want to verify ScholarScribe's privacy claims yourself:

1. **Outbound network calls.** All HTTP code is in `src-tauri/src/ollama.rs`. The base URL is `http://127.0.0.1:11434` — localhost only. The only external host ever contacted is `registry.ollama.ai`, and only when you click "Download" on a model. You can confirm this by running ScholarScribe behind a tool like [GlassWire](https://www.glasswire.com/) or Wireshark.
2. **Frontend CSP.** `src-tauri/tauri.conf.json` restricts the UI's `connect-src` to `self` and `127.0.0.1:11434`. The frontend literally cannot make an outbound request to any other host.
3. **Telemetry.** Search the codebase for "telemetry", "analytics", "tracking", "posthog", "mixpanel", "amplitude" — you will find zero matches.
4. **File system.** ScholarScribe only reads files you explicitly pick via the file dialog. It writes nothing to disk except its own log file (in `%APPDATA%\com.scholarscribe.app\logs\`).
5. **Crash reports.** None. Errors are logged locally only.

If you find a privacy issue, please report it via [SECURITY.md](SECURITY.md).
