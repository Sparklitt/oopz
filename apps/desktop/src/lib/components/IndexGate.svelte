<script lang="ts">
  // Shows its content once the backup's search index is built; until then, the build's progress.
  import { untrack, type Snippet } from "svelte";
  import { strings } from "../i18n";
  import { app, cancelWork, ensureIndex } from "../state.svelte";
  import ErrorNote from "./ErrorNote.svelte";
  import StepLine from "./StepLine.svelte";

  let { id, children }: { id: number; children: Snippet } = $props();
  let t = $derived(strings(app.lang));
  let state = $derived(app.index[id]);

  $effect(() => {
    const backup = id;
    untrack(() => ensureIndex(backup));
  });
</script>

{#if state?.built}
  {@render children()}
{:else if state?.error}
  <ErrorNote error={state.error} />
  <button class="btn" onclick={() => ensureIndex(id)}>{t.tryAgain}</button>
{:else if state?.stopped}
  <div class="building">
    <p>{t.stopped}</p>
    <button class="btn" onclick={() => ensureIndex(id)}>{t.tryAgain}</button>
  </div>
{:else}
  <div class="building" role="status">
    <p>{t.exploreBuilding}</p>
    {#if state?.progress != null}
      <progress max="1000" value={state.progress.per_mille} aria-label={t.exploreBuilding}></progress>
    {:else}
      <progress aria-label={t.exploreBuilding}></progress>
    {/if}
    <StepLine step={state?.progress ?? null} oncancel={cancelWork} />
  </div>
{/if}

<style>
  .building {
    max-width: 480px;
    margin: 48px auto;
    text-align: center;
    color: var(--muted);
  }
  progress {
    width: 100%;
  }
</style>
