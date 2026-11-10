<script lang="ts">
  // A library conversation, read-only. As in the backup viewer, only a window of a few hundred messages is in the
  // page: more are fetched near either end and the far end dropped, keeping what's on screen still.
  import { onMount, tick } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import MessageCard from "../../lib/components/MessageCard.svelte";
  import type { ConversationRow, MessageRow } from "../../lib/backend/types";
  import { date, fmt, longDate } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { appName, convTitle, emptyFilter, handPick, lib, openLibExport, showLibMessage } from "../../lib/library.svelte";
  import { app, be, go } from "../../lib/state.svelte";

  let { row, jump }: { row: ConversationRow; jump: number | null } = $props();
  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);

  const PAGE = 100;
  const MAX = 400;
  const EDGE = 700;
  let len = $state(0);
  let start = $state(0);
  let lines = $state<MessageRow[]>([]);
  let highlight = $state<number | null>(null);
  let atEnd = $state(true);
  let dating = $state(false);
  let picking = $state(false);
  let noLater = $state(false);
  let box: HTMLDivElement;
  let busy = false;

  const dayOf = (ms: number | null) => (ms === null ? "" : new Date(ms).toDateString());

  function element(pos: number): HTMLElement | null {
    return box ? box.querySelector(`[data-pos="${pos}"]`) : null;
  }

  function anchor(): number | null {
    if (!box) return null;
    for (const el of box.querySelectorAll<HTMLElement>("[data-pos]")) {
      if (el.offsetTop + el.offsetHeight > box.scrollTop) return Number(el.dataset.pos);
    }
    return null;
  }

  async function keepingPlace(change: () => void) {
    const pos = anchor();
    const el = pos === null ? null : element(pos);
    const offset = el ? el.offsetTop - box.scrollTop : 0;
    change();
    await tick();
    const again = pos === null ? null : element(pos);
    if (again && box) box.scrollTop = again.offsetTop - offset;
  }

  async function openAt(pos: number | null) {
    busy = true;
    len = await be().libConversationLen(row.id);
    const target = pos ?? len - 1;
    const from = Math.max(0, Math.min(target - PAGE / 2, len - PAGE));
    lines = await be().libMessages(row.id, from, PAGE);
    start = from;
    await tick();
    if (!box) return;
    if (pos === null) box.scrollTop = box.scrollHeight;
    else {
      highlight = pos;
      element(pos)?.scrollIntoView({ block: "center" });
    }
    busy = false;
    await onScroll();
  }

  async function older() {
    const from = Math.max(0, start - PAGE);
    const more = await be().libMessages(row.id, from, start - from);
    await keepingPlace(() => {
      lines = [...more, ...lines].slice(0, MAX);
      start = from;
    });
  }

  async function newer() {
    const end = start + lines.length;
    const more = await be().libMessages(row.id, end, PAGE);
    await keepingPlace(() => {
      const all = [...lines, ...more];
      const drop = Math.max(0, all.length - MAX);
      lines = all.slice(drop);
      start += drop;
    });
  }

  async function onScroll() {
    if (!box) return;
    const end = start + lines.length;
    const fromBottom = box.scrollHeight - box.scrollTop - box.clientHeight;
    atEnd = end >= len && fromBottom < 80;
    if (busy) return;
    busy = true;
    try {
      if (box.scrollTop < EDGE && start > 0) await older();
      else if (fromBottom < EDGE && end < len) await newer();
      else return;
    } finally {
      busy = false;
    }
    await onScroll();
  }

  async function newest() {
    highlight = null;
    if (start + lines.length >= len) box.scrollTo({ top: box.scrollHeight, behavior: "smooth" });
    else await openAt(null);
  }

  async function oldest() {
    highlight = null;
    await openAt(0);
    if (box) box.scrollTop = 0;
  }

  async function toDate(value: string) {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
    if (!m) return;
    const pos = await be().libPositionAt(row.id, new Date(+m[1], +m[2] - 1, +m[3]).getTime());
    noLater = pos === null;
    if (pos !== null) await openAt(pos);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "End") {
      e.preventDefault();
      void newest();
    } else if (e.key === "Home") {
      e.preventDefault();
      void oldest();
    }
  }

  function exportThis() {
    const filter = emptyFilter();
    filter.conversations = [row.id];
    openLibExport({ filter, add: [], remove: [] }, convTitle(row));
  }

  onMount(() => {
    void openAt(jump);
  });
</script>

<header class="conv-head">
  <button class="btn icon-only back" aria-label={t.allChatsBack} title={t.allChatsBack} onclick={() => (lib.conv = null)}><Icon name="chevron" class="flip" /></button>
  <div class="who">
    <h2>{convTitle(row)}</h2>
    <p><strong>{appName(row.app)}</strong> · {fmt(len || row.messages)} {(len || row.messages) === 1 ? t.message : t.messages} · {t.lastMessage} {date(row.last_ms)}{#if row.wall_clock} · {l.wallClock}{/if}</p>
  </div>
  <div class="tools">
    <button class="btn icon-only" aria-label={t.searchThisChat} title={t.searchThisChat} onclick={() => { lib.searchConv = row.id; go("lsearch"); }}><Icon name="search" /></button>
    <button class="btn icon-only" aria-label={t.toOldest} title={t.toOldest} onclick={oldest}><Icon name="arrowUp" /></button>
    <button class="btn icon-only" aria-label={t.jumpDate} title={t.jumpDate} aria-expanded={dating} onclick={() => (dating = !dating)}><Icon name="calendar" /></button>
    <button class="btn icon-only" aria-label={l.pickMessages} title={l.pickMessages} aria-pressed={picking} onclick={() => (picking = !picking)}><Icon name="check" /></button>
    <button class="btn icon-only" aria-label={l.exportThis} title={l.exportThis} onclick={exportThis}><Icon name="download" /></button>
  </div>
</header>

{#if dating}
  <div class="bar">
    <label for="lib-jump-date">{t.jumpDate}</label>
    <input id="lib-jump-date" class="select" type="date" placeholder="YYYY-MM-DD" onchange={(e) => toDate(e.currentTarget.value)} />
    {#if noLater}<span class="muted" role="status">{t.noLater}</span>{/if}
  </div>
{/if}
{#if picking}
  <div class="bar" role="status">
    <span>{l.pickingHint} · {fmt(lib.selection.add.length)} {l.pickedByHand}</span>
    <button class="btn small" onclick={() => go("select")}>{l.openSelect}</button>
  </div>
{/if}

<div class="scroller-wrap">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="scroller" bind:this={box} tabindex="0" role="log" aria-label="{t.viewer}: {convTitle(row)}. {t.viewerKeys}" onscroll={onScroll} onkeydown={onKey}>
    <div class="msgs">
      {#each lines as m, i (m.id)}
        <div class="line" data-pos={start + i}>
          {#if m.time_ms !== null && (i === 0 || dayOf(lines[i - 1].time_ms) !== dayOf(m.time_ms))}
            <div class="day"><span>{longDate(m.time_ms)}</span></div>
          {/if}
          <MessageCard {m} highlight={highlight === start + i} showSender={row.kind !== "direct"} picked={picking ? lib.selection.add.includes(m.id) : null} onpick={(on) => handPick(m.id, on)} onreply={(id) => showLibMessage(id)} />
        </div>
      {/each}
    </div>
  </div>
  {#if !atEnd && lines.length > 0}
    <button class="btn jump" onclick={newest}>{t.jumpNewest}<Icon name="arrowDown" size={16} /></button>
  {/if}
</div>

<footer class="readonly"><Icon name="lock" size={14} />{l.readOnly}</footer>

<style>
  .conv-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .back {
    display: none;
  }
  .back :global(.flip) {
    transform: rotate(180deg);
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  .who h2 {
    margin: 0;
    font-size: 17px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who p {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tools {
    display: flex;
    gap: 6px;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 8px 18px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  .scroller-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .scroller {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    overflow-y: auto;
    overflow-anchor: none;
    padding: 8px 18px 18px;
  }
  .msgs {
    max-width: 860px;
    margin: 0 auto;
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
  .jump {
    position: absolute;
    right: 22px;
    bottom: 16px;
    box-shadow: var(--shadow-2);
    border-radius: 999px;
  }
  .readonly {
    display: flex;
    gap: 6px;
    align-items: center;
    justify-content: center;
    padding: 8px;
    font-size: 12.5px;
    color: var(--muted);
    border-top: 1px solid var(--line);
    background: var(--surface);
  }
  @media (max-width: 599px) {
    .back {
      display: inline-flex;
    }
    .conv-head {
      padding: 8px 10px;
      gap: 6px;
    }
    .tools {
      gap: 2px;
    }
    .scroller {
      padding: 8px 10px 14px;
    }
  }
</style>
