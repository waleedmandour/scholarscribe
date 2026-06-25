<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type ConsistencyReport } from "../lib/api";

  let inputText = "";
  let inputPath = "";
  let report: ConsistencyReport | null = null;
  let busy = false;
  let error = "";

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] }],
    });
    if (!selected || typeof selected !== "string") return;
    try { inputText = await api.readTextFile(selected); inputPath = selected; }
    catch (e) { error = String(e); }
  }

  async function check() {
    if (!inputText.trim()) { error = "Paste text or open a file first."; return; }
    error = ""; busy = true;
    try { report = await api.checkVoiceConsistency(inputText); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }
</script>

<h1>Voice Consistency Checker</h1>
<p class="lead">
  Flags within-document stylistic inconsistencies, passages where sentence length, hedging density,
  or vocabulary diversity abruptly shifts. Sudden stylistic shifts within a single paper are a
  legitimate editorial concern (and a documented AI-detection signal). Helps researchers ensure
  cohesion regardless of AI involvement.
</p>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}<span class="dim" style="font-size: 11px;">{inputPath}</span>{/if}
  </div>
  <textarea bind:value={inputText} rows="8" placeholder="Paste your draft here…"></textarea>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={check} disabled={busy || !inputText.trim()}>
      {busy ? "Checking…" : "Check voice consistency"}
    </button>
  </div>
</div>

{#if report}
  <h2>Overall consistency: {(report.overall_consistency_score * 100).toFixed(0)}%</h2>
  <div class="card">
    <p class="muted" style="font-size: 13px;">{report.explanation}</p>
    <strong>Recommendations:</strong>
    <ul style="margin: 6px 0 0 16px;">
      {#each report.recommendations as r}<li style="font-size: 13px;">{r}</li>{/each}
    </ul>
  </div>

  {#if report.inconsistencies.length > 0}
    <h2>Inconsistencies ({report.inconsistencies.length})</h2>
    <div class="card">
      <table>
        <thead><tr><th>Passage</th><th>Metric</th><th>Value</th><th>Average</th><th>Deviation</th><th>Severity</th><th>Note</th></tr></thead>
        <tbody>
          {#each report.inconsistencies as inc}
            <tr>
              <td style="font-size: 12px;">{inc.passage_label}</td>
              <td><code>{inc.metric}</code></td>
              <td>{inc.value.toFixed(2)}</td>
              <td class="muted">{inc.document_average.toFixed(2)}</td>
              <td style="color: {inc.severity === 'high' ? 'var(--danger)' : 'var(--warning)'};">{inc.deviation_pct}%</td>
              <td><span class="tag" style="background: {inc.severity === 'high' ? 'rgba(192,57,43,0.1)' : 'rgba(183,110,0,0.1)'}; color: {inc.severity === 'high' ? 'var(--danger)' : 'var(--warning)'};">{inc.severity}</span></td>
              <td class="muted" style="font-size: 12px; max-width: 300px;">{inc.note}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <div class="callout info">No inconsistencies detected, your document shows consistent stylistic metrics across all sections.</div>
  {/if}

  <h2>Passage metrics</h2>
  <div class="card">
    <table>
      <thead><tr><th>Passage</th><th>Words</th><th>Avg sentence len</th><th>TTR</th><th>Hedge density</th><th>Passive ratio</th><th>Flesch</th></tr></thead>
      <tbody>
        {#each report.passages as p}
          <tr>
            <td style="font-size: 12px;">{p.label}</td>
            <td>{p.word_count}</td>
            <td>{p.avg_sentence_length.toFixed(1)}</td>
            <td>{p.type_token_ratio.toFixed(3)}</td>
            <td>{p.hedge_density.toFixed(2)}</td>
            <td>{p.passive_ratio.toFixed(2)}</td>
            <td>{p.flesch_reading_ease.toFixed(1)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}
