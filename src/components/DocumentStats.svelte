<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type DocStats } from "../lib/api";

  let inputText = "";
  let inputPath = "";
  let stats: DocStats | null = null;
  let busy = false;
  let error = "";

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "csv", "json", "docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const text = await api.readTextFile(selected);
      inputText = text;
      inputPath = selected;
    } catch (e) {
      error = String(e);
    }
  }

  async function analyze() {
    if (!inputText.trim()) {
      error = "Paste some text or open a file first.";
      return;
    }
    error = "";
    busy = true;
    try {
      stats = await api.documentStats(inputText);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function fmt(n: number, digits = 1): string {
    return Number.isFinite(n) ? n.toFixed(digits) : "—";
  }

  function fleschLabel(score: number): string {
    if (score >= 90) return "very easy (5th grade)";
    if (score >= 70) return "easy (7th grade)";
    if (score >= 60) return "standard (8th–9th grade)";
    if (score >= 50) return "fairly hard (10th–12th grade)";
    if (score >= 30) return "difficult (college)";
    return "very difficult (college graduate)";
  }

  function statusLabel(s: string): string {
    return s === "under" ? "under target" : s === "over" ? "over target" : "near target";
  }

  function statusColor(s: string): string {
    return s === "near" ? "var(--success)" : s === "under" ? "var(--warning)" : "var(--text-muted)";
  }
</script>

<h1>Document Statistics</h1>
<p class="lead">
  A quick health-check panel for your draft: word count, sentence count, section count,
  citation count, reading time, readability scores, and a comparison panel with common
  journal targets. All processing is local.
</p>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="card-title">Input</div>
  <div class="card-subtitle">Paste text or open a file (.txt, .md, .tex, .docx).</div>
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}<span class="dim" style="font-size: 11px; word-break: break-all;">{inputPath}</span>{/if}
  </div>
  <textarea bind:value={inputText} rows="8" placeholder="Paste your draft here, or use Open file…"></textarea>
  <div class="dim" style="font-size: 11px; margin-top: 4px;">{inputText.length.toLocaleString()} characters</div>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={analyze} disabled={busy || !inputText.trim()}>
      {busy ? "Analyzing…" : "Analyze"}
    </button>
  </div>
</div>

{#if stats}
  <h2>Counts</h2>
  <div class="card">
    <div class="row" style="text-align: center; flex-wrap: wrap; gap: 20px;">
      <div>
        <div class="dim" style="font-size: 11px;">WORDS</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.word_count.toLocaleString()}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">SENTENCES</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.sentence_count.toLocaleString()}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">PARAGRAPHS</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.paragraph_count.toLocaleString()}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">SECTIONS</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.section_count.toLocaleString()}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">CITATIONS</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.citation_count.toLocaleString()}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">FIGURES</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.figure_count.toLocaleString()}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">TABLES</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.table_count.toLocaleString()}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">READING TIME</div>
        <div style="font-size: 26px; font-weight: 600;">{stats.estimated_reading_time_minutes}m</div>
      </div>
    </div>
  </div>

  <h2>Style & readability</h2>
  <div class="card">
    <table>
      <thead><tr><th>Metric</th><th>Value</th><th>Interpretation</th></tr></thead>
      <tbody>
        <tr>
          <td>Avg. sentence length</td>
          <td><strong>{fmt(stats.avg_sentence_length, 1)}</strong> words</td>
          <td class="muted">{stats.avg_sentence_length < 15 ? "concise" : stats.avg_sentence_length < 25 ? "standard" : "verbose"}</td>
        </tr>
        <tr>
          <td>Vocabulary diversity (TTR)</td>
          <td><strong>{fmt(stats.type_token_ratio, 3)}</strong></td>
          <td class="muted">Higher = more diverse vocabulary</td>
        </tr>
        <tr>
          <td>Complex-word ratio</td>
          <td><strong>{(stats.complex_word_ratio * 100).toFixed(1)}%</strong></td>
          <td class="muted">Share of 3+ syllable words</td>
        </tr>
        <tr>
          <td>Flesch Reading Ease</td>
          <td><strong>{fmt(stats.flesch_reading_ease, 1)}</strong></td>
          <td class="muted">{fleschLabel(stats.flesch_reading_ease)}</td>
        </tr>
        <tr>
          <td>Flesch-Kincaid Grade Level</td>
          <td><strong>{fmt(stats.flesch_kincaid_grade, 1)}</strong></td>
          <td class="muted">US grade level equivalent</td>
        </tr>
        <tr>
          <td>Gunning Fog Index</td>
          <td><strong>{fmt(stats.gunning_fog, 1)}</strong></td>
          <td class="muted">Years of formal education needed</td>
        </tr>
      </tbody>
    </table>
    <p class="muted" style="font-size: 12px; margin-top: 12px;">
      Most academic journals target a Flesch-Kincaid grade level of 12–16 (upper-high-school to college).
      A Flesch Reading Ease of 30–50 is typical for academic prose; above 60 is accessible to a general audience.
    </p>
  </div>

  <h2>Journal target comparison</h2>
  <div class="card">
    <div class="card-subtitle">How your word count compares to typical limits at major venues. "Near" = within 10% of the target.</div>
    <table>
      <thead><tr><th>Venue</th><th>Typical word count</th><th>Your draft</th><th>Difference</th><th>Status</th></tr></thead>
      <tbody>
        {#each stats.journal_comparison as c}
          <tr>
            <td>{c.venue}</td>
            <td>{c.typical_word_count.toLocaleString()}</td>
            <td>{stats.word_count.toLocaleString()}</td>
            <td class="muted">
              {c.delta > 0 ? "+" : ""}{c.delta.toLocaleString()}
            </td>
            <td>
              <span style="color: {statusColor(c.status)}; font-weight: 500;">
                {statusLabel(c.status)}
              </span>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="muted" style="font-size: 12px; margin-top: 12px;">
      These are approximate typical limits — always check your specific journal's author guide for current limits.
      Many venues also have separate limits for abstracts (~250 words), letters, and supplementary materials.
    </p>
  </div>
{/if}
