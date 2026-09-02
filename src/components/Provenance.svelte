<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    api,
    type ProvenanceAnalysis,
    type GoogleImportAnalysis,
    type ProvenanceKeyStatus,
    type GoogleStatus,
    type DisclosureCopy,
  } from "../lib/api";
  import ProvenanceTimeline from "./ProvenanceTimeline.svelte";
  import ProvenanceExport from "./ProvenanceExport.svelte";

  let enabled = false;
  let checking = true;
  let disclosure: DisclosureCopy | null = null;
  let showDisclosure = false;

  let source: "docx" | "google" = "docx";
  let docxPath = "";
  let docxName = "";
  let analysis: ProvenanceAnalysis | null = null;

  // Google bridge state (Phase 1.5)
  let google: GoogleStatus | null = null;
  let clientId = "";
  let docUrl = "";
  let googleAnalysis: GoogleImportAnalysis | null = null;
  let gBusy = false;
  let gError = "";

  let error = "";
  let busy = false;
  let keyStatus: ProvenanceKeyStatus | null = null;

  onMount(async () => {
    try {
      [enabled, disclosure, keyStatus, google] = await Promise.all([
        api.provenanceStatus(),
        api.provenanceDisclosureText(),
        api.provenanceKeyStatus(),
        api.googleStatus(),
      ]);
      try {
        clientId = localStorage.getItem("scholarscribe-gclient") ?? "";
      } catch {
        /* non-fatal */
      }
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  });

  async function acceptDisclosure() {
    try {
      await api.provenanceEnable();
      enabled = true;
    } catch (e) {
      error = String(e);
    }
    showDisclosure = false;
  }

  async function disableFeature() {
    if (!confirm("Switch Writing Provenance off? Exported packages remain verifiable.")) return;
    await api.provenanceDisable();
    enabled = false;
  }

  async function pickDocx() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Word documents", extensions: ["docx"] }],
    });
    if (!selected || typeof selected !== "string") return;
    busy = true;
    error = "";
    analysis = null;
    try {
      docxPath = selected;
      docxName = selected.split(/[\\/]/).pop() ?? selected;
      analysis = await api.provenanceAnalyzeDocx(docxPath);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function persistClientId() {
    try {
      localStorage.setItem("scholarscribe-gclient", clientId);
    } catch {
      /* non-fatal */
    }
  }

  async function connectGoogle() {
    gBusy = true;
    gError = "";
    persistClientId();
    try {
      await api.googleConnect(clientId);
      google = await api.googleStatus();
    } catch (e) {
      gError = String(e);
    } finally {
      gBusy = false;
    }
  }

  async function disconnectGoogle() {
    gBusy = true;
    gError = "";
    try {
      await api.googleDisconnect();
      google = await api.googleStatus();
    } catch (e) {
      gError = String(e);
    } finally {
      gBusy = false;
    }
  }

  async function importGoogleDoc() {
    gBusy = true;
    gError = "";
    googleAnalysis = null;
    persistClientId();
    try {
      googleAnalysis = await api.googleImportDoc(clientId, docUrl);
    } catch (e) {
      gError = String(e);
    } finally {
      gBusy = false;
    }
  }

  $: activeSessions = source === "docx" ? (analysis?.sessions ?? []) : (googleAnalysis?.sessions ?? []);
  $: activeAnomalies = source === "docx" ? (analysis?.anomalies ?? []) : (googleAnalysis?.anomalies ?? []);
  $: activeRef = source === "docx" ? docxPath : docUrl;
</script>

<h1>Writing Provenance</h1>
<p class="lead">
  A signed, hash-chained record of your document's real revision history —
  built from Word's Track Changes or Google Docs' version history.
  <strong>Evidence, not verdict:</strong> this is not an AI-detection score and
  not proof of authorship.
</p>

{#if checking}
  <p class="muted">Loading…</p>
{:else if !enabled}
  <div class="card">
    <h3>Opt-in feature — currently switched off</h3>
    <p class="muted small">
      Writing Provenance is off by default. Turning it on lets you analyze the
      revision history of documents you choose and export a signed summary.
      Nothing happens until you point it at a document, and no document content
      ever leaves your device.
    </p>
    <button class="primary" on:click={() => (showDisclosure = true)}>
      Turn on Writing Provenance…
    </button>
  </div>
{:else}
  {#if keyStatus}
    <div class="callout info">
      <strong>Signing key:</strong>
      {#if keyStatus.has_key}
        <code>{keyStatus.fingerprint}</code>
      {:else}
        none yet — one will be created in your OS keychain on first export.
      {/if}
      <span class="muted small"> {keyStatus.note}</span>
    </div>
  {/if}

  <div class="card">
    <div class="row">
      <h3 style="margin:0;">Analyze a document</h3>
      <div class="source-toggle">
        <button class:active={source === "docx"} on:click={() => (source = "docx")}>
          .docx (Word)
        </button>
        <button class:active={source === "google"} on:click={() => (source = "google")}>
          Google Doc
        </button>
      </div>
    </div>

    {#if source === "docx"}
      <div class="controls">
        <button class="primary" on:click={pickDocx} disabled={busy}>
          {busy ? "Analyzing…" : "Choose .docx and analyze…"}
        </button>
        {#if docxName}
          <span class="muted small"><code>{docxName}</code></span>
        {/if}
      </div>
      {#if analysis}
        <div class="stat-row">
          <div><span class="dim">SESSIONS</span><div class="big">{analysis.sessions.length}</div></div>
          <div><span class="dim">REVISIONS</span><div class="big">{analysis.revision_count}</div></div>
          <div><span class="dim">AUTHORS</span><div class="big">{analysis.authors.length}</div></div>
          <div><span class="dim">TIME SPAN</span><div class="big">{analysis.time_span_hours} h</div></div>
          <div><span class="dim">LARGEST INSERT</span><div class="big">{analysis.largest_insertion_pct}%</div></div>
        </div>
        <p class="muted small">{analysis.note}</p>
        <ProvenanceTimeline sessions={analysis.sessions} anomalies={analysis.anomalies} />
      {/if}
    {:else}
      <div class="controls">
        {#if google?.connected}
          <span class="ok-pill">Connected · read-only (drive.readonly)</span>
          <button class="shrink" on:click={disconnectGoogle} disabled={gBusy}>Disconnect</button>
        {:else}
          <input
            class="grow"
            placeholder="Google OAuth Client ID (see help below)"
            bind:value={clientId}
          />
          <button class="primary" on:click={connectGoogle} disabled={gBusy || !clientId.trim()}>
            {gBusy ? "Waiting for Google…" : "Connect Google Doc"}
          </button>
        {/if}
      </div>
      {#if google?.connected}
        <div class="controls" style="margin-top: 8px;">
          <input
            class="grow"
            placeholder="Paste a Google Docs URL or file ID"
            bind:value={docUrl}
          />
          <button class="primary" on:click={importGoogleDoc} disabled={gBusy || !docUrl.trim()}>
            {gBusy ? "Importing…" : "Import revision history"}
          </button>
        </div>
      {:else}
        <p class="muted small" style="margin-top: 8px;">
          One-time setup: create an OAuth Client ID (type “Desktop app”) at
          console.cloud.google.com → APIs &amp; Services → credentials, enable
          the Google Drive API, and paste the Client ID above. ScholarScribe
          requests <strong>read-only</strong> access (drive.readonly), the
          refresh token stays in your OS keychain, and every outbound call
          appears in the Privacy Audit tab.
        </p>
      {/if}
      {#if gError}
        <div class="callout warn" style="margin-top: 8px;">{gError}</div>
      {/if}
      {#if googleAnalysis}
        <div class="stat-row">
          <div><span class="dim">SESSIONS</span><div class="big">{googleAnalysis.sessions.length}</div></div>
          <div><span class="dim">REVISIONS</span><div class="big">{googleAnalysis.revision_count}</div></div>
          <div><span class="dim">AUTHORS</span><div class="big">{googleAnalysis.authors.length}</div></div>
        </div>
        <p class="muted small">{googleAnalysis.note}</p>
        <ProvenanceTimeline sessions={googleAnalysis.sessions} anomalies={googleAnalysis.anomalies} />
      {/if}
    {/if}

    {#if error}
      <div class="callout warn" style="margin-top: 10px;">{error}</div>
    {/if}
  </div>

  {#if activeSessions.length > 0}
    <ProvenanceExport {source} docRef={activeRef} {clientId} />
  {/if}

  <p class="muted small" style="margin-top: 16px;">
    <a href="#/" on:click|preventDefault={disableFeature}>Switch Writing Provenance off</a>
    · Specification &amp; limitations: docs/PROVENANCE_SPEC.md · Policy: docs/ETHICS.md
  </p>
{/if}

{#if showDisclosure && disclosure}
  <div class="overlay" role="dialog" aria-modal="true" aria-label={disclosure.title}>
    <div class="dialog card">
      <h3>{disclosure.title}</h3>
      <div class="dialog-body">
        {#each disclosure.body.split("\n\n") as para}
          <p class="small" style="margin: 8px 0;">{para}</p>
        {/each}
      </div>
      <div class="row">
        <button on:click={() => (showDisclosure = false)}>Cancel</button>
        <button class="primary" on:click={acceptDisclosure}>I understand — turn it on</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .controls {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .grow {
    flex: 1;
    min-width: 240px;
  }
  .source-toggle {
    display: flex;
    gap: 4px;
  }
  .source-toggle button {
    border: 1px solid var(--border);
    background: transparent;
    border-radius: var(--radius-sm);
    padding: 4px 10px;
    font-size: 12px;
    cursor: pointer;
  }
  .source-toggle button.active {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .stat-row {
    display: flex;
    gap: 22px;
    margin: 14px 0 6px 0;
    flex-wrap: wrap;
  }
  .stat-row .dim {
    font-size: 10.5px;
    letter-spacing: 0.04em;
  }
  .stat-row .big {
    font-size: 20px;
    font-weight: 700;
  }
  .ok-pill {
    font-size: 12px;
    color: var(--accent);
    background: var(--accent-soft);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 3px 10px;
  }
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .dialog {
    max-width: 560px;
    width: calc(100% - 40px);
    max-height: 80vh;
    overflow: auto;
  }
  .dialog-body {
    margin: 10px 0;
  }
</style>
