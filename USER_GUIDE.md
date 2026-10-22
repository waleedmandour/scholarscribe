# ScholarScribe User Guide

> A privacy-first, local-LLM writing companion for researchers. Runs entirely on your device, no telemetry, no paid APIs, all inference local.
> **Version 2.2.1 (stable) · Windows · macOS · Linux · MIT License · [github.com/waleedmandour/scholarscribe](https://github.com/waleedmandour/scholarscribe)**
> **Dr. Waleed Mandour** · Assistant Lecturer, Sultan Qaboos University · w.abumandour@squ.edu.om

ScholarScribe helps you draft, clean, validate, and disclose your manuscript using open LLMs that run on your own machine. This 2-page guide gets you productive in under 10 minutes. For deep reference, see [`USER_MANUAL.md`](USER_MANUAL.md).

---

## 1. Install (2 minutes)

1. **Install Ollama**, the free, open-source local LLM runtime: download from <https://ollama.com/download> (~150 MB) and run the installer. Look for the llama icon in your system tray.
2. **Install ScholarScribe**: download the installer for your platform from the [Releases page](https://github.com/waleedmandour/scholarscribe/releases): `.msi`/`.exe` (Windows), `.dmg` (macOS), or `.deb`/`.AppImage` (Linux), and run it. Optionally verify your download against the release's `CHECKSUMS.txt` (SHA-256).
3. **Launch ScholarScribe.** The sidebar should show a green **Ollama backend: running** pill. If it's red, start the Ollama service from your tray.

> **Build from source** (optional): `git clone … && npm install && npm run tauri build`. Requires Rust 1.77+, Node 18+, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## 2. Download a model (1 minute)

Open the **Models** tab. The top card shows your CPU and RAM, and the card below it shows whether the local Ollama daemon is connected (version, installed model count, endpoint `127.0.0.1:11434`) or not detected, with setup steps and a Re-check button. A search box filters both your installed models and the recommended catalog by name, tag, or description. Pick a model that fits your memory:

| Your RAM | Recommended models |
|---|---|
| 8 GB | Gemma 3 4B · Qwen 3 4B · Phi-4 Mini |
| 16 GB | Gemma 3 12B · Qwen 3 8B · GPT-OSS 20B |
| 32 GB | Gemma 3 27B · Qwen 3 32B · DeepSeek R1 32B |
| 64 GB+ | Llama 3.3 70B · GPT-OSS 120B |

Click **Download** and wait 2-15 minutes (depending on model size and connection). **Already have a `.gguf` file?** Click **Pick .gguf file…**: ScholarScribe checks your RAM and imports it via Ollama with zero outbound network.

## 3. The 22 tabs at a glance

The sidebar groups the tabs into 7 workflow phases that follow the order you'd typically use while writing a manuscript:

| # | Tab | What it does |
|---|---|---|
| | **Get started** | |
| 1 | **About** | Version, environment, developer credits, re-open the welcome tour |
| 2 | **Models** | Install, import, and manage local LLMs |
| | **Prepare the draft** | |
| 3 | **Text Cleaner** | Fix 23 PDF/OCR/web artifacts in `.txt`, `.md`, or `.docx` |
| 4 | **Language Check** | Offline spelling, grammar, and style checks with one click corrections (English, five dialects) |
| 5 | **Citations** | Validate in-text citations against your `.bib` file |
| 6 | **Word Assist** | As-you-type grammar feedback inside Microsoft Word via a local, opt-in listener (feature branch; off until you enable it) |
| | **Understand the draft** | |
| 7 | **Stats** | Word count, readability, journal-target comparison |
| 8 | **Structure** | Heading tree + missing-section suggestions |
| | **AI writing help** | |
| 9 | **Abstract** | LLM-generated Background/Methods/Results/Conclusions |
| 10 | **Writing Coach** | Local-LLM coaching on a paragraph or argument |
| 11 | **Chat** | Local-only chat (refuses evasion/fabrication requests) |
| | **Authenticity and style** | |
| 12 | **Detector Literacy** | How AI detectors work, and where they fail |
| 13 | **Risk Profile** | Does your draft share surface features with AI text? |
| 14 | **Style Analysis** | Compare a draft to your own prior writing |
| 15 | **Fingerprint** | Multi-paper stylistic fingerprint of your writing |
| 16 | **Voice Check** | Spot within-document stylistic shifts |
| | **Evidence and compliance** | |
| 17 | **Journal** | Auto-saved timestamped snapshots of your draft |
| 18 | **Provenance** | Export a signed record of your document's real revision history (opt-in) |
| 19 | **Appeal Letter** | Generate an evidence-based appeal if falsely flagged |
| 20 | **Disclosure** | Generate venue-compliant AI-use statements |
| | **Privacy and app** | |
| 21 | **Privacy Audit** | Every file read + outbound HTTP call, logged live |
| 22 | **Saved Work** | Opt-in local JSON persistence (off by default) |

The sidebar footer also holds the **A-/A/A+ font-scale control** and the **Comfortable / Compact density toggle**; both persist across restarts.

## 4. Common workflows

**Clean a messy PDF-extracted draft.** In the **Text Cleaner** tab, click **Open file…** (or paste), leave the 12 default cleaners on, and click **Clean text**. For Word documents: **Extract & clean .docx** runs all cleaners and loses formatting; **Clean & save as .docx (preserves format)** modifies text in place and writes `<original>-cleaned.docx` with tables, images, hyperlinks, and styles untouched. **Strict clean** applies all 23 transformations. **Convert to LaTeX (.tex)** maps Title/Heading styles, bulleted lists, bold/italic runs, and tables into a compilable LaTeX article, deterministically and fully local; anything unmapped is listed in the warnings card.

**Validate references before submission.** In the **Citations** tab, **Open draft**, **Open .bib file**, then **Validate citations**. You get undefined citations (in your draft but not your `.bib`, most likely fabricated or wrong), unused references, and per-reference citation counts. You can also paste raw references to convert them to BibTeX (v2.2.1 r4).

**Check whether a draft still sounds like "you".** In the **Style Analysis** tab, paste your draft on the left, paste at least 1,000 words of **your own prior published writing** on the right, then click **Analyze & compare**. The distance score and per-feature interpretations show whether the draft drifts from your usual register.

**Disclose AI assistance (recommended).** In the **Disclosure** tab, pick your venue (ICMJE, Nature, IEEE, Elsevier, ACL, or Generic), fill in tool, task, model, and name, then click **Generate disclosure**. Each template links to the venue's official AI-use policy.

**Writing Provenance in brief (v2.1.0).** Off by default; nothing is recorded until you opt in. Write in Word with Track Changes on, opt in under the Provenance tab, choose the `.docx` (or connect a Google Doc, read-only, every call logged), review the session timeline, and export the signed `.zip`: hashes and counts, never your text. Anyone can verify it offline with `verifier/index.html`: chain intact, signature valid. Full spec: `docs/PROVENANCE_SPEC.md`.

**Audit the privacy claims yourself.** Open the **Privacy Audit** tab and watch the live log as you use other tabs. ScholarScribe's own process should contact only `127.0.0.1:11434` (your local Ollama), plus Google's endpoints only if you connect a Google Doc. Model downloads are fetched by the Ollama daemon itself and do not appear in this in-app log.

## 5. Privacy in one sentence

**Your writing stays on your device.** Drafts, `.bib` files, chat messages, and reference samples stay in memory or in local JSON files, and AI inference runs only through your local Ollama. The one designed exception: if you explicitly connect a Google Doc in the Provenance tab, its revision text is fetched from Google and processed locally; every such call is logged in the Privacy Audit tab. The log itself is in-memory only and is cleared on app close.

## 5a. Word Assist in Word (Preview)

Word Assist is an opt-in, feature-build extra: while it is on, ScholarScribe serves a task-pane add-in inside Microsoft Word that flags **grammar** issues as you type. There is no accuracy claim for this preview: spelling and style flags are off by default, and the optional rewrite feature (Tier 1) is **Experimental** — it stays off until you name a local Ollama model in the Word Assist tab, and every rewrite passes six validation rules (citations, numbers, URLs/DOIs, math, quotes, protected content) before Word shows it.

**One-time setup** (all manual, all undoable — full walkthrough in [`docs/WORD_ASSIST_INSTALL.md`](docs/WORD_ASSIST_INSTALL.md)):

1. In the Word Assist tab, accept the disclosure and **Enable**. This starts a loopback listener on `https://localhost:38080` (port configurable) bound to `127.0.0.1` only.
2. Trust the per-install certificate with the copyable command for your OS (Windows: `certutil -user -addstore Root ...`; macOS: `security add-trusted-cert -r trustRoot ...`). The app never writes your certificate store itself.
3. Export the manifest file and sideload it (Windows: trusted shared-folder catalog; macOS: the `wef` folder), then open the pane in Word.
4. Generate a pairing code in the app and type it into the pane. Codes are 6 characters, expire after 5 minutes, and work once. Each app restart or disable/re-enable invalidates the old session, so re-pair then.

**Using it**: underline marks appear on grammar issues in annotation mode (WordApi 1.7/1.8 + Microsoft 365), or as a list with clickable suggestions in fallback mode (older Word builds). Click a suggestion to apply it; click reject (or just ignore) to do nothing. Use **Re-check** after large pastes. A popup action applies the suggestion **exactly once** and the ledger records the count — never the text.

**Troubleshooting**:

| Symptom | Meaning and fix |
|---|---|
| The pane shows "ScholarScribe is not reachable" | The app is closed or Word Assist is not enabled. Open the app, enable Word Assist; check the port matches the pane's expectation. |
| Certificate / TLS error when Word loads the pane | The per-install certificate is not trusted yet (or was regenerated). Re-run the trust command from step 2 — the certificate file path is shown in the Word Assist tab. |
| "Port 38080 is already in use" | Another process (or a second ScholarScribe copy) holds the port. Change the port in Word Assist settings and re-enable; Word fetches the manifest at the new port automatically. |
| Pairing always fails | The code expired (5 minutes) or was already used. Generate a fresh code; both sides must see the same port. |
| macOS: a keychain prompt appears | The OS is asking whether ScholarScribe may read its own stored key. That prompt is macOS, not a new network action. Denying it makes Word Assist fall back to a 0600 file in the app data folder (disclosed in the audit log). |
| Underlines never appear, only the list | Your Word build lacks WordApi 1.7/1.8 or the Microsoft 365 subscription the annotation APIs require. List mode is the designed fallback; nothing is broken. |
| A rewrite chip never appears | Tier 1 is off until a model name is set in the Word Assist tab, and it needs Ollama running. |

**Uninstall**: remove the add-in (Windows: My Add-ins → Remove / clear the trusted catalog; macOS: delete the manifest from the `wef` folder), then revoke certificate trust (Windows: `certutil -user -delstore Root "ScholarScribe Word Assist (localhost)"`; macOS: `security remove-trusted-cert ...` and `security delete-certificate -c "ScholarScribe Word Assist (localhost)"`), and click **Disable** (or Unpair) in the app. Every step's exact command is in [`docs/WORD_ASSIST_INSTALL.md`](docs/WORD_ASSIST_INSTALL.md).

## 6. Ethical use, please read

ScholarScribe is designed for researchers who have **genuinely written** their manuscript and want transparent, local AI assistance.

- **It does** help you draft, paraphrase, and refine **your own** writing; validate citations; generate disclosure statements; maintain local timestamped snapshots of your writing process; and educate you about how AI detectors work and where they fail.
- **It does not** evade AI detectors or lower detection scores, and does **not** help misrepresent AI-generated text as original human work.
- **It does not** contact any third-party AI API (OpenAI, Anthropic, etc.). The app itself talks only to `127.0.0.1:11434` (always), Google's OAuth/Docs endpoints (only if you explicitly connect a Google Doc), and logs every outbound call live in the Privacy Audit tab; model downloads are fetched by the Ollama daemon at your request from `registry.ollama.ai`/`hf.co` and carry no text.

If you used AI assistance, **disclose it**, the Disclosure tab makes this easy. See [`docs/ETHICS.md`](docs/ETHICS.md) for the full policy.

## 7. Need more detail?

**Full reference manual:** [`USER_MANUAL.md`](USER_MANUAL.md) · **Source code:** <https://github.com/waleedmandour/scholarscribe> · **Report a bug:** <https://github.com/waleedmandour/scholarscribe/issues> · **Security policy:** [`SECURITY.md`](SECURITY.md)

---

*ScholarScribe © 2026 Dr. Waleed Mandour. Released under the MIT License. Persistent identifier: <https://doi.org/10.5281/zenodo.23324356>*
