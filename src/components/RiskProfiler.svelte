<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type RiskProfile } from "../lib/api";

  let inputText = "";
  let inputPath = "";
  let profile: RiskProfile | null = null;
  let busy = false;
  let error = "";

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
    try { profile = await api.analyzeRiskProfile(inputText); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }
</script>

<h1>Authenticity Risk Profiler</h1>
<p class="lead">
  Assesses whether your draft's surface features (vocabulary diversity as a perplexity proxy,
  sentence-length variability as a burstiness proxy) overlap with the typical profile of
  AI-generated text. Based on the documented false-positive risk factors from Liang et al. (2023)
  and Weber-Wulff et al. (2023). <strong>This is not a detection-evasion tool</strong> — it helps
  you understand whether your genuine writing shares surface features with AI text.
</p>

<div class="callout info">
  <strong>Ethical note.</strong> A "high risk" designation means your writing shares surface features
  with AI-generated text — it does NOT mean the text is AI-generated or will be flagged. Per Liang et al.
  (2023), non-native English writers often score in the high-risk zone despite writing entirely original
  work. Use this to understand your stylistic fingerprint, not to evade detection.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}<span class="dim" style="font-size: 11px;">{inputPath}</span>{/if}
  </div>
  <textarea bind:value={inputText} rows="8" placeholder="Paste your draft here…"></textarea>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={analyze} disabled={busy || !inputText.trim()}>
      {busy ? "Analyzing…" : "Analyze risk profile"}
    </button>
  </div>
</div>

{#if profile}
  <h2>Overall risk</h2>
  <div class="card" style="text-align: center;">
    <div style="font-size: 11px;" class="dim">RISK LEVEL</div>
    <div style="font-size: 32px; font-weight: 700; color: {profile.overall_risk_color};">
      {profile.overall_risk_level.toUpperCase()}
    </div>
    <div class="muted" style="margin-top: 8px;">
      Perplexity proxy: {profile.overall_perplexity_proxy.toFixed(3)} ·
      Burstiness proxy: {profile.overall_burstiness_proxy.toFixed(3)}
    </div>
  </div>

  <h2>Passage-by-passage heatmap</h2>
  <div class="card">
    <div class="card-subtitle">Each ~200-word passage colored by risk level. Hover for details.</div>
    <div style="display: flex; flex-wrap: wrap; gap: 4px; margin-top: 12px;">
      {#each profile.section_profiles as s}
        <div
          title="{s.section_label} — Risk: {s.risk_level}, Perplexity: {s.perplexity_proxy.toFixed(2)}, Burstiness: {s.burstiness_proxy.toFixed(2)}"
          style="width: 40px; height: 40px; background: {s.risk_color}; border-radius: 4px; cursor: help; display: flex; align-items: center; justify-content: center; color: white; font-size: 10px; font-weight: 600;"
        >
          {s.word_count}
        </div>
      {/each}
    </div>
    <div class="row" style="margin-top: 12px; gap: 16px; font-size: 12px;">
      <span><span style="display:inline-block;width:12px;height:12px;background:#1a8a52;border-radius:2px;vertical-align:middle;"></span> Low risk</span>
      <span><span style="display:inline-block;width:12px;height:12px;background:#b76e00;border-radius:2px;vertical-align:middle;"></span> Medium risk</span>
      <span><span style="display:inline-block;width:12px;height:12px;background:#c0392b;border-radius:2px;vertical-align:middle;"></span> High risk</span>
    </div>
  </div>

  <div class="card">
    <p class="muted" style="font-size: 13px; margin: 0 0 12px;">{profile.explanation}</p>
    <strong>Recommendations:</strong>
    <ul style="margin: 6px 0 0 16px; padding: 0;">
      {#each profile.recommendations as r}<li style="font-size: 13px; margin-bottom: 4px;">{r}</li>{/each}
    </ul>
  </div>
{/if}
