<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type CitationReport } from "../lib/api";

  let draftPath = "";
  let bibPath = "";
  let report: CitationReport | null = null;
  let busy = false;
  let error = "";

  async function pickDraft() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Draft documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    draftPath = selected;
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
  }

  async function validate() {
    if (!draftPath || !bibPath) {
      error = "Pick both a draft file and a .bib file first.";
      return;
    }
    error = "";
    busy = true;
    try {
      report = await api.validateCitations(draftPath, bibPath);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<h1>Citation Manager</h1>
<p class="lead">
  Validate your draft's in-text citations against your <code>.bib</code> (BibTeX) file.
  Catches undefined citations (likely fabricated or wrong), unused references (trim your
  reference list before submission), and shows how many times each reference is cited.
  All parsing is local, your draft and .bib file never leave your device.
</p>

<div class="callout info">
  <strong>Why this matters.</strong>
  Citation fabrication is one of the most common forms of research misconduct, and
  one of the easiest to commit accidentally when working with AI assistants that
  invent plausible-looking references. This tab gives you a definitive, local check
  that every in-text citation in your draft matches a real entry in your .bib file.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<h2>Pick files</h2>
<div class="card">
  <div class="row" style="align-items: flex-start; gap: 16px;">
    <div style="flex: 1;">
      <label class="dim" for="cm-draft" style="font-size: 11px; display: block; margin-bottom: 4px;">Draft document (.txt, .md, .tex, .docx)</label>
      <div class="row">
        <input id="cm-draft" type="text" bind:value={draftPath} placeholder="Click Pick to choose a file…" readonly style="flex: 1;" />
        <button class="shrink" on:click={pickDraft}>Pick…</button>
      </div>
    </div>
    <div style="flex: 1;">
      <label class="dim" for="cm-bib" style="font-size: 11px; display: block; margin-bottom: 4px;">BibTeX file (.bib)</label>
      <div class="row">
        <input id="cm-bib" type="text" bind:value={bibPath} placeholder="Click Pick to choose a file…" readonly style="flex: 1;" />
        <button class="shrink" on:click={pickBib}>Pick…</button>
      </div>
    </div>
  </div>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={validate} disabled={busy || !draftPath || !bibPath}>
      {busy ? "Validating…" : "Validate citations"}
    </button>
  </div>
</div>

{#if report}
  <h2>Results</h2>

  <div class="card" style="background: var(--bg-elev-2);">
    <div class="row" style="text-align: center; font-size: 13px;">
      <div>
        <div class="dim" style="font-size: 11px;">BIB ENTRIES</div>
        <div style="font-size: 22px; font-weight: 600;">{report.bib_entries.length}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">IN-TEXT CITATIONS</div>
        <div style="font-size: 22px; font-weight: 600;">{report.in_text_citations.length}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">UNDEFINED</div>
        <div style="font-size: 22px; font-weight: 600; color: {report.undefined_citations.length > 0 ? "var(--danger)" : "var(--success)"};">
          {report.undefined_citations.length}
        </div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">UNUSED REFS</div>
        <div style="font-size: 22px; font-weight: 600; color: {report.unused_references.length > 0 ? "var(--warning)" : "var(--success)"};">
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
          <li style="font-size: 13px;">{err}</li>
        {/each}
        {#if report.bib_parse_errors.length > 5}
          <li style="font-size: 13px;" class="dim">… and {report.bib_parse_errors.length - 5} more</li>
        {/if}
      </ul>
    </div>
  {/if}

  {#if report.undefined_citations.length > 0}
    <h2>⚠️ Undefined citations ({report.undefined_citations.length})</h2>
    <div class="card">
      <div class="card-subtitle">In your draft but not in your .bib file. These are likely fabricated or wrong, verify each one.</div>
      <table>
        <thead><tr><th>Citation in draft</th><th>Author</th><th>Year</th></tr></thead>
        <tbody>
          {#each report.undefined_citations as c}
            <tr>
              <td><code>{c.raw}</code></td>
              <td>{c.author || "—"}</td>
              <td>{c.year || (c.numeric ? `[${c.numeric}]` : "—")}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <h2>✓ All citations defined</h2>
    <div class="callout info">
      Every in-text citation in your draft matches a .bib entry. No fabrication detected.
    </div>
  {/if}

  {#if report.unused_references.length > 0}
    <h2>Unused references ({report.unused_references.length})</h2>
    <div class="card">
      <div class="card-subtitle">In your .bib file but never cited in your draft. Consider trimming these before submission.</div>
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
    <h2>✓ All references cited</h2>
    <div class="callout info">
      Every .bib entry is cited at least once in your draft.
    </div>
  {/if}

  <h2>Citation count per reference</h2>
  <div class="card">
    <div class="card-subtitle">How many times each .bib entry is cited. Entries cited only once are often "token citations" worth a second look.</div>
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
              {#if count === 0}<span class="dim" style="font-size: 11px;"> (unused)</span>{/if}
              {#if count === 1}<span class="dim" style="font-size: 11px;"> (token?)</span>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
