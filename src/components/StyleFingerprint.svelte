<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type StyleFingerprint } from "../lib/api";

  type PaperEntry = { label: string; text: string };
  let papers: PaperEntry[] = [];
  let fingerprint: StyleFingerprint | null = null;
  let busy = false;
  let error = "";
  let copied = false;

  async function addPaper() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] }],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const text = await api.readTextFile(selected);
      const name = selected.split(/[\\/]/).pop() || `Paper ${papers.length + 1}`;
      papers = [...papers, { label: name, text }];
    } catch (e) { error = String(e); }
  }

  function addPastedPaper() {
    const label = prompt("Label for this paper:", `Paper ${papers.length + 1}`);
    if (!label) return;
    papers = [...papers, { label, text: "" }];
  }

  function removePaper(idx: number) {
    papers = papers.filter((_, i) => i !== idx);
  }

  async function compute() {
    const valid = papers.filter(p => p.text.trim().length > 100);
    if (valid.length < 2) {
      error = "Add at least 2 papers with 100+ words each to compute a fingerprint.";
      return;
    }
    error = ""; busy = true;
    try {
      fingerprint = await api.computeStyleFingerprint(valid.map(p => [p.label, p.text]));
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }

  async function exportMarkdown() {
    if (!fingerprint) return;
    const path = await save({
      defaultPath: "style-fingerprint.md",
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    try {
      // Write the markdown export to a file via the browser download or a Tauri command
      const blob = new Blob([fingerprint.export_markdown], { type: "text/markdown" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = "style-fingerprint.md";
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) { error = String(e); }
  }

  async function copyJson() {
    if (!fingerprint) return;
    try {
      await navigator.clipboard.writeText(fingerprint.export_json);
      copied = true;
      setTimeout(() => copied = false, 1500);
    } catch { error = "Clipboard not available."; }
  }
</script>

<h1>Multi-Paper Style Fingerprint</h1>
<p class="lead">
  Aggregate style metrics across multiple papers by the same author, producing a shareable
  (privacy-safe) stylometric signature. <strong>Only aggregate metrics are exported, no raw text
  ever leaves your device.</strong> The fingerprint can accompany a manuscript submission as
  supplementary evidence of authorial consistency.
</p>

<div class="callout info">
  <strong>Privacy-safe by design.</strong> The exported fingerprint contains only numbers
  (sentence lengths, vocabulary diversity, readability scores, etc.). It does not contain any
  text from your papers. Reviewers can verify authorial consistency without accessing your
  prior work.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<h2>Your papers ({papers.length})</h2>
<div class="card">
  <div class="row" style="margin-bottom: 12px; gap: 8px;">
    <button class="shrink" on:click={addPaper}>Open file…</button>
    <button class="shrink" on:click={addPastedPaper}>Add by label…</button>
  </div>
  {#if papers.length === 0}
    <p class="no-data">No papers added yet. Add at least 2 papers to compute a fingerprint.</p>
  {:else}
    <table>
      <thead><tr><th>Label</th><th>Characters</th><th></th></tr></thead>
      <tbody>
        {#each papers as p, idx}
          <tr>
            <td><strong>{p.label}</strong></td>
            <td class="muted">{p.text.length.toLocaleString()}</td>
            <td class="right"><button class="danger shrink" on:click={() => removePaper(idx)}>Remove</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={compute} disabled={busy || papers.length < 2}>
      {busy ? "Computing…" : "Compute fingerprint"}
    </button>
  </div>
</div>

{#if fingerprint}
  <h2>Aggregate metrics ({fingerprint.papers_analyzed} papers, {fingerprint.total_words.toLocaleString()} words)</h2>
  <div class="card">
    <table>
      <thead><tr><th>Metric</th><th>Value</th></tr></thead>
      <tbody>
        <tr><td>Avg. sentence length</td><td><strong>{fingerprint.metrics.avg_sentence_length.toFixed(2)}</strong> words</td></tr>
        <tr><td>Sentence length σ</td><td><strong>{fingerprint.metrics.sentence_length_stdev.toFixed(2)}</strong></td></tr>
        <tr><td>Vocabulary diversity (TTR)</td><td><strong>{fingerprint.metrics.type_token_ratio.toFixed(3)}</strong></td></tr>
        <tr><td>Passive-voice density</td><td><strong>{fingerprint.metrics.passive_ratio.toFixed(3)}</strong></td></tr>
        <tr><td>Hedging density</td><td><strong>{fingerprint.metrics.hedge_density.toFixed(3)}</strong></td></tr>
        <tr><td>Connector density</td><td><strong>{fingerprint.metrics.connector_density.toFixed(3)}</strong></td></tr>
        <tr><td>First-person singular</td><td><strong>{fingerprint.metrics.first_person_singular_ratio.toFixed(3)}</strong></td></tr>
        <tr><td>First-person plural</td><td><strong>{fingerprint.metrics.first_person_plural_ratio.toFixed(3)}</strong></td></tr>
        <tr><td>Citation density</td><td><strong>{fingerprint.metrics.citation_density.toFixed(3)}</strong></td></tr>
        <tr><td>Flesch Reading Ease</td><td><strong>{fingerprint.metrics.flesch_reading_ease.toFixed(1)}</strong></td></tr>
        <tr><td>Flesch-Kincaid Grade</td><td><strong>{fingerprint.metrics.flesch_kincaid_grade.toFixed(1)}</strong></td></tr>
        <tr><td>Gunning Fog</td><td><strong>{fingerprint.metrics.gunning_fog.toFixed(1)}</strong></td></tr>
        <tr><td>Avg. syllables/word</td><td><strong>{fingerprint.metrics.avg_syllables_per_word.toFixed(3)}</strong></td></tr>
        <tr><td>Complex-word ratio</td><td><strong>{fingerprint.metrics.complex_word_ratio.toFixed(3)}</strong></td></tr>
      </tbody>
    </table>
  </div>

  <h2>Per-paper summary</h2>
  <div class="card">
    <table>
      <thead><tr><th>Paper</th><th>Words</th><th>Avg. sent. len.</th><th>TTR</th><th>Flesch</th></tr></thead>
      <tbody>
        {#each fingerprint.per_paper_profiles as p}
          <tr>
            <td>{p.label}</td>
            <td>{p.word_count}</td>
            <td>{p.avg_sentence_length.toFixed(1)}</td>
            <td>{p.type_token_ratio.toFixed(3)}</td>
            <td>{p.flesch_reading_ease.toFixed(1)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <h2>Export</h2>
  <div class="card">
    <div class="card-subtitle">Export the privacy-safe fingerprint. No raw text is included, only aggregate metrics.</div>
    <div class="row" style="margin-top: 12px; gap: 8px;">
      <button class="shrink" on:click={exportMarkdown}>Export as Markdown</button>
      <button class="shrink" on:click={copyJson}>{copied ? "Copied!" : "Copy JSON"}</button>
    </div>
    <details style="margin-top: 12px;">
      <summary style="cursor: pointer; font-size: 13px; color: var(--text-muted);">Preview JSON export</summary>
      <pre style="margin-top: 8px; max-height: 300px; overflow-y: auto;">{fingerprint.export_json}</pre>
    </details>
  </div>
{/if}
