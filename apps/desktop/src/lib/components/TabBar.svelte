<script lang="ts">
  // On a phone-wide window, the sidebar becomes five tabs at the bottom; Backups, Library and Tools open a list page.
  // Swiping sideways on the page moves between them.
  import { strings } from "../i18n";
  import { app, current, go, type View } from "../state.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let t = $derived(strings(app.lang));
  let has = $derived(current() !== null);
  const TAB_OF: Record<View, View> = {
    backups: "backups", add: "backups", overview: "backups", insights: "backups",
    chats: "chats", search: "search",
    library: "librarymenu", lchats: "librarymenu", lsearch: "librarymenu", linsights: "librarymenu", contacts: "librarymenu", select: "librarymenu", timeline: "librarymenu", librarymenu: "librarymenu",
    tools: "tools", diagnostics: "tools", receive: "tools", compare: "tools", restore: "tools", phone: "tools", done: "tools", settings: "tools",
  };
  let at = $derived(TAB_OF[app.view]);
  let tabs = $derived<[View, IconName, string, boolean][]>([
    ["backups", "phone", t.navBackups, true],
    ["chats", "chats", t.navChats, has],
    ["search", "search", t.navSearch, has],
    ["librarymenu", "folder", t.lib.nav, true],
    ["tools", "grid", t.navTools, true],
  ]);

  // A sideways swipe on the page moves to the next or previous open tab, as on most phone apps. Not on a sheet,
  // a field, a slider or anything that scrolls sideways itself, and not from the screen's very edges (the system's).
  const PHONE = "(max-width: 599px)";
  let start: { x: number; y: number; at: number } | null = null;

  function swipeable(target: EventTarget | null): boolean {
    if (!(target instanceof Element) || target.closest("dialog, input, textarea, select, [data-noswipe]")) return false;
    for (let el: Element | null = target; el && el !== document.body; el = el.parentElement) {
      const x = getComputedStyle(el).overflowX;
      if ((x === "auto" || x === "scroll") && el.scrollWidth > el.clientWidth + 1) return false;
    }
    return true;
  }

  function onTouchStart(e: TouchEvent) {
    const p = e.touches[0];
    const edge = 24;
    start =
      e.touches.length === 1 && matchMedia(PHONE).matches && p.clientX > edge && p.clientX < innerWidth - edge && swipeable(e.target)
        ? { x: p.clientX, y: p.clientY, at: Date.now() }
        : null;
  }

  function onTouchEnd(e: TouchEvent) {
    const s = start;
    start = null;
    const p = e.changedTouches[0];
    if (!s || !p || Date.now() - s.at > 700 || getSelection()?.toString()) return;
    const dx = p.clientX - s.x;
    const dy = p.clientY - s.y;
    if (Math.abs(dx) < Math.max(64, innerWidth * 0.18) || Math.abs(dx) < Math.abs(dy) * 2) return;
    const open = tabs.filter(([, , , on]) => on).map(([view]) => view);
    const next = open[open.indexOf(at) + (dx < 0 ? 1 : -1)];
    if (!next) return;
    go(next);
    if (!matchMedia("(prefers-reduced-motion: reduce)").matches) {
      const from = dx < 0 ? 32 : -32;
      document.querySelector("main.content")?.animate(
        [{ transform: `translateX(${from}px)`, opacity: 0.4 }, { transform: "none", opacity: 1 }],
        { duration: 180, easing: "ease-out" },
      );
    }
  }
</script>

<svelte:document ontouchstart={onTouchStart} ontouchend={onTouchEnd} ontouchcancel={() => (start = null)} />

<nav class="tabbar" aria-label={t.navLabel}>
  {#each tabs as [view, icon, label, on] (view)}
    <button class="tab" disabled={!on} aria-current={at === view ? "page" : undefined} onclick={() => go(view)}>
      <Icon name={icon} size={22} /><span>{label}</span>
    </button>
  {/each}
</nav>

<style>
  .tabbar {
    display: none;
  }
  @media (max-width: 599px) {
    .tabbar {
      display: grid;
      grid-template-columns: repeat(5, 1fr);
      flex: none;
      border-top: 1px solid var(--line);
      background: var(--surface);
      padding: 4px 4px 6px;
    }
  }
  .tab {
    display: grid;
    justify-items: center;
    gap: 2px;
    min-height: 52px;
    padding: 4px 2px;
    border: 0;
    border-radius: var(--radius-m);
    background: transparent;
    color: var(--muted);
    font-size: 12px;
  }
  .tab span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tab[aria-current] {
    color: var(--selected-text);
    background: var(--selected);
    font-weight: 600;
  }
  .tab:disabled {
    color: var(--faint);
  }
</style>
