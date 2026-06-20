<script lang="ts">
  import { onMount } from "svelte";
  import Models from "./components/Models.svelte";
  import StyleAnalysis from "./components/StyleAnalysis.svelte";
  import Disclosure from "./components/Disclosure.svelte";
  import DetectorLiteracy from "./components/DetectorLiteracy.svelte";
  import Chat from "./components/Chat.svelte";
  import PrivacyAudit from "./components/PrivacyAudit.svelte";
  import AITextCleaner from "./components/AITextCleaner.svelte";
  import SavedWork from "./components/SavedWork.svelte";
  import About from "./components/About.svelte";
  import { api } from "./lib/api";

  type Tab = "models" | "cleaner" | "style" | "chat" | "disclosure" | "literacy" | "audit" | "saved" | "about";
  let active: Tab = "models";
  let ollamaOk = false;
  let checking = true;

  // Theme: "light" | "dark" | "auto". Persisted to localStorage.
  let theme: "light" | "dark" | "auto" = "auto";

  function applyTheme(t: "light" | "dark" | "auto") {
    if (t === "auto") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", t);
    }
    try {
      localStorage.setItem("scholarscribe-theme", t);
    } catch {
      // localStorage may be unavailable in some embedded contexts; non-fatal.
    }
  }

  function cycleTheme() {
    theme = theme === "light" ? "dark" : theme === "dark" ? "auto" : "light";
    applyTheme(theme);
  }

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "models", label: "Models", icon: "M" },
    { id: "cleaner", label: "Text Cleaner", icon: "T" },
    { id: "style", label: "Style Analysis", icon: "S" },
    { id: "chat", label: "Chat", icon: "C" },
    { id: "disclosure", label: "Disclosure", icon: "D" },
    { id: "literacy", label: "Detector Literacy", icon: "L" },
    { id: "audit", label: "Privacy Audit", icon: "P" },
    { id: "saved", label: "Saved Work", icon: "W" },
    { id: "about", label: "About", icon: "A" },
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
    // Restore saved theme on startup.
    try {
      const saved = localStorage.getItem("scholarscribe-theme") as "light" | "dark" | "auto" | null;
      if (saved) {
        theme = saved;
        applyTheme(saved);
      }
    } catch {
      // ignore
    }
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
      <div class="spacer"></div>
      <button
        class="theme-toggle"
        on:click={cycleTheme}
        title="Theme: {theme}"
        aria-label="Toggle theme"
      >
        {#if theme === "light"}☀️{:else if theme === "dark"}🌙{:else}🌗{/if}
      </button>
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
        v0.1.4 · MIT · local-only
      </div>
    </div>
  </aside>

  <main class="main">
    {#if active === "models"}
      <Models {ollamaOk} on:changed={refreshStatus} />
    {:else if active === "cleaner"}
      <AITextCleaner />
    {:else if active === "style"}
      <StyleAnalysis />
    {:else if active === "chat"}
      <Chat {ollamaOk} />
    {:else if active === "disclosure"}
      <Disclosure />
    {:else if active === "literacy"}
      <DetectorLiteracy />
    {:else if active === "audit"}
      <PrivacyAudit />
    {:else if active === "saved"}
      <SavedWork />
    {:else if active === "about"}
      <About />
    {/if}
  </main>
</div>

<style>
  .theme-toggle {
    background: transparent;
    border: 1px solid var(--border);
    padding: 4px 8px;
    font-size: 14px;
    line-height: 1;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .theme-toggle:hover {
    background: var(--bg-elev-2);
  }
  .sidebar .brand {
    gap: 8px;
    align-items: center;
  }
</style>
