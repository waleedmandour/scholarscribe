<script lang="ts">
  import { onMount } from "svelte";
  import Models from "./components/Models.svelte";
  import AITextCleaner from "./components/AITextCleaner.svelte";
  import Proofread from "./components/Proofread.svelte";
  import CitationManager from "./components/CitationManager.svelte";
  import DocumentStats from "./components/DocumentStats.svelte";
  import StructureAnalyzer from "./components/StructureAnalyzer.svelte";
  import AbstractGenerator from "./components/AbstractGenerator.svelte";
  import RiskProfiler from "./components/RiskProfiler.svelte";
  import VoiceConsistency from "./components/VoiceConsistency.svelte";
  import WritingJournal from "./components/WritingJournal.svelte";
  import AppealLetter from "./components/AppealLetter.svelte";
  import StyleFingerprint from "./components/StyleFingerprint.svelte";
  import WritingCoach from "./components/WritingCoach.svelte";
  import StyleAnalysis from "./components/StyleAnalysis.svelte";
  import Disclosure from "./components/Disclosure.svelte";
  import Provenance from "./components/Provenance.svelte";
  import DetectorLiteracy from "./components/DetectorLiteracy.svelte";
  import Chat from "./components/Chat.svelte";
  import PrivacyAudit from "./components/PrivacyAudit.svelte";
  import SavedWork from "./components/SavedWork.svelte";
  import About from "./components/About.svelte";
  import WelcomeTour from "./components/WelcomeTour.svelte";
  import { api } from "./lib/api";
  import { openTour } from "./lib/onboarding";

  type Tab = "models" | "cleaner" | "proofread" | "citations" | "stats" | "structure" | "abstract" | "risk" | "consistency" | "journal" | "appeal" | "fingerprint" | "coach" | "style" | "provenance" | "chat" | "disclosure" | "literacy" | "audit" | "saved" | "about";
  let active: Tab = "models";
  let ollamaOk = false;
  let checking = true;

  // Theme: "light" | "dark" | "auto". Persisted to localStorage.
  let theme: "light" | "dark" | "auto" = "auto";

  // Sidebar density: "comfortable" | "compact". Persisted to localStorage.
  // Compact reduces sidebar padding only; never changes font size.
  let density: "comfortable" | "compact" = "comfortable";

  // Font scale: 0.9 | 1.0 | 1.15. Persisted to localStorage.
  // Set on document.documentElement via --font-scale CSS var.
  let fontScale: number = 1.0;

  function applyDensity(d: "comfortable" | "compact") {
    density = d;
    document.documentElement.setAttribute("data-density", d);
    try {
      localStorage.setItem("scholarscribe-density", d);
    } catch {
      // localStorage may be unavailable in some embedded contexts; non-fatal.
    }
  }

  function applyFontScale(s: number) {
    fontScale = s;
    document.documentElement.style.setProperty("--font-scale", String(s));
    try {
      localStorage.setItem("scholarscribe-font-scale", String(s));
    } catch {
      // localStorage may be unavailable in some embedded contexts; non-fatal.
    }
  }

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

  // Tabs grouped by workflow phase. Group is a presentation-only field;
  // tab IDs, routes, and types are unchanged.
  const tabs: { id: Tab; label: string; icon: string; group: string }[] = [
    // Get started
    { id: "models", label: "Models", icon: "M", group: "Get started" },
    // Prepare the draft
    { id: "cleaner", label: "Text Cleaner", icon: "T", group: "Prepare the draft" },
    { id: "proofread", label: "Proofread", icon: "P", group: "Prepare the draft" },
    { id: "citations", label: "Citations", icon: "C", group: "Prepare the draft" },
    // Understand the draft
    { id: "stats", label: "Stats", icon: "#", group: "Understand the draft" },
    { id: "structure", label: "Structure", icon: "S", group: "Understand the draft" },
    // AI writing help
    { id: "abstract", label: "Abstract", icon: "A", group: "AI writing help" },
    { id: "coach", label: "Writing Coach", icon: "?", group: "AI writing help" },
    { id: "chat", label: "Chat", icon: "C", group: "AI writing help" },
    // Authenticity and style (Detector Literacy appears before Risk Profile;
    // the mechanism is explained before a score is shown)
    { id: "literacy", label: "Detector Literacy", icon: "L", group: "Authenticity and style" },
    { id: "risk", label: "Risk Profile", icon: "R", group: "Authenticity and style" },
    { id: "style", label: "Style Analysis", icon: "S", group: "Authenticity and style" },
    { id: "fingerprint", label: "Fingerprint", icon: "F", group: "Authenticity and style" },
    { id: "consistency", label: "Voice Check", icon: "V", group: "Authenticity and style" },
    // Evidence and compliance
    { id: "journal", label: "Journal", icon: "J", group: "Evidence and compliance" },
    { id: "provenance", label: "Provenance", icon: "≡", group: "Evidence and compliance" },
    { id: "appeal", label: "Appeal Letter", icon: "L", group: "Evidence and compliance" },
    { id: "disclosure", label: "Disclosure", icon: "D", group: "Evidence and compliance" },
    // Privacy and app
    { id: "audit", label: "Privacy Audit", icon: "P", group: "Privacy and app" },
    { id: "saved", label: "Saved Work", icon: "W", group: "Privacy and app" },
    { id: "about", label: "About", icon: "A", group: "Privacy and app" },
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
    // Restore saved density on startup.
    try {
      const savedDensity = localStorage.getItem("scholarscribe-density") as "comfortable" | "compact" | null;
      if (savedDensity) {
        applyDensity(savedDensity);
      }
    } catch {
      // ignore
    }
    // Restore saved font scale on startup. The pre-paint script in
    // index.html also sets this before first paint to avoid FOUC; this
    // call keeps the Svelte state in sync.
    try {
      const savedScale = parseFloat(localStorage.getItem("scholarscribe-font-scale") || "1");
      if (savedScale === 0.9 || savedScale === 1.0 || savedScale === 1.15) {
        fontScale = savedScale;
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

    {#each tabs as t, i}
      {#if i === 0 || tabs[i - 1].group !== t.group}
        <div class="nav-group-label">{t.group}</div>
      {/if}
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

    <div style="padding: 8px 10px; border-top: 1px solid var(--border); font-size: var(--font-sm);">
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
        v2.2.1 · MIT · local-only
      </div>
      <button
        class="walk-through-btn"
        on:click={openTour}
        title="Open the interactive welcome tour"
      >
        ✦ Walk me through the app
      </button>
      <div class="density-toggle" role="group" aria-label="Sidebar density">
        <button
          class="density-btn"
          class:active={density === "comfortable"}
          aria-pressed={density === "comfortable"}
          on:click={() => applyDensity("comfortable")}
          title="Comfortable sidebar spacing"
        >Comfortable</button>
        <button
          class="density-btn"
          class:active={density === "compact"}
          aria-pressed={density === "compact"}
          on:click={() => applyDensity("compact")}
          title="Compact sidebar spacing (does not change font size)"
        >Compact</button>
      </div>
      <div class="font-scale-toggle" role="group" aria-label="Font size">
        <button
          class="font-scale-btn"
          class:active={fontScale === 0.9}
          aria-pressed={fontScale === 0.9}
          on:click={() => applyFontScale(0.9)}
          title="Smaller font size (90%)"
        >A-</button>
        <button
          class="font-scale-btn"
          class:active={fontScale === 1.0}
          aria-pressed={fontScale === 1.0}
          on:click={() => applyFontScale(1.0)}
          title="Default font size (100%)"
        >A</button>
        <button
          class="font-scale-btn"
          class:active={fontScale === 1.15}
          aria-pressed={fontScale === 1.15}
          on:click={() => applyFontScale(1.15)}
          title="Larger font size (115%)"
        >A+</button>
      </div>
    </div>
  </aside>

  <main class="main">
    {#if active === "models"}
      <Models {ollamaOk} on:changed={refreshStatus} />
    {:else if active === "cleaner"}
      <AITextCleaner />
    {:else if active === "proofread"}
      <Proofread />
    {:else if active === "citations"}
      <CitationManager />
    {:else if active === "stats"}
      <DocumentStats />
    {:else if active === "structure"}
      <StructureAnalyzer />
    {:else if active === "abstract"}
      <AbstractGenerator {ollamaOk} />
    {:else if active === "risk"}
      <RiskProfiler />
    {:else if active === "consistency"}
      <VoiceConsistency />
    {:else if active === "journal"}
      <WritingJournal {ollamaOk} />
    {:else if active === "appeal"}
      <AppealLetter />
    {:else if active === "fingerprint"}
      <StyleFingerprint />
    {:else if active === "coach"}
      <WritingCoach {ollamaOk} />
    {:else if active === "style"}
      <StyleAnalysis />
    {:else if active === "provenance"}
      <Provenance />
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

<WelcomeTour on:jump={(e) => (active = e.detail)} />

<style>
  .theme-toggle {
    background: transparent;
    border: 1px solid var(--border);
    padding: 4px 8px;
    font-size: var(--font-md);
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
  .walk-through-btn {
    margin-top: 10px;
    width: 100%;
    font-size: var(--font-sm);
    padding: 6px 10px;
    color: var(--accent);
    background: var(--accent-soft);
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-family: inherit;
    text-align: center;
    transition: background 0.12s ease;
  }
  .walk-through-btn:hover {
    background: var(--accent);
    color: white;
  }
</style>
