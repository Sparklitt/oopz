<script lang="ts">
  // A chat, read-only. Only a window of a few hundred messages is in the page: more are fetched as the person nears
  // either end and the far end is dropped, keeping what is on screen still. A chat of a million scrolls like one of 100.
  import { onMount, tick } from "svelte";
  import Icon from "../components/Icon.svelte";
  import type { ChatRow, Line } from "../backend/types";
  import { chatName, date, fmt, isGroup, longDate } from "../format";
  import { strings } from "../i18n";
  import { app, be, go, openExport } from "../state.svelte";
  import Bubble from "./Bubble.svelte";

  let { id, row, jump }: { id: number; row: ChatRow; jump: number | null } = $props();
  let t = $derived(strings(app.lang));

  const PAGE = 100;
  const MAX = 400;
  const EDGE = 700;
  let len = $state(0);
  let start = $state(0);
  let lines = $state<Line[]>([]);
  let highlight = $state<number | null>(null);
  let atEnd = $state(true);
  let dating = $state(false);
  let noLater = $state(false);
  let box: HTMLDivElement;
  let busy = false;

  const dayOf = (ms: number) => new Date(ms).toDateString();

  function element(pos: number): HTMLElement | null {
    return box.querySelector(`[data-pos="${pos}"]`);
  }

  /** The first message on screen: kept in place while the window changes around it. */
  function anchor(): number | null {
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
    if (again) box.scrollTop = again.offsetTop - offset;
  }

  async function openAt(pos: number | null) {
    busy = true;
    len = await be().chatLen(id, row.number);
    const target = pos ?? len - 1;
    const from = Math.max(0, Math.min(target - PAGE / 2, len - PAGE));
    lines = await be().chatLines(id, row.number, from, PAGE);
    start = from;
    await tick();
    if (pos === null) {
      box.scrollTop = box.scrollHeight;
    } else {
      highlight = pos;
      element(pos)?.scrollIntoView({ block: "center" });
    }
    busy = false;
    await onScroll();
  }

  async function older() {
    const from = Math.max(0, start - PAGE);
    const more = await be().chatLines(id, row.number, from, start - from);
    await keepingPlace(() => {
      lines = [...more, ...lines].slice(0, MAX);
      start = from;
    });
  }

  async function newer() {
    const end = start + lines.length;
    const more = await be().chatLines(id, row.number, end, PAGE);
    await keepingPlace(() => {
      const all = [...lines, ...more];
      const drop = Math.max(0, all.length - MAX);
      lines = all.slice(drop);
      start += drop;
    });
  }

  async function onScroll() {
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
    await onScroll(); // Still near an edge (a fast scroll, or a short window): fetch the next page too.
  }

  async function newest() {
    highlight = null;
    if (start + lines.length >= len) box.scrollTo({ top: box.scrollHeight, behavior: "smooth" });
    else await openAt(null);
    box.focus();
  }

  async function oldest() {
    highlight = null;
    await openAt(0);
    box.scrollTop = 0;
  }

  /** Opens the chat at the first message on or after the chosen day (midnight, this device's time). */
  async function toDate(value: string) {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
    if (!m) return;
    const pos = await be().positionAt(id, row.number, new Date(+m[1], +m[2] - 1, +m[3]).getTime());
    noLater = pos === null;
    if (pos !== null) await openAt(pos);
  }

  async function pickMedia() {
    const folder = await be().pickFolder();
    if (folder) app.mediaFolder = folder;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "End" && (e.ctrlKey || e.metaKey || !e.shiftKey)) {
      e.preventDefault();
      void newest();
    } else if (e.key === "Home") {
      e.preventDefault();
      void oldest();
    }
  }

  onMount(() => {
    void openAt(jump);
  });
</script>

<header class="conv-head">
  <button class="btn icon-only back" aria-label={t.allChatsBack} title={t.allChatsBack} onclick={() => (app.chat = null)}><Icon name="chevron" class="flip" /></button>
  <span class="avatar big" class:group={isGroup(row)} aria-hidden="true"><Icon name={isGroup(row) ? "users" : "user"} size={18} /></span>
  <div class="who">
    <h2>{chatName(row)}</h2>
    <p>{fmt(len || row.messages)} {t.messages} · {t.lastMessage} {date(row.last_ms)}</p>
  </div>
  <div class="tools">
    <button class="btn icon-only" aria-label={t.searchThisChat} title={t.searchThisChat} onclick={() => { app.searchChat = row.number; go("search"); }}><Icon name="search" /></button>
    <button class="btn icon-only" aria-label={t.toOldest} title={t.toOldest} onclick={oldest}><Icon name="arrowUp" /></button>
    <button class="btn icon-only" aria-label={t.jumpDate} title={t.jumpDate} aria-expanded={dating} onclick={() => (dating = !dating)}><Icon name="calendar" /></button>
    <button class="btn icon-only" aria-label={t.showPhotos} title={t.showPhotosHint} aria-pressed={app.mediaFolder !== null} onclick={pickMedia}><Icon name="image" /></button>
    <button class="btn icon-only" aria-label={t.exportChat} title={t.exportChat} onclick={() => openExport(row.number)}><Icon name="download" /></button>
  </div>
</header>

{#if dating}
  <div class="date-bar">
    <label for="jump-date">{t.jumpDate}</label>
    <input id="jump-date" class="select" type="date" placeholder="YYYY-MM-DD" onchange={(e) => toDate(e.currentTarget.value)} />
    {#if noLater}<span class="muted" role="status">{t.noLater}</span>{/if}
  </div>
{/if}

<div class="scroller-wrap">
  <!-- A scrollable region: focusable so the keyboard can scroll it, with Home and End as shortcuts. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="scroller" bind:this={box} tabindex="0" role="log" aria-label="{t.viewer}: {chatName(row)}. {t.viewerKeys}" onscroll={onScroll} onkeydown={onKey}>
    <div class="msgs">
      {#each lines as line, i (line.id)}
        <div class="line" data-pos={start + i}>
          {#if i === 0 || dayOf(lines[i - 1].ts) !== dayOf(line.ts)}
            <div class="day"><span>{longDate(line.ts)}</span></div>
          {/if}
          <Bubble {id} {line} highlight={highlight === start + i} />
        </div>
      {/each}
    </div>
  </div>
  {#if !atEnd && lines.length > 0}
    <button class="btn jump" onclick={newest}>{t.jumpNewest}<Icon name="arrowDown" size={16} /></button>
  {/if}
</div>

<footer class="readonly"><Icon name="lock" size={14} />{t.readOnly}</footer>

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
  .date-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 8px 18px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
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
  .scroller:focus {
    outline: 2px solid var(--focus);
    outline-offset: -3px;
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
    box-shadow: var(--shadow-1);
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
  /* A phone: the list and the chat take turns; Back returns to the list. */
  @media (max-width: 599px) {
    .back {
      display: inline-flex;
    }
    .conv-head {
      padding: 8px 10px;
      gap: 8px;
    }
    .conv-head .avatar {
      display: none;
    }
    .scroller {
      padding: 8px 10px 14px;
    }
  }
</style>
