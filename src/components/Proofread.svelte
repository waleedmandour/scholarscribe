<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    api,
    type CheckResult,
    type DocxCheckResult,
    type DocxApplyResult,
    type DialectName,
    type TextFormat,
    type Tier,
    type RuleKind,
    type LedgerSummary,
    type LintFinding,
    type DocxFinding,
    type Tier2Result,
    type DecisionAction,
    type EngineInfo,
  } from "../lib/api";

  type InputMode = "text" | "docx";

  let mode: InputMode = "text";

  // Text mode state
  let inputText = "";
  let dialect: DialectName = "american";
  let format: TextFormat = "plain";
  let personalWordsRaw = ""; // newline-separated, inside <details>
  let result: CheckResult | null = null;
  let engineInfo: EngineInfo | null = null;

  // Docx mode state
  let docxPath = "";
  let docxResult: DocxCheckResult | null = null;
  let applyResult: DocxApplyResult | null = null;

  // Tier 2 state (gated, off by default)
  let tier2Enabled = false;
  let tier2Busy = false;
  let tier2Model = "gemma3:4b";
  let tier2Sentence = "";
  let tier2Result: Tier2Result | null = null;

  // Ledger state
  let ledger: LedgerSummary | null = null;

  let busy = false;
  let error = "";
  let copied = false;
  let copiedLedger = false;

  const dialects: { id: DialectName; label: string }[] = [
    { id: "american", label: "American English" },
    { id: "british", label: "British English" },
    { id: "australian", label: "Australian English" },
    { id: "canadian", label: "Canadian English" },
    { id: "indian", label: "Indian English" },
  ];

  onMount(async () => {
    try {
      engineInfo = await api.grammarEngineInfo();
    } catch (e) {
      // Engine not available yet; non-fatal.
      console.warn("grammar_engine_info failed:", e);
    }
    try {
      tier2Enabled = await api.grammarTier2Status();
    } catch {
      tier2Enabled = false;
    }
  });

  onDestroy(() => {
    // Best-effort session reset; ignore errors (app closing, backend down, etc.).
    api.grammarSessionReset().catch(() => {});
  });

  async function pickDocx() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Word document", extensions: ["docx"] }],
    });
    if (!selected || typeof selected !== "string") return;
    docxPath = selected;
    docxResult = null;
    applyResult = null;
    error = "";
  }

  function personalWords(): string[] {
    return personalWordsRaw
      .split(/\r?\n/)
      .map((w) => w.trim())
      .filter((w) => w.length > 0);
  }

  async function checkText() {
    if (!inputText.trim()) {
      error = "Paste some text first.";
      return;
    }
    error = "";
    busy = true;
    result = null;
    try {
      result = await api.grammarCheck(inputText, format, dialect, [], personalWords());
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function checkDocx() {
    if (!docxPath) {
      error = "Pick a .docx file first.";
      return;
    }
    error = "";
    busy = true;
    docxResult = null;
    applyResult = null;
    try {
      docxResult = await api.grammarCheckDocx(docxPath, dialect, [], personalWords());
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Decision recording helper. Looks up the finding's index in the
  // current result via `result?.lints.indexOf(lint) ?? -1` for null safety.
  async function recordDecision(
    lint: LintFinding,
    action: DecisionAction,
    charsChanged: number = 0,
  ) {
    try {
      await api.grammarRecordDecision(action, lint.tier, lint.rule_kind, lint.rule_id, charsChanged);
    } catch {
      // Recording is best-effort; never block the UI on it.
    }
  }

  function acceptFinding(lint: LintFinding, replacement: string) {
    const idx = result?.lints.indexOf(lint) ?? -1;
    if (idx < 0 || !result) return;
    // Apply the replacement to the input text (char offsets are utf-8 based
    // in the backend; here we use the utf16 offsets the backend also returns
    // so JS string slicing is correct).
    const before = inputText.slice(0, lint.start_utf16);
    const after = inputText.slice(lint.end_utf16);
    inputText = before + replacement + after;
    const charsChanged = Math.abs(replacement.length - (lint.end_utf16 - lint.start_utf16));
    recordDecision(lint, "accepted", charsChanged);
    // Drop the finding from the visible list so the user can move on.
    result = { ...result, lints: result.lints.filter((l) => l !== lint) };
  }

  function rejectFinding(lint: LintFinding) {
    if (!result) return;
    recordDecision(lint, "rejected");
    result = { ...result, lints: result.lints.filter((l) => l !== lint) };
  }

  function ignoreFinding(lint: LintFinding) {
    if (!result) return;
    recordDecision(lint, "ignored");
    result = { ...result, lints: result.lints.filter((l) => l !== lint) };
  }

  async function applyDocxFix(lint: DocxFinding, replacement: string) {
    if (!docxPath || !docxResult) return;
    const inputName = docxPath.split(/[\\/]/).pop() || "document.docx";
    const stem = inputName.replace(/\.docx$/i, "");
    const outputPath = await save({
      defaultPath: `${stem}-proofread.docx`,
      filters: [{ name: "Word document", extensions: ["docx"] }],
    });
    if (!outputPath) return;
    error = "";
    busy = true;
    applyResult = null;
    try {
      applyResult = await api.grammarApplyDocxFix(docxPath, outputPath, lint as LintFinding, replacement);
      const charsChanged = Math.abs(replacement.length - (lint.end_utf16 - lint.start_utf16));
      recordDecision(lint as LintFinding, "accepted", charsChanged);
      // Remove the resolved finding from the visible list.
      docxResult = {
        ...docxResult,
        findings: docxResult.findings.filter((f) => f !== lint),
      };
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function resolveDocxFinding(lint: DocxFinding) {
    if (!docxResult) return;
    recordDecision(lint as LintFinding, "accepted");
    docxResult = {
      ...docxResult,
      findings: docxResult.findings.filter((f) => f !== lint),
    };
  }

  function rejectDocxFinding(lint: DocxFinding) {
    if (!docxResult) return;
    recordDecision(lint as LintFinding, "rejected");
    docxResult = {
      ...docxResult,
      findings: docxResult.findings.filter((f) => f !== lint),
    };
  }

  function ignoreDocxFinding(lint: DocxFinding) {
    if (!docxResult) return;
    recordDecision(lint as LintFinding, "ignored");
    docxResult = {
      ...docxResult,
      findings: docxResult.findings.filter((f) => f !== lint),
    };
  }

  async function toggleTier2() {
    error = "";
    try {
      if (tier2Enabled) {
        await api.grammarTier2Disable();
        tier2Enabled = false;
        tier2Result = null;
      } else {
        await api.grammarTier2Enable();
        tier2Enabled = true;
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function suggestTier2() {
    if (!tier2Sentence.trim()) {
      error = "Type a sentence to suggest fixes for.";
      return;
    }
    error = "";
    tier2Busy = true;
    tier2Result = null;
    try {
      tier2Result = await api.grammarTier2Suggest(tier2Sentence, dialect, tier2Model);
    } catch (e) {
      error = String(e);
    } finally {
      tier2Busy = false;
    }
  }

  async function exportLedger() {
    error = "";
    try {
      ledger = await api.grammarLedgerExport();
    } catch (e) {
      error = String(e);
    }
  }

  async function copyLedger() {
    if (!ledger) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(ledger, null, 2));
      copiedLedger = true;
      setTimeout(() => (copiedLedger = false), 1500);
    } catch {
      error = "Clipboard not available.";
    }
  }

  async function resetSession() {
    if (!confirm("Reset the proofread session ledger? Counts and decisions will be cleared.")) return;
    error = "";
    try {
      await api.grammarSessionReset();
      ledger = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function copyChecked() {
    if (!inputText) return;
    try {
      await navigator.clipboard.writeText(inputText);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      error = "Clipboard not available.";
    }
  }

  function tierLabel(tier: Tier): string {
    return tier === "harper" ? "Harper (rule-based)" : "Local LLM (Tier 2)";
  }

  function ruleKindLabel(kind: RuleKind): string {
    return kind.charAt(0).toUpperCase() + kind.slice(1);
  }
</script>

<h1>Proofread</h1>
<p class="lead">
  Rule-based spelling, grammar, and punctuation checking via the Harper engine. Tier 1 runs
  entirely on this device with no AI model. Every change is your decision: nothing is applied
  automatically. Optional Phase 3 (off by default) adds local-LLM minimal-edit suggestions via
  Ollama, one sentence at a time, temperature 0, with a system prompt that forbids rewriting.
</p>

<div class="callout info">
  <strong>Rule-based checker.</strong> Runs on this device. No AI model is used.
</div>

{#if error}<div class="callout warn"><strong>Error:</strong> {error}</div>{/if}

<div class="row" style="margin-bottom: 12px; gap: 8px;">
  <button class="shrink" class:primary={mode === "text"} on:click={() => (mode = "text")}>Text mode</button>
  <button class="shrink" class:primary={mode === "docx"} on:click={() => (mode = "docx")}>.docx mode</button>
  {#if engineInfo}
    <span class="dim" style="font-size: var(--font-sm); align-self: center;">
      Engine: {engineInfo.name} v{engineInfo.version} ({engineInfo.tier})
    </span>
  {/if}
</div>

{#if mode === "text"}
  <div class="card">
    <div class="card-title">Input text</div>
    <div class="card-subtitle">Paste your draft. Sensitive spans (URLs, emails, code, DOIs, citations) are masked before checking and never leave the engine.</div>
    <textarea bind:value={inputText} rows="10" placeholder="Paste text to proofread here..."></textarea>
    <div class="dim" style="font-size: var(--font-sm); margin-top: 4px;">
      {inputText.length.toLocaleString()} characters
    </div>

    <div class="row" style="margin-top: 12px; gap: 12px; flex-wrap: wrap;">
      <div>
        <label class="dim" for="pr-dialect" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Dialect</label>
        <select id="pr-dialect" bind:value={dialect}>
          {#each dialects as d}<option value={d.id}>{d.label}</option>{/each}
        </select>
      </div>
      <div>
        <label class="dim" for="pr-format" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Format</label>
        <select id="pr-format" bind:value={format}>
          <option value="plain">Plain text</option>
          <option value="markdown">Markdown</option>
        </select>
      </div>
      <div class="spacer"></div>
      <button class="primary" on:click={checkText} disabled={busy || !inputText.trim()}>
        {busy ? "Checking..." : "Check text"}
      </button>
    </div>

    <details style="margin-top: 12px;">
      <summary style="cursor: pointer; font-size: var(--font-sm); color: var(--text-muted);">
        Personal dictionary (one word per line; matched words are never flagged)
      </summary>
      <textarea bind:value={personalWordsRaw} rows="4" style="margin-top: 6px;" placeholder={"e.g.\nScholarScribe\ndoi\nOllama"}></textarea>
    </details>
  </div>

  {#if result}
    <h2>Findings ({result.lints.length})</h2>
    {#if result.lints.length === 0}
      <div class="card"><div class="no-data">No issues found. The text passed all rule-based checks.</div></div>
    {:else}
      {#each result.lints as lint}
        <div class="card">
          <div class="row" style="align-items: flex-start; gap: 12px;">
            <div style="flex: 1;">
              <div class="row" style="gap: 6px; flex-wrap: wrap; margin-bottom: 6px;">
                <span class="tag">{tierLabel(lint.tier)}</span>
                <span class="tag">{ruleKindLabel(lint.rule_kind)}</span>
                <span class="dim" style="font-size: var(--font-sm);">rule #{lint.rule_id}</span>
              </div>
              <div style="margin-bottom: 4px;">
                <code>{lint.original_text}</code>
                <span class="dim" style="font-size: var(--font-sm);"> (chars {lint.start_char}–{lint.end_char})</span>
              </div>
              <div class="dim" style="font-size: var(--font-sm);">{lint.explanation}</div>
              {#if lint.suggestions.length > 0}
                <div style="margin-top: 8px;">
                  <strong style="font-size: var(--font-sm);">Suggestions:</strong>
                  <div class="row" style="gap: 6px; flex-wrap: wrap; margin-top: 4px;">
                    {#each lint.suggestions as s}
                      <button class="shrink primary" on:click={() => acceptFinding(lint, s.replacement)} title={s.label}>
                        {s.replacement || "(delete)"}
                      </button>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
            <div style="display: flex; flex-direction: column; gap: 4px;">
              <button class="shrink" on:click={() => rejectFinding(lint)} title="Reject this finding">Reject</button>
              <button class="shrink" on:click={() => ignoreFinding(lint)} title="Ignore this finding for the session">Ignore</button>
            </div>
          </div>
        </div>
      {/each}
    {/if}

    <h2>Checked text</h2>
    <div class="card">
      <div class="row" style="margin-bottom: 8px;">
        <div class="dim" style="font-size: var(--font-sm);">
          {inputText.length.toLocaleString()} characters · {result.masked_spans.length} spans masked
        </div>
        <div class="spacer"></div>
        <button class="shrink" on:click={copyChecked}>{copied ? "Copied!" : "Copy"}</button>
      </div>
      <pre style="max-height: 400px; overflow-y: auto; white-space: pre-wrap; word-wrap: break-word;">{inputText}</pre>
    </div>
  {/if}
{:else}
  <div class="card">
    <div class="card-title">.docx input</div>
    <div class="card-subtitle">Pick a Word document. Paragraphs are checked individually, findings carry paragraph indices and in-place apply eligibility.</div>
    <div class="row" style="margin-bottom: 8px; gap: 12px;">
      <button class="shrink" on:click={pickDocx}>Pick .docx...</button>
      {#if docxPath}
        <span class="dim" style="font-size: var(--font-sm); word-break: break-all;">{docxPath}</span>
      {/if}
      <div class="spacer"></div>
      <div>
        <label class="dim" for="pr-docx-dialect" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Dialect</label>
        <select id="pr-docx-dialect" bind:value={dialect}>
          {#each dialects as d}<option value={d.id}>{d.label}</option>{/each}
        </select>
      </div>
      <button class="primary" on:click={checkDocx} disabled={busy || !docxPath}>
        {busy ? "Checking..." : "Check .docx"}
      </button>
    </div>
    <details style="margin-top: 6px;">
      <summary style="cursor: pointer; font-size: var(--font-sm); color: var(--text-muted);">
        Personal dictionary (one word per line)
      </summary>
      <textarea bind:value={personalWordsRaw} rows="4" style="margin-top: 6px;" placeholder={"e.g.\nScholarScribe\ndoi\nOllama"}></textarea>
    </details>
  </div>

  {#if docxResult}
    <h2>Paragraph-level findings ({docxResult.findings.length})</h2>
    {#if docxResult.findings.length === 0}
      <div class="card"><div class="no-data">No issues found across {docxResult.paragraph_count} paragraphs.</div></div>
    {:else}
      {#each docxResult.findings as lint}
        <div class="card">
          <div class="row" style="align-items: flex-start; gap: 12px;">
            <div style="flex: 1;">
              <div class="row" style="gap: 6px; flex-wrap: wrap; margin-bottom: 6px;">
                <span class="tag">Paragraph {lint.paragraph_index + 1} of {lint.paragraph_count}</span>
                <span class="tag">{tierLabel(lint.tier)}</span>
                <span class="tag">{ruleKindLabel(lint.rule_kind)}</span>
                {#if lint.can_apply_in_place}
                  <span class="tag" style="background: var(--success); color: white;">can apply in place</span>
                {:else}
                  <span class="tag" style="background: var(--warning); color: white;" title={lint.apply_blocker}>manual: {lint.apply_blocker}</span>
                {/if}
              </div>
              <div style="margin-bottom: 4px;"><code>{lint.original_text}</code></div>
              <div class="dim" style="font-size: var(--font-sm);">{lint.explanation}</div>
              {#if lint.suggestions.length > 0}
                <div style="margin-top: 8px;">
                  <strong style="font-size: var(--font-sm);">Suggestions:</strong>
                  <div class="row" style="gap: 6px; flex-wrap: wrap; margin-top: 4px;">
                    {#each lint.suggestions as s}
                      <button
                        class="shrink primary"
                        on:click={() => (lint.can_apply_in_place ? applyDocxFix(lint, s.replacement) : resolveDocxFinding(lint))}
                        title={lint.can_apply_in_place ? `Apply "${s.replacement}" and save to a new .docx` : `Cannot apply in place: ${lint.apply_blocker}. Click to mark as resolved.`}
                      >
                        {s.replacement || "(delete)"}
                      </button>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
            <div style="display: flex; flex-direction: column; gap: 4px;">
              <button class="shrink" on:click={() => rejectDocxFinding(lint)}>Reject</button>
              <button class="shrink" on:click={() => ignoreDocxFinding(lint)}>Ignore</button>
            </div>
          </div>
        </div>
      {/each}
    {/if}
  {/if}

  {#if applyResult}
    <h2>Apply result</h2>
    <div class="callout info">
      <strong>Saved:</strong> <code>{applyResult.output_path}</code><br />
      <strong>Applied:</strong> {applyResult.applied_count} · <strong>Skipped:</strong> {applyResult.skipped_count}
    </div>
    {#if applyResult.skipped_summary.length > 0}
      <div class="card">
        <div class="card-title">Skipped reasons</div>
        <ul style="margin: 6px 0 0 16px; padding: 0;">
          {#each applyResult.skipped_summary as s}
            <li style="font-size: var(--font-sm);">{s.reason}: {s.count}</li>
          {/each}
        </ul>
      </div>
    {/if}
  {/if}
{/if}

<h2>Tier 2 (optional, local-LLM suggestions)</h2>
<div class="card">
  <div class="card-subtitle">Off by default. When enabled, runs one sentence at a time against your local Ollama, temperature 0, with a system prompt that forbids rewriting. No detector-score feedback loop, no coupling with Risk/Style/Fingerprint/Voice modules.</div>
  <div class="row" style="margin-top: 8px; gap: 12px; flex-wrap: wrap;">
    <button class="shrink" class:primary={tier2Enabled} on:click={toggleTier2} disabled={busy}>
      {tier2Enabled ? "Tier 2 ON, click to disable" : "Enable Tier 2"}
    </button>
    {#if tier2Enabled}
      <div>
        <label class="dim" for="pr-t2-model" style="font-size: var(--font-sm); display: block; margin-bottom: 4px;">Model</label>
        <input id="pr-t2-model" type="text" bind:value={tier2Model} placeholder="e.g. gemma3:4b" style="width: 200px;" />
      </div>
    {/if}
  </div>
  {#if tier2Enabled}
    <textarea bind:value={tier2Sentence} rows="3" style="margin-top: 10px;" placeholder="Type a single sentence to suggest minimal-edit fixes for..."></textarea>
    <div class="row" style="margin-top: 8px;">
      <button class="primary" on:click={suggestTier2} disabled={tier2Busy || !tier2Sentence.trim()}>
        {tier2Busy ? "Thinking..." : "Suggest fixes"}
      </button>
    </div>
    {#if tier2Result}
      <div style="margin-top: 12px;">
        {#if tier2Result.was_clean}
          <div class="no-data">No fixes suggested for this sentence.</div>
        {:else}
          <strong style="font-size: var(--font-sm);">Suggested fixes (model: {tier2Result.model}):</strong>
          <ul style="margin: 6px 0 0 16px; padding: 0;">
            {#each tier2Result.fixes as f}
              <li style="font-size: var(--font-sm);"><span class="tag">{f.kind}</span> {f.description}</li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<h2>Session ledger</h2>
<div class="card">
  <div class="card-subtitle">Counts and enumerated rule IDs only. No user text, not even hashes. Reset on app close.</div>
  <div class="row" style="margin-top: 8px; gap: 8px;">
    <button class="shrink" on:click={exportLedger}>Export ledger</button>
    <button class="shrink" on:click={copyLedger} disabled={!ledger}>{copiedLedger ? "Copied!" : "Copy JSON"}</button>
    <div class="spacer"></div>
    <button class="shrink danger" on:click={resetSession}>Reset session</button>
  </div>
  {#if ledger}
    <div class="row" style="margin-top: 14px; text-align: center; gap: 12px; flex-wrap: wrap;">
      <div>
        <div class="dim" style="font-size: var(--font-sm);">ACCEPTED</div>
        <div style="font-size: var(--font-xl); font-weight: 600; color: var(--success);">{ledger.accepted_count}</div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">REJECTED</div>
        <div style="font-size: var(--font-xl); font-weight: 600; color: var(--danger);">{ledger.rejected_count}</div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">IGNORED</div>
        <div style="font-size: var(--font-xl); font-weight: 600;">{ledger.ignored_count}</div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">CHARS CHANGED</div>
        <div style="font-size: var(--font-xl); font-weight: 600;">{ledger.total_chars_changed}</div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">SENTENCES</div>
        <div style="font-size: var(--font-xl); font-weight: 600;">{ledger.sentences_scanned}</div>
      </div>
      <div>
        <div class="dim" style="font-size: var(--font-sm);">MASKED SPANS</div>
        <div style="font-size: var(--font-xl); font-weight: 600;">{ledger.mask_count}</div>
      </div>
    </div>
    <div class="dim" style="font-size: var(--font-sm); margin-top: 12px;">
      Engine: {ledger.engine_name} v{ledger.engine_version} · Dialect: {ledger.dialect} ·
      Tier 1: {ledger.tier1_enabled ? "on" : "off"} · Tier 2: {ledger.tier2_enabled ? "on" : "off"}
    </div>
  {/if}
</div>
