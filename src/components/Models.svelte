<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type ModelInfo, type RecommendedModel, type PullProgress } from "../lib/api";

  export let ollamaOk = false;
  const dispatch = createEventDispatcher();

  let installed: ModelInfo[] = [];
  let recommended: RecommendedModel[] = [];
  let loading = false;
  let error = "";
  let pullInProgress: Record<string, PullProgress | null> = {};
  let pullError: Record<string, string> = {};
  let listeners: (() => void)[] = [];

  async function refresh() {
    if (!ollamaOk) return;
    loading = true;
    error = "";
    try {
      installed = await api.ollamaListModels();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(async () => {
    recommended = await api.recommendedModels();
    const u1 = await api.onPullProgress((p) => {
      pullInProgress = { ...pullInProgress, [p.model]: p };
    });
    const u2 = await api.onPullEnd((info) => {
      pullInProgress = { ...pullInProgress, [info.model]: null };
      if (info.ok) {
        pullError = { ...pullError, [info.model]: "" };
        refresh();
        dispatch("changed");
      } else {
        pullError = { ...pullError, [info.model]: "Pull failed. Check that Ollama is running and the model name is correct." };
      }
    });
    listeners.push(u1, u2);
    refresh();
  });

  onDestroy(() => {
    listeners.forEach((u) => u && u());
  });

  async function pull(name: string) {
    pullError = { ...pullError, [name]: "" };
    pullInProgress = { ...pullInProgress, [name]: { model: name, status: "starting", completed: 0, total: 0 } };
    try {
      await api.ollamaPullModel(name);
    } catch (e) {
      pullError = { ...pullError, [name]: String(e) };
      pullInProgress = { ...pullInProgress, [name]: null };
    }
  }

  async function remove(name: string) {
    if (!confirm(`Delete model "${name}"? This frees disk space but you'll need to re-download to use it again.`)) return;
    try {
      await api.ollamaDeleteModel(name);
      await refresh();
      dispatch("changed");
    } catch (e) {
      error = String(e);
    }
  }

  async function installCustom() {
    const name = prompt("Enter the Ollama model name to install (e.g. mistral:7b, gemma2:9b, qwen2.5:14b):");
    if (!name) return;
    await pull(name.trim());
  }

  function fmtSize(bytes: number): string {
    if (!bytes) return "—";
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(0)} MB`;
  }

  function pct(p: PullProgress | null | undefined): number {
    if (!p || !p.total) return 0;
    return Math.min(100, Math.round((p.completed / p.total) * 100));
  }
</script>

<h1>Models</h1>
<p class="lead">
  Install open LLMs on your device. All models run locally via <a href="https://ollama.com" target="_blank" rel="noopener">Ollama</a>;
  no text ever leaves your computer. Pick a model whose <strong>min RAM</strong> fits your machine — running an undersized model
  is the most common cause of slow responses.
</p>

{#if !ollamaOk}
  <div class="callout warn">
    <strong>Ollama is not running.</strong> ScholarScribe needs Ollama as its local LLM runtime.
    <ol style="margin: 8px 0 0 16px; padding: 0;">
      <li>Download Ollama from <a href="https://ollama.com/download" target="_blank" rel="noopener">ollama.com/download</a> (free, ~150 MB).</li>
      <li>Run the installer. On Windows it auto-starts as a background service.</li>
      <li>Confirm the Ollama icon (a llama) appears in your system tray.</li>
      <li>Come back here — the status pill in the sidebar will turn green.</li>
    </ol>
  </div>
{/if}

{#if error}
  <div class="callout warn"><strong>Error:</strong> {error}</div>
{/if}

<h2>Installed on this device</h2>
<div class="card">
  {#if loading}
    <p class="no-data">Loading…</p>
  {:else if installed.length === 0}
    <p class="no-data">No models installed yet. Pick one from the catalog below, or install a custom model by name.</p>
  {:else}
    <table>
      <thead><tr><th>Name</th><th>Size</th><th>Last used</th><th></th></tr></thead>
      <tbody>
        {#each installed as m}
          <tr>
            <td><strong>{m.name}</strong></td>
            <td>{fmtSize(m.size)}</td>
            <td class="muted">{m.modified_at}</td>
            <td class="right"><button class="danger" on:click={() => remove(m.name)}>Delete</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  <div class="row" style="margin-top: 12px;">
    <button class="shrink" on:click={refresh} disabled={!ollamaOk}>Refresh</button>
    <button class="shrink" on:click={installCustom} disabled={!ollamaOk}>Install by name…</button>
  </div>
</div>

<h2>Recommended catalog</h2>
{#each recommended as r}
  <div class="card">
    <div class="row" style="align-items: flex-start;">
      <div style="flex: 1;">
        <div class="card-title">{r.label}</div>
        <div class="card-subtitle"><code>{r.name}</code> · {r.size_gb.toFixed(1)} GB · min RAM {r.min_ram_gb} GB</div>
        <div class="muted" style="font-size: 13px; margin-bottom: 8px;">{r.description}</div>
        {#each r.tags as t}<span class="tag">{t}</span>{/each}
      </div>
      <div class="shrink" style="text-align: right;">
        {#if pullInProgress[r.name]}
          {@const p = pullInProgress[r.name]}
          <div style="margin-bottom: 8px; font-size: 12px;" class="muted">
            {p ? p.status : ""} · {p ? pct(p) : 0}%
          </div>
          <div class="progress"><div style="width: {p ? pct(p) : 0}%"></div></div>
        {:else if installed.find((m) => m.name === r.name)}
          <span class="tag green">installed</span>
        {:else}
          <button class="primary" on:click={() => pull(r.name)} disabled={!ollamaOk}>Download</button>
        {/if}
        {#if pullError[r.name]}<div class="muted" style="margin-top: 6px; font-size: 12px; color: var(--danger);">{pullError[r.name]}</div>{/if}
      </div>
    </div>
  </div>
{/each}

<div class="callout info">
  <strong>Privacy:</strong> Downloading a model only contacts <code>registry.ollama.ai</code> once, to fetch the model weights.
  After download, all inference happens entirely on your device. No prompts, no drafts, and no other text are ever sent anywhere.
</div>
