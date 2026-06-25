<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AuditEntry, type AuditSummary } from "../lib/api";

  let entries: AuditEntry[] = [];
  let summary: AuditSummary | null = null;
  let loading = false;
  let filter = "all";

  async function refresh() {
    loading = true;
    try {
      [entries, summary] = await Promise.all([api.auditList(), api.auditSummary()]);
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  }

  onMount(refresh);

  async function clear() {
    if (!confirm("Clear the audit log for this session?")) return;
    await api.auditClear();
    await refresh();
  }

  function fmtTime(ts: number): string {
    return new Date(ts * 1000).toLocaleTimeString();
  }

  function fmtBytes(n: number): string {
    if (n === 0) return "—";
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(2)} MB`;
  }

  $: filtered = filter === "all"
    ? entries
    : entries.filter((e) => e.kind === filter);
</script>

<h1>Privacy Audit</h1>
<p class="lead">
  Every time ScholarScribe reads a file or makes an outbound HTTP call this session, it's recorded here.
  The log is in-memory only, it's cleared when the app closes. Use it to verify the app is doing
  exactly what it claims: reading only the files you pick, and talking only to the hosts it should.
</p>

<div class="callout info">
  <strong>How to read this.</strong>
  <ul style="margin: 6px 0 0 16px; padding: 0;">
    <li><strong>file_read</strong>, ScholarScribe read a file you picked in a file dialog.</li>
    <li><strong>http_call</strong>, ScholarScribe made an outbound HTTP request. The <em>target</em> column shows the URL.</li>
    <li><strong>ollama_command</strong>, ScholarScribe asked your local Ollama to do something (pull, chat, delete). These calls go to <code>127.0.0.1:11434</code> only, they don't leave your machine.</li>
  </ul>
</div>

{#if summary}
  <h2>Session summary</h2>
  <div class="card">
    <div class="row" style="text-align: center;">
      <div>
        <div class="dim" style="font-size: 11px;">TOTAL EVENTS</div>
        <div style="font-size: 22px; font-weight: 600;">{summary.total_events}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">FILE READS</div>
        <div style="font-size: 22px; font-weight: 600;">{summary.file_reads}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">HTTP CALLS</div>
        <div style="font-size: 22px; font-weight: 600;">{summary.http_calls}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">OLLAMA COMMANDS</div>
        <div style="font-size: 22px; font-weight: 600;">{summary.ollama_commands}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">DATA IN</div>
        <div style="font-size: 22px; font-weight: 600;">{fmtBytes(summary.bytes_in)}</div>
      </div>
      <div>
        <div class="dim" style="font-size: 11px;">DATA OUT</div>
        <div style="font-size: 22px; font-weight: 600;">{fmtBytes(summary.bytes_out)}</div>
      </div>
    </div>
    <div style="margin-top: 16px; padding-top: 12px; border-top: 1px solid var(--border);">
      <div class="dim" style="font-size: 11px;">OUTBOUND HOSTS CONTACTED THIS SESSION</div>
      {#if summary.outbound_hosts.length === 0}
        <div class="no-data" style="padding: 6px 0;">None yet. Download a model or chat with one to see activity here.</div>
      {:else}
        <ul style="margin: 6px 0 0; padding-left: 20px;">
          {#each summary.outbound_hosts as h}<li><code>{h}</code></li>{/each}
        </ul>
      {/if}
      <div class="muted" style="font-size: 12px; margin-top: 8px;">
        Expected hosts: <code>registry.ollama.ai</code> (model downloads only).
        All other hosts appearing here should be reported.
      </div>
    </div>
  </div>
{/if}

<h2>Event log</h2>
<div class="card">
  <div class="row" style="margin-bottom: 12px;">
    <select bind:value={filter} style="flex: 0 0 200px;">
      <option value="all">All events</option>
      <option value="file_read">File reads only</option>
      <option value="http_call">HTTP calls only</option>
      <option value="ollama_command">Ollama commands only</option>
    </select>
    <div class="spacer"></div>
    <button class="shrink" on:click={refresh}>{loading ? "Loading…" : "Refresh"}</button>
    <button class="shrink danger" on:click={clear}>Clear log</button>
  </div>

  {#if filtered.length === 0}
    <p class="no-data">No events to show.</p>
  {:else}
    <table>
      <thead>
        <tr><th>Time</th><th>Kind</th><th>Target</th><th>Detail</th><th>Bytes in</th><th>Bytes out</th></tr>
      </thead>
      <tbody>
        {#each filtered.slice().reverse() as e}
          <tr>
            <td class="muted" style="white-space: nowrap;">{fmtTime(e.timestamp)}</td>
            <td><span class="tag">{e.kind}</span></td>
            <td style="word-break: break-all; max-width: 280px;"><code>{e.target}</code></td>
            <td class="muted" style="max-width: 280px;">{e.detail}</td>
            <td class="muted">{fmtBytes(e.bytes_in)}</td>
            <td class="muted">{fmtBytes(e.bytes_out)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<div class="callout info">
  <strong>Verification tip.</strong>
  Run ScholarScribe behind a network monitor (GlassWire, Wireshark, or Little Snitch on macOS).
  Cross-reference the outbound hosts listed here with what your monitor sees. If they don't match,
  something is wrong, please report it via the project's SECURITY.md process.
</div>
