# ScholarScribe — User Guide

> A privacy-first, local-LLM writing companion for researchers. Runs entirely on your device — no telemetry, no cloud calls, no paid APIs.
> **Version 0.2.x · Windows · MIT License · [github.com/waleedmandour/scholarscribe](https://github.com/waleedmandour/scholarscribe)**

ScholarScribe helps you draft, clean, validate, and disclose your manuscript using open LLMs that run on your own machine. This guide gets you productive in under 10 minutes. For deep reference, see [`USER_MANUAL.md`](USER_MANUAL.md).

---

## 1. Install (2 minutes)

1. **Install Ollama** — the free, open-source local LLM runtime. Download from <https://ollama.com/download> (~150 MB) and run the installer. Look for the llama icon in your system tray.
2. **Install ScholarScribe** — download the latest `.msi` from the [Releases page](https://github.com/waleedmandour/scholarscribe/releases) and double-click. ScholarScribe appears in your Start menu.
3. **Launch ScholarScribe.** The sidebar should show a green **Ollama backend: running** pill. If it's red, start the Ollama service from your tray.

> **Build from source** (optional): `git clone … && npm install && npm run tauri build`. Requires Rust 1.77+, Node 18+, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## 2. Download a model (1 minute)

Open the **Models** tab. The top card shows your CPU and RAM. Pick a model that fits your memory:

| Your RAM | Recommended models |
|---|---|
| 8 GB | Gemma 3 4B · Qwen 3 4B · Phi-4 Mini |
| 16 GB | Gemma 3 12B · Qwen 3 8B · Phi-4 14B |
| 32 GB | Gemma 3 27B · Qwen 3 32B · DeepSeek R1 32B |
| 64 GB+ | Llama 3.3 70B |

Click **Download** and wait 2–15 minutes (depending on model size and connection). **Already have a `.gguf` file?** Click **Pick .gguf file…** — ScholarScribe checks your RAM and imports it via Ollama with zero outbound network.

## 3. The 19 tabs at a glance

The sidebar is organized in the order you'd typically use them while writing a manuscript:

| # | Tab | What it does |
|---|---|---|
| 1 | **Models** | Install, import, and manage local LLMs |
| 2 | **Text Cleaner** | Fix 24 PDF/OCR/web artifacts in `.txt`, `.md`, or `.docx` |
| 3 | **Citations** | Validate in-text citations against your `.bib` file |
| 4 | **Stats** | Word count, readability, journal-target comparison |
| 5 | **Structure** | Heading tree + missing-section suggestions |
| 6 | **Abstract** | LLM-generated Background/Methods/Results/Conclusions |
| 7 | **Risk Profile** | Does your draft share surface features with AI text? |
| 8 | **Voice Check** | Spot within-document stylistic shifts |
| 9 | **Journal** | Auto-saved timestamped snapshots of your draft |
| 10 | **Appeal Letter** | Generate an evidence-based appeal if falsely flagged |
| 11 | **Fingerprint** | Multi-paper stylistic fingerprint of your writing |
| 12 | **Writing Coach** | Local-LLM coaching on a paragraph or argument |
| 13 | **Style Analysis** | Compare a draft to your own prior writing |
| 14 | **Chat** | Local-only chat (refuses evasion/fabrication requests) |
| 15 | **Disclosure** | Generate venue-compliant AI-use statements |
| 16 | **Detector Literacy** | How AI detectors work — and where they fail |
| 17 | **Privacy Audit** | Every file read + outbound HTTP call, logged live |
| 18 | **Saved Work** | Opt-in local JSON persistence (off by default) |
| 19 | **About** | Version, environment, credits |

## 4. Common workflows

### A. Clean a messy PDF-extracted draft
**Text Cleaner** tab → **Open file…** (or paste) → leave the 12 default cleaners on → click **Clean text**. For Word documents, choose:
- **Extract & clean .docx** — runs all cleaners; loses formatting.
- **Clean & save as .docx (preserves format)** — modifies text in place; tables, images, hyperlinks, and styles are untouched. Output: `<original>-cleaned.docx`.
- **⚡ Strict clean** — applies all 24 transformations (BOM, line endings, bullets, markdown headings, etc.).

### B. Validate references before submission
**Citations** tab → **Open draft** (`.txt`/`.docx`) → **Open .bib file** → **Validate citations**. Three lists appear:
- **Undefined citations** — in your draft but not in your `.bib` (most likely fabricated or wrong).
- **Unused references** — in your `.bib` but never cited.
- **Citation count per reference** — spot token citations.

### C. Check whether a draft still sounds like "you"
**Style Analysis** tab → paste your draft on the left → paste ≥1,000 words of **your own prior published writing** on the right → **Analyze & compare**. The overall distance score and per-feature interpretations tell you whether the draft drifts from your usual register.

### D. Disclose AI assistance (recommended)
**Disclosure** tab → pick your venue (ICMJE, Nature, IEEE, Elsevier, ACL, or Generic) → fill in tool, task, model, name → **Generate disclosure** → paste the statement into your manuscript or cover letter. Each template links to the venue's official AI-use policy.

### E. Verify ScholarScribe's privacy claims yourself
**Privacy Audit** tab — watch the live log as you use other tabs. The only outbound host you should ever see is `registry.ollama.ai` (model downloads, no text). Anything else is a red flag.

## 5. Privacy in one sentence

**Nothing you write, paste, or open ever leaves your device.** Drafts, `.bib` files, chat messages, and reference samples stay in memory or in local JSON files. The Privacy Audit log is in-memory only and is cleared on app close.

## 6. Ethical use — please read

ScholarScribe is designed for researchers who have **genuinely written** their manuscript and want transparent, local AI assistance.

- ✅ Helps you draft, paraphrase, and refine **your own** writing.
- ✅ Validates citations and generates disclosure statements.
- ✅ Educates you about how AI detectors work and where they fail.
- ✅ Maintains timestamped evidence of your writing process.
- ❌ Does **not** evade AI detectors or lower detection scores.
- ❌ Does **not** help misrepresent AI-generated text as original human work.
- ❌ Does **not** contact any third-party API (OpenAI, Anthropic, Google, etc.).

If you used AI assistance, **disclose it** — the Disclosure tab makes this easy. See [`docs/ETHICS.md`](docs/ETHICS.md) for the full policy.

## 7. Need more detail?

- **Full reference manual:** [`USER_MANUAL.md`](USER_MANUAL.md)
- **Source code:** <https://github.com/waleedmandour/scholarscribe>
- **Report a bug:** <https://github.com/waleedmandour/scholarscribe/issues>
- **Security policy:** [`SECURITY.md`](SECURITY.md)

---

*ScholarScribe © 2026 Dr. Waleed Mandour. Released under the MIT License.*
*Persistent identifier: <https://doi.org/10.5281/zenodo.20781043>*
