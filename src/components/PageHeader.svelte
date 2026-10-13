<!---->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { IconName } from "../lib/icons";

  export let icon: IconName;
  export let title: string;
</script>

<header class="page-head">
  <div class="page-head-badge">
    <Icon name={icon} size={21} />
  </div>
  <div class="page-head-text">
    <h1>{title}</h1>
    <p class="lead"><slot /></p>
  </div>
</header>

<style>
  .page-head {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    margin: 0 0 22px;
  }
  .page-head-badge {
    flex: 0 0 auto;
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent-soft);
    color: var(--accent);
    border: 1px solid var(--accent-border, rgba(47, 95, 216, 0.28));
  }
  .page-head-text {
    min-width: 0;
  }
  .page-head-text h1 {
    margin: 0;
  }
  .page-head-text :global(.lead) {
    margin: 4px 0 0;
  }
</style>
