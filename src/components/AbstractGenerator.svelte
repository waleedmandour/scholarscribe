<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type AbstractResult, type ModelInfo } from "../lib/api";

  export let ollamaOk = false;

  let inputText = "";
  let inputPath = "";
  let models: ModelInfo[] = [];
  let selectedModel = "";
  let maxWords = 250;
  let venue = "general academic";
  let result: AbstractResult | null = null;
  let busy = false;
  let error = "";
  let copied = false;

  const venues = [
    "general academic",
    "Nature (research article)",
    "ICMJE medical journal (JAMA, NEJM, Lancet)",
    "IEEE conference paper",
    "IEEE transactions",
    "ACM SIGCHI",
    "ACL/EMNLP (NLP conference)",
    "PLOS ONE",
    "computer science thesis chapter",
    "humanities journal",
  ];

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] },
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

  async function loadModels() {
    if (!ollamaOk) return;
    try {
      models = await api.ollamaListModels();
      if (models.length > 0 && !selectedModel) {
        selectedModel = models[0].name;
      }
    } catch (e) {
      console.error(e);
    }
  }

  $: if (ollamaOk && models.length === 0) loadModels();

  async function generate() {
    if (!inputText.trim()) {
      error = "Paste your draft or open a file first.";
      return;
    }
    if (!selectedModel) {
      error = "Pick an installed model first.";
      return;
    }
    error = "";
    busy = true;
    result = null;
    try {
      result = await api.generateAbstract(selectedModel, inputText, maxWords, venue);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function copyAbstract() {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.abstract_text);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      error = "Clipboard not available. Select the text manually and copy.";
    }
  }
</script>

<h1>Abstract Generator</h1>
<p class="lead">
  Generate a structured abstract (Background / Methods / Results / Conclusions) from your
  manuscript using a locally-installed LLM. The model runs entirely on your device via Ollama —
  your draft never leaves your computer.
</p>

<div class="callout info">
  <strong>Ethical use.</strong>
  This tool helps you draft an abstract for a manuscript you wrote. If you use it, disclose
  AI assistance in your cover letter or methods section (see the Disclosure tab). Always
  review and edit the generated abstract — LLMs can hallucinate findings or misrepresent
  your work. The abstract is a draft, not a finished product.
</div>

{#if !ollamaOk}
  <div class="callout warn">
    <strong>Ollama is not running.</strong> Start Ollama and install a model from the Models tab to use the Abstract Generator.
  </div>
{:else if models.length === 0}
  <div class="callout warn">
    <strong>No models installed.</strong> Go to the Models tab and download a model first. Recommended for abstracts: Gemma 3 12B, Qwen 3 8B, or Phi-4 14B.
  </div>
{/if}

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="card-title">Input draft</div>
  <div class="card-subtitle">Paste your manuscript (or the body without the abstract). For best results, include at least 1,000 words.</div>
  <div class="row" style="margin-bottom: 8px;">
    <button class="shrink" on:click={pickFile}>Open file…</button>
    {#if inputPath}<span class="dim" style="font-size: 11px; word-break: break-all;">{inputPath}</span>{/if}
  </div>
  <textarea bind:value={inputText} rows="10" placeholder="Paste your manuscript here, or use Open file… to load a .txt/.md/.docx file"></textarea>
  <div class="dim" style="font-size: 11px; margin-top: 4px;">{inputText.length.toLocaleString()} characters · ~{Math.round(inputText.split(/\s+/).filter(Boolean).length).toLocaleString()} words</div>
</div>

<div class="card">
  <div class="card-title">Generation options</div>
  <div style="display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px; margin-top: 8px;">
    <div>
      <label class="dim" for="ag-model" style="font-size: 11px; display: block; margin-bottom: 4px;">Model</label>
      <select id="ag-model" bind:value={selectedModel} disabled={!ollamaOk || models.length === 0}>
        {#each models as m}<option value={m.name}>{m.name}</option>{/each}
      </select>
    </div>
    <div>
      <label class="dim" for="ag-maxwords" style="font-size: 11px; display: block; margin-bottom: 4px;">Max words (target)</label>
      <input id="ag-maxwords" type="number" bind:value={maxWords} min="100" max="500" step="10" />
    </div>
    <div>
      <label class="dim" for="ag-venue" style="font-size: 11px; display: block; margin-bottom: 4px;">Venue style</label>
      <select id="ag-venue" bind:value={venue}>
        {#each venues as v}<option value={v}>{v}</option>{/each}
      </select>
    </div>
  </div>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={generate} disabled={busy || !inputText.trim() || !selectedModel}>
      {busy ? "Generating… (may take 30-60s)" : "Generate abstract"}
    </button>
  </div>
</div>

{#if result}
  <h2>Generated abstract</h2>
  <div class="card">
    <div class="row" style="margin-bottom: 12px; font-size: 13px;">
      <div><span class="dim">Model:</span> {result.model_used}</div>
      <div><span class="dim">Draft length:</span> {result.draft_length_chars.toLocaleString()} chars</div>
      <div><span class="dim">Approx. prompt tokens:</span> {result.prompt_tokens.toLocaleString()}</div>
      <div class="spacer"></div>
      <button class="shrink" on:click={copyAbstract}>{copied ? "Copied!" : "Copy abstract"}</button>
    </div>
    <pre style="white-space: pre-wrap; word-wrap: break-word; font-family: inherit; font-size: 14px; line-height: 1.6; background: var(--bg-elev-2); padding: 16px; border-radius: var(--radius-sm);">{result.abstract_text}</pre>
  </div>

  <div class="callout warn">
    <strong>Review required.</strong>
    This is a draft generated by a local LLM. It may:
    <ul style="margin: 6px 0 0 16px; padding: 0;">
      <li>Hallucinate findings not present in your draft</li>
      <li>Misrepresent the magnitude or significance of your results</li>
      <li>Use generic phrasing that doesn't match your actual contribution</li>
      <li>Miss nuances that a human reader would catch</li>
    </ul>
    <strong>Edit it carefully before submission. You are responsible for the accuracy of every claim in your abstract.</strong>
  </div>
{/if}
