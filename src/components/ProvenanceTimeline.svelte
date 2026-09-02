<script lang="ts">
  import type { ProvenanceSessionRecord } from "../lib/api";

  export let sessions: ProvenanceSessionRecord[] = [];
  export let anomalies: string[] = [];

  function fmtDate(ts: number): string {
    return new Date(ts * 1000).toLocaleString([], {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function fmtDuration(start: number, end: number): string {
    const mins = Math.max(0, Math.round((end - start) / 60));
    if (mins < 60) return `${mins} min`;
    const h = Math.floor(mins / 60);
    const m = mins % 60;
    return m === 0 ? `${h} h` : `${h} h ${m} min`;
  }

  function shortenHash(h: string): string {
    return h.length > 23 ? h.slice(0, 23) + "…" : h;
  }
</script>

<div class="timeline">
  {#each sessions as s, i}
    <div class="session">
      <div class="marker" class:genesis={s.prev_record_hash.includes("0000")}>
        {i + 1}
      </div>
      <div class="card session-card">
        <div class="row">
          <div>
            <strong>{s.author}</strong>
            <span class="muted"> · {fmtDuration(s.start_time, s.end_time)}</span>
          </div>
          <div class="muted small">{fmtDate(s.start_time)}</div>
        </div>
        <div class="counts">
          <span class="added">+{s.chars_added.toLocaleString()} chars</span>
          <span class="removed">−{s.chars_removed.toLocaleString()} chars</span>
          <span class="muted small">largest single insertion: {s.largest_insertion.toLocaleString()}</span>
        </div>
        <div class="chain muted small">
          <div title={s.prev_record_hash}>prev: {shortenHash(s.prev_record_hash)}</div>
          <div title={s.record_hash}>hash: {shortenHash(s.record_hash)}</div>
        </div>
      </div>
    </div>
  {/each}

  {#if anomalies.length > 0}
    <div class="callout warn" style="margin-top: 12px;">
      <strong>Anomalies observed in the revision history.</strong>
      These are reported honestly — they can have innocent explanations
      (holidays, merged branches, clock changes) and are not accusations.
      <ul style="margin: 6px 0 0 16px; padding: 0;">
        {#each anomalies as a}
          <li class="small">{a}</li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .timeline {
    display: flex;
    flex-direction: column;
  }
  .session {
    display: flex;
    gap: 12px;
    align-items: stretch;
    position: relative;
    padding-bottom: 10px;
  }
  .session:not(:last-of-type)::before {
    content: "";
    position: absolute;
    left: 13px;
    top: 28px;
    bottom: -4px;
    width: 2px;
    background: var(--border);
  }
  .marker {
    flex: 0 0 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    border: 1px solid var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    font-weight: 700;
    z-index: 1;
  }
  .marker.genesis {
    background: var(--accent);
    color: white;
  }
  .session-card {
    flex: 1;
    margin: 0;
  }
  .counts {
    display: flex;
    gap: 14px;
    align-items: baseline;
    margin-top: 6px;
    flex-wrap: wrap;
  }
  .added {
    color: var(--ok, #1a7f37);
    font-weight: 600;
    font-size: 13px;
  }
  .removed {
    color: var(--danger, #c0392b);
    font-weight: 600;
    font-size: 13px;
  }
  .chain {
    margin-top: 6px;
    font-family: var(--mono, monospace);
    font-size: 10.5px;
    line-height: 1.5;
    word-break: break-all;
  }
  .small {
    font-size: 11px;
  }
</style>
