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
      filters: [
        { name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    inputPath = selected;
    const lower = selected.toLowerCase();
    if (lower.endsWith(".docx")) {
      inputKind = "docx";
      inputText = `[.docx file loaded: ${selected}]\nHeadings will be extracted from Word's heading styles. Click "Analyze structure" below.`;
    } else {
      try {
        const text = await api.readTextFile(selected);
        inputText = text;
        inputKind = "text";
      } catch (e) {
        error = String(e);
      }
    }
  }

  async function analyze() {
    error = "";
    busy = true;
    report = null;
    try {
      if (inputKind === "docx" && inputPath) {
        report = await api.analyzeStructure(inputPath);
      } else {
        report = await api.analyzeStructureText(inputText);
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<h1>Structure Analyzer</h1>
<p class="lead">
  Extract your document's heading tree and get suggestions for missing sections.
  For <code>.docx</code> files, headings are detected via Word's built-in heading styles
  (Heading1, Heading2, etc.). For plain text, headings are detected via markdown-style
  <code>#</code> prefixes or ALL-CAPS lines. All processing is local.
</p>

<div class="callout info">
  <strong>Why this matters.</strong>
  Most academic manuscripts follow a standard structure (Introduction, Methods, Results,
  Discussion, Conclusion). This tab tells you which sections you have, which you're missing,
  and which are too short (under 100 words) — before submission, not after the reviewer flags it.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="card-title">Input</div>
  <div class="card-subtitle">Paste text, or open a file (.txt, .md, .docx).</div>
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}
      <span class="dim" style="font-size: 11px; word-break: break-all;">
        {inputPath}
        {#if inputKind === "docx"}<span class="tag" style="margin-left: 6px;">.docx</span>{/if}
      </span>
    {/if}
  </div>
  <textarea bind:value={inputText} rows="10" placeholder="Paste your manuscript here, or use Open file…"></textarea>
  <div class="dim" style="font-size: 11px; margin-top: 4px;">{inputText.length.toLocaleString()} characters</div>
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
      <div class="no-data">No headings detected. If this is a manuscript, add section headings using Word's Heading styles (or markdown # for plain text).</div>
    {:else}
      <div style="font-family: ui-monospace, monospace; font-size: 13px; line-height: 1.8;">
        {#each report.headings as h}
          <div style="padding-left: {(h.level - 1) * 20}px;">
            <span class="dim">H{h.level}:</span> <strong>{h.text}</strong>
            {#if h.word_count > 0}
              <span class="dim" style="font-size: 11px; margin-left: 8px;">({h.word_count} words{#if h.word_count < 100} — <span style="color: var(--warning);">short</span>{/if})</span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if report.missing_sections.length > 0}
    <h2>⚠️ Missing sections ({report.missing_sections.length})</h2>
    <div class="card">
      <div class="card-subtitle">Common academic sections not found in your document. Consider adding these if appropriate for your venue.</div>
      <div style="display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px;">
        {#each report.missing_sections as s}
          <span class="tag" style="background: rgba(192, 57, 43, 0.1); color: var(--danger); font-size: 13px; padding: 4px 12px;">{s}</span>
        {/each}
      </div>
    </div>
  {:else if report.total_sections > 0}
    <h2>✓ All expected sections found</h2>
    <div class="callout info">
      Your document contains all common academic sections (Introduction, Methods, Results, Discussion, Conclusion, References, Abstract, Limitations).
    </div>
  {/if}

  {#if report.short_sections.length > 0}
    <h2>Short sections ({report.short_sections.length})</h2>
    <div class="card">
      <div class="card-subtitle">Sections with fewer than 100 words. These may need more content.</div>
      <ul style="margin: 6px 0 0 16px; padding: 0;">
        {#each report.short_sections as s}
          <li style="font-size: 13px; margin-bottom: 4px;">
            <strong>{s.text}</strong> — {s.word_count} words
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if report.suggestions.length > 0}
    <h2>Suggestions</h2>
    <div class="card">
      <ul style="margin: 6px 0 0 16px; padding: 0; line-height: 1.7;">
        {#each report.suggestions as s}
          <li style="font-size: 13px; margin-bottom: 6px;">{s}</li>
        {/each}
      </ul>
    </div>
  {/if}
{/if}
