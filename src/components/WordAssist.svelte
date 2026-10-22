<script lang="ts">
  // Word Assist. As-you-type grammar and clarity assist for Microsoft
  // Word, served by a loopback listener inside ScholarScribe (feature
  // word-assist). Opt-in by design: nothing binds a port, generates a
  // certificate, or reads Word text until the disclosure below is
  // accepted. Text sent to Word's task pane stays on this machine; the
  // only external fetch the add-in itself makes is the office.js library
  // from Microsoft's CDN (it never carries document text).
  import { onMount } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import PageHeader from "./PageHeader.svelte";
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api";
  import type {
    WaStatus,
    WaEnableOutcome,
    WaAuditEntry,
    WaRegistration,
    WaDisclosurePrefill,
  } from "../lib/api";

  let status: WaStatus | null = null;
  let busy = false;
  let error = "";
  let notice = "";
  let pairingCode = "";
  let audit: WaAuditEntry[] = [];
  let registration: WaRegistration | null = null;
  let prefill: WaDisclosurePrefill | null = null;
  let copiedField = "";

  // Settings form (mirrors persistence.rs word_assist_* fields).
  let strictness = "dev";
  let tier1Enabled = false;
  let tier1Model = "";
  let port = 38080;
  let experimental: Record<string, boolean> = {
    arabic: false,
    cjk: false,
    cyrillic: false,
    greek: false,
    other: false,
  };
  // The disclosure dialog. Must be accepted before enable writes anything.
  let disclosureOpen = false;
  let disclosureRead = false;

  const STRICTNESS_OPTIONS: { value: string; label: string; hint: string }[] = [
    {
      value: "dev",
      label: "Default (provisional)",
      hint: "Grammar only. Spelling and clarity are off while the engine is being calibrated.",
    },
    {
      value: "conservative",
      label: "Conservative",
      hint: "Grammar only.",
    },
    {
      value: "balanced",
      label: "Balanced",
      hint: "Grammar + spelling. Clarity stays off.",
    },
    {
      value: "thorough",
      label: "Thorough",
      hint: "Everything the engine offers, including clarity and style notes.",
    },
  ];

  const EXPERIMENTAL_LABELS: Record<string, string> = {
    arabic: "Arabic",
    cjk: "Chinese / Japanese / Korean",
    cyrillic: "Cyrillic (Russian, Ukrainian, ...)",
    greek: "Greek",
    other: "Other scripts",
  };

  async function refresh() {
    try {
      status = await api.wordAssistStatus();
      if (status.feature_compiled) {
        // Reflect the persisted settings in the form when idle.
        if (!status.running) {
          const s = await api.settingsGet();
          strictness = s.word_assist_strictness || "dev";
          tier1Enabled = !!s.word_assist_tier1_enabled;
          tier1Model = s.word_assist_tier1_model || "";
          port = s.word_assist_port || 38080;
        }
        audit = await api.wordAssistAudit();
      }
    } catch (e) {
      error = String(e);
    }
  }

  function openDisclosure() {
    error = "";
    notice = "";
    disclosureOpen = true;
    disclosureRead = false;
  }

  async function acceptDisclosure() {
    disclosureOpen = false;
    await persistSettings();
    await enable();
  }

  async function persistSettings() {
    const s = await api.settingsGet();
    s.word_assist_strictness = strictness;
    // H8: Tier 1 stays OFF until the user explicitly picks a model.
    // An empty model name is not "last used chat model" any more — the
    // engine coerces the same way (AssistSettings::from_persisted).
    const modelPicked = tier1Model.trim() !== "";
    s.word_assist_tier1_enabled = tier1Enabled && modelPicked;
    if (tier1Enabled && !modelPicked) {
      notice =
        "Rewrite suggestions stay off until you enter a model name in the field below.";
    }
    s.word_assist_tier1_model = tier1Model.trim();
    s.word_assist_port = port;
    s.word_assist_experimental_languages = { ...experimental };
    await api.settingsSet(s);
  }

  async function enable() {
    busy = true;
    error = "";
    notice = "";
    try {
      const outcome: WaEnableOutcome = await api.wordAssistEnable();
      if (outcome.ok) {
        notice = `Word Assist is listening on 127.0.0.1:${outcome.port}. Pair Word with the code below.`;
        await newPairingCode();
      } else {
        error = outcome.error || "Word Assist could not start.";
      }
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function disable() {
    busy = true;
    error = "";
    try {
      await api.wordAssistDisable();
      pairingCode = "";
      registration = null;
      notice = "Word Assist stopped. The pairing session was wiped.";
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function newPairingCode() {
    error = "";
    try {
      pairingCode = await api.wordAssistNewPairingCode();
    } catch (e) {
      error = String(e);
    }
  }

  async function unpair() {
    busy = true;
    try {
      await api.wordAssistUnpair();
      notice = "Pairing cleared. Word must pair again.";
      pairingCode = "";
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function loadRegistration() {
    error = "";
    try {
      registration = await api.wordAssistRegistration();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadPrefill() {
    error = "";
    try {
      prefill = await api.wordAssistDisclosurePrefill();
    } catch (e) {
      error = String(e);
    }
  }

  async function copy(text: string, field: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedField = field;
      setTimeout(() => (copiedField = ""), 1500);
    } catch {
      error = "Clipboard unavailable in this context.";
    }
  }

  async function exportManifest() {
    error = "";
    notice = "";
    try {
      const path = await save({
        title: "Export Word Assist manifest",
        defaultPath: "ScholarScribe-Word-Assist-manifest.xml",
        filters: [{ name: "Office add-in manifest", extensions: ["xml"] }],
      });
      if (!path) return; // user cancelled
      notice = await api.wordAssistExportManifest(path);
    } catch (e) {
      error = String(e);
    }
  }

  async function applySettings() {
    busy = true;
    error = "";
    notice = "";
    try {
      await persistSettings();
      if (status?.running) {
        // A settings change on a running listener restarts it cleanly:
        // stop, re-read, start (the same flow as enable/disable).
        await api.wordAssistDisable();
        await api.wordAssistEnable();
        notice = "Settings applied; the listener restarted.";
      } else {
        notice = "Settings saved. Enable Word Assist to start the listener.";
      }
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function strictnessLabel(name: string): string {
    return name === "dev" ? "Default (provisional)" : name;
  }
  function hintFor(value: string): string {
    const opt = STRICTNESS_OPTIONS.find((o) => o.value === value);
    return opt ? opt.hint : "";
  }

  onMount(refresh);
</script>

<PageHeader icon="plug" title="Word Assist">
  As-you-type grammar and clarity feedback inside Microsoft Word. Runs on
  this machine only; nothing is sent to any server.
</PageHeader>

{#if status && !status.feature_compiled}
  <div class="card notice-card">
    <p>
      This build of ScholarScribe was compiled without the Word Assist
      feature. Nothing is missing from your writing tools — Language Check
      covers the same checks offline, right here in the app.
    </p>
  </div>
{:else}
  {#if error}<div class="error" role="alert">{error}</div>{/if}
  {#if notice}<div class="notice" role="status">{notice}</div>{/if}

  <section class="card">
    <h2>Status</h2>
    {#if status?.running}
      <div class="status-row ok">
        <span class="dot" aria-hidden="true"></span>
        <div>
          <strong>Running</strong> on <code>https://localhost:{status.port}</code>
          · engine <code>{status.engine}</code>
          · strictness <strong>{strictnessLabel(status.strictness)}</strong>
          {#if status.paired}· <em>Word is paired</em>{/if}
        </div>
      </div>
      <div class="row-gap">
        <button class="btn secondary" on:click={disable} disabled={busy}>
          Stop Word Assist
        </button>
        <button class="btn secondary" on:click={unpair} disabled={busy}>
          Clear pairing
        </button>
        <button class="btn secondary" on:click={loadRegistration}>
          Registration &amp; certificate
        </button>
        <button class="btn secondary" on:click={loadPrefill}>
          Disclosure pre-fill
        </button>
      </div>
      {#if pairingCode}
        <div class="pairing">
          <div class="pairing-label">Pairing code (5 minutes, one-time use)</div>
          <div class="pairing-code">{pairingCode}</div>
          <div class="pairing-hint">
            In Word: open the ScholarScribe Word Assist pane and enter this code.
          </div>
          <button class="btn secondary" on:click={newPairingCode} disabled={busy}>
            New code
          </button>
        </div>
      {/if}
    {:else}
      <p>
        Word Assist connects ScholarScribe to Microsoft Word through a
        small local service. It is off, and nothing has been installed or
        changed on this computer.
      </p>
      <button class="btn" on:click={openDisclosure} disabled={busy}>
        Enable Word Assist…
      </button>
    {/if}
  </section>

  <section class="card">
    <h2>Settings</h2>
    <div class="form-grid">
      <label for="wa-strictness">Strictness</label>
      <div>
        <select id="wa-strictness" bind:value={strictness}>
          {#each STRICTNESS_OPTIONS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        <p class="hint">{hintFor(strictness)}</p>
      </div>

      <label for="wa-tier1">Rewrite suggestions (local model) · Experimental</label>
      <div>
        <label class="check">
          <input id="wa-tier1" type="checkbox" bind:checked={tier1Enabled} />
          Suggest whole-sentence rewrites from your local Ollama model
        </label>
        {#if tier1Enabled}
          <input
            class="text"
            type="text"
            bind:value={tier1Model}
            placeholder="Model name, e.g. gemma3:4b (required)"
          />
          <p class="hint">
            <strong>Experimental.</strong> No precision evaluation exists
            for this feature yet, so suggestions may be wrong. Tier 1
            stays off until you pick a model by name. Each suggestion
            sends one sentence to your local Ollama daemon. Every rewrite
            is checked against six validation rules before Word ever sees
            it; rewrites that touch citations, numbers, URLs, math or
            quotes are discarded.
          </p>
        {/if}
      </div>

      <label for="wa-port">Listener port</label>
      <div>
        <input class="text narrow" id="wa-port" type="number" bind:value={port} min="1024" max="65535" />
        <p class="hint">
          Default 38080. Changing the port invalidates the Word pairing
          (Word will ask you to pair again).
        </p>
      </div>

      <label>Experimental languages</label>
      <div>
        {#each Object.keys(experimental) as key}
          <label class="check">
            <input type="checkbox" bind:checked={experimental[key]} />
            {EXPERIMENTAL_LABELS[key]}
          </label>
        {/each}
        <p class="hint">
          The grammar engine is English-only. For other scripts it currently
          reports "language not supported yet" and does nothing. Enabling a
          script here lets checks run anyway, marked
          <strong>experimental and unvalidated</strong>.
        </p>
      </div>
    </div>
    <button class="btn" on:click={applySettings} disabled={busy}>Save settings</button>
  </section>

  {#if registration}
    <section class="card">
      <h2>Registration &amp; certificate</h2>
      <p class="hint">
        Word needs two one-time consents: trust the <strong>CA
        certificate</strong> ScholarScribe generated (an ephemeral,
        name-constrained root scoped to <code>localhost</code> — its
        signing key was destroyed after issuing this install's leaf),
        and side-load the manifest. Nothing here touches the system
        certificate store by itself — you perform the install, and you
        can undo it at any time.
      </p>
      <ol class="steps">
        {#each registration.instructions as step}
          <li>{step}</li>
        {/each}
      </ol>
      <div class="row-gap">
        <button class="btn secondary" on:click={() => copy(registration!.cert_pem, "cert")}>
          {copiedField === "cert" ? "Copied" : "Copy CA certificate (PEM)"}
        </button>
        <button class="btn secondary" on:click={() => copy(registration!.manifest_url, "url")}>
          {copiedField === "url" ? "Copied" : "Copy manifest URL"}
        </button>
        <button class="btn secondary" on:click={exportManifest}>
          Export manifest file…
        </button>
      </div>
      <details>
        <summary>CA certificate text</summary>
        <pre class="cert">{registration.cert_pem}</pre>
      </details>
    </section>
  {/if}

  {#if prefill}
    <section class="card">
      <h2>Disclosure pre-fill</h2>
      <p class="hint">
        Counts from the assist ledger (hashes and numbers only — your text
        is never recorded). Paste the sentence into the Disclosure tab if
        you want to acknowledge the assistance.
      </p>
      <p class="prefill-note">“{prefill.note}”</p>
      <div class="row-gap">
        <button class="btn secondary" on:click={() => copy(prefill!.note, "note")}>
          {copiedField === "note" ? "Copied" : "Copy sentence"}
        </button>
      </div>
    </section>
  {/if}

  {#if audit.length}
    <section class="card">
      <h2>Word Assist activity (this session)</h2>
      <ul class="audit">
        {#each audit.slice(-12).reverse() as entry}
          <li>
            <span class="audit-kind">{entry.kind}</span>
            <span class="audit-detail">{entry.detail}</span>
          </li>
        {/each}
      </ul>
      <p class="hint">
        Counts and identifiers only — your text is never written to this log.
      </p>
    </section>
  {/if}
{/if}

{#if disclosureOpen}
  <div class="overlay" role="dialog" aria-modal="true" aria-labelledby="wa-disclosure-title">
    <div class="dialog">
      <h3 id="wa-disclosure-title">Before you enable Word Assist</h3>
      <p>
        Enabling starts a <strong>local listener</strong> on
        <code>https://localhost:{port}</code> (127.0.0.1 only — never the
        network) and creates a certificate for that address so Word can
        connect securely. It does not modify your system certificate store;
        the registration step below stays in your hands.
      </p>
      <p>
        The Word pane is loaded from this machine. It fetches the
        <strong>office.js library from Microsoft's CDN</strong>
        (appsforoffice.microsoft.com) — that fetch carries no document
        text. Everything else, including your sentences, stays between Word
        and ScholarScribe on this computer. If you turn on rewrite
        suggestions, one sentence at a time goes to your local Ollama
        daemon, never further.
      </p>
      <p>
        Pairing uses a one-time code shown here. You can stop everything
        with one click at any time, and stopping wipes the pairing.
      </p>
      <label class="check">
        <input type="checkbox" bind:checked={disclosureRead} />
        I understand what will run on this machine
      </label>
      <div class="row-gap">
        <button class="btn" on:click={acceptDisclosure} disabled={!disclosureRead || busy}>
          Enable Word Assist
        </button>
        <button class="btn secondary" on:click={() => (disclosureOpen = false)}>
          Cancel
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .card { background: var(--bg-card, #fff); border: 1px solid var(--border, #e2e2e2); border-radius: 10px; padding: 16px; margin-bottom: 14px; }
  .card h2 { font-size: 0.95rem; margin: 0 0 10px; color: var(--text-secondary, #555); text-transform: uppercase; letter-spacing: 0.04em; }
  .status-row { display: flex; gap: 10px; align-items: baseline; }
  .status-row .dot { width: 9px; height: 9px; border-radius: 50%; background: #2e9e4f; flex: none; }
  .row-gap { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
  .btn { font: inherit; padding: 7px 14px; border-radius: 8px; border: 1px solid var(--accent, #23407a); background: var(--accent, #23407a); color: #fff; cursor: pointer; }
  .btn.secondary { background: transparent; color: var(--accent, #23407a); }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .pairing { margin-top: 14px; padding: 12px; border: 1px dashed var(--border, #ccc); border-radius: 8px; }
  .pairing-label { font-size: 0.85rem; color: var(--text-secondary, #555); }
  .pairing-code { font-size: 1.8rem; letter-spacing: 0.35em; font-weight: 600; margin: 6px 0; font-variant-numeric: tabular-nums; }
  .pairing-hint { font-size: 0.85rem; color: var(--text-secondary, #666); margin-bottom: 8px; }
  .form-grid { display: grid; grid-template-columns: minmax(140px, 220px) 1fr; gap: 12px 16px; align-items: start; margin-bottom: 12px; }
  .form-grid > label { font-weight: 600; font-size: 0.9rem; padding-top: 4px; }
  .hint { font-size: 0.82rem; color: var(--text-secondary, #666); margin: 6px 0 0; }
  .check { display: flex; gap: 8px; align-items: center; font-weight: normal; font-size: 0.9rem; margin-bottom: 6px; }
  input.text { font: inherit; padding: 6px 9px; border: 1px solid var(--border, #bbb); border-radius: 6px; width: min(340px, 100%); }
  input.text.narrow { width: 120px; }
  select { font: inherit; padding: 6px 9px; border: 1px solid var(--border, #bbb); border-radius: 6px; }
  .steps { padding-left: 18px; font-size: 0.88rem; }
  .steps li { margin-bottom: 6px; }
  .cert { font-family: var(--font-mono, monospace); font-size: 0.72rem; white-space: pre-wrap; word-break: break-all; background: var(--bg-subtle, #f5f5f5); padding: 10px; border-radius: 8px; max-height: 180px; overflow: auto; }
  .prefill-note { background: var(--bg-subtle, #f5f5f5); border-left: 3px solid var(--accent, #23407a); padding: 10px 12px; border-radius: 0 8px 8px 0; font-size: 0.9rem; line-height: 1.55; }
  .audit { list-style: none; padding: 0; margin: 0; font-size: 0.82rem; }
  .audit li { display: flex; gap: 10px; padding: 4px 0; border-top: 1px solid var(--border, #eee); }
  .audit-kind { flex: none; font-family: var(--font-mono, monospace); color: var(--accent, #23407a); }
  .error { background: #fbe9e7; color: #8a2c1d; border-radius: 8px; padding: 10px 12px; margin-bottom: 12px; font-size: 0.9rem; }
  .notice { background: #e6f4e8; color: #1d5a26; border-radius: 8px; padding: 10px 12px; margin-bottom: 12px; font-size: 0.9rem; }
  .notice-card p { margin: 0; }
  .overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); display: flex; align-items: center; justify-content: center; z-index: 60; }
  .dialog { background: var(--bg-card, #fff); border-radius: 12px; padding: 20px; max-width: 560px; width: min(92vw, 560px); box-shadow: 0 12px 40px rgba(0, 0, 0, 0.25); }
  .dialog h3 { margin-top: 0; }
  .dialog p { font-size: 0.9rem; line-height: 1.55; }
  @media (max-width: 720px) {
    .form-grid { grid-template-columns: 1fr; }
  }
</style>
