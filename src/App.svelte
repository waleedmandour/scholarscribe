<script lang="ts">
  import { onMount } from "svelte";
  import Models from "./components/Models.svelte";
  import StyleAnalysis from "./components/StyleAnalysis.svelte";
  import Disclosure from "./components/Disclosure.svelte";
  import DetectorLiteracy from "./components/DetectorLiteracy.svelte";
  import Chat from "./components/Chat.svelte";
  import { api } from "./lib/api";

  type Tab = "models" | "style" | "chat" | "disclosure" | "literacy";
  let active: Tab = "models";
  let ollamaOk = false;
  let checking = true;

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "models", label: "Models", icon: "M" },
    { id: "style", label: "Style Analysis", icon: "S" },
    { id: "chat", label: "Chat", icon: "C" },
    { id: "disclosure", label: "Disclosure", icon: "D" },
    { id: "literacy", label: "Detector Literacy", icon: "L" },
  ];

  async function refreshStatus() {
    checking = true;
    try {
      ollamaOk = await api.ollamaStatus();
    } catch {
      ollamaOk = false;
    } finally {
      checking = false;
    }
  }

  onMount(() => {
    refreshStatus();
    const id = setInterval(refreshStatus, 5000);
    return () => clearInterval(id);
  });
</script>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <span class="dot"></span>
      ScholarScribe
    </div>

    {#each tabs as t}
      <div
        class="nav-item"
        class:active={active === t.id}
        on:click={() => (active = t.id)}
        role="button"
        tabindex="0"
        on:keydown={(e) => e.key === "Enter" && (active = t.id)}
      >
        <span class="dim" style="width: 18px; text-align: center; font-weight: 600;">{t.icon}</span>
        {t.label}
      </div>
    {/each}

    <div class="spacer"></div>

    <div style="padding: 8px 10px; border-top: 1px solid var(--border); font-size: 11px;">
      <div class="dim">Ollama backend</div>
      <div style="margin-top: 4px;">
        {#if checking}
          <span class="status-pill bad"><span class="pulse"></span>checking…</span>
        {:else if ollamaOk}
          <span class="status-pill ok"><span class="pulse"></span>running</span>
        {:else}
          <span class="status-pill bad"><span class="pulse"></span>not running</span>
        {/if}
      </div>
      <div class="dim" style="margin-top: 10px;">
        v0.1.0 · MIT · local-only
      </div>
    </div>
  </aside>

  <main class="main">
    {#if active === "models"}
      <Models {ollamaOk} on:changed={refreshStatus} />
    {:else if active === "style"}
      <StyleAnalysis />
    {:else if active === "chat"}
      <Chat {ollamaOk} />
    {:else if active === "disclosure"}
      <Disclosure />
    {:else if active === "literacy"}
      <DetectorLiteracy />
    {/if}
  </main>
</div>
