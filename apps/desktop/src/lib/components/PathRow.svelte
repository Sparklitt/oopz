<script lang="ts">
  // A path oopz made, with a button that shows its folder in the file manager.
  import { strings } from "../i18n";
  import { app, be, run } from "../state.svelte";
  import Icon from "./Icon.svelte";

  let { folder, shown = folder }: { folder: string; shown?: string } = $props();
  let t = $derived(strings(app.lang));
</script>

<div class="path-row">
  <div class="path"><Icon name="folder" /><code>{shown}</code></div>
  <button class="btn" onclick={() => run(() => be().showFolder(folder))}><Icon name="folder" />{t.showFolder}</button>
</div>

<style>
  .path-row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .path {
    flex: 1 1 260px;
    min-width: 0;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--surface-2);
    color: var(--muted);
  }
  code {
    color: var(--text);
    overflow-wrap: anywhere;
  }
</style>
