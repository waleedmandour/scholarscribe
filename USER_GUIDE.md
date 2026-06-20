# ScholarScribe — User Guide (5-minute quick start)

*A focused walkthrough to get you productive in 5 minutes. For full reference, see [USER_MANUAL.md](USER_MANUAL.md).*

---

## Step 1 — Install (2 minutes)

1. **Install Ollama** from <https://ollama.com/download> (free, ~150 MB). Look for the llama icon in your system tray.
2. **Install ScholarScribe** — download the latest `.msi` from the [Releases page](https://github.com/waleedmandour/scholarscribe/releases) and double-click.

That's it. Launch ScholarScribe from your Start menu.

## Step 2 — Download a model (1 minute)

1. Open ScholarScribe. The sidebar shows **Ollama backend: running** (green pill).
2. Go to the **Models** tab.
3. The top card shows your PC's specs (CPU, RAM). Pick a model that fits your RAM:
   - **8 GB RAM** → Gemma 3 4B or Qwen 3 4B or Phi-4 Mini
   - **16 GB RAM** → Gemma 3 12B or Qwen 3 8B or Phi-4 14B
   - **32 GB RAM** → Gemma 3 27B or Qwen 3 32B or DeepSeek R1 32B
4. Click **Download**. Wait 2-15 minutes depending on the model and your connection.

**Already have a `.gguf` file?** Skip the catalog. Use "Pick .gguf file…" — ScholarScribe checks your RAM, then imports it via Ollama. Zero network.

## Step 3 — Try one feature (2 minutes)

Pick whichever matches what you're doing right now:

### If you have a messy PDF-extracted draft

1. **Text Cleaner** tab → "Open file…" → pick your `.txt`, `.md`, or `.docx` file (or just paste text).
2. Leave the 12 cleaning options at their defaults.
3. Click **Clean text** (or **Extract & clean .docx** for Word files).
4. See the diff stats and cleaned output. Click **Copy cleaned** to copy to your clipboard.

**Need to preserve tables/images in a .docx?** Use **Clean & save as .docx (preserves format)** instead. It modifies each text run in place — your tables, images, hyperlinks, and styles are untouched.

### If you have a .bib file and a draft with citations

1. **Citation Manager** tab → "Open draft" → pick your `.txt` or `.docx` file.
2. "Open .bib file" → pick your `.bib` (BibTeX) file.
3. Click **Validate citations**.
4. Three lists appear:
   - **Undefined citations** — in your draft but not in your `.bib`. **These are the dangerous ones** — likely fabricated or wrong.
   - **Unused references** — in your `.bib` but never cited. Trim your reference list before submission.
   - **Citation count per reference** — how many times each entry is cited.

### If you want a quick health check on your draft

1. **Document Statistics** tab → "Open file…" or paste text.
2. See word count, sentence count, paragraph count, section count, citation count, reading time, readability scores.
3. Compare to common journal targets (Nature, ICMJE, IEEE, ACL) to see if you're in the right ballpark.

### If you want to know if your draft sounds like "you"

1. **Style Analysis** tab → paste your draft (or open a file) in the left panel.
2. Paste a sample of **your own prior published writing** in the right panel (1,000+ words).
3. Click **Analyze & compare**.
4. See the overall distance score + feature-by-feature comparison. Use this to spot drafts that drift away from your voice.

### If you used AI assistance and need to disclose it

1. **Disclosure** tab → pick your target venue (ICMJE, Nature, IEEE, Elsevier, ACL, or Generic).
2. Fill in the tool name (e.g. "ChatGPT"), what you used it for, your name (optional).
3. Click **Generate disclosure** → copy the statement into your manuscript or cover letter.

---

## That's it — you're productive

The other tabs are:

- **Chat** — local-only chat with your installed model (refuses evasion/fabrication requests)
- **Detector Literacy** — read this if you've been accused of AI use or want to understand how detectors work
- **Privacy Audit** — see exactly what ScholarScribe is doing (every file read + outbound HTTP call)
- **Saved Work** — opt-in to save drafts locally (off by default)
- **About** — credits and version info

## Privacy in one sentence

**Nothing you write, paste, or open ever leaves your device.** The only outbound network call is to `registry.ollama.ai` when you click "Download" on a model — and that carries no text or usage data. Verify this yourself in the Privacy Audit tab.

## Need more detail?

- Full reference manual: **[USER_MANUAL.md](USER_MANUAL.md)**
- Troubleshooting: see "Troubleshooting" section of the manual
- Source code: <https://github.com/waleedmandour/scholarscribe>
- Report a bug: <https://github.com/waleedmandour/scholarscribe/issues>

## Ethical use

ScholarScribe does **not** evade AI detectors. It does **not** help misrepresent AI-generated text as original human work. If you used AI assistance, **disclose it** — the Disclosure tab makes this easy. See `docs/ETHICS.md` for the full policy.
