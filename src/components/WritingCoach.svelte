<script lang="ts">
  import { api, type ModelInfo, type ChatMessage } from "../lib/api";

  export let ollamaOk = false;
  let models: ModelInfo[] = [];
  let selectedModel = "";
  let messages: ChatMessage[] = [];
  let input = "";
  let busy = false;
  let error = "";

  const COACH_INTRO = "I'm your academic writing coach. I'll help you develop your ideas through questions — not by writing for you. Share a paragraph or describe what you're working on, and I'll ask probing questions about your reasoning, evidence, and argument structure.";

  async function loadModels() {
    if (!ollamaOk) return;
    try {
      models = await api.ollamaListModels();
      if (models.length > 0 && !selectedModel) selectedModel = models[0].name;
    } catch (e) { console.error(e); }
  }
  $: if (ollamaOk && models.length === 0) loadModels();

  async function send() {
    if (!input.trim() || !selectedModel) return;
    error = "";
    const userMsg: ChatMessage = { role: "user", content: input };
    messages = [...messages, userMsg];
    input = "";
    busy = true;
    try {
      const response = await api.writingCoachChat(selectedModel, messages);
      messages = [...messages, response];
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); send(); }
  }

  function clear() { messages = []; }
</script>

<h1>Structured Writing Coach</h1>
<p class="lead">
  A specialized chat mode where the LLM acts as a discipline-aware writing coach — asking Socratic
  questions that draw out your genuine reasoning rather than suggesting text. "What was your reasoning
  for choosing this method over X?" is fundamentally different from "rewrite this paragraph." This
  keeps the intellectual content authentically yours while providing scaffolding.
</p>

<div class="callout info">
  <strong>How this works.</strong> The coach will NEVER write or rewrite text for you. Instead, it asks
  probing questions about your reasoning, evidence, and argument structure. If you ask it to rewrite
  something, it will decline and redirect you to articulating your intent first. This ensures the
  intellectual content remains authentically yours.
</div>

{#if !ollamaOk}
  <div class="callout warn"><strong>Ollama is not running.</strong> Start Ollama and install a model first.</div>
{:else if models.length === 0}
  <div class="callout warn"><strong>No models installed.</strong> Download a model from the Models tab first.</div>
{:else}
  <div class="card">
    <div class="row">
      <div>
        <label class="dim" for="wc-model" style="font-size: 11px; display: block; margin-bottom: 4px;">Model</label>
        <select id="wc-model" bind:value={selectedModel}>
          {#each models as m}<option value={m.name}>{m.name}</option>{/each}
        </select>
      </div>
      <div class="spacer"></div>
      <button class="shrink danger" on:click={clear}>Clear</button>
    </div>
  </div>

  <div class="card" style="min-height: 320px; display: flex; flex-direction: column;">
    <div style="flex: 1; overflow-y: auto; padding: 4px;">
      {#if messages.length === 0}
        <div class="callout info" style="margin: 0;">{COACH_INTRO}</div>
      {:else}
        {#each messages as m}
          <div style="margin-bottom: 12px;">
            <div class="dim" style="font-size: 11px; margin-bottom: 2px; text-transform: capitalize;">
              {m.role === "user" ? "You" : "Coach"}
            </div>
            <div style="background: var(--bg-elev-2); padding: 8px 12px; border-radius: var(--radius-sm); white-space: pre-wrap;">{m.content}</div>
          </div>
        {/each}
      {/if}
    </div>
    <div style="border-top: 1px solid var(--border); padding-top: 12px;">
      <textarea
        bind:value={input}
        on:keydown={onKeydown}
        rows="3"
        placeholder="Share a paragraph or describe what you're working on… (Ctrl+Enter to send)"
        disabled={busy}
      ></textarea>
      <div class="row" style="margin-top: 6px;">
        <span class="dim" style="font-size: 11px;">The coach asks questions — it does not write for you.</span>
        <div class="spacer"></div>
        <button class="primary shrink" on:click={send} disabled={busy || !input.trim()}>
          {busy ? "Thinking…" : "Send"}
        </button>
      </div>
    </div>
  </div>

  {#if error}<div class="callout warn" style="margin-top: 12px;">{error}</div>{/if}
{/if}
