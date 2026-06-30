<script lang="ts">
  import { onMount } from "svelte";
  import { showTour, completeTour, closeTour } from "../lib/onboarding";

  type TabId =
    | "models" | "cleaner" | "citations" | "stats" | "structure"
    | "abstract" | "risk" | "consistency" | "journal" | "appeal"
    | "fingerprint" | "coach" | "style" | "chat" | "disclosure"
    | "literacy" | "audit" | "saved" | "about";

  // Steps array — each entry renders one tour panel.
  const steps = [
    {
      title: "Welcome to ScholarScribe",
      icon: "✦",
    },
    {
      title: "Your data never leaves this device",
      icon: "🔒",
    },
    {
      title: "Start here: install a model",
      icon: "⬇",
    },
    {
      title: "19 tools at your service",
      icon: "▦",
    },
    {
      title: "Ethical use — please read",
      icon: "⚖",
    },
  ] as const;

  let step = 0;
  let dontShowAgain = false;
  let modalEl: HTMLDivElement | null = null;
  let previouslyFocused: HTMLElement | null = null;

  // Feature grid for step 4 — categorized, each cell clickable to jump.
  const featureGroups: { category: string; items: { id: TabId; name: string; desc: string }[] }[] = [
    {
      category: "Writing",
      items: [
        { id: "cleaner", name: "Text Cleaner", desc: "Fix 24 PDF/OCR/web artifacts" },
        { id: "coach",   name: "Writing Coach", desc: "Local-LLM paragraph coaching" },
        { id: "chat",    name: "Chat",          desc: "Local-only chat with guardrails" },
        { id: "abstract",name: "Abstract",      desc: "Generate a structured abstract" },
      ],
    },
    {
      category: "Validation",
      items: [
        { id: "citations", name: "Citations",  desc: "Validate citations vs .bib" },
        { id: "stats",     name: "Stats",      desc: "Word count + readability" },
        { id: "structure", name: "Structure",  desc: "Heading tree + missing sections" },
      ],
    },
    {
      category: "Authenticity",
      items: [
        { id: "risk",        name: "Risk Profile",   desc: "AI-text surface features?" },
        { id: "consistency", name: "Voice Check",    desc: "Within-document shifts" },
        { id: "style",       name: "Style Analysis", desc: "Compare to your prior writing" },
        { id: "fingerprint", name: "Fingerprint",    desc: "Multi-paper stylistic profile" },
        { id: "journal",     name: "Journal",        desc: "Timestamped draft snapshots" },
        { id: "appeal",      name: "Appeal Letter",  desc: "Evidence-based appeal draft" },
      ],
    },
    {
      category: "Transparency",
      items: [
        { id: "disclosure", name: "Disclosure",        desc: "Venue-compliant AI-use statements" },
        { id: "literacy",   name: "Detector Literacy", desc: "How detectors work + fail" },
        { id: "audit",      name: "Privacy Audit",     desc: "Live log of file/HTTP events" },
        { id: "saved",      name: "Saved Work",        desc: "Opt-in local JSON storage" },
        { id: "about",      name: "About",             desc: "Version, environment, credits" },
        { id: "models",     name: "Models",            desc: "Install / manage local LLMs" },
      ],
    },
  ];

  $: if (!$showTour) step = 0;

  function next() {
    if (step < steps.length - 1) step += 1;
    else finish();
  }
  function back() {
    if (step > 0) step -= 1;
  }
  function skip() {
    if (dontShowAgain) completeTour();
    else closeTour();
  }
  function finish() {
    completeTour();
  }
  function jumpToTab(tab: TabId) {
    // Dispatch an event the parent App.svelte listens for.
    dispatchTabSwitch(tab);
    completeTour();
  }

  import { createEventDispatcher } from "svelte";
  const dispatch = createEventDispatcher<{ jump: TabId }>();
  function dispatchTabSwitch(tab: TabId) {
    dispatch("jump", tab);
  }

  // Keyboard navigation: Esc = skip, ←/→ = nav
  function onKeydown(e: KeyboardEvent) {
    if (!$showTour) return;
    if (e.key === "Escape") {
      e.preventDefault();
      skip();
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      next();
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      back();
    }
  }

  onMount(() => {
    previouslyFocused = document.activeElement as HTMLElement | null;
    window.addEventListener("keydown", onKeydown);
  });

  // Focus the modal when it opens; restore focus when it closes.
  $: if ($showTour) {
    setTimeout(() => modalEl?.focus(), 50);
  } else if (previouslyFocused) {
    previouslyFocused?.focus?.();
    previouslyFocused = null;
  }

  $: isLast = step === steps.length - 1;
</script>

{#if $showTour}
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
  <div
    class="tour-backdrop"
    on:click={skip}
    role="presentation"
  >
    <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
    <div
      class="tour-modal"
      on:click|stopPropagation
      on:keydown|stopPropagation={(e) => e.stopPropagation()}
      tabindex="-1"
      bind:this={modalEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="tour-title"
    >
      <!-- Stepper -->
      <div class="tour-stepper" aria-label="Tour progress">
        {#each steps as s, i}
          <button
            class="tour-dot"
            class:active={i === step}
            class:done={i < step}
            on:click={() => (step = i)}
            aria-label={`Step ${i + 1}: ${s.title}`}
            title={`Step ${i + 1}: ${s.title}`}
          >
            {#if i < step}<span>✓</span>{:else}<span>{i + 1}</span>{/if}
          </button>
          {#if i < steps.length - 1}
            <div class="tour-line" class:filled={i < step}></div>
          {/if}
        {/each}
      </div>

      <!-- Step body -->
      <div class="tour-body">
        {#if step === 0}
          <div class="tour-icon-big">✦</div>
          <h2 id="tour-title" class="tour-title">{steps[0].title}</h2>
          <p class="tour-tagline">
            A privacy-first, local-LLM writing companion for researchers.
          </p>
          <p class="tour-text">
            ScholarScribe runs entirely on your device — no telemetry, no cloud calls,
            no paid APIs. This 5-step tour will walk you through the essentials in
            about a minute. You can re-open it any time from the sidebar or the
            About tab.
          </p>
          <div class="tour-meta">
            <span class="tag">v0.2.0</span>
            <span class="tag">MIT License</span>
            <span class="tag green">Local-only</span>
          </div>
        {:else if step === 1}
          <div class="tour-icon-big">🔒</div>
          <h2 id="tour-title" class="tour-title">{steps[1].title}</h2>
          <p class="tour-text">
            <strong>Nothing you write, paste, or open ever leaves your device.</strong>
            Drafts, <code>.bib</code> files, chat messages, and reference samples stay
            in memory or in local JSON files.
          </p>
          <ul class="tour-list">
            <li><strong>No telemetry</strong> — no analytics, no crash reports.</li>
            <li><strong>No third-party APIs</strong> — no OpenAI, Anthropic, or Google calls.</li>
            <li><strong>One outbound host</strong>: <code>registry.ollama.ai</code>, only when you click "Download" on a model — and that carries no text or usage data.</li>
          </ul>
          <div class="callout info" style="margin-top: 14px; font-size: 13.5px;">
            <strong>Verify it yourself.</strong> The <strong>Privacy Audit</strong> tab
            shows a live log of every file read and outbound HTTP call. Anything other
            than <code>registry.ollama.ai</code> is a red flag.
          </div>
        {:else if step === 2}
          <div class="tour-icon-big">⬇</div>
          <h2 id="tour-title" class="tour-title">{steps[2].title}</h2>
          <p class="tour-text">
            Open the <strong>Models</strong> tab when you finish this tour. The top card
            shows your CPU and RAM. Pick a model that fits your memory:
          </p>
          <table class="tour-ram-table">
            <thead><tr><th>Your RAM</th><th>Recommended models</th></tr></thead>
            <tbody>
              <tr><td><strong>8 GB</strong></td><td>Gemma 3 4B · Qwen 3 4B · Phi-4 Mini</td></tr>
              <tr><td><strong>16 GB</strong></td><td>Gemma 3 12B · Qwen 3 8B · Phi-4 14B</td></tr>
              <tr><td><strong>32 GB</strong></td><td>Gemma 3 27B · Qwen 3 32B · DeepSeek R1 32B</td></tr>
              <tr><td><strong>64 GB+</strong></td><td>Llama 3.3 70B</td></tr>
            </tbody>
          </table>
          <p class="tour-text" style="margin-top: 12px;">
            Already have a <code>.gguf</code> file? Use <strong>Pick .gguf file…</strong> —
            ScholarScribe checks your RAM and imports it via Ollama with zero outbound
            network.
          </p>
        {:else if step === 3}
          <div class="tour-icon-big">▦</div>
          <h2 id="tour-title" class="tour-title">{steps[3].title}</h2>
          <p class="tour-text" style="margin-bottom: 12px;">
            The sidebar organizes 19 tools into 4 categories. <strong>Click any cell
            below to jump straight to that tab.</strong>
          </p>
          <div class="tour-grid">
            {#each featureGroups as group}
              <div class="tour-group">
                <div class="tour-group-title">{group.category}</div>
                <div class="tour-group-items">
                  {#each group.items as item}
                    <button
                      class="tour-cell"
                      on:click={() => jumpToTab(item.id)}
                      title={item.desc}
                    >
                      <span class="tour-cell-name">{item.name}</span>
                      <span class="tour-cell-desc">{item.desc}</span>
                    </button>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {:else if step === 4}
          <div class="tour-icon-big">⚖</div>
          <h2 id="tour-title" class="tour-title">{steps[4].title}</h2>
          <p class="tour-text">
            ScholarScribe is designed for researchers who have
            <strong>genuinely written</strong> their manuscript and want transparent,
            local AI assistance.
          </p>
          <div class="tour-ethics">
            <div class="tour-ethics-col tour-ethics-does">
              <div class="tour-ethics-h">ScholarScribe DOES</div>
              <ul>
                <li>Help you draft, paraphrase, and refine <strong>your own</strong> writing.</li>
                <li>Validate citations and generate disclosure statements.</li>
                <li>Educate you about how AI detectors work and where they fail.</li>
                <li>Maintain timestamped evidence of your writing process.</li>
              </ul>
            </div>
            <div class="tour-ethics-col tour-ethics-doesnot">
              <div class="tour-ethics-h">ScholarScribe DOES NOT</div>
              <ul>
                <li>Evade AI detectors or lower detection scores.</li>
                <li>Misrepresent AI-generated text as original human work.</li>
                <li>Contact any third-party API (OpenAI, Anthropic, Google, …).</li>
                <li>Collect telemetry, analytics, or crash reports.</li>
              </ul>
            </div>
          </div>
          <p class="tour-text" style="margin-top: 12px;">
            If you used AI assistance, <strong>disclose it</strong> — the Disclosure
            tab makes this easy.
          </p>
        {/if}
      </div>

      <!-- Footer -->
      <div class="tour-footer">
        <label class="tour-checkbox">
          <input type="checkbox" bind:checked={dontShowAgain} />
          <span>Don't show this again at start</span>
        </label>
        <div class="tour-footer-actions">
          <span class="tour-step-counter">Step {step + 1} of {steps.length}</span>
          <button on:click={skip} class="tour-skip">Skip tour</button>
          {#if step > 0}
            <button on:click={back}>Back</button>
          {/if}
          {#if isLast}
            <button class="primary" on:click={finish}>Start using ScholarScribe</button>
          {:else}
            <button class="primary" on:click={next}>Next →</button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .tour-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 18, 24, 0.55);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: tour-fade-in 200ms ease-out;
    cursor: default;
  }

  .tour-modal {
    width: 640px;
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.25), 0 4px 12px rgba(0, 0, 0, 0.1);
    display: flex;
    flex-direction: column;
    outline: none;
    animation: tour-pop-in 200ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  /* Stepper */
  .tour-stepper {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 18px 24px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-elev-2);
    border-radius: 12px 12px 0 0;
  }
  .tour-dot {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 1.5px solid var(--border);
    background: var(--bg-elev);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .tour-dot:hover {
    border-color: var(--text-dim);
  }
  .tour-dot.active {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
    transform: scale(1.08);
  }
  .tour-dot.done {
    background: var(--success);
    border-color: var(--success);
    color: white;
  }
  .tour-line {
    flex: 1;
    height: 2px;
    background: var(--border);
    border-radius: 1px;
    transition: background 0.2s ease;
  }
  .tour-line.filled {
    background: var(--success);
  }

  /* Body */
  .tour-body {
    padding: 24px 28px 16px;
    flex: 1;
    overflow-y: auto;
  }
  .tour-icon-big {
    font-size: 38px;
    line-height: 1;
    margin-bottom: 8px;
    color: var(--accent);
    font-weight: 700;
  }
  .tour-title {
    font-size: 22px;
    font-weight: 600;
    margin: 0 0 8px;
    color: var(--text);
    line-height: 1.25;
  }
  .tour-tagline {
    font-size: 15.5px;
    color: var(--accent);
    font-style: italic;
    margin: 0 0 14px;
  }
  .tour-text {
    font-size: 14px;
    line-height: 1.65;
    color: var(--text);
    margin: 0 0 12px;
  }
  .tour-text code, .tour-list code, .callout code {
    background: var(--code-bg);
    padding: 1px 5px;
    border-radius: 3px;
    font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
    font-size: 12.5px;
  }
  .tour-list {
    margin: 8px 0 0;
    padding-left: 18px;
    font-size: 13.5px;
    line-height: 1.7;
    color: var(--text);
  }
  .tour-list li { margin-bottom: 4px; }
  .tour-meta {
    margin-top: 16px;
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  /* RAM table */
  .tour-ram-table {
    margin: 8px 0 0;
    font-size: 13px;
  }
  .tour-ram-table th {
    font-size: 11.5px;
  }
  .tour-ram-table td {
    padding: 7px 10px;
  }

  /* Feature grid */
  .tour-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-top: 4px;
  }
  .tour-group {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px;
  }
  .tour-group-title {
    font-size: 11.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin-bottom: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border);
  }
  .tour-group-items {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .tour-cell {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    text-align: left;
    padding: 7px 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: var(--bg-elev);
    cursor: pointer;
    transition: all 0.12s ease;
    font-family: inherit;
    font-size: 13px;
    width: 100%;
  }
  .tour-cell:hover {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .tour-cell-name {
    font-weight: 600;
    color: var(--text);
  }
  .tour-cell-desc {
    font-size: 11.5px;
    color: var(--text-muted);
    margin-top: 1px;
  }

  /* Ethics table */
  .tour-ethics {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 12px;
  }
  .tour-ethics-col {
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .tour-ethics-h {
    padding: 8px 12px;
    font-size: 12.5px;
    font-weight: 600;
    color: white;
  }
  .tour-ethics-does .tour-ethics-h { background: var(--success); }
  .tour-ethics-doesnot .tour-ethics-h { background: var(--danger); }
  .tour-ethics-col ul {
    margin: 0;
    padding: 10px 14px 12px 22px;
    font-size: 12.5px;
    line-height: 1.55;
    background: var(--bg-elev);
  }
  .tour-ethics-does ul { background: rgba(26, 138, 82, 0.06); }
  .tour-ethics-doesnot ul { background: rgba(192, 57, 43, 0.06); }
  .tour-ethics-col li { margin-bottom: 5px; }

  /* Footer */
  .tour-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 24px;
    border-top: 1px solid var(--border);
    background: var(--bg-elev-2);
    border-radius: 0 0 12px 12px;
    flex-wrap: wrap;
  }
  .tour-checkbox {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    color: var(--text-muted);
    cursor: pointer;
    user-select: none;
  }
  .tour-checkbox input {
    width: auto;
    cursor: pointer;
  }
  .tour-footer-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .tour-step-counter {
    font-size: 12px;
    color: var(--text-dim);
    margin-right: 4px;
  }
  .tour-skip {
    color: var(--text-muted);
  }

  @keyframes tour-fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes tour-pop-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  /* Tighter layout on small viewports */
  @media (max-width: 700px) {
    .tour-modal {
      width: calc(100vw - 24px);
      max-height: calc(100vh - 24px);
    }
    .tour-grid, .tour-ethics {
      grid-template-columns: 1fr;
    }
    .tour-body {
      padding: 18px 18px 12px;
    }
    .tour-footer {
      padding: 12px 18px;
    }
  }
</style>
