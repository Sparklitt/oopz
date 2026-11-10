<script lang="ts">
  // A screen's title bar: its title, a line under it, and its toolbar actions on the right.
  import type { Snippet } from "svelte";

  let { title, sub, badge, actions }: { title: string; sub?: string; badge?: Snippet; actions?: Snippet } = $props();
</script>

<header class="head">
  <div class="words">
    <div class="title-row">
      <h1>{title}</h1>
      {#if badge}{@render badge()}{/if}
    </div>
    {#if sub}<p class="sub">{sub}</p>{/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>

<style>
  .head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 22px;
  }
  .words {
    flex: 1 1 300px;
    min-width: 0;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 26px;
    line-height: 1.2;
    font-weight: 700;
    letter-spacing: -0.015em;
    margin: 0;
    overflow-wrap: anywhere;
  }
  .sub {
    margin: 4px 0 0;
    color: var(--muted);
    max-width: 68ch;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
</style>
