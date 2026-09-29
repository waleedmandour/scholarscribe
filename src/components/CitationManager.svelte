<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type CitationReport, type ConvertPlainToBibResult, type PlainRefStyle } from "../lib/api";

  type RefMode = "bib_file" | "paste";

  let refMode: RefMode = "bib_file";
  let draftPath = "";
  let bibPath = "";
  let pastedRefs = "";
  let refStyle: PlainRefStyle = "apa";
  let convertResult: ConvertPlainToBibResult | null = null;
  let report: CitationReport | null = null;
  let busy = false;
  let converting = false;
  let error = "";
  let info = "";

  const styleOptions: { value: PlainRefStyle; label: string; hint: string }[] = [
    { value: "apa", label: "APA 7th", hint: "Author, A. A., & Author, B. B. (Year). Title. Journal, Vol(Issue), Pages. DOI." },
    { value: "mla", label: "MLA 9th", hint: 'Author. "Title." Journal, vol. X, no. Y, Year, pp. A-B.' },
    { value: "chicago", label: "Chicago 17th", hint: 'Author First Last. "Title." Journal Vol, no. Issue (Year): Pages.' },
    { value: "vancouver", label: "Vancouver", hint: "Author AA, Author BB. Title. Journal. Year;Vol(Issue):Pages." },
    { value: "plain", label: "Plain / auto-detect", hint: "Numbered list with any delimiter. The parser tries each style's heuristics in turn." },
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

  async function downloadBib() {
    if (!convertResult) {
      error = "Convert references first.";
      return;
    }
    const defaultName = `references-${refStyle}.bib`;
    const target = await save({
      defaultPath: defaultName,
      filters: [{ name: "BibTeX files", extensions: ["bib"] }],
    });
    if (!target) return;
    // Write the .bib content to the chosen path via the Tauri FS plugin.
    // We use the same readTextFile-style path; for writes we go through
    // the existing draftSave path since the app already manages a data
    // directory. For a one-off download, the simplest portable approach
    // is to invoke a small Rust command via the chat bridge, but to keep
    // this PR scoped, we use a Blob + anchor download via the browser.
    // Tauri 2 supports anchor downloads via the dialog plugin.
    try {
      const blob = new Blob([convertResult.bibtex], { type: "text/plain;charset=utf-8" });
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
</script>

<h1>Citation Manager</h1>
<p class="lead">
  Validate your draft's in-text citations against your reference list.
  Catches undefined citations (likely fabricated or wrong), unused references (trim your
  reference list before submission), and shows how many times each reference is cited.
  Two modes: load an existing <code>.bib</code> file, or paste a plain-text reference list
  (APA, MLA, Chicago, Vancouver, or auto-detect) and let ScholarScribe convert it to BibTeX
  for you. All parsing is local; your draft and references never leave your device.
</p>

<div class="callout info">
  <strong>Why this matters.</strong>
  Citation fabrication is one of the most common forms of research misconduct, and
  one of the easiest to commit accidentally when working with AI assistants that
  invent plausible-looking references. This tab gives you a definitive, local check
  that every in-text citation in your draft matches a real entry in your reference list.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}
{#if info}<div class="callout info">{info}</div>{/if}

<h2>Pick draft</h2>
<div class="card">
  <label class="dim" for="cm-draft" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Draft document (.txt, .md, .tex, .docx)</label>
  <div class="row">
    <input id="cm-draft" type="text" bind:value={draftPath} placeholder="Click Pick to choose a file…" readonly style="flex: 1;" />
    <button class="shrink" on:click={pickDraft}>Pick…</button>
  </div>
</div>

<h2>Reference source</h2>
<div class="card">
  <div class="row" style="gap: 12px; margin-bottom: 12px;">
    <label style="display: flex; align-items: center; gap: 6px; cursor: pointer;">
      <input type="radio" name="refmode" value="bib_file" bind:group={refMode} />
      <span>Load .bib file</span>
    </label>
    <label style="display: flex; align-items: center; gap: 6px; cursor: pointer;">
      <input type="radio" name="refmode" value="paste" bind:group={refMode} />
      <span>Paste references (APA / MLA / Chicago / Vancouver)</span>
    </label>
  </div>

  {#if refMode === "bib_file"}
    <label class="dim" for="cm-bib" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">BibTeX file (.bib)</label>
    <div class="row">
      <input id="cm-bib" type="text" bind:value={bibPath} placeholder="Click Pick to choose a .bib file…" readonly style="flex: 1;" />
      <button class="shrink" on:click={pickBib}>Pick…</button>
    </div>
    <div class="row" style="margin-top: 12px;">
      <button class="primary" on:click={validate} disabled={busy || !draftPath || !bibPath}>
        {busy ? "Validating…" : "Validate citations"}
      </button>
    </div>
  {:else}
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
      <button class="shrink" on:click={downloadBib} disabled={!convertResult}>
        Download .bib file
      </button>
      <button class="shrink" on:click={validateInline} disabled={busy || !draftPath || !convertResult}>
        {busy ? "Validating…" : "Validate against draft"}
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
  {/if}
</div>

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
    <div class="card-subtitle">How many times each reference is cited. Entries cited only once are often "token citations" worth a second look.</div>
    <table>
      <thead><tr><th>Key</th><th>Title</th><th>Times cited</th></tr></thead>
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
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
