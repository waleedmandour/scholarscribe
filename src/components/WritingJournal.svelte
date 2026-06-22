<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type JournalSession, type Snapshot } from "../lib/api";

  export let ollamaOk = false;

  let sessions: JournalSession[] = [];
  let activeSessionId = "";
  let currentText = "";
  let snapshots: Snapshot[] = [];
  let selectedSnapshot: Snapshot | null = null;
  let autoSaveEnabled = false;
  let autoSaveInterval: ReturnType<typeof setInterval> | null = null;
  let error = "";
  let busy = false;

  async function refresh() {
    try { sessions = await api.journalListSessions(); }
    catch (e) { error = String(e); }
  }

  async function createSession() {
    try {
      const snap = await api.journalCreateSession("New session");
      activeSessionId = snap.session_id;
      currentText = "";
      await refresh();
      await loadSnapshots();
    } catch (e) { error = String(e); }
  }

  async function saveSnapshot() {
    if (!activeSessionId) { error = "Create a session first."; return; }
    try {
      await api.journalSaveSnapshot(activeSessionId, currentText);
      await loadSnapshots();
      await refresh();
    } catch (e) { error = String(e); }
  }

  async function loadSnapshots() {
    if (!activeSessionId) return;
    try { snapshots = await api.journalGetSnapshots(activeSessionId); }
    catch (e) { error = String(e); }
  }

  async function selectSession(id: string) {
    activeSessionId = id;
    await loadSnapshots();
    if (snapshots.length > 0) {
      currentText = snapshots[snapshots.length - 1].content;
    }
  }

  function toggleAutoSave() {
    autoSaveEnabled = !autoSaveEnabled;
    if (autoSaveEnabled) {
      autoSaveInterval = setInterval(() => {
        if (activeSessionId && currentText.trim()) saveSnapshot();
      }, 60000); // every 60 seconds
    } else if (autoSaveInterval) {
      clearInterval(autoSaveInterval);
      autoSaveInterval = null;
    }
  }

  async function exportSession() {
    if (!activeSessionId) return;
    const path = await save({
      defaultPath: `journal-export-${activeSessionId.slice(0, 8)}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    try { await api.journalExportSession(activeSessionId, path); }
    catch (e) { error = String(e); }
  }

  async function deleteSession(id: string) {
    if (!confirm("Delete this session and all its snapshots?")) return;
    try {
      await api.journalDeleteSession(id);
      if (activeSessionId === id) { activeSessionId = ""; currentText = ""; snapshots = []; }
      await refresh();
    } catch (e) { error = String(e); }
  }

  function fmtTime(ts: number): string { return new Date(ts * 1000).toLocaleString(); }

  import { onMount, onDestroy } from "svelte";
  onMount(refresh);
  onDestroy(() => { if (autoSaveInterval) clearInterval(autoSaveInterval); });
</script>

<h1>Writing Process Journal</h1>
<p class="lead">
  Auto-saves timestamped snapshots of your draft at user-defined intervals, creating a verifiable
  process record that can serve as evidence of authentic authorship. Each snapshot is diffed from
  the previous one. Exports to a timestamped Markdown file. Aligns with the opt-in persistence
  architecture — requires persistence to be enabled in the Saved Work tab.
</p>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="row" style="align-items: flex-start;">
  <div class="card" style="flex: 1;">
    <div class="card-title">Draft editor</div>
    <div class="card-subtitle">
      {#if activeSessionId}Session: <code>{activeSessionId.slice(0, 8)}</code>{:else}No active session{/if}
    </div>
    <textarea bind:value={currentText} rows="12" placeholder="Type or paste your draft here…"></textarea>
    <div class="dim" style="font-size: 11px; margin-top: 4px;">{currentText.split(/\s+/).filter(Boolean).length} words</div>
    <div class="row" style="margin-top: 12px; gap: 8px;">
      <button class="shrink" on:click={createSession}>New session</button>
      <button class="primary shrink" on:click={saveSnapshot} disabled={!activeSessionId}>Save snapshot</button>
      <button class="shrink" on:click={toggleAutoSave} disabled={!activeSessionId}>
        {autoSaveEnabled ? "⏸ Stop auto-save" : "▶ Auto-save (60s)"}
      </button>
      <button class="shrink" on:click={exportSession} disabled={!activeSessionId}>Export…</button>
    </div>
  </div>

  <div class="card" style="flex: 1;">
    <div class="card-title">Sessions ({sessions.length})</div>
    <div class="card-subtitle">Click to load. Each session contains timestamped snapshots.</div>
    {#if sessions.length === 0}
      <p class="no-data">No sessions yet. Click "New session" to start.</p>
    {:else}
      <table>
        <thead><tr><th>Created</th><th>Snapshots</th><th>Words</th><th></th></tr></thead>
        <tbody>
          {#each sessions as s}
            <tr class:active={s.session_id === activeSessionId} on:click={() => selectSession(s.session_id)} style="cursor: pointer;">
              <td class="muted" style="font-size: 12px;">{fmtTime(s.created_at)}</td>
              <td>{s.snapshot_count}</td>
              <td>{s.total_words_final}</td>
              <td><button class="danger shrink" on:click|stopPropagation={() => deleteSession(s.session_id)}>Delete</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if snapshots.length > 0}
  <h2>Snapshots ({snapshots.length})</h2>
  <div class="card">
    <table>
      <thead><tr><th>#</th><th>Timestamp</th><th>Words</th><th>Diff</th><th>Similarity</th><th></th></tr></thead>
      <tbody>
        {#each snapshots as snap, i}
          <tr>
            <td>{i + 1}</td>
            <td class="muted" style="font-size: 12px;">{fmtTime(snap.timestamp)}</td>
            <td>{snap.word_count}</td>
            <td class="muted">
              {#if snap.diff_from_previous}
                +{snap.diff_from_previous.words_added} / -{snap.diff_from_previous.words_removed}
              {:else}—{/if}
            </td>
            <td class="muted">
              {#if snap.diff_from_previous}{snap.diff_from_previous.similarity_pct}%{:else}—{/if}
            </td>
            <td><button class="shrink" on:click={() => selectedSnapshot = snap}>View</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if selectedSnapshot}
    <h2>Snapshot {snapshots.indexOf(selectedSnapshot) + 1} — {fmtTime(selectedSnapshot.timestamp)}</h2>
    <div class="card">
      <pre style="max-height: 400px; overflow-y: auto; white-space: pre-wrap;">{selectedSnapshot.content}</pre>
      <button class="shrink" style="margin-top: 8px;" on:click={() => selectedSnapshot = null}>Close</button>
    </div>
  {/if}
{/if}
