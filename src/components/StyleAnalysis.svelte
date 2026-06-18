<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    api,
    type StyleProfile,
    type StyleComparison,
  } from "../lib/api";

  let draftText = "";
  let draftPath = "";
  let refText = "";
  let refPath = "";
  let draftProfile: StyleProfile | null = null;
  let refProfile: StyleProfile | null = null;
  let comparison: StyleComparison | null = null;
  let busy = false;
  let error = "";

  async function pickFile(target: "draft" | "ref") {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Text", extensions: ["txt", "md", "markdown", "tex", "rst", "csv", "json"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const text = await api.readTextFile(selected);
      if (target === "draft") {
        draftText = text;
        draftPath = selected;
      } else {
        refText = text;
        refPath = selected;
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function analyze() {
    error = "";
    if (!draftText.trim()) {
      error = "Please paste a draft or open a file.";
      return;
    }
    if (!refText.trim()) {
      error = "Please paste a reference sample of your own prior writing, or open a file.";
      return;
    }
    busy = true;
    try {
      draftProfile = await api.analyzeStyle(draftText);
      refProfile = await api.analyzeStyle(refText);
      comparison = await api.compareStyle(draftProfile, refProfile);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function fmt(n: number, digits = 2): string {
    return Number.isFinite(n) ? n.toFixed(digits) : "—";
  }

  const featureLabels: Record<string, string> = {
    avg_sentence_length: "Avg. sentence length (words)",
    sentence_length_stdev: "Sentence-length variability (σ)",
    type_token_ratio: "Vocabulary diversity (TTR)",
    passive_ratio: "Passive-voice density",
    hedge_density: "Hedging density",
    connector_density: "Connector density",
    first_person_plural_ratio: "First-person plural (\"we\")",
    citation_density: "Citation density",
  };
</script>

<h1>Style Analysis</h1>
<p class="lead">
  Compare a draft against a sample of <em>your own</em> prior writing. ScholarScribe extracts descriptive stylistic
  statistics — sentence length, hedging, passive voice, citation density, and others — and reports how closely the
  draft matches your established voice. Use this to spot drafts that drift away from your usual register, or to confirm
  that a co-author's edit still sounds like the team.
</p>

<div class="callout info">
  <strong>What this is — and isn't.</strong>
  Style Analysis tells <em>you</em> how your draft compares to <em>your own</em> writing. It does not predict or attempt
  to lower AI-detector scores. If you want to understand how detectors work, see the Detector Literacy tab.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="row" style="align-items: flex-start;">
  <div class="card" style="flex: 1;">
    <div class="card-title">Draft</div>
    <div class="card-subtitle">The text you're working on now.</div>
    <div class="row" style="margin-bottom: 8px;">
      <button class="shrink" on:click={() => pickFile("draft")}>Open file…</button>
      {#if draftPath}<span class="dim" style="font-size: 11px;">{draftPath}</span>{/if}
    </div>
    <textarea bind:value={draftText} rows="10" placeholder="Paste your draft, or use Open file…"></textarea>
    <div class="dim" style="font-size: 11px; margin-top: 4px;">{draftText.length.toLocaleString()} characters</div>
  </div>

  <div class="card" style="flex: 1;">
    <div class="card-title">Reference (your prior writing)</div>
    <div class="card-subtitle">A paper, chapter, or section that sounds like "you". 1,000+ words gives the most reliable profile.</div>
    <div class="row" style="margin-bottom: 8px;">
      <button class="shrink" on:click={() => pickFile("ref")}>Open file…</button>
      {#if refPath}<span class="dim" style="font-size: 11px;">{refPath}</span>{/if}
    </div>
    <textarea bind:value={refText} rows="10" placeholder="Paste a sample of your own published writing"></textarea>
    <div class="dim" style="font-size: 11px; margin-top: 4px;">{refText.length.toLocaleString()} characters</div>
  </div>
</div>

<div class="row">
  <button class="primary" on:click={analyze} disabled={busy}>
    {busy ? "Analyzing…" : "Analyze & compare"}
  </button>
</div>

{#if comparison}
  <h2>Result</h2>
  <div class="card">
    <div class="card-title">Summary</div>
    <p class="muted" style="font-size: 13px; margin: 6px 0 12px;">
      Overall distance: <strong>{fmt(comparison.overall_distance, 3)}</strong>
      <span class="dim">(lower = more similar. Typical within-author distance is &lt; 0.5.)</span>
    </p>
    <ul style="margin: 0; padding-left: 20px;">
      {#each comparison.notes as note}
        <li style="margin-bottom: 4px;">{note}</li>
      {/each}
    </ul>
  </div>

  <div class="card">
    <div class="card-title">Feature-by-feature comparison</div>
    <table>
      <thead>
        <tr><th>Feature</th><th>Draft</th><th>Reference</th><th>Diff %</th><th>Interpretation</th></tr>
      </thead>
      <tbody>
        {#each comparison.feature_distances as f}
          <tr>
            <td>{featureLabels[f.feature] || f.feature}</td>
            <td>{fmt(f.draft_value)}</td>
            <td>{fmt(f.reference_value)}</td>
            <td class="muted">{fmt(f.relative_diff_pct, 0)}%</td>
            <td class="muted">{f.interpretation}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <div class="callout info">
    <strong>How to use this.</strong>
    "Notable" or "substantial" differences aren't inherently bad — they may simply mean you're writing in a different
    register (a methods section reads differently from a discussion). Use the table to ask: "is the difference a
    deliberate choice, or did the draft drift away from my voice without me noticing?"
  </div>
{/if}
