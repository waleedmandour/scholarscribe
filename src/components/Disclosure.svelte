<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    type VenueTemplate,
    type DisclosureOutput,
    type DisclosureInput,
  } from "../lib/api";

  let venues: VenueTemplate[] = [];
  let selectedVenue = "icmje";
  let toolName = "";
  let taskDescription = "";
  let modelUsed = "";
  let authorName = "";
  let result: DisclosureOutput | null = null;
  let error = "";
  let copied = false;

  onMount(async () => {
    venues = await api.listVenueTemplates();
  });

  async function generate() {
    error = "";
    if (!toolName.trim() || !taskDescription.trim()) {
      error = "Tool name and task description are required.";
      return;
    }
    const input: DisclosureInput = {
      venue_id: selectedVenue,
      tool_name: toolName.trim(),
      task_description: taskDescription.trim(),
      model_used: modelUsed.trim() || null,
      author_name: authorName.trim() || null,
    };
    try {
      result = await api.generateDisclosure(input);
    } catch (e) {
      error = String(e);
    }
  }

  async function copy() {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.statement);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      error = "Clipboard not available. Select the text manually and copy.";
    }
  }

  $: currentVenue = venues.find((v) => v.id === selectedVenue);
</script>

<h1>Disclosure Assistant</h1>
<p class="lead">
  Generate a venue-compliant AI-use disclosure statement in one click. Major academic publishers, ICMJE journals,
  Nature, IEEE, Elsevier, ACL, now require explicit disclosure of AI assistance. Failing to disclose is itself
  a research-integrity violation. Use this tool to draft a statement you can paste into your manuscript or cover letter.
</p>

<div class="card">
  <div class="card-title">Generate a disclosure statement</div>
  <div class="card-subtitle">All fields are processed locally, nothing is sent anywhere.</div>

  <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-top: 12px;">
    <div>
      <label class="dim" for="disc-venue" style="font-size: 11px; display: block; margin-bottom: 4px;">Target venue</label>
      <select id="disc-venue" bind:value={selectedVenue}>
        {#each venues as v}<option value={v.id}>{v.label}</option>{/each}
      </select>
    </div>
    <div>
      <label class="dim" for="disc-tool" style="font-size: 11px; display: block; margin-bottom: 4px;">Tool used</label>
      <input id="disc-tool" type="text" bind:value={toolName} placeholder="e.g. ChatGPT (GPT-4o), Gemini, Claude" />
    </div>
    <div>
      <label class="dim" for="disc-model" style="font-size: 11px; display: block; margin-bottom: 4px;">Model (optional)</label>
      <input id="disc-model" type="text" bind:value={modelUsed} placeholder="e.g. GPT-4o, gemma2:9b" />
    </div>
    <div>
      <label class="dim" for="disc-author" style="font-size: 11px; display: block; margin-bottom: 4px;">Your name (optional, signs the statement)</label>
      <input id="disc-author" type="text" bind:value={authorName} placeholder="e.g. Waleed Mandour" />
    </div>
    <div style="grid-column: 1 / -1;">
      <label class="dim" for="disc-task" style="font-size: 11px; display: block; margin-bottom: 4px;">What did you use the tool for?</label>
      <textarea id="disc-task"
        bind:value={taskDescription}
        rows="3"
        placeholder="e.g. improve the language and readability of the manuscript, generate an outline for the introduction, suggest alternative phrasings for the abstract"
      ></textarea>
    </div>
  </div>

  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={generate}>Generate disclosure</button>
    {#if currentVenue?.policy_url}
      <a href={currentVenue.policy_url} target="_blank" rel="noopener" class="dim" style="font-size: 12px; align-self: center;">
        Read the venue's official policy →
      </a>
    {/if}
  </div>

  {#if error}<div class="callout warn" style="margin-top: 12px;">{error}</div>{/if}
</div>

{#if currentVenue}
  <div class="card">
    <div class="card-title">{currentVenue.label}</div>
    <div class="card-subtitle">Quick policy summary</div>
    <p class="muted" style="font-size: 13px; margin: 0 0 8px;">{currentVenue.notes}</p>
    <p class="muted" style="font-size: 13px; margin: 0;">
      Requires disclosure in:
      {#if currentVenue.requires_in_manuscript}<span class="tag">manuscript</span>{/if}
      {#if currentVenue.requires_in_cover_letter}<span class="tag">cover letter</span>{/if}
    </p>
  </div>
{/if}

{#if result}
  <h2>Generated statement</h2>
  <div class="card">
    <div class="row" style="margin-bottom: 12px;">
      <strong>Where to include</strong>
      <div class="spacer"></div>
      <button class="shrink" on:click={copy}>{copied ? "Copied!" : "Copy"}</button>
    </div>
    <p class="muted" style="font-size: 13px; margin: 0 0 12px; white-space: pre-wrap;">{result.where_to_include}</p>
    <pre style="white-space: pre-wrap;">{result.statement}</pre>
    {#each result.warnings as w}
      <div class="callout warn" style="margin-top: 12px;">{w}</div>
    {/each}
  </div>
{/if}

<div class="callout info">
  <strong>Why this matters.</strong>
  Disclosure protects <em>you</em>. If a reviewer or editor later learns AI was used and not disclosed, the
  consequence is far worse than disclosing upfront. When in doubt, disclose. When still in doubt, ask the editor.
</div>
