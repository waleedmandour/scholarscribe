<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type CleanOptions, type CleanResult, defaultCleanOptions } from "../lib/api";

  let inputText = "";
  let inputPath = "";
  let inputKind: "text" | "docx" = "text";
  let result: CleanResult | null = null;
  let busy = false;
  let error = "";
  let copied = false;
  let docxSourcePath = "";

  // Result from the preserve-format .docx cleaning (separate from `result`)
  let preserveResult: {
    output_path: string;
    parts_cleaned: string[];
    runs_cleaned: number;
    stats: any;
    transformations_applied: string[];
    skipped_operations: string[];
  } | null = null;

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
      filters: [
        { name: "Text + Word documents", extensions: ["txt", "md", "markdown", "tex", "rst", "csv", "json", "docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    const lower = selected.toLowerCase();
    if (lower.endsWith(".docx")) {
      inputPath = selected;
      inputKind = "docx";
      docxSourcePath = selected;
      inputText = `[.docx file loaded: ${selected}]\nChoose an action below:\n  • "Extract & clean text" — extract text and clean it (loses formatting)\n  • "Clean & save as .docx" — clean in place, preserving all tables/images/formatting`;
      result = null;
      preserveResult = null;
    } else {
      try {
        const text = await api.readTextFile(selected);
        inputText = text;
        inputPath = selected;
        inputKind = "text";
        docxSourcePath = "";
        result = null;
        preserveResult = null;
      } catch (e) {
        error = String(e);
      }
    }
  }

  async function clean() {
    if (inputKind === "docx" && !docxSourcePath) {
      error = "Pick a .docx file first.";
      return;
    }
    if (inputKind === "text" && !inputText.trim()) {
      error = "Paste some text or open a file first.";
      return;
    }
    error = "";
    busy = true;
    preserveResult = null;
    try {
      if (inputKind === "docx") {
        const out = await api.cleanDocxFile(docxSourcePath, opts);
        result = out.extracted;
      } else {
        result = await api.cleanText(inputText, opts);
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // v0.1.5: clean the .docx in place, preserving all formatting, tables,
  // images, hyperlinks, headers/footers. Saves to a new .docx file the user
  // picks via a save dialog.
  async function cleanAndSaveDocx() {
    if (!docxSourcePath) {
      error = "Pick a .docx file first.";
      return;
    }
    // Suggest a default output name based on the input.
    const inputName = docxSourcePath.split(/[\\/]/).pop() || "document.docx";
    const stem = inputName.replace(/\.docx$/i, "");
    const defaultOutput = `${stem}-cleaned.docx`;

    const outputPath = await save({
      defaultPath: defaultOutput,
      filters: [{ name: "Word document", extensions: ["docx"] }],
    });
    if (!outputPath) return;  // user cancelled

    error = "";
    busy = true;
    result = null;
    try {
      preserveResult = await api.cleanDocxPreserveFormat(docxSourcePath, outputPath, opts);
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
    inputKind = "text";
    inputPath = "";
    docxSourcePath = "";
    result = null;
    preserveResult = null;
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
</script>

<h1>AI Text Cleaner</h1>
<p class="lead">
  Cleans common artifacts from text you paste or import — especially from copy-pasting out of
  PDFs, scanned documents, web pages, and word processors. All cleaning runs locally on your
  device as deterministic rule-based transformations. No text is sent to any server.
  <strong>Two .docx modes</strong>: extract text only (loses formatting), or clean in place
  (preserves all tables, images, hyperlinks, headers, footers, and styles).
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
    <div class="card-subtitle">
      Paste text, or open a file. Supports <code>.txt</code>, <code>.md</code>, <code>.tex</code>,
      <code>.rst</code>, <code>.csv</code>, <code>.json</code>, and <code>.docx</code>.
    </div>
    <div class="row" style="margin-bottom: 8px;">
      <button class="shrink" on:click={pickFile}>Open file…</button>
      {#if inputPath}
        <span class="dim" style="font-size: 11px; word-break: break-all;">
          {inputPath}
          {#if inputKind === "docx"}<span class="tag" style="margin-left: 6px;">.docx</span>{/if}
        </span>
      {/if}
    </div>
    <textarea bind:value={inputText} rows="14" placeholder="Paste your text here, or use Open file… to load a .txt/.md/.docx file"></textarea>
    <div class="dim" style="font-size: 11px; margin-top: 4px;">
      {inputText.length.toLocaleString()} characters
      {#if inputKind === "docx"}· .docx loaded — pick an action below{/if}
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

<h2>Action</h2>
<div class="row" style="margin: 8px 0 16px; gap: 12px;">
  <button
    class="primary"
    on:click={clean}
    disabled={busy || (inputKind === "text" ? !inputText.trim() : !docxSourcePath)}
    title={inputKind === "docx" ? "Extract text from the .docx and clean it. Tables, images, and formatting are lost — output is plain text." : "Clean the pasted/loaded text."}
  >
    {busy ? "Working…" : inputKind === "docx" ? "Extract & clean text" : "Clean text"}
  </button>

  {#if inputKind === "docx"}
    <button
      class="primary"
      on:click={cleanAndSaveDocx}
      disabled={busy || !docxSourcePath}
      title="Clean the .docx in place — modifies each text run, preserves all tables/images/hyperlinks/headers/footers/styles. Saves to a new .docx file."
    >
      {busy ? "Working…" : "Clean & save as .docx (preserves format)"}
    </button>
  {/if}
</div>

{#if result}
  <h2>Result — extracted text</h2>
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

{#if preserveResult}
  <h2>Result — .docx cleaned in place</h2>
  <div class="callout info">
    <strong>Saved:</strong> <code>{preserveResult.output_path}</code><br />
    <strong>Text runs cleaned:</strong> {preserveResult.runs_cleaned.toLocaleString()}<br />
    <strong>Document parts modified:</strong> {preserveResult.parts_cleaned.length} ({preserveResult.parts_cleaned.join(", ")})
  </div>

  <div class="card">
    <div class="card-title">What was changed</div>
    {#if preserveResult.transformations_applied.length > 0}
      <ul style="margin: 6px 0 0 16px; padding: 0;">
        {#each preserveResult.transformations_applied as t}
          <li style="font-size: 13px;">{t}</li>
        {/each}
      </ul>
    {:else}
      <div class="no-data">No per-run transformations were needed.</div>
    {/if}
  </div>

  <div class="card">
    <div class="card-title">Skipped operations (don't apply to in-place .docx cleaning)</div>
    <ul style="margin: 6px 0 0 16px; padding: 0;">
      {#each preserveResult.skipped_operations as s}
        <li style="font-size: 13px;" class="muted">{s}</li>
      {/each}
    </ul>
    <p class="muted" style="font-size: 12px; margin-top: 10px;">
      These operations require cross-paragraph context (e.g. joining sentences that span
      paragraph breaks). Applying them would require restructuring the document, which would
      defeat the purpose of preserving your formatting. To apply them, use
      "Extract &amp; clean text" instead — output is plain text but all transformations run.
    </p>
  </div>

  <div class="card">
    <div class="card-title">What's preserved in the output .docx</div>
    <ul style="margin: 6px 0 0 16px; padding: 0; line-height: 1.7;">
      <li>All tables (structure, cells, formatting)</li>
      <li>All images and embedded figures</li>
      <li>All hyperlinks (link targets and anchor text)</li>
      <li>Headers and footers (also cleaned for consistency)</li>
      <li>Footnotes and endnotes (also cleaned)</li>
      <li>All character and paragraph styles</li>
      <li>Track changes, comments, and review markup</li>
      <li>Document theme, fonts, colors</li>
      <li>Page setup, margins, orientation</li>
    </ul>
  </div>
{/if}
