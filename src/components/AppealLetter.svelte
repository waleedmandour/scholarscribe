<script lang="ts">
  import { api, type AppealLetterInput, type AppealLetterOutput } from "../lib/api";

  let input: AppealLetterInput = {
    researcher_name: "", researcher_title: "Dr.", institution: "",
    manuscript_title: "", venue: "", editor_name: "",
    detector_used: "Turnitin AI", detector_score: "",
    process_description: "", additional_evidence: "",
  };
  let output: AppealLetterOutput | null = null;
  let busy = false;
  let error = "";
  let copied = false;

  async function generate() {
    if (!input.researcher_name.trim() || !input.manuscript_title.trim()) {
      error = "Researcher name and manuscript title are required."; return;
    }
    error = ""; busy = true;
    try { output = await api.generateAppealLetter(input); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }

  async function copy() {
    if (!output) return;
    try { await navigator.clipboard.writeText(output.letter); copied = true; setTimeout(() => copied = false, 1500); }
    catch { error = "Clipboard not available."; }
  }
</script>

<h1>False-Positive Appeal Letter Generator</h1>
<p class="lead">
  Drafts a professional, evidence-based appeal letter for researchers whose work has been falsely
  flagged by AI detection tools. References Liang et al. (2023) and Weber-Wulff et al. (2023) by name,
  and Turnitin's own guidance that scores are indicators, not proof. All processing is local.
</p>

<div class="callout info">
  <strong>Entirely ethical.</strong> False positives are documented in the peer-reviewed literature.
  This tool helps researchers respond to them using an evidence-based approach — the same standard
  the Disclosure Assistant uses for voluntary AI-use disclosure.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="card-title">Your details</div>
  <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px;">
    <div><label class="dim" style="font-size:11px;">Title</label><input type="text" bind:value={input.researcher_title} /></div>
    <div><label class="dim" style="font-size:11px;">Name *</label><input type="text" bind:value={input.researcher_name} placeholder="Dr. Waleed Mandour" /></div>
    <div><label class="dim" style="font-size:11px;">Institution</label><input type="text" bind:value={input.institution} /></div>
    <div><label class="dim" style="font-size:11px;">Manuscript title *</label><input type="text" bind:value={input.manuscript_title} /></div>
    <div><label class="dim" style="font-size:11px;">Venue</label><input type="text" bind:value={input.venue} placeholder="Journal of X" /></div>
    <div><label class="dim" style="font-size:11px;">Editor name (if known)</label><input type="text" bind:value={input.editor_name} /></div>
    <div><label class="dim" style="font-size:11px;">Detector used</label><input type="text" bind:value={input.detector_used} /></div>
    <div><label class="dim" style="font-size:11px;">Detector score</label><input type="text" bind:value={input.detector_score} placeholder="92% AI-generated" /></div>
  </div>
  <div style="margin-top: 12px;">
    <label class="dim" style="font-size:11px; display:block; margin-bottom:4px;">Describe your writing process</label>
    <textarea bind:value={input.process_description} rows="4" placeholder="I drafted the manuscript over 3 weeks, starting from an outline I created on [date]. I conducted the literature review using [databases], collected data on [date], and wrote the methods section first, followed by results, discussion, and introduction. I revised the manuscript 5 times based on feedback from co-authors."></textarea>
  </div>
  <div style="margin-top: 8px;">
    <label class="dim" style="font-size:11px; display:block; margin-bottom:4px;">Additional evidence (optional)</label>
    <textarea bind:value={input.additional_evidence} rows="3" placeholder="I can provide version history from Google Docs showing edits over time, draft comments from co-authors, and dated research notes."></textarea>
  </div>
  <div class="row" style="margin-top: 12px;">
    <button class="primary" on:click={generate} disabled={busy}>Generate appeal letter</button>
  </div>
</div>

{#if output}
  <h2>Generated letter</h2>
  <div class="card">
    <div class="row" style="margin-bottom: 12px;">
      <div class="spacer"></div>
      <button class="shrink" on:click={copy}>{copied ? "Copied!" : "Copy letter"}</button>
    </div>
    <pre style="white-space: pre-wrap; word-wrap: break-word; font-family: inherit; font-size: 14px; line-height: 1.6; background: var(--bg-elev-2); padding: 16px; border-radius: var(--radius-sm); max-height: 600px; overflow-y: auto;">{output.letter}</pre>
  </div>
{/if}
