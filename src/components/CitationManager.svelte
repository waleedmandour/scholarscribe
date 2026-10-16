<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    api,
    type CitationReport,
    type ConvertPlainToBibResult,
    type ManuscriptReport,
    type BibEntry,
    type PlainRefStyle,
  } from "../lib/api";
  import PageHeader from "./PageHeader.svelte";

  // v2.2.1 r7: third mode "manuscript" added (Upload manuscript .docx).
  // The existing bib_file and paste modes are unchanged.
  type RefMode = "bib_file" | "paste" | "manuscript";

  let refMode: RefMode = "bib_file";
  let draftPath = "";
  let bibPath = "";
  let pastedRefs = "";
  let manuscriptPath = "";
  let refStyle: PlainRefStyle = "apa";
  let convertResult: ConvertPlainToBibResult | null = null;
  let manuscriptResult: ManuscriptReport | null = null;
  let report: CitationReport | null = null;
  let busy = false;
  let converting = false;
  let error = "";
  let info = "";
  let copiedKey = "";

  // v2.2.1 r7: added AMA and IEEE options. The Rust parser has full
  // style-specific heuristics for both (parse_ama and parse_ieee).
  const styleOptions: { value: PlainRefStyle; label: string; hint: string }[] = [
    { value: "apa", label: "APA 7th", hint: "Author, A. A., & Author, B. B. (Year). Title. Journal, Vol(Issue), Pages. DOI." },
    { value: "mla", label: "MLA 9th", hint: 'Author. "Title." Journal, vol. X, no. Y, Year, pp. A-B.' },
    { value: "chicago", label: "Chicago 17th", hint: 'Author First Last. "Title." Journal Vol, no. Issue (Year): Pages.' },
    { value: "vancouver", label: "Vancouver", hint: "Author AA, Author BB. Title. Journal. Year;Vol(Issue):Pages." },
    { value: "ama", label: "AMA (American Medical Association)", hint: "Author AA, Author BB. Title. Journal. Year;Vol(Issue):Pages. Used by JAMA and many medical journals." },
    { value: "ieee", label: "IEEE (engineering)", hint: '[N] A. Author, B. Coauthor, "Title," Journal, vol. X, no. Y, pp. A-B, Year. Used by all IEEE journals and conferences.' },
    { value: "plain", label: "Plain / auto-detect", hint: "Numbered list with any delimiter. The parser sniffs each reference's signature and dispatches to the right per-style parser (APA, MLA, Chicago, Vancouver, AMA, IEEE)." },
  ];

  async function pickDraft() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Draft documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    draftPath = selected;
    // Auto-validate when both draft and references are loaded.
    await maybeAutoValidate();
  }

  async function pickBib() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "BibTeX files", extensions: ["bib", "bibtex"] },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    bibPath = selected;
    await maybeAutoValidate();
  }

  // v2.2.1 r7: pick a full manuscript .docx. The app extracts both the
  // body text (for in-text citation scanning) and the references section
  // (for BibTeX parsing), then runs the citation validator end-to-end.
  async function pickManuscript() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Manuscript documents", extensions: ["docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    manuscriptPath = selected;
    await validateManuscript();
  }

  async function convertRefs() {
    if (!pastedRefs.trim()) {
      error = "Paste your reference list first.";
      return;
    }
    error = ""; info = ""; converting = true; convertResult = null;
    try {
      convertResult = await api.convertPlainToBib(pastedRefs, refStyle);
      info = `Converted ${convertResult.entry_count} references to BibTeX.`
        + (convertResult.warnings.length > 0 ? ` ${convertResult.warnings.length} warnings.` : "");
      // Auto-validate when both draft and converted refs are ready.
      await maybeAutoValidate();
    } catch (e) {
      error = String(e);
    } finally {
      converting = false;
    }
  }

  // v2.2.1 r7: re-parse the references with a different style, without
  // re-uploading the manuscript or re-pasting the references. The user
  // picks a style from the dropdown and clicks "Re-parse"; the app
  // re-runs the conversion with the new style on the same source text.
  async function reparseWithStyle() {
    if (refMode === "manuscript" && manuscriptPath) {
      await validateManuscript();
    } else if (refMode === "paste" && pastedRefs.trim()) {
      await convertRefs();
    }
  }

  // v2.2.1 r7: validate the manuscript end-to-end. Called when the user
  // picks a manuscript file, and also when they click "Re-parse with
  // style" in manuscript mode (with the new style).
  async function validateManuscript() {
    if (!manuscriptPath) {
      error = "Pick a manuscript .docx file first.";
      return;
    }
    error = ""; info = ""; busy = true; manuscriptResult = null; report = null;
    try {
      manuscriptResult = await api.validateDocxManuscript(manuscriptPath, refStyle);
      report = manuscriptResult.validation;
      // Use single quotes for the inner fallback string so the template
      // literal's ${...} interpolation doesn't confuse svelte-check
      // (nested double quotes inside ${} make the parser think the
      // literal text terminated early).
      const headingLabel = manuscriptResult.references_heading || '(no heading matched)';
      const parts = [`Extracted ${manuscriptResult.entry_count} references from "${headingLabel}"`];
      if (manuscriptResult.parse_warnings.length > 0) {
        parts.push(`${manuscriptResult.parse_warnings.length} parse warnings`);
      }
      if (!manuscriptResult.split_succeeded) {
        parts.push("warning: no References section heading was found — the body and references were not split");
      }
      info = parts.join(", ") + ".";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // v2.2.1 r7: Export menu actions. Three options:
  //   1. Download .bib file (the parsed BibTeX from paste or manuscript mode).
  //   2. Copy BibTeX to clipboard (same content, quick paste-into-overleaf).
  //   3. Export validation report as Markdown (undefined citations, unused
  //      references, citation counts per reference — for sharing with
  //      co-authors or attaching to a peer-review submission).
  // For bib_file mode, only option 3 is offered (the user already has a .bib).
  function currentBibtex(): string | null {
    if (refMode === "manuscript" && manuscriptResult) return manuscriptResult.bibtex;
    if (refMode === "paste" && convertResult) return convertResult.bibtex;
    return null;
  }

  async function downloadBib() {
    const bibtex = currentBibtex();
    if (!bibtex) {
      error = "Convert or upload references first.";
      return;
    }
    const defaultName = `references-${refStyle}.bib`;
    const target = await save({
      defaultPath: defaultName,
      filters: [{ name: "BibTeX files", extensions: ["bib"] }],
    });
    if (!target) return;
    // Blob + anchor download via the browser. Tauri 2 supports anchor
    // downloads via the dialog plugin.
    try {
      const blob = new Blob([bibtex], { type: "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = target.split(/[\\/]/).pop() || defaultName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      info = `Saved ${defaultName} to your downloads.`;
    } catch (e) {
      error = `Could not save file: ${String(e)}`;
    }
  }

  async function copyBibtex() {
    const bibtex = currentBibtex();
    if (!bibtex) {
      error = "Convert or upload references first.";
      return;
    }
    try {
      await navigator.clipboard.writeText(bibtex);
      info = `Copied ${bibtex.length.toLocaleString()} characters of BibTeX to clipboard.`;
    } catch {
      error = "Clipboard not available. Select the BibTeX manually and copy.";
    }
  }

  // v2.2.1 r7: per-reference Copy BibTeX button. Generates a single
  // @entry string from a BibEntry. This mirrors the Rust
  // entries_to_bibtex_string function so the per-entry output matches
  // what the .bib download would contain for that single entry.
  function bibEntryToBibtex(entry: BibEntry): string {
    const lines: string[] = [`@${entry.entry_type}{${entry.key},`];
    if (entry.author) lines.push(`  author = {${entry.author}},`);
    if (entry.title) lines.push(`  title = {${escapeBibtex(entry.title)}},`);
    if (entry.journal) lines.push(`  journal = {${escapeBibtex(entry.journal)}},`);
    if (entry.year) lines.push(`  year = {${entry.year}},`);
    if (entry.volume) lines.push(`  volume = {${entry.volume}},`);
    if (entry.number) lines.push(`  number = {${entry.number}},`);
    if (entry.pages) lines.push(`  pages = {${entry.pages}},`);
    if (entry.doi) lines.push(`  doi = {${entry.doi}},`);
    if (entry.publisher) lines.push(`  publisher = {${escapeBibtex(entry.publisher)}},`);
    if (entry.booktitle) lines.push(`  booktitle = {${escapeBibtex(entry.booktitle)},`);
    // Strip trailing comma from the last field.
    let out = lines.join("\n");
    if (out.endsWith(",")) out = out.slice(0, -1);
    out += "\n}";
    return out;
  }

  function escapeBibtex(s: string): string {
    return s.replace(/&/g, "\\&").replace(/%/g, "\\%").replace(/_/g, "\\_").replace(/#/g, "\\#");
  }

  async function copyEntryBibtex(entry: BibEntry) {
    const bibtex = bibEntryToBibtex(entry);
    try {
      await navigator.clipboard.writeText(bibtex);
      copiedKey = entry.key;
      setTimeout(() => (copiedKey = ""), 1500);
    } catch {
      error = "Clipboard not available. Select the BibTeX manually and copy.";
    }
  }

  async function exportMarkdownReport() {
    if (!report) {
      error = "Run a validation first to generate a report.";
      return;
    }
    const defaultName = `citation-report-${refMode}.md`;
    const target = await save({
      defaultPath: defaultName,
      filters: [{ name: "Markdown files", extensions: ["md", "markdown"] }],
    });
    if (!target) return;
    const md = buildMarkdownReport();
    try {
      const blob = new Blob([md], { type: "text/markdown;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = target.split(/[\\/]/).pop() || defaultName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      info = `Saved citation report as Markdown.`;
    } catch (e) {
      error = `Could not save report: ${String(e)}`;
    }
  }

  // v2.2.1 r7: build a Markdown report from the current validation
  // result. Includes the source, summary stats, undefined citations,
  // unused references, and per-reference citation counts. Suitable for
  // sharing with co-authors or attaching to a peer-review submission.
  function buildMarkdownReport(): string {
    if (!report) return "";
    const lines: string[] = [];
    lines.push("# Citation Validation Report");
    lines.push("");
    lines.push(`Generated: ${new Date().toISOString()}`);
    if (refMode === "manuscript" && manuscriptResult) {
      lines.push(`Source manuscript: \`${manuscriptResult.source_path}\``);
      // Single-quote fallback for the heading (avoids nested-double-quote
      // parsing issue inside ${...}).
      const headingLabel = manuscriptResult.references_heading || '(none)';
      lines.push(`References heading matched: "${headingLabel}"`);
      lines.push(`References extracted: ${manuscriptResult.entry_count}`);
    } else if (refMode === "paste" && convertResult) {
      lines.push(`Source: pasted plain-text references (${convertResult.entry_count} entries)`);
      lines.push(`Style: ${refStyle}`);
    } else if (refMode === "bib_file") {
      lines.push(`Source .bib: \`${bibPath}\``);
    }
    if (draftPath) {
      lines.push(`Draft document: \`${draftPath}\``);
    }
    lines.push("");
    lines.push("## Summary");
    lines.push("");
    lines.push(`| Metric | Count |`);
    lines.push(`| --- | --- |`);
    lines.push(`| Bib entries | ${report.bib_entries.length} |`);
    lines.push(`| In-text citations | ${report.in_text_citations.length} |`);
    lines.push(`| Undefined citations | ${report.undefined_citations.length} |`);
    lines.push(`| Unused references | ${report.unused_references.length} |`);
    lines.push("");

    if (report.undefined_citations.length > 0) {
      lines.push("## Undefined citations");
      lines.push("");
      lines.push("In the draft but not in the reference list. These are likely fabricated or wrong; verify each one.");
      lines.push("");
      lines.push("| Citation in draft | Author | Year |");
      lines.push("| --- | --- | --- |");
      for (const c of report.undefined_citations) {
        lines.push(`| \`${c.raw}\` | ${c.author || "-"} | ${c.year || (c.numeric ? `[${c.numeric}]` : "-")} |`);
      }
      lines.push("");
    } else {
      lines.push("## All citations defined");
      lines.push("");
      lines.push("Every in-text citation in the draft matches a reference. No fabrication detected.");
      lines.push("");
    }

    if (report.unused_references.length > 0) {
      lines.push("## Unused references");
      lines.push("");
      lines.push("In the reference list but never cited in the draft. Consider trimming these before submission.");
      lines.push("");
      lines.push("| Key | Title | Author | Year |");
      lines.push("| --- | --- | --- | --- |");
      for (const r of report.unused_references) {
        lines.push(`| \`${r.key}\` | ${mdEscape(r.title)} | ${mdEscape(r.author)} | ${r.year} |`);
      }
      lines.push("");
    }

    lines.push("## Citation count per reference");
    lines.push("");
    lines.push("How many times each reference is cited. Entries cited only once are often \"token citations\" worth a second look.");
    lines.push("");
    lines.push("| Key | Title | Times cited | Notes |");
    lines.push("| --- | --- | --- | --- |");
    for (const [entry, count] of report.citation_counts) {
      const notes = count === 0 ? "(unused)" : count === 1 ? "(token?)" : "";
      lines.push(`| \`${entry.key}\` | ${mdEscape(entry.title)} | ${count} | ${notes} |`);
    }
    lines.push("");
    return lines.join("\n");
  }

  function mdEscape(s: string): string {
    return s.replace(/\|/g, "\\|").replace(/\n/g, " ");
  }

  async function maybeAutoValidate() {
    // .bib file mode: auto-validate when both draftPath and bibPath are set.
    if (refMode === "bib_file" && draftPath && bibPath) {
      await validate();
      return;
    }
    // Paste mode: auto-validate when draftPath is set AND we have
    // converted BibTeX in memory (convertResult.bibtex).
    if (refMode === "paste" && draftPath && convertResult) {
      await validateInline();
      return;
    }
    // Manuscript mode: auto-validation happens in pickManuscript() so
    // we don't need to also call it here.
  }

  async function validate() {
    if (!draftPath || !bibPath) {
      error = "Pick both a draft file and a .bib file first.";
      return;
    }
    error = ""; info = ""; busy = true; report = null;
    try {
      report = await api.validateCitations(draftPath, bibPath);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function validateInline() {
    if (!draftPath || !convertResult) {
      error = "Pick a draft and convert references first.";
      return;
    }
    error = ""; info = ""; busy = true; report = null;
    try {
      report = await api.validateCitationsInline(draftPath, convertResult.bibtex);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // v2.2.1 r7: when the user switches modes, clear the report so the
  // Export menu and Results section don't show stale data from a
  // different mode.
  function switchMode(newMode: RefMode) {
    if (newMode === refMode) return;
    refMode = newMode;
    report = null;
    manuscriptResult = null;
    convertResult = null;
    info = "";
    error = "";
  }
</script>

<PageHeader icon="quote" title="Citation Manager">
  Validate your draft's in-text citations against your reference list.
  Catches undefined citations (likely fabricated or wrong), unused references (trim your
  reference list before submission), and shows how many times each reference is cited.
  Three modes: load an existing <code>.bib</code> file, paste a plain-text reference list
  (APA, MLA, Chicago, Vancouver, AMA, IEEE, or auto-detect), or upload a full
  <code>.docx</code> manuscript and let ScholarScribe extract the references section for
  you. All parsing is local; your draft and references never leave your device.
</PageHeader>

<div class="callout info">
  <strong>Why this matters.</strong>
  Citation fabrication is one of the most common forms of research misconduct, and
  one of the easiest to commit accidentally when working with AI assistants that
  invent plausible-looking references. This tab gives you a definitive, local check
  that every in-text citation in your draft matches a real entry in your reference list.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}
{#if info}<div class="callout info">{info}</div>{/if}

<h2>Reference source</h2>
<div class="card">
  <div class="row" style="gap: 12px; margin-bottom: 12px;">
    <label style="display: flex; align-items: center; gap: 6px; cursor: pointer;">
      <input type="radio" name="refmode" value="bib_file" checked={refMode === "bib_file"} on:change={() => switchMode("bib_file")} />
      <span>Load .bib file</span>
    </label>
    <label style="display: flex; align-items: center; gap: 6px; cursor: pointer;">
      <input type="radio" name="refmode" value="paste" checked={refMode === "paste"} on:change={() => switchMode("paste")} />
      <span>Paste references (APA / MLA / Chicago / Vancouver / AMA / IEEE)</span>
    </label>
    <label style="display: flex; align-items: center; gap: 6px; cursor: pointer;">
      <input type="radio" name="refmode" value="manuscript" checked={refMode === "manuscript"} on:change={() => switchMode("manuscript")} />
      <span>Upload manuscript (.docx)</span>
    </label>
  </div>

  {#if refMode === "bib_file"}
    <label class="dim" for="cm-bib" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">BibTeX file (.bib)</label>
    <div class="row">
      <input id="cm-bib" type="text" bind:value={bibPath} placeholder="Click Pick to choose a .bib file…" readonly style="flex: 1;" />
      <button class="shrink" on:click={pickBib}>Pick…</button>
    </div>
    <div class="row" style="margin-top: 12px;">
      <button class="primary" on:click={validate} disabled={busy || !bibPath}>
        {busy ? "Validating…" : "Validate citations"}
      </button>
    </div>
  {:else if refMode === "paste"}
    <label class="dim" for="cm-style" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Reference style</label>
    <select id="cm-style" bind:value={refStyle} style="margin-bottom: 8px;">
      {#each styleOptions as opt}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
    <div class="dim" style="font-size: var(--font-sm); margin-bottom: 8px;">
      {styleOptions.find((o) => o.value === refStyle)?.hint}
    </div>
    <textarea
      bind:value={pastedRefs}
      rows="10"
      placeholder={"Paste your reference list here, one reference per line. Numbered prefixes (1., [1], (1)) are auto-detected. Hanging indents work too.\n\nExample (APA):\nSmith, J., & Jones, K. (2023). The effects of caffeine on memory. Journal of Cognition, 12(3), 45-67. https://doi.org/10.1234/joc.2023.0012"}
    ></textarea>
    <div class="row" style="margin-top: 12px; gap: 8px; flex-wrap: wrap;">
      <button class="primary shrink" on:click={convertRefs} disabled={converting || !pastedRefs.trim()}>
        {converting ? "Converting…" : "Convert to BibTeX"}
      </button>
      <button class="shrink" on:click={reparseWithStyle} disabled={converting || !pastedRefs.trim()} title="Re-parse the same pasted references with the currently selected style. Useful when auto-detect mangles a reference.">
        Re-parse with style
      </button>
    </div>
    {#if convertResult}
      <div class="dim" style="font-size: var(--font-sm); margin-top: 12px;">
        {convertResult.entry_count} references converted to BibTeX{#if convertResult.warnings.length > 0}, {convertResult.warnings.length} warnings{/if}.
      </div>
      {#if convertResult.warnings.length > 0}
        <details style="margin-top: 6px;">
          <summary class="dim" style="font-size: var(--font-sm); cursor: pointer;">Show {convertResult.warnings.length} warnings</summary>
          <ul style="margin: 6px 0 0 16px; padding: 0;">
            {#each convertResult.warnings.slice(0, 20) as w}
              <li style="font-size: var(--font-sm);">{w}</li>
            {/each}
            {#if convertResult.warnings.length > 20}
              <li class="dim" style="font-size: var(--font-sm);">… and {convertResult.warnings.length - 20} more</li>
            {/if}
          </ul>
        </details>
      {/if}
    {/if}
  {:else if refMode === "manuscript"}
    <label class="dim" for="cm-manuscript" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Manuscript (.docx) — the app extracts the references section automatically</label>
    <div class="row">
      <input id="cm-manuscript" type="text" bind:value={manuscriptPath} placeholder="Click Pick to choose a .docx manuscript…" readonly style="flex: 1;" />
      <button class="shrink" on:click={pickManuscript}>Pick…</button>
    </div>
    <label class="dim" for="cm-style-m" style="font-size: var(--font-sm); display: block; margin-top: 12px; margin-bottom: 4px;">Reference style (for re-parsing if auto-detect mangles a reference)</label>
    <select id="cm-style-m" bind:value={refStyle} style="margin-bottom: 8px;">
      {#each styleOptions as opt}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
    <div class="dim" style="font-size: var(--font-sm); margin-bottom: 8px;">
      {styleOptions.find((o) => o.value === refStyle)?.hint}
    </div>
    <div class="row" style="margin-top: 12px; gap: 8px; flex-wrap: wrap;">
      <button class="primary shrink" on:click={validateManuscript} disabled={busy || !manuscriptPath}>
        {busy ? "Extracting & validating…" : "Extract references & validate"}
      </button>
      <button class="shrink" on:click={reparseWithStyle} disabled={busy || !manuscriptPath} title="Re-extract the references with the currently selected style. Useful when auto-detect mangles a reference.">
        Re-parse with style
      </button>
    </div>
    {#if manuscriptResult}
      <div class="dim" style="font-size: var(--font-sm); margin-top: 12px;">
        Extracted {manuscriptResult.entry_count} references from the "{manuscriptResult.references_heading || "(no heading matched)"} section.
        {#if manuscriptResult.parse_warnings.length > 0}{manuscriptResult.parse_warnings.length} parse warnings.{/if}
        {#if !manuscriptResult.split_succeeded}<strong> Warning: no References section heading was found — the body and references were not split, so the validation may include false "unused references".</strong>{/if}
      </div>
      {#if manuscriptResult.parse_warnings.length > 0}
        <details style="margin-top: 6px;">
          <summary class="dim" style="font-size: var(--font-sm); cursor: pointer;">Show {manuscriptResult.parse_warnings.length} parse warnings</summary>
          <ul style="margin: 6px 0 0 16px; padding: 0;">
            {#each manuscriptResult.parse_warnings.slice(0, 20) as w}
              <li style="font-size: var(--font-sm);">{w}</li>
            {/each}
            {#if manuscriptResult.parse_warnings.length > 20}
              <li class="dim" style="font-size: var(--font-sm);">… and {manuscriptResult.parse_warnings.length - 20} more</li>
            {/if}
          </ul>
        </details>
      {/if}
      <details style="margin-top: 8px;">
        <summary class="dim" style="font-size: var(--font-sm); cursor: pointer;">Show raw extracted references text (pre-parsing)</summary>
        <pre style="margin: 6px 0 0; padding: 10px; background: var(--bg-elev-2); border-radius: var(--radius-sm); max-height: 300px; overflow-y: auto; white-space: pre-wrap; word-wrap: break-word; font-size: var(--font-sm);">{manuscriptResult.references_text}</pre>
      </details>
    {/if}
  {/if}
</div>

{#if refMode !== "bib_file"}
  <h2>Draft (for in-text citation scanning)</h2>
  <div class="card">
    <div class="dim" style="font-size: var(--font-sm); margin-bottom: 8px;">
      {#if refMode === "manuscript"}
        In manuscript mode, the manuscript file IS the draft — the app scans the body of the same .docx for in-text citations. You don't need to pick a separate draft file.
      {:else}
        Pick a draft document so the validator can scan it for in-text citations and match them against the parsed references.
      {/if}
    </div>
    {#if refMode === "paste"}
      <label class="dim" for="cm-draft" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Draft document (.txt, .md, .tex, .docx)</label>
      <div class="row">
        <input id="cm-draft" type="text" bind:value={draftPath} placeholder="Click Pick to choose a file…" readonly style="flex: 1;" />
        <button class="shrink" on:click={pickDraft}>Pick…</button>
      </div>
      <div class="row" style="margin-top: 12px;">
        <button class="primary" on:click={validateInline} disabled={busy || !draftPath || !convertResult}>
          {busy ? "Validating…" : "Validate against draft"}
        </button>
      </div>
    {/if}
  </div>
{/if}

{#if report}
  <h2>Results</h2>

  <div class="card" style="background: var(--bg-elev-2);">
    <div class="row" style="text-align: center; font-size: var(--font-sm);">
      <div>
        <div class="dim" style="font-size: var(--font-sm);">BIB ENTRIES</div>
        <div style="font-size: var(--font-xl); font-weight: 600;">{report.bib_entries.length}</div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">IN-TEXT CITATIONS</div>
        <div style="font-size: var(--font-xl); font-weight: 600;">{report.in_text_citations.length}</div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">UNDEFINED</div>
        <div style="font-size: var(--font-xl); font-weight: 600; color: {report.undefined_citations.length > 0 ? "var(--danger)" : "var(--success)"};">
          {report.undefined_citations.length}
        </div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">UNUSED REFS</div>
        <div style="font-size: var(--font-xl); font-weight: 600; color: {report.unused_references.length > 0 ? "var(--warning)" : "var(--success)"};">
          {report.unused_references.length}
        </div>
      </div>
    </div>
  </div>

  {#if report.bib_parse_errors.length > 0}
    <div class="callout warn">
      <strong>BibTeX parse warnings ({report.bib_parse_errors.length}):</strong>
      <ul style="margin: 6px 0 0 16px; padding: 0;">
        {#each report.bib_parse_errors.slice(0, 5) as err}
          <li style="font-size: var(--font-sm);">{err}</li>
        {/each}
        {#if report.bib_parse_errors.length > 5}
          <li style="font-size: var(--font-sm);" class="dim">… and {report.bib_parse_errors.length - 5} more</li>
        {/if}
      </ul>
    </div>
  {/if}

  {#if report.undefined_citations.length > 0}
    <h2>Undefined citations ({report.undefined_citations.length})</h2>
    <div class="card">
      <div class="card-subtitle">In your draft but not in your reference list. These are likely fabricated or wrong, verify each one.</div>
      <table>
        <thead><tr><th>Citation in draft</th><th>Author</th><th>Year</th></tr></thead>
        <tbody>
          {#each report.undefined_citations as c}
            <tr>
              <td><code>{c.raw}</code></td>
              <td>{c.author || "-"}</td>
              <td>{c.year || (c.numeric ? `[${c.numeric}]` : "-")}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <h2>All citations defined</h2>
    <div class="callout info">
      Every in-text citation in your draft matches a reference. No fabrication detected.
    </div>
  {/if}

  {#if report.unused_references.length > 0}
    <h2>Unused references ({report.unused_references.length})</h2>
    <div class="card">
      <div class="card-subtitle">In your reference list but never cited in your draft. Consider trimming these before submission.</div>
      <table>
        <thead><tr><th>Key</th><th>Title</th><th>Author</th><th>Year</th></tr></thead>
        <tbody>
          {#each report.unused_references as r}
            <tr>
              <td><code>{r.key}</code></td>
              <td style="max-width: 350px;">{r.title}</td>
              <td style="max-width: 200px;">{r.author}</td>
              <td>{r.year}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <h2>All references cited</h2>
    <div class="callout info">
      Every reference is cited at least once in your draft.
    </div>
  {/if}

  <h2>Citation count per reference</h2>
  <div class="card">
    <div class="card-subtitle">How many times each reference is cited. Entries cited only once are often "token citations" worth a second look. v2.2.1 r7: each row has a Copy button to grab a single @entry's BibTeX.</div>
    <table>
      <thead><tr><th>Key</th><th>Title</th><th>Times cited</th><th>Copy BibTeX</th></tr></thead>
      <tbody>
        {#each report.citation_counts as [entry, count]}
          <tr>
            <td><code>{entry.key}</code></td>
            <td style="max-width: 400px;">{entry.title}</td>
            <td>
              <strong style="color: {count === 0 ? "var(--text-dim)" : count === 1 ? "var(--warning)" : "var(--text)"};">
                {count}
              </strong>
              {#if count === 0}<span class="dim" style="font-size: var(--font-sm);"> (unused)</span>{/if}
              {#if count === 1}<span class="dim" style="font-size: var(--font-sm);"> (token?)</span>{/if}
            </td>
            <td>
              <button class="shrink" style="font-size: var(--font-sm); padding: 3px 8px; min-height: 24px;" on:click={() => copyEntryBibtex(entry)} title="Copy this entry's BibTeX to the clipboard.">
                {copiedKey === entry.key ? "Copied!" : "Copy"}
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <!-- v2.2.1 r7: Export menu. The user explicitly asked for a "button
       for options including exporting the list of references to a BibTeX
       file". The <details> element is a native dropdown that requires
       no extra JS state. For bib_file mode the user already has a .bib
       file, so only the Markdown report option is offered. For paste
       and manuscript modes, all three options are available. -->
  <h2>Export</h2>
  <details class="card" style="padding: 0;">
    <summary style="cursor: pointer; padding: 20px 24px; font-weight: 600; font-size: var(--font-md);">
      Export options ▾
    </summary>
    <div style="padding: 0 24px 20px; display: flex; flex-direction: column; gap: 10px;">
      {#if refMode !== "bib_file"}
        <button class="primary shrink" style="width: fit-content;" on:click={downloadBib} disabled={!currentBibtex()} title="Download the parsed references as a .bib file you can use with LaTeX/BibTeX.">
          Download .bib file
        </button>
        <button class="shrink" style="width: fit-content;" on:click={copyBibtex} disabled={!currentBibtex()} title="Copy the parsed BibTeX to the clipboard for quick paste-into-overleaf.">
          Copy BibTeX to clipboard
        </button>
      {/if}
      <button class="shrink" style="width: fit-content;" on:click={exportMarkdownReport} disabled={!report} title="Export the full validation report (undefined citations, unused references, per-reference citation counts) as a Markdown file you can share with co-authors or attach to a peer-review submission.">
        Export validation report as Markdown
      </button>
    </div>
  </details>
{/if}
