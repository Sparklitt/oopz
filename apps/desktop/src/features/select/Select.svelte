<script lang="ts">
  // Building a selection to export: a filter (words, apps, sources, contacts, types, dates), plus messages ticked by
  // hand in a conversation, minus those unticked here. Selections are saved by name in the library.
  import { onMount } from "svelte";
  import ErrorNote from "../../lib/components/ErrorNote.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import LibFilterFields from "../../lib/components/LibFilterFields.svelte";
  import MessageCard from "../../lib/components/MessageCard.svelte";
  import PageHead from "../../lib/components/PageHead.svelte";
  import type { ConversationRow, MessageRow, SavedSelection } from "../../lib/backend/types";
  import { date, fmt } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { convTitle, emptyFilter, emptySelection, everything, lib, openLibExport, showLibMessage, toggleInSelection } from "../../lib/library.svelte";
  import { app, be, run } from "../../lib/state.svelte";

  const PAGE = 50;
  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let ids = $state<number[]>([]);
  let shown = $state<MessageRow[]>([]);
  let saved = $state<SavedSelection[]>([]);
  let name = $state("");
  let convs = $state<ConversationRow[]>([]);
  let titles = $derived(new Map(convs.map((c) => [c.id, convTitle(c)])));
  let bump = $state(0);
  let timer: number | undefined;

  $effect(() => {
    void bump;
    void lib.version;
    const sel = $state.snapshot(lib.selection);
    window.clearTimeout(timer);
    let live = true;
    timer = window.setTimeout(async () => {
      const found = await be().libSelected(sel, "time").catch(() => [] as number[]);
      if (!live) return;
      ids = found;
      shown = await be().libMessagesById(found.slice(0, PAGE)).catch(() => []);
    }, 150);
    return () => (live = false);
  });

  onMount(() => {
    void loadSaved();
    be()
      .libConversations(emptyFilter())
      .then((c) => (convs = c))
      .catch(() => {});
  });

  async function loadSaved() {
    saved = (await run(() => be().libSelections())) ?? [];
  }

  async function more() {
    const next = await be().libMessagesById(ids.slice(shown.length, shown.length + PAGE)).catch(() => []);
    shown = [...shown, ...next];
  }

  async function save(e: Event) {
    e.preventDefault();
    if (!name.trim()) return;
    const id = await run(() => be().libSaveSelection(name.trim(), $state.snapshot(lib.selection)));
    if (id !== undefined) {
      name = "";
      await loadSaved();
    }
  }

  function use(s: SavedSelection) {
    lib.selection = JSON.parse(JSON.stringify(s.selection));
    bump++;
  }
</script>

<div class="page">
  <PageHead title={l.selectTitle} sub={l.selectLead}>
    {#snippet actions()}
      <button class="btn primary" disabled={ids.length === 0} onclick={() => openLibExport($state.snapshot(lib.selection), "")}><Icon name="download" />{t.exportShort}</button>
    {/snippet}
  </PageHead>
  <ErrorNote error={app.error} words={l.errors} />

  {#if saved.length}
    <h2 class="h3">{l.savedSelections}</h2>
    <ul class="inset saved">
      {#each saved as s (s.id)}
        <li>
          <button class="link grow" onclick={() => use(s)}>{s.name}</button>
          <span class="muted">{date(s.saved_ms)}</span>
          <button class="btn small" aria-label="{l.delete}: {s.name}" onclick={() => run(() => be().libDeleteSelection(s.id)).then(loadSaved)}>{l.delete}</button>
        </li>
      {/each}
    </ul>
  {/if}

  <h2 class="h3">{l.whatToSelect}</h2>
  <div class="row">
    <input class="select words" type="search" placeholder={l.searchPlaceholder} aria-label={l.searchPlaceholder} bind:value={lib.selection.filter.words} />
    <button class="btn small" onclick={() => { lib.selection.filter = everything(); bump++; }}>{l.selectEverything}</button>
    <button class="btn small" onclick={() => { lib.selection = emptySelection(); bump++; }}>{l.startOver}</button>
  </div>
  <LibFilterFields bind:filter={lib.selection.filter} onchange={() => bump++} />
  <p class="count" aria-live="polite">
    <strong>{fmt(ids.length)} {t.messages}</strong>
    {#if lib.selection.add.length} · {fmt(lib.selection.add.length)} {l.pickedByHand}{/if}
    {#if lib.selection.remove.length} · {fmt(lib.selection.remove.length)} {l.takenOut}{/if}
    {#if lib.selection.add.length || lib.selection.remove.length}<button class="btn small" onclick={() => { lib.selection.add = []; lib.selection.remove = []; }}>{l.clearHand}</button>{/if}
  </p>
  <p class="hint">{l.selectHint}</p>

  <form class="row" onsubmit={save}>
    <input class="select" placeholder={l.selectionName} aria-label={l.selectionName} bind:value={name} />
    <button class="btn small" disabled={!name.trim() || ids.length === 0}>{l.saveSelection}</button>
  </form>

  {#if shown.length}
    <div class="msgs">
      {#each shown as m (m.id)}
        <MessageCard {m} where={titles.get(m.conversation) ?? `#${m.conversation}`} picked={!lib.selection.remove.includes(m.id)} onpick={(on) => toggleInSelection(m.id, on)} onreply={(id) => showLibMessage(id)} />
      {/each}
    </div>
    {#if shown.length < ids.length}<button class="btn small" onclick={more}>{t.showMore}</button>{/if}
  {/if}
</div>

<style>
  .saved li {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
    text-align: left;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    margin: 8px 0;
  }
  .words {
    flex: 1 1 240px;
    min-width: 0;
  }
  .count {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .msgs {
    max-width: 860px;
    margin: 12px 0;
    padding: 8px;
    border-radius: var(--radius-m);
    background: var(--conv-bg);
  }
</style>
