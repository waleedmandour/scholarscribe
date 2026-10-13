<script lang="ts">
  // Language Check (v2.2.1 r5). Offline spelling, grammar and style checks
  // powered by harper-core, embedded in the Rust backend. Nothing leaves
  // the machine: no network, no helper process, no telemetry.
  import PageHeader from "./PageHeader.svelte";
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api";
  import type { LcCategory, LcIssue, LcSettings, LcDictionary, LcCheckResult, LcEdit } from "../lib/api";
  import { open, save } from "@tauri-apps/plugin-dialog";

  let text = "";
  let dialect = "American";
  let supportedDialects: string[] = ["American", "British", "Canadian", "Australian", "Indian"];
  let result: LcCheckResult | null = null;
  let settings: LcSettings | null = null;
  let dictionary: LcDictionary | null = null;
  let ignoredKeys = new Set<string>();
  let activeTab: "all" | LcCategory = "all";
  let busy = false;
  let importing = false;
  let exporting = false;
  let error = "";
  let notice = "";
  let copied = false;
  let newWord = "";
  let showRuleSettings = false;
  let highlightedId: number | null = null;

  const CATEGORY_LABEL: Record<LcCategory, string> = {
    spelling: "Spelling",
    grammar: "Grammar",
    style: "Style",
  };

  $: totalChars = text.length;
  $: visibleIssues = result ? result.issues.filter((i) => !ignoredKeys.has(ignoreKey(i))) : [];
  $: liveCounts = countVisible(visibleIssues);
  $: shownIssues = activeTab === "all" ? visibleIssues : visibleIssues.filter((i) => i.category === activeTab);
  $: firedRules = buildFiredRules(visibleIssues);

  function ignoreKey(issue: LcIssue): string {
    return `${issue.rule_id}|${issue.span_start}|${issue.problem_text}`;
  }

  function countVisible(issues: LcIssue[]) {
    const counts = { spelling: 0, grammar: 0, style: 0, total: issues.length };
    for (const i of issues) counts[i.category] += 1;
    return counts;
  }

  function buildFiredRules(issues: LcIssue[]): { rule_id: string; enabled: boolean }[] {
    const seen = new Map<string, boolean>();
    for (const i of issues) {
      if (!seen.has(i.rule_id) && settings) {
        const override = settings.rules[i.rule_id];
        seen.set(i.rule_id, override === undefined ? true : (override !== null && override !== false));
      }
    }
    return Array.from(seen, ([rule_id, enabled]) => ({ rule_id, enabled })).sort((a, b) =>
      a.rule_id.localeCompare(b.rule_id),
    );
  }

  function ruleEnabled(ruleId: string): boolean {
    if (!settings) return true;
    const override = settings.rules[ruleId];
    return override === undefined ? true : override !== null && override !== false;
  }

  async function refreshSettings() {
    try {
      settings = await api.languageCheckSettingsGet();
      dialect = settings.dialect || "American";
      supportedDialects = settings.supported_dialects;
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshDictionary() {
    try {
      dictionary = await api.languageCheckDictionaryList();
    } catch (e) {
      error = String(e);
    }
  }

  async function onMountActions() {
    await Promise.all([refreshSettings(), refreshDictionary()]);
  }

  async function runCheck() {
    if (!text.trim()) {
      error = "Paste or import some text first.";
      return;
    }
    error = "";
    notice = "";
    busy = true;
    try {
      result = await api.languageCheckRun(text, dialect);
      ignoredKeys = new Set();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function changeDialect(next: string) {
    dialect = next;
    try {
      settings = await api.languageCheckSettingsSet(next, null);
      if (result) await runCheck();
    } catch (e) {
      error = String(e);
    }
  }

  function suggestionLabel(kind: string, replacement: string | undefined): string {
    if (kind === "replace_with") return `Replace with "${replacement ?? ""}"`;
    if (kind === "insert_after") return `Insert "${replacement ?? ""}" after`;
    return "Remove";
  }

  function contextFor(issue: LcIssue): { pre: string; post: string } {
    const chars = Array.from(text);
    const from = Math.max(0, issue.span_start - 55);
    const to = Math.min(chars.length, issue.span_end + 55);
    return {
      pre: (from > 0 ? "..." : "") + chars.slice(from, issue.span_start).join(""),
      post: chars.slice(issue.span_end, to).join("") + (to < chars.length ? "..." : ""),
    };
  }

  async function applySuggestion(issue: LcIssue, index: number) {
    if (!result) return;
    const suggestion = issue.suggestions[index];
    if (!suggestion) return;
    error = "";
    busy = true;
    try {
      const edit: LcEdit = {
        span_start: issue.span_start,
        span_end: issue.span_end,
        suggestion,
      };
      const applied = await api.languageCheckApply(text, [edit]);
      text = applied.corrected_text;
      await runCheck();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function applyAllCategory(category: LcCategory) {
    if (!result) return;
    const edits: LcEdit[] = visibleIssues
      .filter((i) => i.category === category && i.suggestions.length > 0)
      .map((i) => ({
        span_start: i.span_start,
        span_end: i.span_end,
        suggestion: i.suggestions[0],
      }));
    if (edits.length === 0) {
      notice = `No proposed corrections to apply in ${CATEGORY_LABEL[category]}.`;
      return;
    }
    error = "";
    busy = true;
    try {
      const applied = await api.languageCheckApply(text, edits);
      text = applied.corrected_text;
      notice = `Applied ${applied.edits_applied} correction${applied.edits_applied === 1 ? "" : "s"}.`;
      await runCheck();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function ignoreIssue(issue: LcIssue) {
    ignoredKeys = new Set([...ignoredKeys, ignoreKey(issue)]);
  }

  function isSingleWord(issue: LcIssue): boolean {
    return issue.problem_text.trim().length > 0 && issue.problem_text.trim().split(/\s+/).length === 1;
  }

  async function addWordToDictionary(word: string) {
    error = "";
    try {
      dictionary = await api.languageCheckDictionaryAdd(word);
      notice = `"${word}" added to your personal dictionary.`;
      await runCheck();
    } catch (e) {
      error = String(e);
    }
  }

  async function addWordManually() {
    const word = newWord.trim();
    if (!word) return;
    newWord = "";
    await addWordToDictionary(word);
  }

  async function removeWord(word: string) {
    error = "";
    try {
      dictionary = await api.languageCheckDictionaryRemove(word);
    } catch (e) {
      error = String(e);
    }
  }

  async function turnOffRule(ruleId: string) {
    if (!settings) return;
    error = "";
    try {
      const nextRules = { ...settings.rules, [ruleId]: false };
      settings = await api.languageCheckSettingsSet(null, nextRules);
      notice = `Rule "${ruleId}" is now off. Use Rule settings to restore curated defaults.`;
      await runCheck();
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleRule(ruleId: string, enabled: boolean) {
    if (!settings) return;
    error = "";
    try {
      const nextRules = { ...settings.rules, [ruleId]: enabled ? null : false };
      settings = await api.languageCheckSettingsSet(null, nextRules);
      await runCheck();
    } catch (e) {
      error = String(e);
    }
  }

  async function resetRules() {
    error = "";
    try {
      settings = await api.languageCheckSettingsSet(null, {});
      notice = "All rule overrides cleared; curated defaults restored.";
      await runCheck();
    } catch (e) {
      error = String(e);
    }
  }

  async function importFile() {
    error = "";
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Text or document", extensions: ["txt", "md", "tex", "docx"] },
      ],
    });
    if (!selected || typeof selected !== "string") return;
    importing = true;
    try {
      const imported = await api.languageCheckImportFile(selected);
      text = imported.text;
      result = null;
      ignoredKeys = new Set();
      notice = `Imported ${imported.source_path.split(/[\\/]/).pop()}.`;
    } catch (e) {
      error = String(e);
    } finally {
      importing = false;
    }
  }

  async function copyText() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      error = "Clipboard not available. Select the text in the editor and copy manually.";
    }
  }

  async function exportTxt() {
    if (!text.trim()) {
      error = "There is no text to save yet.";
      return;
    }
    const target = await save({
      defaultPath: "corrected-text.txt",
      filters: [{ name: "Plain text", extensions: ["txt"] }],
    });
    if (!target) return;
    exporting = true;
    error = "";
    try {
      await api.languageCheckExportTxt(target, text);
      notice = `Saved to ${target}.`;
    } catch (e) {
      error = String(e);
    } finally {
      exporting = false;
    }
  }

  function scrollToCard(id: number) {
    highlightedId = id;
    const el = document.getElementById(`lc-card-${id}`);
    if (el) {
      const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      el.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "center" });
    }
    setTimeout(() => (highlightedId = null), 1600);
  }

  interface Segment {
    text: string;
    issue: LcIssue | null;
  }

  function previewSegments(): Segment[] {
    if (!result || visibleIssues.length === 0) return [{ text, issue: null }];
    const chars = Array.from(text);
    const segments: Segment[] = [];
    let cursor = 0;
    for (const issue of visibleIssues) {
      if (issue.span_start > chars.length || issue.span_end > chars.length) continue;
      if (issue.span_start > cursor) {
        segments.push({ text: chars.slice(cursor, issue.span_start).join(""), issue: null });
      }
      segments.push({ text: chars.slice(issue.span_start, issue.span_end).join(""), issue });
      cursor = issue.span_end;
    }
    if (cursor < chars.length) {
      segments.push({ text: chars.slice(cursor).join(""), issue: null });
    }
    return segments;
  }

  onMountActions();
</script>

<PageHeader icon="spellcheck" title="Language Check">
  Offline proofreading for spelling, grammar, and style, with one click corrections. The engine
  runs entirely inside ScholarScribe; your text never leaves this machine.
</PageHeader>

{#if error}
  <div class="callout warn" role="alert"><strong>Error:</strong> {error}</div>
{/if}
{#if notice}
  <div class="callout info" role="status">{notice}</div>
{/if}

<div class="card">
  <div class="card-title"><Icon name="file-text" size={17} /> 1. Input</div>
  <div class="card-subtitle">
    Paste text, or import a .txt, .md, .tex, or .docx file. Checking is English only in this
    release.
  </div>
  <div class="row" style="margin: 10px 0;">
    <label class="shrink" for="lc-dialect" style="font-size: var(--font-sm);">English dialect</label>
    <select id="lc-dialect" class="shrink" value={dialect} on:change={(e) => changeDialect(e.currentTarget.value)}>
      {#each supportedDialects as d}
        <option value={d}>{d}</option>
      {/each}
    </select>
    <span class="spacer"></span>
    <button class="shrink" on:click={importFile} disabled={importing}>
      {importing ? "Importing..." : "Open file"}
    </button>
    <button class="shrink" on:click={() => { text = ""; result = null; ignoredKeys = new Set(); notice = ""; }}>
      Clear
    </button>
  </div>
  <textarea
    class="lc-input"
    bind:value={text}
    rows={9}
    placeholder="Paste the draft you want to check..."
    aria-label="Text to check"
  ></textarea>
  <div class="row" style="margin-top: 10px;">
    <span class="dim" style="font-size: var(--font-sm);">{totalChars.toLocaleString()} characters</span>
    <span class="spacer"></span>
    <button class="primary" on:click={runCheck} disabled={busy || !text.trim()}>
      <Icon name={busy ? "refresh" : "spellcheck"} size={15} />
      {busy ? "Checking..." : "Check language"}
    </button>
  </div>
</div>

{#if result}
  <div class="card">
    <div class="card-title"><Icon name="eye" size={17} /> 2. Preview</div>
    <div class="card-subtitle">
      Underlined spans are flagged issues, colored by category. Click one to jump to its card.
    </div>
    <div class="lc-preview">
      {#each previewSegments() as seg}
        {#if seg.issue}
          <span
            class="lc-span {seg.issue.category}"
            class:highlight={highlightedId === seg.issue.id}
            role="button"
            tabindex="0"
            title="{CATEGORY_LABEL[seg.issue.category]}: {seg.issue.message}"
            on:click={() => scrollToCard(seg.issue?.id ?? -1)}
            on:keydown={(e) => e.key === "Enter" && scrollToCard(seg.issue?.id ?? -1)}
          >{seg.text}</span>
        {:else}
          {seg.text}
        {/if}
      {/each}
    </div>
    <div class="lc-legend">
      <span class="lc-dot spelling"></span> Spelling
      <span class="lc-dot grammar"></span> Grammar
      <span class="lc-dot style"></span> Style
    </div>
  </div>

  <div class="card">
    <div class="card-title"><Icon name="check" size={17} /> 3. Issues</div>
    <div class="lc-tabs" role="tablist">
      <button class="lc-tab" class:active={activeTab === "all"} role="tab" aria-selected={activeTab === "all"} on:click={() => (activeTab = "all")}>
        All <span class="tag">{liveCounts.total}</span>
      </button>
      {#each Object.keys(CATEGORY_LABEL) as cat}
        <button class="lc-tab" class:active={activeTab === cat} role="tab" aria-selected={activeTab === cat} on:click={() => (activeTab = cat as LcCategory)}>
          {CATEGORY_LABEL[cat as LcCategory]}
          <span class="tag">{liveCounts[cat as LcCategory]}</span>
        </button>
      {/each}
      <span class="spacer"></span>
      {#if activeTab !== "all"}
        <button class="shrink" on:click={() => applyAllCategory(activeTab as LcCategory)} disabled={busy}>
          Apply all in {CATEGORY_LABEL[activeTab as LcCategory]}
        </button>
      {/if}
    </div>

    {#if shownIssues.length === 0}
      <div class="no-data">
        {liveCounts.total === 0
          ? "No issues found. That does not guarantee the text is perfect; a rule based checker cannot catch everything."
          : `No ${activeTab === "all" ? "" : CATEGORY_LABEL[activeTab as LcCategory].toLowerCase() + " "}issues to show.`}
      </div>
    {:else}
      {#each shownIssues as issue (issue.id)}
        <article class="lc-card" class:highlight={highlightedId === issue.id} id="lc-card-{issue.id}">
          <div class="lc-card-head">
            <span class="tag {issue.category}">{CATEGORY_LABEL[issue.category]}</span>
            <code class="lc-rule">{issue.rule_id}</code>
            <span class="spacer"></span>
            <button class="lc-mini" on:click={() => ignoreIssue(issue)} title="Hide this issue for this session">
              Ignore
            </button>
            {#if isSingleWord(issue)}
              <button class="lc-mini" on:click={() => addWordToDictionary(issue.problem_text.trim())} title="Stop flagging this word from now on">
                Add "{issue.problem_text.trim()}" to dictionary
              </button>
            {/if}
            <button class="lc-mini" on:click={() => turnOffRule(issue.rule_id)} title="Turn this rule off in settings">
              Turn off rule
            </button>
          </div>
          <p class="lc-message">{issue.message}</p>
          <p class="lc-context">
            {contextFor(issue).pre}<mark class="lc-mark {issue.category}">{issue.problem_text}</mark>{contextFor(issue).post}
          </p>
          {#if issue.description}
            <p class="lc-desc">{issue.description}</p>
          {/if}
          {#if issue.suggestions.length > 0}
            <div class="lc-actions">
              {#each issue.suggestions as s, i}
                <button class={i === 0 ? "primary" : ""} disabled={busy} on:click={() => applySuggestion(issue, i)}>
                  {suggestionLabel(s.kind, s.text)}
                </button>
              {/each}
            </div>
          {:else}
            <p class="dim" style="font-size: var(--font-sm);">No automatic correction for this one; edit the text manually.</p>
          {/if}
        </article>
      {/each}
    {/if}
  </div>

  <div class="card">
    <div class="card-title"><Icon name="save" size={17} /> 4. Export</div>
    <div class="card-subtitle">
      The editor text updates in place as corrections are applied. Copy it, or save it as a .txt
      file. Corrections are only ever applied when you click them.
    </div>
    <div class="row" style="margin-top: 10px;">
      <button on:click={copyText} disabled={!text.trim()}>
        {copied ? "Copied" : "Copy corrected text"}
      </button>
      <button on:click={exportTxt} disabled={exporting || !text.trim()}>
        {exporting ? "Saving..." : "Save as .txt"}
      </button>
    </div>
  </div>
{/if}

<div class="card">
  <div class="card-title"><Icon name="book" size={17} /> Personal dictionary</div>
  <div class="card-subtitle">
    Names, technical terms, and other words you do not want flagged. Your dictionary
    {dictionary?.persisted
      ? "is saved with your other local data (persistence is on)."
      : "lives in memory for this session only. Turn on persistence in Saved Work to keep it between runs of the app."}
  </div>
  <div class="row" style="margin: 10px 0;">
    <input
      type="text"
      bind:value={newWord}
      placeholder="Add a word, e.g. CRISPR"
      aria-label="New dictionary word"
      on:keydown={(e) => e.key === "Enter" && addWordManually()}
    />
    <button class="shrink" on:click={addWordManually} disabled={!newWord.trim()}>Add word</button>
  </div>
  {#if dictionary && dictionary.words.length > 0}
    <div class="lc-chips">
      {#each dictionary.words as word (word)}
        <span class="lc-chip">
          {word}
          <button class="lc-chip-x" on:click={() => removeWord(word)} title="Remove {word} from the dictionary" aria-label="Remove {word}">
            <Icon name="x" size={11} />
          </button>
        </span>
      {/each}
    </div>
  {:else}
    <div class="no-data">Your dictionary is empty.</div>
  {/if}
</div>

<div class="card">
  <div class="card-title">
    <Icon name="cpu" size={17} /> Rule settings
    <span class="spacer"></span>
    <button class="shrink" on:click={() => (showRuleSettings = !showRuleSettings)}>
      {showRuleSettings ? "Hide" : "Show"}
    </button>
  </div>
  <div class="card-subtitle">
    Rules that fired in this check, with their current state. {settings?.overrides ?? 0} rule
    override{settings?.overrides === 1 ? "" : "s"} stored.
  </div>
  {#if showRuleSettings}
    {#if firedRules.length === 0}
      <div class="no-data">Run a check first; the rules it used appear here.</div>
    {:else}
      <table class="lc-rules">
        <thead>
          <tr><th>Rule</th><th>State</th><th></th></tr>
        </thead>
        <tbody>
          {#each firedRules as r (r.rule_id)}
            <tr>
              <td><code class="lc-rule">{r.rule_id}</code></td>
              <td>{r.enabled ? "On" : "Off"}</td>
              <td class="right">
                <button class="lc-mini" on:click={() => toggleRule(r.rule_id, !r.enabled)}>
                  {r.enabled ? "Turn off" : "Turn on"}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <div class="row" style="margin-top: 10px;">
        <button class="danger" on:click={resetRules} disabled={(settings?.overrides ?? 0) === 0}>
          Reset all rule overrides
        </button>
      </div>
    {/if}
  {/if}
</div>

<div class="card">
  <div class="card-title"><Icon name="shield" size={17} /> Privacy and limits</div>
  <ul class="lc-notes">
    <li>Checking runs in the app process with the embedded harper engine. No text is sent anywhere, and no new network hosts are introduced.</li>
    <li>English only in this release (American, British, Canadian, Australian, and Indian dialects).</li>
    <li>A rule based checker is deterministic but not complete: it can miss errors a larger commercial service would catch, and it can flag correct academic wording. Ignore or add such words to your dictionary.</li>
    <li>Suggestions are proposals. Nothing in your text changes until you click a correction.</li>
  </ul>
</div>

<style>
  .lc-input {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    min-height: 140px;
    font: inherit;
    line-height: var(--line-body);
    color: var(--text);
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 10px 12px;
  }
  .lc-input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .lc-preview {
    white-space: pre-wrap;
    line-height: 1.7;
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-sm);
    background: var(--bg-elev);
    padding: 14px 16px;
    margin-top: 10px;
    max-height: 340px;
    overflow: auto;
  }
  .lc-span {
    cursor: pointer;
    border-radius: 3px;
  }
  .lc-span.spelling {
    text-decoration: underline wavy var(--danger) 1.5px;
    text-underline-offset: 3px;
  }
  .lc-span.grammar {
    text-decoration: underline wavy var(--warning) 1.5px;
    text-underline-offset: 3px;
  }
  .lc-span.style {
    text-decoration: underline wavy var(--accent) 1.5px;
    text-underline-offset: 3px;
  }
  .lc-span:hover,
  .lc-span.highlight,
  .lc-card.highlight {
    background: var(--accent-soft);
  }
  .lc-legend {
    display: flex;
    gap: 14px;
    align-items: center;
    margin-top: 10px;
    font-size: var(--font-sm);
    color: var(--text-muted);
  }
  .lc-dot {
    width: 22px;
    height: 0;
    border-top: 2.5px solid;
    display: inline-block;
    margin-right: 5px;
    vertical-align: middle;
  }
  .lc-dot.spelling { border-color: var(--danger); }
  .lc-dot.grammar { border-color: var(--warning); }
  .lc-dot.style { border-color: var(--accent); }
  .lc-tabs {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 12px 0;
    flex-wrap: wrap;
  }
  .lc-tab {
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text-muted);
    border-radius: 999px;
    padding: 5px 12px;
    font-size: var(--font-sm);
  }
  .lc-tab .tag {
    margin-left: 6px;
  }
  .lc-tab.active {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent);
    font-weight: 600;
  }
  .lc-card {
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-sm);
    padding: 12px 14px;
    margin: 10px 0;
    background: var(--bg-elev);
  }
  .lc-card-head {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .lc-mini {
    font-size: var(--font-sm);
    padding: 3px 9px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
  }
  .lc-mini:hover {
    background: var(--bg-elev-2);
    color: var(--text);
  }
  .lc-rule {
    font-size: var(--font-sm);
    background: var(--code-bg);
    padding: 1px 6px;
    border-radius: 4px;
  }
  .lc-message {
    margin: 8px 0 4px;
    font-weight: 600;
  }
  .lc-context {
    margin: 4px 0;
    color: var(--text-muted);
    font-size: var(--font-sm);
    line-height: 1.6;
  }
  .lc-mark {
    background: transparent;
    font-weight: 600;
    color: var(--text);
  }
  .lc-mark.spelling { text-decoration: underline wavy var(--danger) 1.5px; text-underline-offset: 3px; }
  .lc-mark.grammar { text-decoration: underline wavy var(--warning) 1.5px; text-underline-offset: 3px; }
  .lc-mark.style { text-decoration: underline wavy var(--accent) 1.5px; text-underline-offset: 3px; }
  .lc-desc {
    margin: 4px 0;
    color: var(--text-dim);
    font-size: var(--font-sm);
  }
  .lc-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .tag.spelling { background: var(--danger-soft); color: var(--danger); }
  .tag.grammar { background: var(--warning-soft); color: var(--warning); }
  .tag.style { background: var(--accent-soft); color: var(--accent); }
  .lc-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .lc-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--border);
    background: var(--bg-elev-2);
    border-radius: 999px;
    padding: 3px 6px 3px 10px;
    font-size: var(--font-sm);
  }
  .lc-chip-x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: none;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    padding: 0;
  }
  .lc-chip-x:hover {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .lc-rules {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-sm);
  }
  .lc-rules th,
  .lc-rules td {
    text-align: left;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border-soft);
  }
  .lc-notes {
    margin: 6px 0 0;
    padding-left: 20px;
    line-height: 1.65;
    color: var(--text-muted);
    font-size: var(--font-sm);
  }
  input[type="text"] {
    font: inherit;
    color: var(--text);
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 7px 10px;
  }
  select {
    font: inherit;
    color: var(--text);
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 7px 10px;
  }
</style>
