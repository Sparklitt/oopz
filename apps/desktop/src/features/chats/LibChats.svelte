<script lang="ts">
  // Every conversation in the library, from every app, the latest first; narrowed by app or source, and by a few
  // letters of its name. The one picked opens beside the list (on a phone, instead of it).
  import Icon from "../../lib/components/Icon.svelte";
  import type { AppCode, ConversationRow } from "../../lib/backend/types";
  import { date, fmt } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { appName, convTitle, emptyFilter, lib } from "../../lib/library.svelte";
  import { app, be, go } from "../../lib/state.svelte";
  import LibConversation from "./LibConversation.svelte";

  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let rows = $state<ConversationRow[]>([]);
  let filter = $state("");
  let limit = $state(300);
  let apps = $derived([...new Set(lib.sources.map((s) => s.app))] as AppCode[]);

  $effect(() => {
    const f = emptyFilter();
    f.apps = [...lib.chatApps];
    f.sources = lib.chatSource === null ? [] : [lib.chatSource];
    void lib.version;
    let live = true;
    be()
      .libConversations(f)
      .then((r) => live && (rows = r))
      .catch(() => live && (rows = []));
    return () => (live = false);
  });

  let shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    return q ? rows.filter((c) => convTitle(c).toLowerCase().includes(q)) : rows;
  });
  let selected = $derived(rows.find((c) => c.id === lib.conv) ?? null);

  function toggleApp(a: AppCode) {
    lib.chatApps = lib.chatApps.includes(a) ? lib.chatApps.filter((x) => x !== a) : [...lib.chatApps, a];
  }
  const ICON = { direct: "user", group: "users", channel: "megaphone", broadcast: "megaphone", notes: "file", unknown: "chats" } as const;
</script>

<div class="chats" class:open={selected !== null}>
  <section class="list-pane" aria-label={l.chatsTitle}>
    <h1 class="sr-only">{l.chatsTitle}</h1>
    {#if lib.sources.length === 0}
      <div class="none">
        <p>{l.emptyTitle}</p>
        <button class="btn primary small" onclick={() => go("library")}><Icon name="plus" size={16} />{l.add}</button>
      </div>
    {:else}
      <div class="chips" role="group" aria-label={l.byApp}>
        {#each apps as a (a)}<button class="chip" aria-pressed={lib.chatApps.includes(a)} onclick={() => toggleApp(a)}>{appName(a)}</button>{/each}
        {#if lib.sources.length > 1}
          <select class="select small" aria-label={l.source} value={lib.chatSource ?? 0} onchange={(e) => (lib.chatSource = Number(e.currentTarget.value) || null)}>
            <option value={0}>{l.allSources}</option>
            {#each lib.sources as s (s.id)}<option value={s.id}>{appName(s.app)} · {s.name}</option>{/each}
          </select>
        {/if}
      </div>
      <div class="filter">
        <Icon name="search" size={16} />
        <input type="search" bind:value={filter} placeholder={t.filterChats} aria-label={t.filterChats} />
      </div>
      <ul>
        {#each shown.slice(0, limit) as c (c.id)}
          <li>
            <button class="chat" aria-current={c.id === lib.conv ? "true" : undefined} onclick={() => { lib.conv = c.id; lib.jump = null; }}>
              <span class="avatar" class:group={c.kind !== "direct"} aria-hidden="true"><Icon name={ICON[c.kind]} size={16} /></span>
              <span class="words">
                <span class="name">{convTitle(c)}</span>
                <span class="sub"><span class="app">{appName(c.app)}</span> {c.preview ?? `${fmt(c.messages)} ${t.messages}`}</span>
              </span>
              <span class="when">{date(c.last_ms)}</span>
            </button>
          </li>
        {/each}
      </ul>
      {#if shown.length > limit}<button class="btn small more" onclick={() => (limit += 500)}>{t.showMore}</button>{/if}
      {#if shown.length === 0}<p class="hint none">{t.noChats}</p>{/if}
    {/if}
  </section>

  <section class="conv-pane" aria-label={selected ? convTitle(selected) : t.viewer}>
    {#if selected}
      {#key `${selected.id}:${lib.jump?.pos ?? "end"}`}
        <LibConversation row={selected} jump={lib.jump?.conv === selected.id ? lib.jump.pos : null} />
      {/key}
    {:else}
      <div class="pick">
        <Icon name="chats" size={40} />
        <p>{t.pickChat}</p>
      </div>
    {/if}
  </section>
</div>

<style>
  .chats {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .list-pane {
    width: 300px;
    flex: none;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--line);
    background: var(--surface);
    min-height: 0;
  }
  @media (max-width: 1000px) {
    .list-pane {
      width: 250px;
    }
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 12px 12px 0;
  }
  .chip {
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 3px 10px;
    font-size: 12.5px;
    background: transparent;
    color: var(--text);
  }
  .chip[aria-pressed="true"] {
    background: var(--selected);
    color: var(--selected-text);
    border-color: transparent;
    font-weight: 600;
  }
  .chips select {
    max-width: 100%;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px;
    padding: 0 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--surface-2);
    color: var(--muted);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    min-height: 34px;
    color: var(--text);
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 6px 8px;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .chat {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 8px;
    border: 0;
    border-radius: var(--radius-m);
    background: transparent;
    color: var(--text);
    text-align: left;
  }
  .chat:hover {
    background: var(--hover);
  }
  .chat[aria-current] {
    background: var(--selected);
  }
  .words {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub,
  .when {
    font-size: 12.5px;
    color: var(--muted);
  }
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .app {
    font-weight: 700;
    color: var(--accent-text);
  }
  .when {
    align-self: flex-start;
    white-space: nowrap;
  }
  .more {
    margin: 8px 12px 12px;
  }
  .none {
    padding: 16px;
  }
  .conv-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--conv-bg);
  }
  .pick {
    margin: auto;
    text-align: center;
    color: var(--muted);
  }
  @media (max-width: 599px) {
    .list-pane {
      width: 100%;
      border-right: 0;
    }
    .open .list-pane,
    .chats:not(.open) .conv-pane {
      display: none;
    }
  }
</style>
