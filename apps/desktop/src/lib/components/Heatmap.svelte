<script lang="ts">
  // Messages by day of the week and hour: 7 rows (Sunday first, as Rust sends them, shown from the week's first day
  // in the chosen language) of 24 cells, darker for more. Each cell's count is in its title and for screen readers.
  import { fmt } from "../format";
  import { strings } from "../i18n";
  import { app } from "../state.svelte";

  let { values }: { values: number[][] } = $props();
  let t = $derived(strings(app.lang));
  let top = $derived(Math.max(1, ...values.flat()));
  // Spanish weeks start on Monday, English ones on Sunday.
  let order = $derived(app.lang === "es" ? [1, 2, 3, 4, 5, 6, 0] : [0, 1, 2, 3, 4, 5, 6]);
  // 2023-01-01 was a Sunday.
  const dayName = (d: number, style: "short" | "long") => new Date(2023, 0, 1 + d).toLocaleDateString(app.lang, { weekday: style });
</script>

<h2 class="h3">{t.heatTitle}</h2>
<div class="heat" role="table" aria-label={t.heatTitle}>
  {#each order as d (d)}
    <div class="row" role="row">
      <span class="day" role="rowheader" title={dayName(d, "long")}>{dayName(d, "short")}</span>
      {#each values[d] ?? [] as n, h (h)}
        <span class="cell" role="cell" style:opacity={n ? 0.15 + 0.85 * (n / top) : 1} class:zero={!n} title="{dayName(d, 'long')} {h}:00 · {fmt(n)}">
          <span class="sr-only">{h}:00 {fmt(n)}</span>
        </span>
      {/each}
    </div>
  {/each}
  <div class="row hours" aria-hidden="true">
    <span class="day"></span>
    {#each Array.from({ length: 24 }, (_, h) => h) as h (h)}<span class="hour">{h % 6 === 0 ? h : ""}</span>{/each}
  </div>
</div>
<p class="legend" aria-hidden="true">{t.heatLess}<span class="cell" style:opacity="0.2"></span><span class="cell" style:opacity="0.55"></span><span class="cell"></span>{t.heatMore}</p>

<style>
  .heat {
    display: grid;
    gap: 3px;
    overflow-x: auto;
  }
  .row {
    display: grid;
    grid-template-columns: 3.2em repeat(24, minmax(10px, 1fr));
    gap: 3px;
    align-items: center;
  }
  .day,
  .hour,
  .legend {
    font-size: 11.5px;
    color: var(--muted);
  }
  .cell {
    display: inline-block;
    height: 18px;
    min-width: 10px;
    border-radius: 3px;
    background: var(--accent);
  }
  .cell.zero {
    background: var(--line);
  }
  .legend {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    margin: 8px 0 0;
  }
  .legend .cell {
    width: 14px;
    height: 14px;
  }
</style>
