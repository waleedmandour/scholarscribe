# ScholarScribe: A Privacy-First Local LLM Writing Companion for Researchers
https://doi.org/10.5281/zenodo.20781043
**Version 0.1.8 (Pre-release) · MIT License · © 2026 Dr. Waleed Mandour**

---

## Abstract

ScholarScribe is a desktop application that supports researchers in the preparation of scholarly manuscripts through a suite of local, privacy-preserving tools. The application runs entirely on the user's device, requires no cloud-based application programming interfaces (APIs), collects no telemetry, and transmits no user-generated content to any external host. Drawing on contemporary research into the limitations of AI-generated text detectors (Liang et al., 2023; Weber-Wulff et al., 2023), ScholarScribe is explicitly designed *not* to facilitate the evasion of such detectors. Instead, it offers transparent, auditable tools for text cleaning, citation validation, document structure analysis, style analysis, AI-use disclosure generation, and structured abstract drafting—functions that align with established norms of research integrity.

The application is built on the Tauri 2 framework (Rust backend, Svelte frontend) and uses Ollama as its local large language model (LLM) runtime, enabling the use of open-weight models such as Google's Gemma 3, Alibaba's Qwen 3, Microsoft's Phi-4, DeepSeek R1, and Meta's Llama 3.3 without reliance on proprietary inference services.

---

## Table of Contents

1. [Introduction and Ethical Framework](#1-introduction-and-ethical-framework)
2. [System Requirements and Installation](#2-system-requirements-and-installation)
3. [Feature Inventory](#3-feature-inventory)
4. [Privacy and Security Architecture](#4-privacy-and-security-architecture)
5. [Project Structure](#5-project-structure)
6. [Development Roadmap](#6-development-roadmap)
7. [Contributing](#7-contributing)
8. [Acknowledgments](#8-acknowledgments)
9. [License](#9-license)
10. [References](#10-references)

---

## 1. Introduction and Ethical Framework

### 1.1 Purpose

ScholarScribe is designed for researchers who have authored or substantially contributed to a manuscript and who seek to work with AI assistance in a manner that is transparent, locally verifiable, and consistent with institutional and journal policies on research integrity. The application provides a consolidated environment for common writing-support tasks without compromising user privacy or facilitating academic misconduct.

### 1.2 What ScholarScribe Does

The application offers eleven modules:

- **Local LLM Management** — Installation and execution of open-weight LLMs via Ollama, including compatibility checking for user-supplied GGUF model files.
- **AI Text Cleaner** — Twenty-four rule-based text transformations for the remediation of artifacts introduced by PDF extraction, optical character recognition (OCR), and cross-application copy-paste operations.
- **In-place .docx Cleaning** — Modification of Word Open XML (OOXML) text runs while preserving document formatting, tables, images, hyperlinks, headers, footers, and tracked changes.
- **Citation Manager** — Validation of in-text citations against a user-supplied BibTeX bibliography, identifying undefined citations, unused references, and per-reference citation counts.
- **Document Statistics** — Quantitative analysis of draft documents including word counts, readability metrics (Flesch Reading Ease, Flesch–Kincaid Grade Level, Gunning Fog Index), and comparison with typical journal length expectations.
- **Structure Analyzer** — Extraction of document heading hierarchies and identification of potentially missing sections (e.g., Introduction, Methods, Results, Discussion, Conclusion).
- **Abstract Generator** — LLM-assisted generation of structured abstracts (Background, Methods, Results, Conclusions) using locally-installed models.
- **Style Analysis** — Comparison of a draft's stylistic profile against a sample of the author's own prior writing, including sentence-length statistics, hedging density, passive-voice density, and vocabulary diversity.
- **Local Chat** — A conversational interface to installed LLMs with a system prompt that explicitly refuses requests to evade AI detectors or fabricate citations.
- **Disclosure Assistant** — Generation of venue-compliant AI-use disclosure statements for ICMJE-affiliated journals, *Nature* Portfolio, IEEE, Elsevier, and ACL/EMNLP/NAACL.
- **Detector Literacy** — An educational module summarizing the operational principles and documented failure modes of AI-generated text detectors, with references to peer-reviewed evaluations.
- **Privacy Audit Log** — A real-time, in-session log of all file reads and outbound network calls, enabling users to verify the application's privacy claims directly.

### 1.3 What ScholarScribe Does Not Do

The following capabilities are deliberately excluded from ScholarScribe, irrespective of user demand, because their inclusion would facilitate academic misconduct or undermine research integrity:

1. **Detection-evasion functionality.** No feature targets the reduction of AI-detection scores (e.g., Turnitin AI, GPTZero, Originality.ai). No "marker-targeting" engine, no stealth modes, no adversarial-perturbation pipeline.
2. **Misrepresentation aids.** No feature whose purpose is to obscure the use of AI assistance, including fabricated revision histories or false draft-trail generators.
3. **Citation fabrication.** The chat module's system prompt explicitly forbids the generation of fictitious references. The Citation Manager exists to detect and surface accidental fabrication.
4. **Third-party AI APIs.** No calls to OpenAI, Anthropic, Google AI, or any other hosted inference service. The only outbound network call is to `registry.ollama.ai` when a user elects to download a model, and that call carries no user text or usage data.
5. **Telemetry.** No analytics, no crash reporting that transmits data, no usage tracking.

### 1.4 Ethical Rationale

The decision to exclude detection-evasion features reflects three considerations grounded in the published literature. First, AI-generated text detectors exhibit well-documented false-positive biases, particularly against writing by non-native English speakers (Liang et al., 2023); an arms race between evaders and detectors would exacerbate these harms rather than mitigate them. Second, the appropriate response to concerns about AI assistance is transparent disclosure, which is now required by major editorial policies (International Committee of Medical Journal Editors [ICMJE], 2024; *Nature* Editorial Policy, 2024). Third, the underlying signal produced by current detectors is insufficiently reliable to serve as a basis for integrity findings (Weber-Wulff et al., 2023), rendering evasion both unnecessary and counterproductive. The full ethical-use policy is documented in [`docs/ETHICS.md`](docs/ETHICS.md).

---

## 2. System Requirements and Installation

### 2.1 System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| Operating System | Windows 10 64-bit (version 1809 or later) | Windows 11 |
| System Memory | 8 GB RAM | 16 GB or more |
| Disk Space | 3 GB (application plus one small model) | 20 GB or more (multiple models) |
| WebView2 Runtime | Pre-installed on Windows 11; installed automatically on Windows 10 | — |

Individual models specify their own memory requirements; the Models tab displays these for each catalog entry.

### 2.2 Installation of Ollama

ScholarScribe uses [Ollama](https://ollama.com) as its local LLM runtime. Ollama is a free, open-source application that manages the download, registration, and execution of open-weight models on the user's device.

1. Download Ollama from <https://ollama.com/download> (Windows installer, approximately 150 MB).
2. Execute the installer. Ollama starts automatically as a background service.
3. Verify the presence of the Ollama icon in the system tray.

ScholarScribe communicates with Ollama via its local HTTP API at `http://127.0.0.1:11434`. No direct interaction with Ollama is required.

### 2.3 Installation of ScholarScribe

**Option A: Pre-built installer (recommended).** Download the latest `.msi` or `.exe` from the [Releases page](https://github.com/waleedmandour/scholarscribe/releases) and execute the installer.

**Option B: Build from source.** This option requires Rust 1.77 or later, Node.js 18 or later, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) (Microsoft Visual Studio C++ Build Tools and WebView2).

```powershell
git clone https://github.com/waleedmandour/scholarscribe.git
cd scholarscribe
npm install
npm run tauri build
```

The resulting installer is written to `src-tauri/target/release/bundle/`.

**Quick start.** See [`USER_GUIDE.md`](USER_GUIDE.md) for a five-minute walkthrough. The full reference manual is [`USER_MANUAL.md`](USER_MANUAL.md).

---

## 3. Feature Inventory

### 3.1 Models Tab

The Models tab provides a curated catalog of fourteen open-weight models spanning the 4 GB to 64 GB RAM range, including the Gemma 3 family (Google, 2025), the Qwen 3 family (Alibaba, 2025), Phi-4 (Microsoft, 2024), DeepSeek R1 (DeepSeek, 2025), Llama 3.3 (Meta, 2024), and SciGLM 6B (Tsinghua University). Users may also install any Ollama-supported model by name or import a local GGUF file. GGUF imports invoke Ollama's `/api/create` endpoint with zero outbound network traffic beyond the initial Ollama service call.

A compatibility checker compares the GGUF file size (multiplied by a factor of 1.5 to account for key–value cache and activation memory) against the system's total and available RAM, producing a verdict of `ok`, `tight`, or `insufficient`.

### 3.2 AI Text Cleaner Tab

The Text Cleaner implements twenty-four deterministic, rule-based text transformations organized into two presets:

- **Default preset (12 operations):** mojibake remediation, ligature expansion, quote and dash normalization, zero-width character removal, control character removal, hyphenated line-break joining, broken-sentence joining, broken-URL joining, broken-citation remediation, page-number removal, and whitespace collapse.
- **Strict preset (24 operations):** all default operations plus Byte Order Mark (BOM) removal, line-ending normalization (CRLF to LF), non-breaking space conversion, Unicode whitespace normalization, soft hyphen removal, variation selector removal, ellipsis conversion, asterisk removal, markdown heading removal, bullet normalization, and repeated-punctuation collapse.

Two modes are provided for Microsoft Word documents:

1. **Extract and clean text** — Extracts plain text from the OOXML and applies all enabled transformations. Document formatting is not preserved.
2. **Clean and save as .docx (preserves format)** — Modifies each `<w:t>` text run in place within the OOXML document parts (`word/document.xml`, `word/header*.xml`, `word/footer*.xml`, `word/footnotes.xml`, `word/endnotes.xml`), preserving all tables, images, hyperlinks, headers, footers, styles, themes, and tracked changes. Twenty per-run operations are applied; four cross-paragraph operations (join broken lines, join broken URLs, fix broken citations, remove page numbers) are skipped because they require structural modifications incompatible with format preservation.

A dedicated **"Strict clean and save as .docx"** button combines the strict preset with format-preserving output in a single action.

### 3.3 Citation Manager Tab

The Citation Manager validates in-text citations in a draft against a user-supplied BibTeX (`.bib`) file. Three checks are performed:

1. **Undefined citations** — In-text citations (parenthetical, narrative, or numeric style) that do not correspond to any entry in the bibliography. These are the most likely candidates for accidental fabrication.
2. **Unused references** — Bibliography entries that are never cited in the draft.
3. **Citation counts per reference** — The number of times each entry is cited, flagging entries cited only once as potential "token citations."

The BibTeX parser is hand-written in Rust and does not depend on third-party parsing libraries. All processing is local.

### 3.4 Document Statistics Tab

The Document Statistics tab provides a quantitative overview of the draft, including word, sentence, paragraph, section, citation, figure, and table counts; estimated reading time at 200 words per minute; and the three readability metrics (Flesch Reading Ease, Flesch–Kincaid Grade Level, Gunning Fog Index). A comparison panel reports the difference between the draft's word count and typical limits for nine venue categories (e.g., *Nature* research articles at approximately 5,000 words; ICMJE-affiliated medical journals at approximately 3,500 words; IEEE conference papers at approximately 6,000 words).

### 3.5 Structure Analyzer Tab

The Structure Analyzer extracts the document's heading hierarchy. For `.docx` files, headings are detected via Word's built-in heading styles (Heading1 through Heading6, Title) in the OOXML. For plain text, headings are detected via markdown-style `#` prefixes or lines composed predominantly of uppercase alphabetic characters. The analyzer reports the total number of sections, maximum heading depth, sections with fewer than 100 words (flagged as potentially underdeveloped), and a list of expected academic sections that are absent from the document (Introduction, Methods, Results, Discussion, Conclusion, References, Abstract, Limitations).

### 3.6 Abstract Generator Tab

The Abstract Generator uses a locally-installed LLM to produce a structured abstract comprising four labeled paragraphs (Background, Methods, Results, Conclusions) from the body of a manuscript. The system prompt instructs the model to produce an abstract of a specified maximum word count (default 250) tailored to a specified venue style. The draft text is transmitted only to the local Ollama instance; no portion of the manuscript leaves the user's device. A prominent review warning reminds the user that LLMs may produce hallucinated findings and that the output is a draft requiring verification.

### 3.7 Style Analysis Tab

The Style Analysis tab computes twelve descriptive stylistic metrics for a draft and a reference sample of the author's own prior writing, then reports the feature-wise differences. Metrics include average sentence length and standard deviation, type–token ratio, passive-voice density, hedging density (e.g., *perhaps*, *may*, *possibly*), connector density (e.g., *however*, *moreover*, *therefore*), first-person singular and plural ratios, citation density, and the three readability scores. An overall distance score summarizes the cumulative difference. This module is designed for authorial self-assessment and is not a predictor of AI-detection scores.

### 3.8 Chat Tab

The Chat tab provides a conversational interface to installed LLMs. The system prompt includes a guardrail that explicitly refuses requests to evade AI detectors, fabricate citations, or submit AI-generated content as original work.

### 3.9 Disclosure Assistant Tab

The Disclosure Assistant generates venue-compliant AI-use disclosure statements for six venue categories: ICMJE-affiliated medical journals (JAMA, NEJM, *The Lancet*, BMJ), *Nature* Portfolio journals, IEEE, Elsevier, ACL/EMNLP/NAACL, and a generic fallback. Each template includes a link to the venue's official AI-use policy.

### 3.10 Detector Literacy Tab

The Detector Literacy tab presents an educational summary of the operational principles of AI-generated text detectors (perplexity, burstiness, and marker-based approaches) and their documented limitations, including false-positive bias against non-native English writers (Liang et al., 2023), unreliability on short passages, sensitivity to editing, and adversarial fragility (Weber-Wulff et al., 2023). The module is informational only and does not facilitate detection evasion.

### 3.11 Privacy Audit Tab

The Privacy Audit tab maintains an in-memory log of every file read and outbound HTTP call performed by the application during the current session. The log is cleared on application close and is never persisted to disk. A summary card displays total events, file reads, HTTP calls, Ollama commands, bytes transferred, and the set of unique outbound hosts contacted (which should contain only `registry.ollama.ai`).

### 3.12 Saved Work Tab

The Saved Work tab provides opt-in local persistence of drafts, chat transcripts, and disclosure statements as plain JSON files in the operating system's application data directory (on Windows, `%APPDATA%\com.scholarscribe.app\data\`). Persistence is disabled by default; the user must explicitly enable it after reviewing a privacy disclosure. The Privacy Audit log is never persisted.

---

## 4. Privacy and Security Architecture

### 4.1 Privacy Commitments

| Property | Status |
|----------|--------|
| Telemetry | None. No analytics, no crash reporting, no usage tracking. |
| Outbound network calls | One host (`registry.ollama.ai`), contacted only when the user elects to download a model. The call carries no user text or usage data. GGUF imports make zero outbound calls. |
| User text | Never leaves the user's device. Drafts, reference samples, chat messages, and `.bib` files remain in memory or in local files. |
| Third-party AI APIs | None. The application does not call OpenAI, Anthropic, Google AI, or any other hosted inference service. |
| Saved drafts | Opt-in only. Plain JSON files in the application data directory. Never synchronized to cloud storage by the application. |
| Audit log | In-memory only. Cleared on application close. Never persisted. |

### 4.2 Verification

Users may verify these claims through three mechanisms:

1. **Source code audit.** All outbound HTTP calls are confined to `src-tauri/src/ollama.rs`. The Content Security Policy in `src-tauri/tauri.conf.json` restricts the frontend's `connect-src` to `self` and `127.0.0.1:11434`.
2. **In-application audit.** The Privacy Audit tab displays every file read and outbound call in real time.
3. **Network monitoring.** The application may be run behind a network monitor (e.g., GlassWire, Wireshark) to corroborate the in-application log.

The full security policy is documented in [`SECURITY.md`](SECURITY.md).

---

## 5. Project Structure

```
scholarscribe/
├── src-tauri/                       Rust backend (Tauri 2)
│   ├── src/
│   │   ├── main.rs                  Binary entry point
│   │   ├── lib.rs                   Library: run() function, command registration
│   │   ├── commands.rs              Tauri command handlers
│   │   ├── ollama.rs                Ollama HTTP client (sole outbound network code)
│   │   ├── style.rs                 Style analysis and readability metrics
│   │   ├── text_cleaner.rs          24 cleaning operations
│   │   ├── docx_reading.rs          .docx plain-text extraction
│   │   ├── citation_manager.rs      BibTeX parser and citation validator
│   │   ├── document_stats.rs        Document statistics
│   │   ├── structure_analyzer.rs    Heading-tree extraction and section analysis
│   │   ├── abstract_generator.rs    LLM-based abstract generation
│   │   ├── disclosure.rs            Disclosure-statement generator
│   │   ├── persistence.rs           Opt-in local storage
│   │   └── audit.rs                 In-memory privacy audit log
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                             Svelte frontend
│   ├── App.svelte                   Shell, sidebar, theme toggle
│   ├── lib/api.ts                   Typed Tauri invoke wrappers
│   └── components/                  Thirteen Svelte components (one per tab)
├── docs/ETHICS.md                   Full ethical-use policy
├── USER_GUIDE.md                    Five-minute quick start
├── USER_MANUAL.md                   Full reference manual
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE
└── README.md                        This document
```

---

## 6. Development Roadmap

- **v0.1.0** — Initial release: Models, Chat, Style Analysis, Disclosure, Detector Literacy.
- **v0.1.1** — Console-window remediation; GGUF import with compatibility check; reading-level metrics; Privacy Audit log; About/Credits.
- **v0.1.2** — GGUF import HTTP 400 remediation; dark/light/auto theme toggle; expanded models catalog (fourteen academic-focused models).
- **v0.1.3** — AI Text Cleaner (twelve rule-based transformations); opt-in local persistence.
- **v0.1.4** — `.docx` file reading via ZIP and OOXML traversal.
- **v0.1.5** — In-place `.docx` cleaning preserving all formatting.
- **v0.1.6** — Citation Manager (BibTeX validation); Document Statistics panel; comprehensive README and user guide.
- **v0.1.7** — Strict cleaning mode: eleven additional operations (24 total).
- **v0.1.8** (current) — Structure Analyzer; Abstract Generator; "Strict clean and save as .docx" button.
- **v0.2.0** (planned) — Multi-reference style profile (analysis against a folder of the author's papers).
- **v0.3.0** (planned) — Bundled llama.cpp runtime for systems without Ollama.
- **v0.4.0** (planned) — Citation-aware chat (LLM consultation of the user's `.bib` file).

---

## 7. Contributing

Contributions are welcome provided they align with the ethical-use policy documented in [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`docs/ETHICS.md`](docs/ETHICS.md). Bug reports, documentation improvements, accessibility fixes, and feature requests consistent with the stated scope will be considered. Pull requests that introduce detection-evasion functionality, telemetry, or dependencies on third-party AI APIs will be declined.

---

## 8. Acknowledgments

### 8.1 Authorship and Direction

ScholarScribe was conceived and directed by **Dr. Waleed Mandour** in 2026, who defined the project's ethical scope, feature priorities, and privacy requirements.

### 8.2 Engineering Collaboration

The application was engineered in collaboration with the GLM (General Language Model) family of AI agents developed by Z.ai. Specifically, **GLM 5.1** contributed to the initial architectural design and ethical-use policy formulation, and **GLM 5.2** implemented the Rust backend, Svelte frontend, GitHub Actions continuous integration and release workflows, and the iterative debugging required to achieve successful builds on the Windows platform. The collaboration between the human author and the AI agents is documented in the project's commit history.

The AI agents functioned as engineering collaborators under the direction of Dr. Mandour, who reviewed all code, verified all claims, and retained final editorial authority over every aspect of the application.

### 8.3 Open-Source Dependencies

ScholarScribe is built upon the following open-source projects, whose contributions are gratefully acknowledged:

- [Tauri](https://tauri.app) — Cross-platform desktop application framework.
- [Ollama](https://ollama.com) — Local LLM runtime.
- [Svelte](https://svelte.dev) — Frontend framework.
- [Rust programming language](https://www.rust-lang.org) and its ecosystem.
- The `zip`, `regex`, `serde`, `reqwest`, `tokio`, `sysinfo`, and `once_cell` crates.

### 8.4 Open-Weight Model Authors

The application supports models produced by the following organizations, whose open-weight releases make local, private AI assistance possible:

- Google (Gemma 3)
- Alibaba (Qwen 3)
- Meta (Llama 3.3)
- Microsoft (Phi-4)
- DeepSeek (DeepSeek R1)
- Tsinghua University / KEG (SciGLM)

### 8.5 Research Community

The Detector Literacy module is built upon the work of researchers who have rigorously evaluated the limitations of AI-generated text detectors, particularly Liang et al. (2023) and Weber-Wulff et al. (2023). Their findings inform the application's ethical stance and its refusal to facilitate detection evasion.

---

## 9. License

ScholarScribe is released under the **MIT License**. See [`LICENSE`](LICENSE) for the full text.

© 2026 Dr. Waleed Mandour. All rights are reserved to the extent permitted by the MIT License.

---

## 10. References

### American Psychological Association (APA) Style (7th Edition)

International Committee of Medical Journal Editors. (2024). *Recommendations for the conduct, reporting, editing, and publication of scholarly work in medical journals*. Retrieved from <https://www.icmje.org/recommendations/>

Liang, W., Yuksekgonul, M., Mao, Y., Wu, E., & Zou, J. (2023). GPT detectors are biased against non-native English writers. *Patterns, 4*(7), 100779. https://doi.org/10.1016/j.patter.2023.100779

*Nature* Editorial Policy. (2024). *Tools such as ChatGPT threaten transparent science*. Retrieved from <https://www.nature.com/editorial-policies/ai>

Weber-Wulff, D., Anohina-Naumeca, A., Bjelobaba, S., Foltýnek, T., Guerrero-Dib, J., Popoola, O., Šigut, P., & Waddington, L. (2023). Testing of detection tools for AI-generated text. *International Journal for Educational Integrity, 19*, 26. https://doi.org/10.1007/s40979-023-00146-z

### Modern Language Association (MLA) Style (9th Edition)

International Committee of Medical Journal Editors. "Recommendations for the Conduct, Reporting, Editing, and Publication of Scholarly Work in Medical Journals." *ICMJE*, 2024, www.icmje.org/recommendations/. Accessed 21 June 2026.

Liang, Weixin, et al. "GPT Detectors Are Biased against Non-Native English Writers." *Patterns*, vol. 4, no. 7, 2023, article 100779, https://doi.org/10.1016/j.patter.2023.100779.

"Tools Such as ChatGPT Threaten Transparent Science." *Nature Editorial Policy*, 2024, www.nature.com/editorial-policies/ai. Accessed 21 June 2026.

Weber-Wulff, Debora, et al. "Testing of Detection Tools for AI-Generated Text." *International Journal for Educational Integrity*, vol. 19, 2023, article 26, https://doi.org/10.1007/s40979-023-00146-z.

---

<footer>

**Repository:** <https://github.com/waleedmandour/scholarscribe> · **Issues:** <https://github.com/waleedmandour/scholarscribe/issues> · **Releases:** <https://github.com/waleedmandour/scholarscribe/releases>

---

*Built with ❤️ to the Academic Community.*

</footer>
