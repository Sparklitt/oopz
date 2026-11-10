<script lang="ts">
  // Every app's messages merged by time: pick a contact (or apps, dates…) and read WhatsApp, Telegram, SMS… with
  // them as one thread, each message marked with its app and conversation.
  import { onMount } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import LibFilterFields from "../../lib/components/LibFilterFields.svelte";
  import MessageCard from "../../lib/components/MessageCard.svelte";
  import PageHead from "../../lib/components/PageHead.svelte";
  import type { ConversationRow, MessageRow, Order } from "../../lib/backend/types";
  import { fmt, longDate } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { convTitle, emptyFilter, everything, lib, openLibExport, showLibMessage } from "../../lib/library.svelte";
  import { app, be } from "../../lib/state.svelte";

  const PAGE = 100;
  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let filter = $state(emptyFilter());
  let order = $state<Order>("time");
  let rows = $state<MessageRow[]>([]);
  let total = $state(0);
  let convs = $state<ConversationRow[]>([]);
  let bump = $state(0);
  let titles = $derived(new Map(convs.map((c) => [c.id, convTitle(c)])));
  const dayOf = (ms: number | null) => (ms === null ? "" : new Date(ms).toDateString());

  $effect(() => {
    void bump;
    void lib.version;
    const f = $state.snapshot(filter);
    const o = order;
    let live = true;
    Promise.all([be().libFind(f, o, 0, PAGE), be().libCount(f)])
      .then(([r, n]) => {
        if (!live) return;
        rows = r;
        total = n;
      })
      .catch(() => live && (rows = []));
    return () => (live = false);
  });

  onMount(() => {
    be()
      .libConversations(emptyFilter())
      .then((c) => (convs = c))
      .catch(() => {});
  });

  async function more() {
    const next = await be().libFind($state.snapshot(filter), order, rows.length, PAGE).catch(() => []);
    rows = [...rows, ...next];
  }

  function exportThese() {
    const f = $state.snapshot(filter);
    const empty = JSON.stringify(f) === JSON.stringify(emptyFilter());
    openLibExport({ filter: empty ? everything() : f, add: [], remove: [] }, l.timelineTitle);
  }
</script>

<div class="page">
  <PageHead title={l.timelineTitle} sub={l.timelineLead}>
    {#snippet actions()}
      <label class="order">
        <span class="sr-only">{l.order}</span>
        <select class="select" bind:value={order}>
          <option value="time">{l.oldestFirst}</option>
          <option value="newest">{l.newestFirst}</option>
        </select>
      </label>
      <button class="btn" disabled={total === 0} onclick={exportThese}><Icon name="download" />{t.exportShort}</button>
    {/snippet}
  </PageHead>
  <LibFilterFields bind:filter onchange={() => bump++} />
  <p class="hint" aria-live="polite">{fmt(total)} {(total) === 1 ? t.message : t.messages}</p>
  {#if rows.length}
    <div class="msgs">
      {#each rows as m, i (m.id)}
        {#if m.time_ms !== null && (i === 0 || dayOf(rows[i - 1].time_ms) !== dayOf(m.time_ms))}
          <div class="day"><span>{longDate(m.time_ms)}</span></div>
        {/if}
        <MessageCard {m} where={titles.get(m.conversation) ?? `#${m.conversation}`} onreply={(id) => showLibMessage(id)} />
      {/each}
    </div>
    {#if rows.length < total}<button class="btn small" onclick={more}>{t.showMore}</button>{/if}
  {:else}
    <p class="hint">{lib.sources.length ? t.searchNone : l.emptyTitle}</p>
  {/if}
</div>

<style>
  .msgs {
    max-width: 860px;
    margin: 0 0 12px;
    padding: 8px;
    border-radius: var(--radius-m);
    background: var(--conv-bg);
  }
  .day {
    display: flex;
    justify-content: center;
    padding: 12px 0 8px;
  }
  .day span {
    font-size: 12px;
    font-weight: 600;
    padding: 4px 12px;
    border-radius: 999px;
    background: var(--chip-strong);
    color: var(--muted);
  }
</style>
