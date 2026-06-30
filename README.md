# ScholarScribe

> A privacy-first, local-LLM writing companion for researchers. Runs entirely on your device — no telemetry, no cloud calls, no paid APIs.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: v0.2.0](https://img.shields.io/badge/Status-v0.2.0-brightgreen.svg)](https://github.com/waleedmandour/scholarscribe/releases/tag/v0.2.0)
[![Platform: Cross-platform](https://img.shields.io/badge/Platform-Linux%20·%20Windows%20·%20macOS-blue.svg)]()
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

ScholarScribe helps researchers who are writing their own manuscripts to:

- **Get a guided 5-step welcome tour** *(new in v0.2.0)* — on first launch, an interactive modal walks you through privacy, model install, the 19 tools, and ethical use. Re-openable any time from the sidebar or the About tab.
- **Enjoy comfortable typography** *(new in v0.2.0)* — every text size has been bumped for reading and navigation comfort (body 15px, h1 22px, h2 17px, line-height 1.6). No fonts changed — just sizes and spacing.
- **Run open LLMs fully offline** — Gemma 3, Qwen 3, Phi-4, DeepSeek R1, Llama 3.3, and more. No paid APIs, no OpenAI/Anthropic/Google calls.
- **Import local `.gguf` files** — pick a model file you already downloaded (e.g. from HuggingFace); ScholarScribe checks whether your device has enough RAM, then registers it with Ollama.
- **Clean messy text** with the AI Text Cleaner — 24 rule-based transformations (12 default + 11 strict) for PDF/web/OCR artifacts: broken hyphens, ligatures, mojibake, page numbers, broken citations, hidden chars, asterisks, markdown headings, ellipsis, bullets, BOM, non-breaking spaces, Unicode whitespace, and more. One-click "⚡ Strict clean" applies all 24.
- **Two `.docx` modes**: extract text only (loses formatting, runs all cleaners), or clean in place (preserves all tables, images, hyperlinks, headers/footers, styles, track changes).
- **Validate citations** against your `.bib` file — lists undefined citations, unused references, and broken in-text citations. Reduces the risk of fabricated references.
- **See document statistics** — word count, section count, citation count, reading time, and comparison with common journal targets.
- **Analyze document structure** — extract heading tree, get suggestions for missing sections (Introduction, Methods, Results, Discussion, Conclusion, etc.), spot short sections.
- **Generate a structured abstract** — local LLM produces a Background/Methods/Results/Conclusions abstract from your draft. Also generates section-by-section commentary.
- **Assess authenticity risk** — surface whether your draft's perplexity/burstiness proxies overlap with typical AI-text profiles (based on Liang et al. 2023; Weber-Wulff et al. 2023). Not an evasion tool — helps you understand your writing's stylistic fingerprint.
- **Check voice consistency** — flag within-document stylistic inconsistencies where sentence length, hedging, or vocabulary abruptly shifts.
- **Build a multi-paper stylistic fingerprint** — analyze several of your prior papers together to characterize your baseline writing style, then compare a draft against that fingerprint.
- **Maintain a writing process journal** — auto-saves timestamped snapshots of your draft, creating a verifiable process record for authentic authorship evidence.
- **Generate a false-positive appeal letter** — if falsely flagged by an AI detector, generate a professional, evidence-based appeal letter citing the peer-reviewed literature.
- **Get local-LLM coaching on a paragraph or argument** — the Writing Coach tab gives you focused, in-context feedback without leaving the app.
- **Analyze whether a draft sounds like *your own* prior writing** — descriptive statistics including sentence length, hedging, passive voice, plus readability metrics (Flesch, Flesch-Kincaid, Gunning Fog).
- **Generate venue-compliant AI-use disclosure statements** for ICMJE, Nature, IEEE, Elsevier, ACL, and more.
- **Understand how AI detectors actually work** — and where they fail. Educational content with peer-reviewed citations.
- **Verify the app's own privacy claims** via an in-app Privacy Audit log of every file read and outbound HTTP call.
- **Save drafts locally** — opt-in persistence stores your work as plain JSON files on your device. Never synced to the cloud.
- **Light, dark, and auto themes** — click the icon in the sidebar to cycle.

---

## Ethical Use — please read

ScholarScribe is designed for researchers who have genuinely written or substantially contributed to a manuscript and wish to work with AI assistance transparently and on their own device.

### What ScholarScribe does

- Helps you draft, paraphrase, and refine **your own** writing with a local LLM.
- Compares a draft's stylistic profile to a sample of **your own prior writing** so you can decide whether the draft still sounds like you.
- Generates **disclosure statements** so you can comply with journal and conference AI-use policies.
- Educates you about how AI-detection tools work and what their known limitations are.
- Validates your citations against your `.bib` file — reducing the risk of fabricated references in your manuscript.
- Assesses whether your writing shares surface features with AI-generated text (not an evasion tool — an awareness tool).
- Maintains a timestamped draft history as verifiable evidence of authentic authorship.
- Generates evidence-based appeal letters for researchers falsely flagged by AI detectors.
- Cleans text artifacts from copy-pasted content without rewriting it.

### What ScholarScribe explicitly does NOT do

- **Does not target or attempt to lower AI-detection scores.** No "marker-targeting" engine. No Turnitin/GPTZero/Originality score-reduction pipeline. No stealth modes.
- **Does not help misrepresent AI-generated text as original human work.** The chat module's system prompt explicitly refuses requests to evade detectors or submit AI output as one's own.
- **Does not contact any third-party API.** The only network call is to `registry.ollama.ai` when you choose to download a model — and that call carries no text, no prompts, no usage data.
- **Does not collect telemetry, analytics, or crash reports.**
- **Does not read any file you didn't explicitly pick in a file dialog.**
- **Does not fabricate citations.** The Chat tab's system prompt forbids this. The Citation Manager tab exists precisely to catch accidental fabrication by listing every in-text citation that doesn't match a real `.bib` entry.

If you are looking for a tool to "humanize" AI text to bypass Turnitin, this is not that tool. The author of this project believes detection-evasion tools cause net harm to research integrity — and disproportionately harm honest researchers, especially non-native English writers, who are most likely to be falsely accused of AI use and would be most harmed by an arms race between evaders and detectors.

### Your responsibility

Users are responsible for compliance with their institution's AI-use policies and the policies of any venue to which they submit. When in doubt, disclose AI assistance. When still in doubt, ask your editor.

See [`docs/ETHICS.md`](docs/ETHICS.md) for the full ethical-use policy.

---

## Installation

### 1. Install Ollama (the local LLM runtime)

ScholarScribe uses [Ollama](https://ollama.com) as its backend — a free, open-source local LLM runner.

1. Download Ollama from <https://ollama.com/download> (installer for Windows, macOS, or Linux, ~150 MB).
2. Run the installer. Ollama starts automatically as a background service.
3. On Windows, look for the Ollama icon (a llama) in your system tray. On macOS, in your menu bar. On Linux, it runs as a systemd service.

That's it — you don't need to use Ollama directly. ScholarScribe talks to it on `http://127.0.0.1:11434`.

### 2. Install ScholarScribe

**Option A — download the pre-built installer (recommended for most users)**

Grab the latest installer for your platform from the [Releases page](https://github.com/waleedmandour/scholarscribe/releases):

| Platform | Installer | Status |
|---|---|---|
| **Linux** (Debian/Ubuntu) | `.deb` | ✅ Built for v0.2.0 |
| **Linux** (Fedora/RHEL/SUSE) | `.rpm` | ✅ Built for v0.2.0 |
| **Windows** | `.msi` / `.exe` | Build from source, or wait for the v0.2.1 Windows build |
| **macOS** (Intel / Apple Silicon) | `.dmg` | Build from source, or wait for the v0.2.1 macOS build |

Install with your platform's package manager:

```bash
# Linux (Debian/Ubuntu)
sudo apt install ./ScholarScribe_0.2.0_amd64.deb

# Linux (Fedora/RHEL/SUSE)
sudo dnf install ./ScholarScribe-0.2.0-1.x86_64.rpm

# Windows / macOS — double-click the .msi / .dmg
```

**Option B — build from source**

Requires Rust 1.77+, Node.js 18+, and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform (Microsoft Visual Studio C++ Build Tools + WebView2 on Windows; Xcode Command Line Tools on macOS; `libwebkit2gtk-4.1-dev` and friends on Linux).

```bash
git clone https://github.com/waleedmandour/scholarscribe.git
cd scholarscribe
npm install
npm run tauri build
```

The installers appear in `src-tauri/target/release/bundle/`.

**Option C — let a bootstrap script do it (Windows only)**

If you're on a fresh Windows machine, `scripts/build-windows.ps1` will check for / install all prerequisites (Rust, Node, MSVC, WebView2, Ollama), run `npm install` and `cargo check`, then build the .msi. Run it from PowerShell:

```powershell
.\scripts\build-windows.ps1
```

### Quick start (5 minutes)

When you first launch ScholarScribe, an **interactive 5-step welcome tour** appears automatically: Welcome → Privacy → Install a model → 19 tools at a glance → Ethical use. You can dismiss it and re-open it any time from the sidebar footer ("✦ Walk me through the app") or the About tab.

See **[`USER_GUIDE.md`](USER_GUIDE.md)** for a focused 2-page walkthrough. The longer reference manual is in **[`USER_MANUAL.md`](USER_MANUAL.md)**. A polished PDF copy of the user guide is attached to every [release](https://github.com/waleedmandour/scholarscribe/releases).

---

## Feature overview

### Models tab

Install and manage open LLMs. The catalog includes 14 models spanning 4 GB to 64 GB RAM:

- **Gemma 3 family** (Google 2025): 1B / 4B / 12B / 27B — multimodal, strong on academic text
- **Qwen 3 family** (Alibaba 2025): 4B / 8B / 14B / 32B — multilingual (30+ languages), hybrid thinking-mode
- **Phi-4** (Microsoft 2024): 14B + Mini 3.8B — STEM-specialized, trained on synthetic academic data
- **DeepSeek R1** (2025): 14B / 32B — open reasoning model with explicit chain-of-thought
- **Llama 3.3 70B** (Meta 2024) — flagship, requires 64 GB+ RAM
- **SciGLM 6B** (Tsinghua) — academic-tuned, trained on scientific papers

You can also:
- **Install by name** — any Ollama-supported model (e.g. `mistral:7b`, `command-r`)
- **Import a local `.gguf` file** — pick a model file you downloaded directly from HuggingFace. ScholarScribe checks your device's RAM against the model's needs (file size × 1.5) and shows a verdict: `ok`, `tight` (close other apps), or `insufficient` (model too large for your RAM). Import calls Ollama's `/api/create` — zero outbound network.

### Text Cleaner tab

**24 rule-based text transformations** that fix common artifacts from copy-pasted text — especially from PDFs, web pages, OCR, and word processors. Two presets:

- **Default** (12 ops): fix mojibake, expand ligatures, normalize quotes/dashes, strip zero-width/control chars, join hyphenated line breaks, join broken sentences, join broken URLs, fix broken citations, remove page numbers, collapse whitespace.
- **Strict** (24 ops, ⚡ button): all of the above PLUS strip BOM, normalize line endings (CRLF→LF), convert non-breaking spaces, normalize Unicode whitespace, strip soft hyphens, strip variation selectors, convert ellipsis (…→...), remove asterisks (*), remove markdown headings (#), normalize bullets (•→-), collapse repeated punctuation (!!!→!).

| Default operation | What it fixes |
|---|---|
| Fix mojibake | `â€™` → `'`, `â€"` → `—`, `Ã©` → `é` (UTF-8 decoded as Latin-1) |
| Expand ligatures | `ﬁ` → `fi`, `ﬂ` → `fl`, `ﬀ` → `ff`, `ﬃ` → `ffi` |
| Normalize quotes | Curly → straight (off by default; preserves academic style) |
| Normalize dashes | `--` → `—`, en-dash → hyphen |
| Strip zero-width chars | U+200B/200C/200D/FEFF/2060 (invisible but cause issues) |
| Strip control chars | Non-printable C0/C1 chars except tab/newline |
| Join hyphenated line breaks | `exam-\nple` → `example` (classic PDF artifact) |
| Join broken sentences | Lines ending mid-sentence joined (preserves paragraph breaks) |
| Join broken URLs | `https://example.\ncom` → `https://example.com` |
| Fix broken citations | `(Smith,\n2020)` → `(Smith, 2020)` |
| Remove page numbers | Standalone number lines from PDF extraction |
| Collapse whitespace | Multiple spaces → one, trim trailing, 3+ newlines → 2 |

| Strict-only operation | What it fixes |
|---|---|
| Strip BOM | U+FEFF at start of file |
| Normalize line endings | CRLF → LF, lone CR → LF |
| Convert non-breaking spaces | U+00A0, U+2007, U+202F → ASCII space |
| Normalize Unicode whitespace | en/em/thin/hair/figure/ideographic spaces → ASCII space |
| Strip soft hyphens | U+00AD (invisible chars that cause search misses) |
| Strip variation selectors | U+FE00–FE0F, U+E0100–E01EF (emoji/symbol modifiers) |
| Convert ellipsis | Unicode `…` → three ASCII dots `...` |
| Remove asterisks | All `*` characters (markdown bold/italic markers, footnote refs) |
| Remove markdown headings | Leading `#`, `##`, `###` from lines (preserves heading text) |
| Normalize bullets | `• ◦ ▪ ‣ ⁃` → ASCII hyphen `-` |
| Collapse repeated punctuation | `!!!` → `!`, `???` → `?`, `;;` → `;` |

**Two `.docx` modes** when a Word document is loaded:

1. **Extract & clean text** — extracts text and runs all enabled transformations. Loses formatting but applies every cleaner including cross-paragraph ones.
2. **Clean & save as .docx (preserves format)** — modifies each `<w:t>` text run in place. Preserves all tables, images, hyperlinks, headers/footers, footnotes/endnotes, styles, theme, and track changes. Saves to a new `.docx` (default name `<original>-cleaned.docx`). Cross-paragraph operations (join broken lines, fix broken citations, remove page numbers) are skipped because they'd require restructuring the document.

### Citation Manager tab

Validates your draft's in-text citations against your `.bib` (BibTeX) file. Three checks:

1. **Undefined citations** — every `(Author, Year)` or `(Author et al., Year)` in your draft that doesn't match an entry in your `.bib` file. These are the citations most likely to be fabricated or wrong.
2. **Unused references** — every `.bib` entry that's never cited in your draft. Helps you trim your reference list before submission.
3. **Citation count per reference** — how many times each `.bib` entry is cited, so you can spot references that are cited only once (often a sign of a token citation).

All parsing is local. No `.bib` content leaves your device. The BibTeX parser is hand-written (no third-party BibTeX dependency).

### Document Statistics tab

A quick health-check panel for your draft:

- Word count, sentence count, paragraph count, section count (extracted from headings)
- Citation count (any `(Author, Year)` or `[N]` pattern)
- Average sentence length, type-token ratio, complex-word ratio
- Estimated reading time (at 200 wpm)
- Flesch Reading Ease, Flesch-Kincaid Grade Level, Gunning Fog Index
- Comparison panel: how your draft compares to common journal targets (e.g. Nature articles average ~5,000 words; ICMJE medical articles ~3,500 words; IEEE conference papers ~6,000 words)

### Structure Analyzer tab

Extract a hierarchical view of your draft's heading tree and spot structural issues before submission:

- **Heading tree** — visual outline of all headings (`#`, `##`, `###`) detected in the document, with section lengths.
- **Missing-section suggestions** — flags missing canonical sections (Introduction, Methods, Results, Discussion, Conclusion, etc.) based on common academic structures.
- **Short-section warnings** — highlights sections under a configurable word-count threshold (often a sign of an under-developed argument).

All parsing is local; no document content leaves your device.

### Abstract Generator tab

Generate a structured abstract from your draft using a local LLM. Produces:

- A **Background / Methods / Results / Conclusions** abstract in the IMRAD-abstract style favored by most journals.
- Optional **section-by-section commentary** — short notes on each section of your draft, useful for catching weak spots.

Requires an installed model (see the Models tab).

### Risk Profile tab

Assesses whether your draft shares surface features with typical AI-generated text. Reports:

- **Perplexity and burstiness proxies** — the two main signals most AI-text detectors use.
- **Comparison to known AI-text and human-text distributions** (based on Liang et al. 2023; Weber-Wulff et al. 2023).
- **Plain-English interpretation** — "overlap is low / moderate / high".

**Not an evasion tool.** Risk Profile helps *you* understand whether your writing has stylistic fingerprints that overlap with AI text. It does not modify your draft or attempt to lower detection scores. See the Detector Literacy tab for context on why these proxies are imperfect.

### Voice Consistency tab

Flags within-document stylistic inconsistencies where sentence length, hedging, or vocabulary abruptly shifts between paragraphs. Useful when you've written sections at different times or co-authored with others — it surfaces the seams.

Reports per-paragraph metrics (sentence length, hedge density, vocabulary diversity) and flags paragraphs whose profile deviates significantly from the document's mean.

### Writing Journal tab

Auto-saves timestamped snapshots of your draft as you work, creating a verifiable process record for authentic authorship evidence. Each snapshot records:

- Timestamp (local time + UTC)
- Word count at time of snapshot
- Full text of the draft

Snapshots are stored locally as plain JSON files. You can export the entire journal as a single archive for evidence purposes (e.g. to demonstrate an authentic writing process if falsely accused of AI use).

### Appeal Letter tab

If you're falsely flagged by an AI detector, generate a professional, evidence-based appeal letter citing the peer-reviewed literature. Fill in your details (name, institution, manuscript title, venue, detector used, detector score) and your writing process — the local LLM produces a formal letter you can edit and send to your editor or institutional integrity office.

The letter template draws on Liang et al. (2023), Weber-Wulff et al. (2023), and Laban et al. (2024) — the same research that informs the Detector Literacy tab.

### Style Fingerprint tab

Build a multi-paper stylistic fingerprint of **your own** prior writing. Upload several of your prior papers; ScholarScribe aggregates their stylistic features (sentence length, vocabulary diversity, hedging, passive-voice density, connector density, readability metrics) into a single "fingerprint" profile. You can then compare a new draft against that fingerprint to spot drafts that drift away from your voice.

Differs from the Style Analysis tab (which compares a draft to a single reference sample) — Fingerprint compares against the *aggregate* of multiple papers, so it's more robust to variation between individual papers.

### Writing Coach tab

Focused, in-context coaching on a paragraph or argument. Paste a paragraph and ask the local LLM for feedback ("Is this argument clear?", "Suggest a stronger topic sentence", "What's missing from this methods section?"). The system prompt includes the same guardrails as the Chat tab — refuses evasion and fabrication requests.

### Style Analysis tab

Compare a draft's stylistic profile to a sample of **your own** prior writing. Reports 12 metrics:

- Sentence length (mean + standard deviation)
- Vocabulary diversity (type-token ratio)
- Passive-voice density
- Hedging density (perhaps, possibly, may, etc.)
- Connector density (however, moreover, therefore, etc.)
- First-person singular/plural ratio
- Citation density
- Reading-level metrics (Flesch, Flesch-Kincaid, Gunning Fog)
- Complex-word ratio

Output: overall distance score + feature-by-feature comparison with interpretations ("very close", "minor difference", "notable difference", "substantial difference"). Use this to spot drafts that drift away from your usual register.

**What this is — and isn't.** Style Analysis tells *you* how your draft compares to *your own* writing. It does not predict or attempt to lower AI-detector scores.

### Chat tab

Local-only chat with any installed model. The system prompt includes a guardrail that refuses requests to evade AI detectors or fabricate citations. Use it for:

- Brainstorming phrasing
- Asking for critique of a paragraph
- Generating outlines
- Sanity-checking an argument

### Disclosure Assistant tab

Generate venue-compliant AI-use disclosure statements for:

- **ICMJE** (medical journals: JAMA, NEJM, Lancet, BMJ, etc.)
- **Nature Portfolio** journals
- **IEEE** (all societies)
- **Elsevier** (2,500+ journals)
- **ACL / EMNLP / NAACL** (NLP conferences)
- **Generic / custom venue**

Fill in the tool used, the task, the model (optional), and your name (optional). One click generates a properly-formatted statement ready to paste into your manuscript or cover letter. Each template includes a link to the venue's official AI-use policy.

### Detector Literacy tab

Plain-English explainer of how AI-detection tools (Turnitin AI, GPTZero, Originality.ai) work and where they fail:

- **Perplexity and burstiness** — the two main signals most detectors use
- **Where detectors fail** — false-positive bias against non-native English writers, unreliability on short passages, sensitivity to editing, adversarial fragility
- **What this means for you** — practical guidance depending on whether you wrote the draft yourself, used AI assistance, or are an instructor/reviewer
- **Further reading** — Liang et al. (2023), Weber-Wulff et al. (2023), Laban et al. (2024)

Educational only — does not help you evade detection.

### Privacy Audit tab

In-session log of every file read + outbound HTTP call. The summary card shows:

- Total events, file reads, HTTP calls, Ollama commands
- Bytes in / out
- **Unique outbound hosts contacted** — should only ever show `registry.ollama.ai` (model downloads). Any other host is a red flag.

Filterable event table with timestamps. The audit log is **in-memory only** — cleared on app close. This is intentional: persisting it would create a record of every file you read, which is the opposite of privacy.

### Saved Work tab

Opt-in local persistence. Disabled by default. When enabled:

- Drafts, chat transcripts, disclosure statements save as plain JSON files in your platform's app-data directory (`%APPDATA%\com.scholarscribe.app\data\` on Windows, `~/Library/Application Support/com.scholarscribe.app/data/` on macOS, `~/.local/share/com.scholarscribe.app/data/` on Linux)
- Plain JSON — inspectable in any text editor
- Never synced to cloud (no OneDrive/Dropbox/iCloud integration by ScholarScribe)
- Per-draft delete + "Delete all"
- "Open folder in Explorer/Finder/File Manager" button to see exactly what's stored
- Full privacy disclosure dialog before enabling — explains what gets saved, where, encryption status, deletion behavior

The Privacy Audit log is **never** persisted — it stays in-memory only.

### About tab

Version, environment (CPU, RAM, OS), developer credentials (Dr. Waleed Mandour), acknowledgments crediting GLM 5.1 and GLM 5.2 (Z.ai) as engineering collaborators, and a "Walk me through the app" button to re-open the welcome tour.

---

## Privacy

| Property | Status |
|---|---|
| Telemetry | **None.** No analytics, no crash reporting, no usage tracking. |
| Network calls | One outbound call, to `registry.ollama.ai`, only when you click "Download" on a model. Carries no text or usage data. GGUF imports make zero outbound calls. |
| User text | **Never leaves your device.** Drafts, reference samples, chat messages, .bib files — all stay in memory or local files. |
| Third-party APIs | **None.** No OpenAI, Anthropic, Google AI, or any other cloud LLM API. |
| Crash reports | None collected. Errors are written to a local log file only. |
| Saved drafts | Opt-in only. Plain JSON files on your device: `%APPDATA%\com.scholarscribe.app\data\` (Windows), `~/Library/Application Support/com.scholarscribe.app/data/` (macOS), `~/.local/share/com.scholarscribe.app/data/` (Linux). Never synced. |
| Audit log | In-memory only. Cleared on app close. Never persisted. |

The CSP in `tauri.conf.json` explicitly restricts outbound connections from the UI to `127.0.0.1:11434` (your local Ollama). The Rust backend only contacts `ollama.com` for model downloads and nothing else.

If you want to verify this yourself:
1. Audit `src-tauri/src/ollama.rs` — every outbound HTTP call is in that file.
2. Watch the Privacy Audit tab while interacting with the app.
3. Run ScholarScribe behind a network monitor (GlassWire, Wireshark) and cross-reference outbound hosts.

See [`SECURITY.md`](SECURITY.md) for the full security policy.

---

## Project structure

```
scholarscribe/
├── src-tauri/                  Rust backend (Tauri 2)
│   ├── src/
│   │   ├── main.rs             Binary entry point (windows_subsystem attr here)
│   │   ├── lib.rs              Library: run() function, command registration
│   │   ├── commands.rs         Tauri command handlers (the API surface)
│   │   ├── ollama.rs           Ollama HTTP client (the ONLY outbound network code)
│   │   ├── style.rs            Style analysis (descriptive statistics + readability)
│   │   ├── text_cleaner.rs     12 cleaning operations + per-run variant for .docx
│   │   ├── docx_reading.rs     .docx → plain text extraction (zip + OOXML walk)
│   │   ├── citation_manager.rs BibTeX parser + citation validator
│   │   ├── document_stats.rs   Document statistics
│   │   ├── structure_analyzer.rs  Heading tree + missing-section detection
│   │   ├── abstract_generator.rs  Local-LLM structured abstract generation
│   │   ├── risk_profiler.rs    AI-text surface-feature analysis (awareness, not evasion)
│   │   ├── voice_consistency.rs   Within-document stylistic-shift detection
│   │   ├── writing_journal.rs  Timestamped draft snapshots for authorship evidence
│   │   ├── appeal_letter.rs    Evidence-based false-positive appeal generator
│   │   ├── style_fingerprint.rs   Multi-paper stylistic fingerprint aggregation
│   │   ├── disclosure.rs       Disclosure-statement generator
│   │   ├── persistence.rs      Opt-in local storage (settings + drafts)
│   │   └── audit.rs            In-memory privacy audit log
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                        Svelte frontend
│   ├── App.svelte              Shell + sidebar + theme toggle + welcome-tour mount
│   ├── lib/api.ts              Typed Tauri invoke wrappers
│   ├── lib/onboarding.ts       Welcome-tour visibility store (localStorage-backed)
│   └── components/
│       ├── Models.svelte
│       ├── AITextCleaner.svelte
│       ├── CitationManager.svelte
│       ├── DocumentStats.svelte
│       ├── StructureAnalyzer.svelte
│       ├── AbstractGenerator.svelte
│       ├── RiskProfiler.svelte
│       ├── VoiceConsistency.svelte
│       ├── WritingJournal.svelte
│       ├── AppealLetter.svelte
│       ├── StyleFingerprint.svelte
│       ├── WritingCoach.svelte
│       ├── StyleAnalysis.svelte
│       ├── Chat.svelte
│       ├── Disclosure.svelte
│       ├── DetectorLiteracy.svelte
│       ├── PrivacyAudit.svelte
│       ├── SavedWork.svelte
│       ├── WelcomeTour.svelte     5-step interactive first-run tour (v0.2.0+)
│       └── About.svelte
├── docs/
│   └── ETHICS.md               Full ethical-use policy
├── USER_GUIDE.md               2-page quick start (also shipped as PDF in releases)
├── USER_MANUAL.md              Full reference manual
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE
└── README.md (this file)
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The short version: bug reports and feature requests that align with the ethical-use policy are very welcome; pull requests attempting to add detection-evasion features will be closed without merge.

---

## Acknowledgments

ScholarScribe v0.2.0 — © 2026 **Dr. Waleed Mandour**, released under the MIT License.

### Developer

**Dr. Waleed Mandour**
Email: waleedmandour@github.com
GitHub: github.com/waleedmandour
ORCID: 0000-0002-9262-5993

### Engineering Collaborators

Designed and directed by Dr. Waleed Mandour, 2026. Gratefully developed with engineering support from:

- **GLM 5.1** (Z.ai) — architectural design and ethical-use policy formulation
- **GLM 5.2** (Z.ai) — implementation, continuous integration/continuous deployment, and iterative debugging

The AI agents functioned as engineering collaborators under the direction of Dr. Mandour, who reviewed all code, verified all claims, and retained final editorial authority over every aspect of the application.

### Open-Source Dependencies

Built on top of outstanding open-source work, including:

- [Tauri](https://tauri.app) — the cross-platform desktop framework that keeps the installer tiny
- [Ollama](https://ollama.com) — the local LLM runtime that does the heavy lifting of model management
- [Svelte](https://svelte.dev) — the frontend framework
- The open LLM authors: Google (Gemma), Alibaba (Qwen), Meta (Llama), Microsoft (Phi), DeepSeek

### Research Community

The Detector Literacy and Authenticity Risk Profiler modules are built upon the work of researchers who have rigorously evaluated the limitations of AI-generated text detectors, particularly:

- Liang et al. (2023) — documented false-positive bias against non-native English writers
- Weber-Wulff et al. (2023) — evaluated 14 detection tools and found all exhibited high false-positive rates

Their findings inform the application's ethical stance and its refusal to facilitate detection evasion.

---

## License

MIT © 2026 Dr. Waleed Mandour. See [LICENSE](LICENSE).

**Persistent identifier:** https://doi.org/10.5281/zenodo.20781043

**Funding Disclaimer:** This work is not funded by any institution. It was independently developed by the researcher and is dedicated to the academic community as a free and open-source tool.

---

*Built with ❤ to the Academic Community.*
