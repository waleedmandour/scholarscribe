<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    api,
    type ModelInfo,
    type ChatMessage,
  } from "../lib/api";

  export let ollamaOk = false;

  let models: ModelInfo[] = [];
  let selectedModel = "";
  let messages: ChatMessage[] = [];
  let input = "";
  let busy = false;
  let error = "";
  let temperature = 0.7;
  let modelsLoaded = false;
  // Attached manuscript / paper section. When set, the file content is
  // prepended to the next user message so the model has the manuscript
  // as context for the user's question. The attachment is "consumed"
  // (cleared) after the first send so subsequent questions rely on the
  // conversation history rather than re-sending the full manuscript.
  let attachedFile: { name: string; content: string } | null = null;
  let attaching = false;

  const SYSTEM_PROMPT: ChatMessage = {
    role: "system",
    content:
      "You are ScholarScribe, a Socratic writing coach for academic researchers. Your role is to help the user think through their own writing, never to write for them.\n\n" +
      "ABSOLUTE PROHIBITION: You must NEVER rewrite, paraphrase, rephrase, or generate any part of the user's manuscript, not even a single sentence or a single clause. If the user asks you to 'rewrite this', 'rephrase this', 'give me a better version', 'fix this paragraph', 'draft this for me', or any equivalent, you DECLINE and explain that you only coach, you do not write. Offer instead to ask guiding questions that help the user find their own revision.\n\n" +
      "Your default mode is Socratic questioning. For any writing the user shares, respond with one to three questions that help them diagnose the issue themselves, not with corrections or rewritten text. Good Socratic questions:\n" +
      "- Point to a specific phrase or sentence and ask about its intent ('What did you mean the reader to take from this sentence?').\n" +
      "- Surface tension between claims ('You say X here and Y in the next paragraph. How do they fit together?').\n" +
      "- Probe audience awareness ('Would a reader outside your field follow this? Where might they get lost?').\n" +
      "- Surface unstated assumptions ('What assumption are you making here that the reader might not share?').\n" +
      "- Probe evidence ('What evidence would make this claim stronger? Is that evidence you have, or evidence you need to find?').\n" +
      "- Surface structure ('If you had to give this paragraph a one-sentence headline, what would it be? Does the rest of the paragraph serve that headline?').\n\n" +
      "Other effective techniques to use when Socratic questioning is not enough:\n" +
      "- Active recall: ask the user to articulate their intent for a passage before discussing whether it achieves that intent.\n" +
      "- Metacognition prompts: ask the user to reflect on their writing process ('Where did you get stuck when writing this? What did you try first?').\n" +
      "- Scaffolding: break a complex revision task into smaller questions the user can answer one at a time.\n" +
      "  Modeling thinking (not the text): 'When I read a claim like this, I ask: is the evidence sufficient, is the counterargument addressed, is the language precise. Which of those do you want to look at first?'.\n" +
      "- Self-assessment: prompt the user to evaluate their own writing against a checklist ('For each paragraph, can you state its job in one sentence?').\n" +
      "- Specific, actionable feedback: when you must give feedback, point to a specific passage and phrase the feedback as a question ('This sentence uses three abstractions in a row. Could you ground any of them in a concrete example?').\n\n" +
      "Be concise. Do not invent citations. If the user asks you to 'evade AI detection' or 'beat Turnitin', decline and explain that detection-evasion is not a service you provide; offer instead to help improve clarity or rigor.\n\n" +
      "Remember: the user is the author and makes all final editorial decisions. Your job is to make them a better writer, not to write for them.",
  };

  async function loadModels() {
    if (!ollamaOk || modelsLoaded) return;
    try {
      models = await api.ollamaListModels();
      if (models.length > 0 && !selectedModel) selectedModel = models[0].name;
      modelsLoaded = true;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(loadModels);

  // Re-fetch when ollamaOk flips true (e.g. user starts Ollama while app is open).
  $: if (ollamaOk && !modelsLoaded) loadModels();

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Manuscript + text documents", extensions: ["txt", "md", "markdown", "tex", "rst", "docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    attaching = true;
    error = "";
    try {
      const content = await api.readTextFile(selected);
      // Extract just the filename from the path for display.
      const name = selected.split(/[\\/]/).pop() || selected;
      attachedFile = { name, content };
    } catch (e) {
      error = String(e);
    } finally {
      attaching = false;
    }
  }

  function removeAttached() {
    attachedFile = null;
  }

  async function send() {
    if (!input.trim() || !selectedModel) return;
    error = "";
    // If a file is attached, prepend its content to the user's
    // message so the model has the manuscript as context for this
    // turn. The attachment is then cleared so subsequent questions
    // rely on the conversation history (the manuscript is already
    // in the messages list from this turn).
    let userContent = input;
    if (attachedFile) {
      const separator = "\n\n---\n\n";
      const header = `[Attached manuscript: ${attachedFile.name}]\n\n${attachedFile.content}`;
      userContent = `${header}${separator}${input}`;
    }
    const userMsg: ChatMessage = { role: "user", content: userContent };
    messages = [...messages, userMsg];
    input = "";
    attachedFile = null;
    busy = true;
    try {
      const response = await api.ollamaChat({
        model: selectedModel,
        messages: [SYSTEM_PROMPT, ...messages],
        temperature,
      });
      messages = [...messages, response];
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      send();
    }
  }

  function clear() {
    messages = [];
    attachedFile = null;
  }

  // Word count of the attached file content, for display in the
  // context panel. Reactive: re-computes whenever attachedFile
  // changes.
  $: attachedWordCount = attachedFile
    ? attachedFile.content.split(/\s+/).filter(Boolean).length
    : 0;
</script>

<h1>Chat</h1>
<p class="lead">
  A simple local-only chat interface to your installed models. Useful for brainstorming phrasing, asking a model to
  critique a paragraph, or running a quick sanity check on an idea. Attach a manuscript or paper section to discuss
  it directly with the model. Everything stays on your device.
</p>

{#if !ollamaOk}
  <div class="callout warn"><strong>No backend.</strong> Start Ollama to use chat.</div>
{:else if models.length === 0}
  <div class="callout warn"><strong>No models installed.</strong> Go to the Models tab and download one first.</div>
{:else}
  <div class="card">
    <div class="row">
      <div>
        <label class="dim" for="chat-model" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Model</label>
        <select id="chat-model" bind:value={selectedModel}>
          {#each models as m}<option value={m.name}>{m.name}</option>{/each}
        </select>
      </div>
      <div style="flex: 0 0 200px;">
        <label class="dim" for="chat-temp" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">
          Temperature: {temperature.toFixed(2)}
        </label>
        <input id="chat-temp" type="range" min="0" max="1" step="0.05" bind:value={temperature} style="padding: 0; width: 100%;" />
      </div>
      <button class="shrink danger" on:click={clear}>Clear</button>
    </div>
  </div>

  <div class="card" style="min-height: 320px; display: flex; flex-direction: column;">
    <div style="flex: 1; overflow-y: auto; padding: 4px;">
      {#if messages.length === 0}
        <p class="no-data">No messages yet. Try: paste a paragraph and ask "What is this paragraph trying to say?" or attach a manuscript and ask "Where might a reader outside my field get lost?". The model will ask you questions, not rewrite your text. You make all the revisions.</p>
      {:else}
        {#each messages as m}
          <div style="margin-bottom: 12px;">
            <div class="dim" style="font-size: var(--font-sm); margin-bottom: 2px; text-transform: capitalize;">{m.role}</div>
            <div style="background: var(--bg-elev-2); padding: 8px 12px; border-radius: var(--radius-sm); white-space: pre-wrap;">{m.content}</div>
          </div>
        {/each}
      {/if}
    </div>

    {#if attachedFile}
      <div style="border-top: 1px solid var(--border); padding: 8px 12px; background: var(--accent-soft); display: flex; align-items: center; gap: 8px;">
        <span style="font-size: var(--font-sm); color: var(--accent); font-weight: 600;">Attached:</span>
        <code style="font-size: var(--font-sm); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{attachedFile.name}</code>
        <span class="dim" style="font-size: var(--font-sm);">{attachedWordCount.toLocaleString()} words</span>
        <button class="shrink" on:click={removeAttached} style="font-size: var(--font-sm); padding: 2px 8px;">Remove</button>
      </div>
    {/if}

    <div style="border-top: 1px solid var(--border); padding-top: 12px;">
      <textarea
        bind:value={input}
        on:keydown={onKeydown}
        rows="3"
        placeholder={attachedFile ? "Ask a question about the attached manuscript. Ctrl+Enter to send." : "Type a message. Ctrl+Enter to send."}
        disabled={busy}
      ></textarea>
      <div class="row" style="margin-top: 6px;">
        <span class="dim" style="font-size: var(--font-sm);">Runs entirely on-device via Ollama.</span>
        <div class="spacer"></div>
        <button class="shrink" on:click={pickFile} disabled={busy || attaching}>
          {attaching ? "Loading…" : attachedFile ? "Replace manuscript…" : "Attach manuscript…"}
        </button>
        <button class="primary shrink" on:click={send} disabled={busy || !input.trim()}>
          {busy ? "Working…" : "Send"}
        </button>
      </div>
    </div>
  </div>

  {#if error}<div class="callout warn" style="margin-top: 12px;">{error}</div>{/if}

  <div class="callout info" style="margin-top: 12px;">
    <strong>Socratic coach, not a ghostwriter.</strong> The system prompt strictly forbids the model from rewriting, paraphrasing, or generating any part of your manuscript. It must ask guiding questions instead. If you ask it to "rewrite this" or "give me a better version", it will decline and offer to coach. The model also refuses requests to evade AI detectors or submit AI-generated content as original work. If you find a model still rewriting your text or complying with evasion requests, please open an issue, the guardrail wording is part of the project's ethical commitments.
  </div>

  {#if attachedFile}
    <div class="callout info" style="margin-top: 8px;">
      <strong>How the attachment works.</strong>
      When you send your next message, the attached manuscript is prepended to your question so the model has the
      full text as context. The attachment is then cleared so subsequent questions rely on the conversation
      history (the manuscript is already in the messages list from this turn). For very long manuscripts, consider
      attaching just the section you want to discuss rather than the full document.
    </div>
  {/if}
{/if}
