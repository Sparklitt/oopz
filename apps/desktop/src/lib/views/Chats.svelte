<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import IndexGate from "../components/IndexGate.svelte";
  import { chatName, date, fmt, isGroup, previewText } from "../format";
  import { strings } from "../i18n";
  import { app, current } from "../state.svelte";
  import Conversation from "./Conversation.svelte";

  let t = $derived(strings(app.lang));
  let s = $derived(current()!);
  let filter = $state("");
  let limit = $state(300);
  let list: HTMLUListElement;

  let rows = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const all = [...s.chat_list].sort((a, b) => (b.last_ms ?? 0) - (a.last_ms ?? 0));
    return q ? all.filter((c) => `${c.number} ${chatName(c)} ${c.ending}`.toLowerCase().includes(q)) : all;
  });
  let selected = $derived(s.chat_list.find((c) => c.number === app.chat) ?? null);
  // The one row reachable with Tab; arrows move between rows (a roving tab stop).
  let focusable = $derived(rows.some((r) => r.number === app.chat) ? app.chat : (rows[0]?.number ?? null));

  function pick(n: number) {
    app.chat = n;
    app.jump = null;
  }

  function onKey(e: KeyboardEvent) {
    const buttons = [...list.querySelectorAll<HTMLButtonElement>("button.chat")];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const to = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: buttons.length - 1 }[e.key];
    if (to === undefined || at < 0) return;
    e.preventDefault();
    buttons[Math.max(0, Math.min(buttons.length - 1, to))]?.focus();
  }
</script>

<div class="chats" class:open={selected !== null}>
  <section class="list-pane" aria-label={t.chatsTitle}>
    <h1 class="sr-only">{t.chatsTitle}</h1>
    <div class="filter">
      <Icon name="search" size={16} />
      <input type="search" bind:value={filter} placeholder={t.filterChats} aria-label={t.filterChats} />
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <ul bind:this={list} onkeydown={onKey}>
      {#each rows.slice(0, limit) as c (c.number)}
        <li>
          <button class="chat" tabindex={c.number === focusable ? 0 : -1} aria-current={c.number === app.chat ? "true" : undefined} onclick={() => pick(c.number)}>
            <span class="avatar" class:group={isGroup(c)} aria-hidden="true"><Icon name={c.kind === "broadcast" ? "megaphone" : isGroup(c) ? "users" : "user"} size={16} /></span>
            <span class="words">
              <span class="name">{chatName(c)}</span>
              <span class="sub">{previewText(c) ?? `${fmt(c.messages)} ${t.messages}`}</span>
            </span>
            <span class="when">{date(c.last_ms)}</span>
          </button>
        </li>
      {/each}
    </ul>
    {#if rows.length > limit}<button class="btn small more" onclick={() => (limit += 500)}>{t.showMore}</button>{/if}
    {#if rows.length === 0}<p class="hint empty">{t.noChats}</p>{/if}
  </section>

  <section class="conv-pane" aria-label={selected ? chatName(selected) : t.viewer}>
    {#if selected}
      <IndexGate id={s.id}>
        {#key `${s.id}:${selected.number}:${app.jump?.pos ?? "end"}`}
          <Conversation id={s.id} row={selected} jump={app.jump?.chat === selected.number ? app.jump.pos : null} />
        {/key}
      </IndexGate>
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
      width: 240px;
    }
  }
  /* A narrow window: the list keeps the names; the date is in the chat's header. */
  @media (max-width: 760px) {
    .list-pane {
      width: 210px;
    }
    .when {
      display: none;
    }
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
  .filter:focus-within {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
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
  .when {
    align-self: flex-start;
    white-space: nowrap;
  }
  .more {
    margin: 8px 12px 12px;
  }
  .empty {
    padding: 0 16px;
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
  .conv-pane > :global(.building),
  .conv-pane > :global(.note) {
    margin: 48px 24px;
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
    .when {
      display: block;
    }
  }
</style>
