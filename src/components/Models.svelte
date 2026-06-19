<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    api,
    type ModelInfo,
    type RecommendedModel,
    type PullProgress,
    type GgufCompatResult,
    type SystemInfo,
  } from "../lib/api";

  export let ollamaOk = false;
  const dispatch = createEventDispatcher();

  let installed: ModelInfo[] = [];
  let recommended: RecommendedModel[] = [];
  let loading = false;
  let error = "";
  let pullInProgress: Record<string, PullProgress | null> = {};
  let pullError: Record<string, string> = {};
  let listeners: (() => void)[] = [];

  // GGUF import state
  let sysInfo: SystemInfo | null = null;
  let ggufCompat: GgufCompatResult | null = null;
  let ggufPath = "";
  let ggufModelName = "";
  let ggufImporting = false;
  let ggufError = "";

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
    try {
      sysInfo = await api.systemInfo();
    } catch (e) {
      console.error("system_info failed", e);
    }
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
        pullError = {
          ...pullError,
          [info.model]: "Pull failed. Check that Ollama is running and the model name is correct.",
        };
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
    pullInProgress = {
      ...pullInProgress,
      [name]: { model: name, status: "starting", completed: 0, total: 0 },
    };
    try {
      await api.ollamaPullModel(name);
    } catch (e) {
      pullError = { ...pullError, [name]: String(e) };
      pullInProgress = { ...pullInProgress, [name]: null };
    }
  }

  async function remove(name: string) {
    if (
      !confirm(
        `Delete model "${name}"? This frees disk space but you'll need to re-download to use it again.`
      )
    )
      return;
    try {
      await api.ollamaDeleteModel(name);
      await refresh();
      dispatch("changed");
    } catch (e) {
      error = String(e);
    }
  }

  async function installCustom() {
    const name = prompt(
      "Enter the Ollama model name to install (e.g. mistral:7b, gemma2:9b, qwen2.5:14b):"
    );
    if (!name) return;
    await pull(name.trim());
  }

  // v0.1.1+: pick a .gguf file already on the device, check compatibility, import.
  async function pickGguf() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "GGUF model files", extensions: ["gguf"] }],
    });
    if (!selected || typeof selected !== "string") return;
    ggufPath = selected;
    ggufCompat = null;
    ggufError = "";
    // Suggest a model name from the filename.
    const base = ggufPath.split(/[\\/]/).pop() || "imported-model";
    const stem = base.replace(/\.gguf$/i, "").replace(/[^a-zA-Z0-9_.-]/g, "-").toLowerCase();
    ggufModelName = `local-${stem}`.slice(0, 60);
    try {
      ggufCompat = await api.checkGgufCompatibility(ggufPath);
    } catch (e) {
      ggufError = String(e);
    }
  }

  async function importGguf() {
    if (!ggufPath || !ggufModelName.trim()) {
      ggufError = "Pick a .gguf file and provide a model name first.";
      return;
    }
    if (ggufCompat && ggufCompat.verdict === "insufficient") {
      if (
        !confirm(
          `This model is larger than your total RAM. Inference will likely fail or be extremely slow. Import anyway?`
        )
      )
        return;
    }
    ggufImporting = true;
    ggufError = "";
    try {
      await api.ollamaImportGguf(ggufPath, ggufModelName.trim());
      await refresh();
      dispatch("changed");
      // Reset form.
      ggufPath = "";
      ggufModelName = "";
      ggufCompat = null;
    } catch (e) {
      ggufError = String(e);
    } finally {
      ggufImporting = false;
    }
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
  Install open LLMs on your device. All models run locally via
  <a href="https://ollama.com" target="_blank" rel="noopener">Ollama</a>; no text ever leaves your
  computer. Pick a model whose <strong>min RAM</strong> fits your machine — running an undersized model
  is the most common cause of slow responses.
</p>

{#if sysInfo}
  <div class="card" style="padding: 12px 16px;">
    <div class="row" style="font-size: 13px;">
      <div><span class="dim">CPU:</span> {sysInfo.cpu_brand} ({sysInfo.cpu_cores} cores)</div>
      <div><span class="dim">Total RAM:</span> {sysInfo.total_ram_gb.toFixed(1)} GB</div>
      <div><span class="dim">Free RAM:</span> {sysInfo.available_ram_gb.toFixed(1)} GB</div>
      <div><span class="dim">OS:</span> {sysInfo.os_name}</div>
    </div>
  </div>
{/if}

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
    <p class="no-data">No models installed yet. Pick one from the catalog below, import a .gguf file, or install by name.</p>
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

<h2>Import a local .gguf file</h2>
<div class="card">
  <div class="card-title">Already have a model file on this device?</div>
  <div class="card-subtitle">
    If you've downloaded a <code>.gguf</code> file directly (e.g. from HuggingFace), pick it here.
    ScholarScribe checks whether your device has enough RAM to run it, then imports it into Ollama's
    model registry via <code>ollama create</code>.
  </div>

  <div class="row" style="align-items: flex-start;">
    <div style="flex: 1;">
      <button class="shrink" on:click={pickGguf}>Pick .gguf file…</button>
      {#if ggufPath}
        <div class="muted" style="font-size: 12px; margin-top: 6px; word-break: break-all;">
          {ggufPath}
        </div>
      {/if}
    </div>
    <div style="flex: 1;">
      <label class="dim" for="gguf-name" style="font-size: 11px; display: block; margin-bottom: 4px;">Model name (what Ollama will call it)</label>
      <input id="gguf-name" type="text" bind:value={ggufModelName} placeholder="e.g. local-llama-7b" />
    </div>
    <button class="primary shrink" on:click={importGguf} disabled={ggufImporting || !ggufPath || !ggufModelName.trim() || !ollamaOk}>
      {ggufImporting ? "Importing…" : "Import"}
    </button>
  </div>

  {#if ggufCompat}
    <div class="callout {ggufCompat.verdict === 'ok' ? 'info' : 'warn'}" style="margin-top: 12px;">
      <strong>Compatibility check</strong><br />
      File size: <strong>{ggufCompat.file_size_gb.toFixed(2)} GB</strong> ·
      Recommended RAM for inference: <strong>{ggufCompat.recommended_ram_gb.toFixed(1)} GB</strong> ·
      Your total RAM: <strong>{ggufCompat.total_ram_gb.toFixed(1)} GB</strong> ·
      Currently free: <strong>{ggufCompat.available_ram_gb.toFixed(1)} GB</strong>
      <div style="margin-top: 6px;">{ggufCompat.message}</div>
    </div>
  {/if}

  {#if ggufError}
    <div class="callout warn" style="margin-top: 12px;"><strong>Error:</strong> {ggufError}</div>
  {/if}
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
  Importing a local <code>.gguf</code> file makes <em>zero</em> outbound network calls — Ollama registers the file from disk.
</div>

