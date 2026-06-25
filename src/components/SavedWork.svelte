<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-shell";
  import { api, type DraftMeta, type Draft } from "../lib/api";

  let enabled = false;
  let drafts: DraftMeta[] = [];
  let loading = false;
  let error = "";
  let dataDir = "";
  let selectedDraft: Draft | null = null;
  let showEnableDialog = false;

  async function refresh() {
    loading = true;
    error = "";
    try {
      enabled = await api.persistenceStatus();
      dataDir = await api.dataDirPath();
      if (enabled) {
        drafts = await api.draftList();
      } else {
        drafts = [];
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  async function enablePersistence() {
    try {
      await api.persistenceEnable();
      showEnableDialog = false;
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function disablePersistence() {
    if (
      !confirm(
        "Disable persistence? Existing drafts will be kept on disk but you won't be able to save new ones from inside the app. You can re-enable any time."
      )
    )
      return;
    try {
      await api.persistenceDisable();
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function deleteDraft(id: string) {
    if (!confirm("Delete this draft? This cannot be undone.")) return;
    try {
      await api.draftDelete(id);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function deleteAll() {
    if (
      !confirm(
        `Delete ALL ${drafts.length} saved drafts? This permanently removes the JSON files from your disk and cannot be undone.`
      )
    )
      return;
    try {
      const count = await api.draftDeleteAll();
      await refresh();
      alert(`Deleted ${count} draft(s).`);
    } catch (e) {
      error = String(e);
    }
  }

  async function loadDraft(id: string) {
    try {
      selectedDraft = await api.draftLoad(id);
    } catch (e) {
      error = String(e);
    }
  }

  async function openDataDir() {
    try {
      await open(dataDir);
    } catch (e) {
      error = `Could not open folder: ${e}`;
    }
  }

  function fmtTime(ts: number): string {
    return new Date(ts * 1000).toLocaleString();
  }

  function fmtSize(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(2)} MB`;
  }

  const kindLabels: Record<string, string> = {
    style_draft: "Style draft",
    style_reference: "Style reference",
    chat: "Chat transcript",
    disclosure: "Disclosure statement",
    cleaner: "Cleaned text",
  };
</script>

<h1>Saved Work</h1>
<p class="lead">
  ScholarScribe can save your drafts, chat history, and disclosure statements locally on your
  device. <strong>Persistence is OFF by default</strong>, you must explicitly enable it below.
  Nothing is ever synced to the cloud.
</p>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="card">
  <div class="card-title">Persistence status</div>
  {#if loading}
    <p class="no-data">Loading…</p>
  {:else if enabled}
    <p style="margin: 0 0 12px;">
      <span class="status-pill ok"><span class="pulse"></span>enabled</span>
      Drafts, chat history, and disclosure statements you save from inside the app will be stored
      as plain JSON files in:
    </p>
    <pre style="margin: 0 0 12px; word-break: break-all; white-space: pre-wrap;">{dataDir}</pre>
    <div class="row">
      <button class="shrink" on:click={openDataDir}>Open folder in Explorer</button>
      <button class="shrink danger" on:click={disablePersistence}>Disable persistence</button>
    </div>
  {:else}
    <p style="margin: 0 0 12px;">
      <span class="status-pill bad"><span class="pulse"></span>disabled</span>
      Nothing you do in ScholarScribe is saved to disk. Closing the app loses all in-session
      work. Enable persistence to save drafts and reload them later.
    </p>
    <button class="primary shrink" on:click={() => (showEnableDialog = true)}>Enable persistence</button>
  {/if}
</div>

{#if showEnableDialog}
  <div class="card" style="border-color: var(--accent);">
    <div class="card-title">Before you enable persistence</div>
    <p style="font-size: 13px;">
      Enabling persistence means ScholarScribe will write plain JSON files to your device's
      app-data folder. Please read and acknowledge:
    </p>
    <ul style="font-size: 13px; line-height: 1.7; padding-left: 20px;">
      <li><strong>What gets saved:</strong> drafts you explicitly save, chat transcripts you explicitly save, disclosure statements you explicitly save. Nothing is autosaved.</li>
      <li><strong>What doesn't get saved:</strong> the Privacy Audit log (always in-memory, cleared on app close), model files (managed by Ollama), app logs.</li>
      <li><strong>Where:</strong> <code>{dataDir}</code>, a folder only your user account can read.</li>
      <li><strong>Cloud sync:</strong> none. The folder is not synced to OneDrive, Dropbox, or any cloud service by ScholarScribe. (If your Windows profile redirects %APPDATA% to OneDrive, that's an OS-level setting outside our control, check your OneDrive settings if you don't want this.)</li>
      <li><strong>Encryption:</strong> files are plain JSON. If you need encryption, enable Windows BitLocker on your system drive or store the app data folder in an encrypted container.</li>
      <li><strong>Deletion:</strong> you can delete any draft or all drafts at any time from this tab. Uninstalling ScholarScribe leaves the data folder in place, you must delete it manually if you want a clean removal.</li>
    </ul>
    <div class="row" style="margin-top: 12px;">
      <button class="primary shrink" on:click={enablePersistence}>I understand, enable persistence</button>
      <button class="shrink" on:click={() => (showEnableDialog = false)}>Cancel</button>
    </div>
  </div>
{/if}

{#if enabled}
  <h2>Saved drafts ({drafts.length})</h2>
  <div class="card">
    {#if drafts.length === 0}
      <p class="no-data">
        No drafts saved yet. Use the "Save" button in the Style Analysis, Chat, Disclosure, or
        AI Text Cleaner tabs to store your work here.
      </p>
    {:else}
      <table>
        <thead>
          <tr><th>Title</th><th>Type</th><th>Updated</th><th>Size</th><th></th></tr>
        </thead>
        <tbody>
          {#each drafts as d}
            <tr>
              <td><strong>{d.title}</strong></td>
              <td><span class="tag">{kindLabels[d.kind] || d.kind}</span></td>
              <td class="muted" style="white-space: nowrap;">{fmtTime(d.updated_at)}</td>
              <td class="muted">{fmtSize(d.size_bytes)}</td>
              <td class="right">
                <button class="shrink" on:click={() => loadDraft(d.id)}>View</button>
                <button class="shrink danger" on:click={() => deleteDraft(d.id)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <div class="row" style="margin-top: 12px;">
        <button class="shrink" on:click={refresh}>Refresh</button>
        <div class="spacer"></div>
        <button class="shrink danger" on:click={deleteAll} disabled={drafts.length === 0}>
          Delete all
        </button>
      </div>
    {/if}
  </div>

  {#if selectedDraft}
    <h2>Viewing: {selectedDraft.title}</h2>
    <div class="card">
      <div class="row" style="margin-bottom: 12px; font-size: 13px;">
        <div><span class="dim">Type:</span> {kindLabels[selectedDraft.kind] || selectedDraft.kind}</div>
        <div><span class="dim">Created:</span> {fmtTime(selectedDraft.created_at)}</div>
        <div><span class="dim">Updated:</span> {fmtTime(selectedDraft.updated_at)}</div>
        <div class="spacer"></div>
        <button class="shrink" on:click={() => (selectedDraft = null)}>Close</button>
      </div>
      <pre style="max-height: 400px; overflow-y: auto; white-space: pre-wrap; word-wrap: break-word;">{selectedDraft.content}</pre>
    </div>
  {/if}
{/if}

<div class="callout info">
  <strong>Privacy commitment.</strong>
  ScholarScribe's "no telemetry, nothing leaves your device" promise applies to saved data
  too. Saved drafts are read by the app only when you click "View", they are never sent
  anywhere. If you ever doubt this, watch the Privacy Audit tab while interacting with saved
  drafts: you'll see file_read events pointing at your local data folder, and zero HTTP calls.
</div>
