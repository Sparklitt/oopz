<script lang="ts">
  // Where a long piece of work is, in words: its step, the percentage, the time left; and a Cancel button when the
  // work can be stopped.
  import type { Step } from "../backend/types";
  import { strings } from "../i18n";
  import { app } from "../state.svelte";

  let { step, oncancel }: { step: Step | null; oncancel?: () => void } = $props();
  let t = $derived(strings(app.lang));
  let stopping = $state(false);

  function cancel() {
    stopping = true;
    oncancel?.();
  }
</script>

<p class="step" aria-live="polite">
  {#if step}
    <span class="name">{t.steps[step.step] ?? step.step}</span>
    <span class="nums">{Math.round(step.per_mille / 10)}%{#if step.secs_left !== null} · {t.secsLeft(step.secs_left)}{/if}</span>
  {/if}
  {#if oncancel}<button type="button" class="btn small" disabled={stopping} onclick={cancel}>{t.cancel}</button>{/if}
</p>

<style>
  .step {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 6px 12px;
    margin: 8px 0 0;
    color: var(--muted);
  }
  .name {
    color: var(--text);
    font-weight: 600;
  }
  .nums {
    font-variant-numeric: tabular-nums;
  }
</style>
