<script lang="ts">
  // Shared "verified statistic" component. Every statistic in the UI that
  // cites an external source should be rendered via this component so the
  // source link and "Last verified" date are always present and consistent.
  //
  // Usage:
  //   <VerifiedStat
  //     figure="61.3% false-positive rate..."
  //     sourceUrl="https://doi.org/10.1016/j.patter.2023.100779"
  //     sourceLabel="Liang et al. (2023), Patterns"
  //     lastVerified="2026-09-29"
  //   />

  export let figure: string;
  export let sourceUrl: string;
  export let sourceLabel: string;
  export let lastVerified: string; // ISO date string (YYYY-MM-DD)
</script>

<div class="verified-stat">
  <p class="verified-stat-figure">{figure}</p>
  <p class="verified-stat-source">
    Source:
    <a href={sourceUrl} target="_blank" rel="noopener noreferrer">{sourceLabel}</a>
    <span class="dim">Last verified: {lastVerified}</span>
  </p>
</div>

<style>
  .verified-stat {
    margin: 12px 0;
    padding: 12px 14px;
    background: var(--accent-soft);
    border-left: 3px solid var(--accent);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
  }
  .verified-stat-figure {
    margin: 0 0 6px;
    font-size: 0.875rem;
    line-height: 1.5;
    color: var(--text);
  }
  .verified-stat-source {
    margin: 0;
    font-size: 0.75rem;
    color: var(--text-muted);
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .verified-stat-source a {
    color: var(--accent);
    text-decoration: underline;
  }
  .verified-stat-source a:hover {
    color: var(--accent-hover);
  }
</style>
