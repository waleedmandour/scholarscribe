<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type CleanOptions, type CleanResult, defaultCleanOptions } from "../lib/api";

  let inputText = "";
  let inputPath = "";
  let result: CleanResult | null = null;
  let busy = false;
  let error = "";
  let copied = false;

  // Per-option toggles, initialized to defaults
  let opts: CleanOptions = { ...defaultCleanOptions };

  const optionLabels: { key: keyof CleanOptions; label: string; hint: string }[] = [
    { key: "fix_mojibake", label: "Fix mojibake", hint: "Repair text decoded with the wrong charset (e.g. â€” → —)" },
    { key: "expand_ligatures", label: "Expand ligatures", hint: "ﬁ → fi, ﬂ → fl, ﬀ → ff, etc." },
    { key: "normalize_quotes", label: "Normalize quotes", hint: "Curly → straight quotes (off by default; preserves academic style)" },
    { key: "normalize_dashes", label: "Normalize dashes", hint: "-- → —, en-dash → hyphen" },
    { key: "strip_zero_width", label: "Strip zero-width chars", hint: "Remove U+200B/200C/200D/FEFF/2060 (often invisible but cause issues)" },
    { key: "strip_control_chars", label: "Strip control chars", hint: "Remove non-printable C0/C1 chars (except tab/newline)" },
    { key: "join_hyphenated_words", label: "Join hyphenated line breaks", hint: "exam-\\nple → example (common PDF artifact)" },
    { key: "join_broken_lines", label: "Join broken sentences", hint: "Lines ending mid-sentence → joined (preserves paragraph breaks)" },
    { key: "join_broken_urls", label: "Join broken URLs", hint: "https://example.\\ncom → https://example.com" },
    { key: "fix_broken_citations", label: "Fix broken citations", hint: "(Smith,\\n2020) → (Smith, 2020)" },
    { key: "remove_page_numbers", label: "Remove page numbers", hint: "Strip lines that are just numbers (PDF extraction artifact)" },
    { key: "collapse_whitespace", label: "Collapse whitespace", hint: "Multiple spaces → one, trim trailing, collapse 3+ newlines to 2" },
  ];

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Text files", extensions: ["txt", "md", "markdown", "tex", "rst", "csv", "json"] }],
    });
    if (!selected || typeof selected !== "string") return;
    try {
      const text = await api.readTextFile(selected);
      inputText = text;
      inputPath = selected;
    } catch (e) {
      error = String(e);
    }
  }

  async function clean() {
    if (!inputText.trim()) {
      error = "Paste some text or open a file first.";
      return;
    }
    error = "";
    busy = true;
    try {
      result = await api.cleanText(inputText, opts);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function copyCleaned() {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.cleaned);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      error = "Clipboard not available. Select the text manually and copy.";
    }
  }

  function useCleanedAsInput() {
    if (!result) return;
    inputText = result.cleaned;
    result = null;
  }

  function resetToDefaults() {
    opts = { ...defaultCleanOptions };
  }

  function enableAll() {
    const allOn = {} as CleanOptions;
    (Object.keys(opts) as (keyof CleanOptions)[]).forEach((k) => {
      allOn[k] = true;
    });
    opts = allOn;
  }

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(2)} MB`;
  }
</script>

<h1>AI Text Cleaner</h1>
<p class="lead">
  Cleans common artifacts from text you paste or import — especially from copy-pasting out of
  PDFs, scanned documents, web pages, and word processors. All cleaning runs locally on your
  device as deterministic rule-based transformations. No text is sent to any server.
</p>

<div class="callout info">
  <strong>What this does — and doesn't — do.</strong>
  The cleaner fixes <em>artifacts</em> (broken hyphens, ligatures, mojibake, page numbers,
  control characters) but does <strong>not</strong> rewrite the content or change its meaning.
  It is not a paraphraser and not an AI-detector evader. For rephrasing, use the Chat tab
  with a local LLM.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="row" style="align-items: flex-start;">
  <div class="card" style="flex: 1;">
    <div class="card-title">Input</div>
    <div class="card-subtitle">Paste the text you want to clean, or open a file.</div>
    <div class="row" style="margin-bottom: 8px;">
      <button class="shrink" on:click={pickFile}>Open file…</button>
      {#if inputPath}<span class="dim" style="font-size: 11px; word-break: break-all;">{inputPath}</span>{/if}
    </div>
    <textarea bind:value={inputText} rows="14" placeholder="Paste your text here…"></textarea>
    <div class="dim" style="font-size: 11px; margin-top: 4px;">
      {inputText.length.toLocaleString()} characters · {fmtBytes(new Blob([inputText]).size)}
    </div>
  </div>

  <div class="card" style="flex: 1;">
    <div class="card-title">Cleaning options</div>
    <div class="card-subtitle">Toggle which transformations to apply. Defaults are sensible for most academic text.</div>
    <div style="display: grid; grid-template-columns: 1fr; gap: 6px; margin-top: 8px;">
      {#each optionLabels as o}
        <label style="display: flex; align-items: flex-start; gap: 8px; font-size: 13px; cursor: pointer;">
          <input type="checkbox" bind:checked={opts[o.key]} style="flex: 0 0 auto; margin-top: 3px;" />
          <span>
            <strong>{o.label}</strong><br />
            <span class="dim" style="font-size: 11px;">{o.hint}</span>
          </span>
        </label>
      {/each}
    </div>
    <div class="row" style="margin-top: 12px;">
      <button class="shrink" on:click={resetToDefaults}>Reset to defaults</button>
      <button class="shrink" on:click={enableAll}>Enable all</button>
    </div>
  </div>
</div>

<div class="row" style="margin: 12px 0;">
  <button class="primary" on:click={clean} disabled={busy || !inputText.trim()}>
    {busy ? "Cleaning…" : "Clean text"}
  </button>
</div>

{#if result}
  <h2>Result</h2>
  <div class="card">
    <div class="row" style="margin-bottom: 12px;">
      <div>
        <div class="dim" style="font-size: 11px;">ORIGINAL</div>
        <div style="font-size: 16px; font-weight: 600;">{result.original_length.toLocaleString()} chars</div>
      </div>
      <div>→</div>
      <div>
        <div class="dim" style="font-size: 11px;">CLEANED</div>
        <div style="font-size: 16px; font-weight: 600;">{result.cleaned_length.toLocaleString()} chars</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">SAVED</div>
        <div style="font-size: 16px; font-weight: 600; color: var(--success);">
          {result.original_length > result.cleaned_length
            ? `${(result.original_length - result.cleaned_length).toLocaleString()}`
            : `+${(result.cleaned_length - result.original_length).toLocaleString()}`}
        </div>
      </div>
      <div class="spacer"></div>
      <button class="shrink" on:click={copyCleaned}>{copied ? "Copied!" : "Copy cleaned"}</button>
      <button class="shrink" on:click={useCleanedAsInput}>Use as input</button>
    </div>

    {#if result.transformations_applied.length > 0}
      <div style="margin-bottom: 12px;">
        <strong>Transformations applied:</strong>
        <ul style="margin: 6px 0 0 16px; padding: 0;">
          {#each result.transformations_applied as t}
            <li style="font-size: 13px;">{t}</li>
          {/each}
        </ul>
      </div>
    {:else}
      <div class="no-data">No transformations were needed — the input was already clean.</div>
    {/if}

    <details>
      <summary style="cursor: pointer; font-size: 13px; color: var(--text-muted);">Detailed stats</summary>
      <table style="margin-top: 8px;">
        <thead><tr><th>Operation</th><th>Count</th></tr></thead>
        <tbody>
          <tr><td>Whitespace collapsed</td><td>{result.stats.whitespace_collapsed}</td></tr>
          <tr><td>Line breaks joined</td><td>{result.stats.line_breaks_joined}</td></tr>
          <tr><td>Hyphenated words joined</td><td>{result.stats.hyphenated_words_joined}</td></tr>
          <tr><td>Ligatures expanded</td><td>{result.stats.ligatures_expanded}</td></tr>
          <tr><td>Zero-width chars stripped</td><td>{result.stats.zero_width_chars_stripped}</td></tr>
          <tr><td>Control chars stripped</td><td>{result.stats.control_chars_stripped}</td></tr>
          <tr><td>Page numbers removed</td><td>{result.stats.page_numbers_removed}</td></tr>
          <tr><td>Quotes normalized</td><td>{result.stats.quotes_normalized}</td></tr>
          <tr><td>Dashes normalized</td><td>{result.stats.dashes_normalized}</td></tr>
          <tr><td>Mojibake fixed</td><td>{result.stats.mojibake_fixed}</td></tr>
          <tr><td>URLs joined</td><td>{result.stats.urls_joined}</td></tr>
          <tr><td>Citations fixed</td><td>{result.stats.citations_fixed}</td></tr>
        </tbody>
      </table>
    </details>
  </div>

  <div class="card">
    <div class="card-title">Cleaned output</div>
    <pre style="max-height: 400px; overflow-y: auto; white-space: pre-wrap; word-wrap: break-word;">{result.cleaned}</pre>
  </div>
{/if}
