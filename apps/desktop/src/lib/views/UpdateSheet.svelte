<script lang="ts">
  import AppMark from "../components/AppMark.svelte";
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon from "../components/Icon.svelte";
  import Sheet from "../components/Sheet.svelte";
  import StepLine from "../components/StepLine.svelte";
  import { date } from "../format";
  import { blocks } from "../notes";
  import { strings } from "../i18n";
  import { app, installUpdate } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let u = $derived(app.updates);
  let installing = $derived(u.status === "installing");
</script>

<Sheet label={t.updatesTitle} size="small" onclose={installing ? undefined : () => (app.sheet = null)} footer={installing ? undefined : buttons}>
  <div class="top">
    <AppMark size={44} />
    <h1 class="sheet-title">{u.found ? t.updateFound(u.found.version) : t.updatesTitle}</h1>
  </div>
  {#if u.found?.date_ms}<p class="muted">{t.updatePublished} {date(u.found.date_ms)}</p>{/if}
  {#if u.found?.notes}
    <h2 class="h3">{t.updateNotes}</h2>
    <!-- From the release's Markdown: formatted, but always as text, never as HTML. -->
    <div class="notes">
      {#each blocks(u.found.notes) as b, i (i)}
        <p class={b.kind}>{#each b.spans as s, j (j)}{#if s.bold}<strong>{s.text}</strong>{:else if s.italic}<em>{s.text}</em>{:else if s.code}<code>{s.text}</code>{:else}{s.text}{/if}{/each}</p>
      {/each}
    </div>
  {/if}
  <p class="safe"><Icon name="shield" size={16} />{t.updateSigned}</p>
  {#if installing}
    <div role="status">
      <p>{t.updateInstalling}</p>
      {#if u.progress !== null}<progress max="1000" value={u.progress.per_mille} aria-label={t.updateInstalling}></progress>{:else}<progress aria-label={t.updateInstalling}></progress>{/if}
      <StepLine step={u.progress} />
    </div>
  {/if}
  {#if u.status === "failed"}<ErrorNote error={u.error} />{/if}
</Sheet>

{#snippet buttons()}
  <button class="btn" onclick={() => (app.sheet = null)}>{t.later}</button>
  {#if u.found && u.status !== "failed"}<button class="btn primary" onclick={installUpdate}>{t.updateInstall}</button>{/if}
{/snippet}

<style>
  .top {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .notes p {
    margin: 0 0 6px;
  }
  .notes .heading {
    font-weight: 600;
  }
  .notes .item {
    padding-inline-start: 1.1em;
    text-indent: -0.8em;
  }
  .notes .item::before {
    content: "• ";
  }
  .notes {
    overflow-wrap: anywhere;
    font: inherit;
    font-size: 13.5px;
    max-height: 220px;
    overflow: auto;
    margin: 0 0 12px;
    padding: 10px 12px;
    border-radius: var(--radius-s);
    background: var(--surface-2);
  }
  .safe {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    color: var(--muted);
    font-size: 13.5px;
  }
  progress {
    width: 100%;
  }
</style>
