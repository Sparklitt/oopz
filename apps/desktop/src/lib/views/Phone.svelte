<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import PathRow from "../components/PathRow.svelte";
  import { strings } from "../i18n";
  import { app, go } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let checks = app.phoneChecks;
  let ticked = $derived(t.phoneSteps.filter((_, i) => checks[i]).length);
</script>

<div class="page narrow">
  <PageHead title={t.phoneTitle} sub={t.phoneLead} />
  {#if app.outcome}<PathRow folder={app.outcome.folder} />{/if}
  <ol class="stepper">
    {#each t.phoneSteps as [title, text], i (i)}
      <li class:done={checks[i]}>
        <span class="dot" aria-hidden="true">{#if checks[i]}<Icon name="check" size={14} />{:else}{i + 1}{/if}</span>
        <label class="check-row">
          <input type="checkbox" bind:checked={checks[i]} />
          <span><strong>{title}</strong><span class="text">{text}</span></span>
        </label>
      </li>
    {/each}
  </ol>
  <div class="foot">
    <span class="muted" aria-live="polite">{ticked}/{t.phoneSteps.length} {t.ticked}</span>
    <button class="btn" onclick={() => go("restore")}>{t.back}</button>
    <button class="btn primary" disabled={t.phoneSteps.some((_, i) => !checks[i])} onclick={() => go("done")}>{t.phoneDone}</button>
  </div>
</div>

<style>
  .stepper {
    list-style: none;
    margin: 20px 0;
    padding: 0;
    display: grid;
  }
  .stepper li {
    position: relative;
    display: flex;
    gap: 14px;
    padding-bottom: 18px;
  }
  .stepper li:not(:last-child)::before {
    content: "";
    position: absolute;
    left: 13px;
    top: 30px;
    bottom: 2px;
    width: 2px;
    background: var(--line-strong);
  }
  .dot {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 13px;
    font-weight: 600;
    border: 2px solid var(--accent);
    color: var(--accent-text);
    background: var(--surface);
    z-index: 1;
  }
  .done .dot {
    background: var(--accent);
    color: var(--on-accent);
  }
  .check-row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 4px 0;
    cursor: pointer;
  }
  .check-row input {
    margin-top: 4px;
  }
  .check-row strong {
    display: block;
  }
  .text {
    display: block;
    color: var(--muted);
    margin-top: 2px;
  }
  .done .text {
    opacity: 0.8;
  }
  .foot {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .foot .muted {
    margin-right: auto;
  }
</style>
