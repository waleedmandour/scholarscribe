<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type ModelInfo } from "../lib/api";

  export let ollamaOk = false;
  let inputText = "";
  let inputPath = "";
  let models: ModelInfo[] = [];
  let selectedModel = "";
  let result: any = null;
  let sectionResult: any = null;
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

  async function loadModels() {
    if (!ollamaOk) return;
    try { models = await api.ollamaListModels(); if (models.length > 0 && !selectedModel) selectedModel = models[0].name; }
    catch (e) { console.error(e); }
  }
  $: if (ollamaOk && models.length === 0) loadModels();

  async function generate() {
    if (!inputText.trim() || !selectedModel) { error = "Load a draft and pick a model."; return; }
    error = ""; busy = true; result = null;
    try { result = await api.generateAbstract(selectedModel, inputText, 250, "general academic"); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }

  async function generateSectionCommentary() {
    if (!inputText.trim() || !selectedModel) { error = "Load a draft and pick a model."; return; }
    error = ""; busy = true; sectionResult = null;
    try { sectionResult = await api.generateSectionCommentary(selectedModel, inputText); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }
</script>

<h1>Abstract Generator</h1>
<p class="lead">
  Generate a structured abstract (Background/Methods/Results/Conclusions) from your manuscript
  using a locally-installed LLM. Also generates section-by-section commentary. Runs entirely on
  your device.
</p>

{#if !ollamaOk}
  <div class="callout warn"><strong>Ollama is not running.</strong> Start Ollama and install a model first.</div>
{:else if models.length === 0}
  <div class="callout warn"><strong>No models installed.</strong> Download a model from the Models tab first.</div>
{/if}

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="card-title">Draft input</div>
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}<span class="dim" style="font-size: 11px;">{inputPath}</span>{/if}
  </div>
  <textarea bind:value={inputText} rows="8" placeholder="Paste your manuscript here…"></textarea>
  <div class="dim" style="font-size: 11px;">{inputText.length.toLocaleString()} chars</div>
</div>

<div class="card">
  <div class="card-title">Model</div>
  <select bind:value={selectedModel} disabled={!ollamaOk || models.length === 0}>
    {#each models as m}<option value={m.name}>{m.name}</option>{/each}
  </select>
  <div class="row" style="margin-top: 12px; gap: 8px;">
    <button class="primary" on:click={generate} disabled={busy || !inputText.trim() || !selectedModel}>
      {busy ? "Generating…" : "Generate abstract"}
    </button>
    <button on:click={generateSectionCommentary} disabled={busy || !inputText.trim() || !selectedModel}>
      Generate section commentary
    </button>
  </div>
</div>

{#if result}
  <h2>Generated abstract</h2>
  <div class="card">
    <pre style="white-space: pre-wrap; word-wrap: break-word; font-family: inherit; font-size: 14px; line-height: 1.6; background: var(--bg-elev-2); padding: 16px; border-radius: var(--radius-sm);">{result.abstract_text}</pre>
  </div>
  <div class="callout warn"><strong>Review required.</strong> LLMs may hallucinate findings. Edit carefully before submission.</div>
{/if}

{#if sectionResult}
  <h2>Section commentary</h2>
  <div class="card">
    {#each sectionResult.commentaries as c}
      <div style="margin-bottom: 12px;">
        <strong>{c.section_name}</strong>
        <p class="muted" style="margin: 4px 0 0;">{c.summary}</p>
      </div>
    {/each}
  </div>
{/if}
