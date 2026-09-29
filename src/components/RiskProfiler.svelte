<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type RiskProfile } from "../lib/api";
  import VerifiedStat from "./VerifiedStat.svelte";

  let inputText = "";
  let inputPath = "";
  let profile: RiskProfile | null = null;
  let busy = false;
  let error = "";

  // Click-to-expand detail panel state.
  let selectedPassageIdx: number | null = null;
  let copied = false;
  let textareaEl: HTMLTextAreaElement;
  let isJumpHighlight = false;

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] }],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      inputText = await api.readTextFile(selected);
      inputPath = selected;
    } catch (e) { error = String(e); }
  }

  async function analyze() {
    if (!inputText.trim()) { error = "Paste text or open a file first."; return; }
    error = ""; busy = true;
    try { profile = await api.analyzeRiskProfile(inputText); selectedPassageIdx = null; }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }

  function selectPassage(idx: number) {
    selectedPassageIdx = selectedPassageIdx === idx ? null : idx;
  }

  function handleBlockKeydown(e: KeyboardEvent, idx: number, total: number) {
    if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
      e.preventDefault();
      focusBlock((idx - 1 + total) % total);
    } else if (e.key === "ArrowRight" || e.key === "ArrowDown") {
      e.preventDefault();
      focusBlock((idx + 1) % total);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      selectPassage(idx);
    } else if (e.key === "Escape") {
      selectedPassageIdx = null;
    }
  }

  function focusBlock(idx: number) {
    const btn = document.getElementById(`passage-block-${idx}`);
    btn?.focus();
  }

  function jumpToPassage(s: { start_char: number; end_char: number }) {
    if (!textareaEl) return;
    textareaEl.focus();
    try {
      textareaEl.setSelectionRange(s.start_char, s.end_char);
      // The native selection gives the user a visible highlight of the
      // passage range. Add a brief border glow to draw the eye to the
      // textarea as a whole.
      isJumpHighlight = true;
      setTimeout(() => (isJumpHighlight = false), 1200);
    } catch {
      // Range may be invalid if the text was edited after profiling;
      // non-fatal.
    }
  }

  async function copyReason(reason: string) {
    try {
      await navigator.clipboard.writeText(reason);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // Clipboard may be unavailable in some embedded contexts; non-fatal.
    }
  }

  // Tier copy. Spec wording, verbatim. No em or en dashes anywhere.
  const tierCopy: Record<string, string> = {
    low: "This passage's vocabulary and sentence rhythm are typical of human academic writing. No action needed.",
    medium: "This passage shows some overlap with AI text patterns, such as more uniform vocabulary or sentence length. This is common in technical writing and is not evidence of AI authorship.",
    high: "This passage shows substantial overlap with AI text surface patterns, which are frequently seen in non-native English or heavily edited prose. It is not proof of AI generation.",
  };
</script>

<h1>Authenticity Risk Profiler</h1>
<p class="lead">
  Assesses whether your draft's surface features (vocabulary diversity as a perplexity proxy,
  sentence-length variability as a burstiness proxy) overlap with the typical profile of
  AI-generated text. Based on the documented false-positive risk factors from Liang et al. (2023)
  and Weber-Wulff et al. (2023). <strong>This is not a detection-evasion tool</strong>; it helps
  you understand whether your genuine writing shares surface features with AI text.
</p>

<div class="callout info">
  <strong>Ethical note.</strong> A "high risk" designation means your writing shares surface features
  with AI-generated text; it does NOT mean the text is AI-generated or will be flagged. Per Liang et al.
  (2023), non-native English writers often score in the high-risk zone despite writing entirely original
  work. Use this to understand your stylistic fingerprint, not to evade detection.
</div>

<VerifiedStat
  figure="Testing seven widely used GPT detectors on 91 TOEFL essays written by non-native English speakers, more than half of the essays were incorrectly labeled AI-generated, with an average false-positive rate of 61.3%. Native-speaker essays were misclassified at near-zero rates, and some detectors flagged up to 97.8% of TOEFL essays."
  sourceUrl="https://doi.org/10.1016/j.patter.2023.100779"
  sourceLabel="Liang et al. (2023), Patterns"
  lastVerified="2026-09-29"
/>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}<span class="dim" style="font-size: 0.6875rem;">{inputPath}</span>{/if}
  </div>
  <textarea bind:this={textareaEl} bind:value={inputText} rows="8" class:jump-highlight={isJumpHighlight} placeholder="Paste your draft here…"></textarea>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={analyze} disabled={busy || !inputText.trim()}>
      {busy ? "Analyzing…" : "Analyze risk profile"}
    </button>
  </div>
</div>

{#if profile}
  {@const passageCount = profile.section_profiles.length}
  <h2>Overall risk</h2>
  <div class="card" style="text-align: center;">
    <div style="font-size: 0.6875rem;" class="dim">RISK LEVEL</div>
    <div style="font-size: 2rem; font-weight: 700; color: {profile.overall_risk_color};">
      {profile.overall_risk_level.toUpperCase()}
    </div>
    <div class="muted" style="margin-top: 8px;">
      Perplexity proxy: {profile.overall_perplexity_proxy.toFixed(3)} ·
      Burstiness proxy: {profile.overall_burstiness_proxy.toFixed(3)}
    </div>
  </div>

  <h2>Passage-by-passage heatmap</h2>
  <div class="card">
    <div class="card-subtitle">Each ~200-word passage colored by risk level. Click a block for details. Use arrow keys to move between blocks, Enter or Space to open, Escape to close.</div>
    <div class="heatmap" role="grid" aria-label="Passage-by-passage risk heatmap">
      {#each profile.section_profiles as s, i}
        <button
          id="passage-block-{i}"
          class="heatmap-block"
          class:selected={selectedPassageIdx === i}
          style="background: {s.risk_color};"
          on:click={() => selectPassage(i)}
          on:keydown={(e) => handleBlockKeydown(e, i, passageCount)}
          aria-label="Passage {i + 1}, {s.risk_level} risk, {s.word_count} words"
          aria-pressed={selectedPassageIdx === i}
          tabindex={selectedPassageIdx === i || (selectedPassageIdx === null && i === 0) ? 0 : -1}
        >
          {s.word_count}
        </button>
      {/each}
    </div>
    <div class="row heatmap-legend">
      <span><span style="display:inline-block;width:12px;height:12px;background:#1a8a52;border-radius:2px;vertical-align:middle;"></span> Low risk</span>
      <span><span style="display:inline-block;width:12px;height:12px;background:#b76e00;border-radius:2px;vertical-align:middle;"></span> Medium risk</span>
      <span><span style="display:inline-block;width:12px;height:12px;background:#c0392b;border-radius:2px;vertical-align:middle;"></span> High risk</span>
    </div>
  </div>

  {#if selectedPassageIdx !== null && profile.section_profiles[selectedPassageIdx]}
    {@const s = profile.section_profiles[selectedPassageIdx]}
    {@const tier = s.risk_level.toLowerCase()}
    <div class="card passage-detail" role="region" aria-label="Passage {selectedPassageIdx + 1} details">
      <div class="card-title">{s.section_label}</div>
      <div class="passage-meta">
        <span class="passage-tier" style="background: {s.risk_color}">{s.risk_level.toUpperCase()}</span>
        <span class="dim">{s.word_count} words</span>
      </div>
      {#if s.excerpt}
        <div class="passage-excerpt">{s.excerpt}</div>
      {:else}
        <div class="passage-excerpt dim">(Excerpt unavailable; the text may have been edited after profiling.)</div>
      {/if}
      <div class="passage-reason">
        <strong>Why this score:</strong> {s.reason}
      </div>
      <div class="callout info passage-tier-copy">
        {tierCopy[tier] || tierCopy.medium}
      </div>
      <div class="passage-actions">
        <button class="shrink" on:click={() => jumpToPassage(s)}>Jump to passage</button>
        <button class="shrink" on:click={() => copyReason(s.reason)}>
          {copied ? "Copied" : "Copy reason"}
        </button>
        <button class="shrink" on:click={() => (selectedPassageIdx = null)}>Close</button>
      </div>
    </div>
  {/if}

  <div class="card">
    <p class="muted" style="margin: 0 0 12px;">{profile.explanation}</p>
    <strong>Recommendations:</strong>
    <ul style="margin: 6px 0 0 16px; padding: 0;">
      {#each profile.recommendations as r}<li style="margin-bottom: 4px;">{r}</li>{/each}
    </ul>
  </div>
{/if}

<style>
  .heatmap {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 12px;
  }
  .heatmap-block {
    width: 40px;
    height: 40px;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0;
    border: none;
    background: transparent;
  }
  .heatmap-block:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .heatmap-block.selected {
    box-shadow: 0 0 0 3px var(--accent);
  }
  .heatmap-legend {
    margin-top: 12px;
    gap: 16px;
    font-size: 0.75rem;
  }
  .passage-detail {
    margin-top: 12px;
  }
  .passage-meta {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-bottom: 8px;
    font-size: 0.875rem;
  }
  .passage-tier {
    display: inline-block;
    padding: 2px 10px;
    border-radius: 999px;
    color: white;
    font-weight: 600;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .passage-excerpt {
    font-style: italic;
    color: var(--text-muted);
    margin: 8px 0 12px;
    padding-left: 12px;
    border-left: 3px solid var(--border);
    font-size: 0.875rem;
  }
  .passage-reason {
    margin: 8px 0 12px;
    font-size: 0.875rem;
  }
  .passage-tier-copy {
    margin-top: 12px;
  }
  .passage-actions {
    display: flex;
    gap: 8px;
    margin-top: 12px;
    flex-wrap: wrap;
  }
  .passage-actions button {
    flex: 0 0 auto;
  }
  .jump-highlight {
    box-shadow: 0 0 0 3px var(--accent);
    transition: box-shadow 0.2s ease;
  }
</style>
