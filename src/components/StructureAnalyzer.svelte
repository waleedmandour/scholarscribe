<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type StructureReport } from "../lib/api";

  let inputText = "";
  let inputPath = "";
  let inputKind: "text" | "docx" = "text";
  let report: StructureReport | null = null;
  let busy = false;
  let error = "";

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] }],
    });
    if (!selected || typeof selected !== "string") return;
    inputPath = selected;
    if (selected.toLowerCase().endsWith(".docx")) {
      inputKind = "docx";
      inputText = `[.docx file loaded: ${selected}]`;
    } else {
      try { inputText = await api.readTextFile(selected); inputKind = "text"; }
      catch (e) { error = String(e); }
    }
  }

  async function analyze() {
    error = ""; busy = true; report = null;
    try {
      if (inputKind === "docx" && inputPath) { report = await api.analyzeStructure(inputPath); }
      else { report = await api.analyzeStructureText(inputText); }
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }
</script>

<h1>Structure Analyzer</h1>
<p class="lead">
  Extract your document's heading tree and get suggestions for missing sections.
  For <code>.docx</code> files, headings are detected via Word's built-in heading styles.
  For plain text, headings are detected via markdown <code>#</code> prefixes or ALL-CAPS lines.
</p>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}<span class="dim" style="font-size: 11px;">{inputPath}{#if inputKind === "docx"} <span class="tag">.docx</span>{/if}</span>{/if}
  </div>
  <textarea bind:value={inputText} rows="8" placeholder="Paste your manuscript here…"></textarea>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={analyze} disabled={busy || (inputKind === "docx" ? !inputPath : !inputText.trim())}>
      {busy ? "Analyzing…" : "Analyze structure"}
    </button>
  </div>
</div>

{#if report}
  <h2>Heading tree ({report.total_sections} sections, max depth {report.max_depth})</h2>
  <div class="card">
    {#if report.headings.length === 0}
      <div class="no-data">No headings detected.</div>
    {:else}
      <div style="font-family: ui-monospace, monospace; font-size: 13px; line-height: 1.8;">
        {#each report.headings as h}
          <div style="padding-left: {(h.level - 1) * 20}px;">
            <span class="dim">H{h.level}:</span> <strong>{h.text}</strong>
            {#if h.word_count > 0}<span class="dim" style="font-size: 11px; margin-left: 8px;">({h.word_count} words{#if h.word_count < 100} — short{/if})</span>{/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if report.missing_sections.length > 0}
    <h2>Missing sections</h2>
    <div class="card">
      <div style="display: flex; flex-wrap: wrap; gap: 8px;">
        {#each report.missing_sections as s}<span class="tag" style="background: rgba(192,57,43,0.1); color: var(--danger);">{s}</span>{/each}
      </div>
    </div>
  {/if}

  {#if report.suggestions.length > 0}
    <h2>Suggestions</h2>
    <div class="card">
      <ul style="margin: 6px 0 0 16px; line-height: 1.7;">
        {#each report.suggestions as s}<li style="font-size: 13px;">{s}</li>{/each}
      </ul>
    </div>
  {/if}
{/if}
