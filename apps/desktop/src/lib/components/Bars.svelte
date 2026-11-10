<script lang="ts">
  // One series of bars (no legend: the heading names it), a tooltip per bar, labels at the ends.
  import { fmt } from "../format";

  let { values, label }: { values: [string, number][]; label: string } = $props();
  let max = $derived(Math.max(1, ...values.map(([, n]) => n)));
</script>

<div class="chart">
  <h2 class="h3">{label}</h2>
  <div class="bars" role="img" aria-label="{label}: max {fmt(max)}">
    {#each values as [name, n] (name)}
      <div class="bar" title="{name}: {fmt(n)}" style:height="{Math.max(1, (n / max) * 100)}%"></div>
    {/each}
  </div>
  {#if values.length}
    <div class="ends">
      <span>{values[0][0]}</span><span>max {fmt(max)}</span>{#if values.length > 1}<span>{values[values.length - 1][0]}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 150px;
    padding-bottom: 1px;
    border-bottom: 1px solid var(--line-strong);
  }
  .bar {
    flex: 1 1 0;
    max-width: 40px;
    min-width: 1px;
    background: var(--accent);
    border-radius: 2px 2px 0 0;
    opacity: 0.85;
  }
  .bar:hover {
    opacity: 1;
    background: var(--accent-strong);
  }
  .ends {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--muted);
    margin-top: 4px;
  }
</style>
