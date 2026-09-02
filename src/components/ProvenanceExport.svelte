<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import {
    api,
    type ProvenanceExportResult,
  } from "../lib/api";

  export let source: "docx" | "google" = "docx";
  /** File path (docx) or Google Doc URL/ID (google). */
  export let docRef = "";
  /** Google OAuth client id, required for the google source. */
  export let clientId = "";

  let busy = false;
  let error = "";
  let result: ProvenanceExportResult | null = null;
  let baselineText = "";
  let showAdvanced = false;

  async function doExport() {
    if (!docRef) return;
    busy = true;
    error = "";
    result = null;
    try {
      const path = await save({
        title: "Export provenance package",
        defaultPath: "provenance-export.zip",
        filters: [{ name: "Provenance package", extensions: ["zip"] }],
      });
      if (!path) return;
      const baseline = baselineText.trim().length >= 40 ? baselineText : null;
      result =
        source === "docx"
          ? await api.provenanceExportZip(docRef, path, baseline)
          : await api.googleExportZip(clientId, docRef, path, baseline);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function exportPublicKey() {
    busy = true;
    error = "";
    try {
      const path = await save({
        title: "Export public key",
        defaultPath: "scholarscribe-signing-key.pub.json",
        filters: [{ name: "Public key", extensions: ["json"] }],
      });
      if (!path) return;
      const pk = await api.provenanceExportPublicKey(path);
      result = result ?? null;
      error = "";
      resultNote = `Public key written to ${pk.written_to}. Share this with reviewers so they can verify your exports.`;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  let resultNote = "";
</script>

<div class="card">
  <h3>Export signed package</h3>
  <p class="muted small">
    Produces a .zip with a hash-chained, Ed25519-signed manifest: hashes and
    counts only, never document text. Anyone can verify it with the bundled
    verifier (works offline, no installation).
  </p>

  <div class="controls">
    <button class="primary" on:click={doExport} disabled={busy || !docRef}>
      {busy ? "Working…" : "Export provenance (.zip)"}
    </button>
    <button class="shrink" on:click={exportPublicKey} disabled={busy}>
      Export public key…
    </button>
    <button class="shrink" on:click={() => (showAdvanced = !showAdvanced)}>
      {showAdvanced ? "Hide" : "Optional: style baseline…"}
    </button>
  </div>

  {#if showAdvanced}
    <div class="advanced">
      <label class="small" for="baseline">
        Baseline text (optional): a sample of your earlier writing (e.g. your
        previous paper). If empty, the earliest tracked session is used as the
        baseline and labeled as such.
      </label>
      <textarea
        id="baseline"
        rows="4"
        bind:value={baselineText}
        placeholder="Paste a representative sample of your earlier writing…"
      ></textarea>
      <p class="muted small" style="margin: 4px 0 0 0;">
        The exported package contains aggregate metrics only, never this text
        and never the document text.
      </p>
    </div>
  {/if}

  {#if error}
    <div class="callout warn" style="margin-top: 10px;">{error}</div>
  {/if}

  {#if resultNote}
    <div class="callout info" style="margin-top: 10px;">{resultNote}</div>
  {/if}

  {#if result}
    <div class="callout info" style="margin-top: 10px;">
      <strong>Exported.</strong>
      <div class="small" style="margin-top: 6px;">
        <div><span class="muted">File:</span> <code>{result.output_path}</code></div>
        <div><span class="muted">Sessions:</span> {result.session_count}</div>
        <div><span class="muted">Time span:</span> {result.time_span_hours} h</div>
        <div><span class="muted">Largest insertion:</span> {result.largest_insertion_pct}% of added text</div>
        <div>
          <span class="muted">Chain intact:</span> {result.chain_intact ? "yes" : "NO"}
          · <span class="muted">Signature:</span> {result.signature_valid ? "valid" : "INVALID"}
        </div>
        <div><span class="muted">Signing key:</span> <code>{result.manifest_fingerprint}</code></div>
      </div>
      <p class="small" style="margin: 8px 0 0 0;">
        Reminder: this package records process. It is not an AI-detection
        score and not proof of authorship.
      </p>
    </div>
  {/if}
</div>

<style>
  .controls {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .advanced {
    margin-top: 10px;
  }
  textarea {
    width: 100%;
    margin-top: 6px;
    font-family: inherit;
    font-size: 13px;
    resize: vertical;
  }
</style>
