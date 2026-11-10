<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Icon from "../components/Icon.svelte";
  import IndexGate from "../components/IndexGate.svelte";
  import PageHead from "../components/PageHead.svelte";
  import type { Filter, Hit } from "../backend/types";
  import { chatName, dateTime, fmt, isGroup } from "../format";
  import { strings } from "../i18n";
  import { app, be, current, showMessage } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let s = $derived(current()!);
  let hits = $state<Hit[]>([]);
  let ms = $state(0);
  let searched = $state("");
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLUListElement>();
  let seq = 0;
  let timer: number | undefined;

  const rowOf = (chat: number) => s.chat_list.find((c) => c.number === chat);

  // The filters: a type (some are several WhatsApp codes), who sent it, and a range of days.
  const KINDS: [string, number[]][] = [
    ["text", [0]],
    ["photo", [1]],
    ["video", [3, 13, 81]],
    ["audio", [2]],
    ["document", [9]],
    ["sticker", [20]],
    ["location", [5, 16]],
    ["poll", [46]],
    ["call", [90]],
  ];
  let kind = $state("");
  let who = $state(""); // "", "me", "them", or a sender's 3-character ending
  let after = $state("");
  let before = $state("");
  let senders = $state<string[]>([]);
  let filtered = $derived(kind !== "" || who !== "" || after !== "" || before !== "");
  const kindLabel = (codes: number[]) => codes.map((c) => t.kinds[c]).join(" / ");

  /** A yyyy-mm-dd field as the start of that day, this device's time; `next` gives the start of the day after. */
  function dayStart(value: string, next = false): number | null {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
    return m ? new Date(+m[1], +m[2] - 1, +m[3] + (next ? 1 : 0)).getTime() : null;
  }

  function filter(): Filter {
    return {
      chat: app.searchChat,
      kinds: KINDS.find(([k]) => k === kind)?.[1] ?? [],
      from_me: who === "me" ? true : who === "them" ? false : null,
      sender: who.length === 3 ? who : null,
      after_ms: dayStart(after),
      before_ms: dayStart(before, true),
    };
  }

  // A group's senders, by the end of their id, for the From choice.
  $effect(() => {
    const chat = app.searchChat;
    const row = chat === null ? null : rowOf(chat);
    senders = [];
    untrack(() => {
      if (who.length === 3) who = "";
    });
    if (row && isGroup(row)) {
      be()
        .stats(s.id, chat)
        .then((st) => {
          if (app.searchChat === chat) senders = st.senders.map(([e]) => e);
        })
        .catch(() => {});
    }
  });

  function clearFilters() {
    kind = who = after = before = "";
    search();
  }

  function search() {
    window.clearTimeout(timer);
    timer = window.setTimeout(async () => {
      const mine = ++seq;
      const query = app.query;
      const started = performance.now();
      const found = await be().search(s.id, query, filter()).catch(() => [] as Hit[]);
      if (mine !== seq) return; // A newer search has started.
      ms = Math.round(performance.now() - started);
      hits = found;
      // With no words, a filter alone lists the newest messages that match it.
      searched = query.trim() || (filtered ? "\u0000" : "");
    }, 80);
  }

  /** Splits a snippet into plain and matched parts; never parsed as HTML. */
  const parts = (snippet: string) => snippet.split(/[\u0002\u0003]/).map((text, i) => ({ text, mark: i % 2 === 1 }));

  function onKey(e: KeyboardEvent) {
    const items = [...(list?.querySelectorAll<HTMLButtonElement>("button.hit") ?? [])];
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const to = e.key === "ArrowDown" ? at + 1 : at - 1;
      if (to < 0) input?.focus();
      else items[Math.min(items.length - 1, to)]?.focus();
    }
  }

  onMount(() => {
    if (app.query) search();
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="page" onkeydown={onKey}>
  <PageHead title={t.navSearch} />
  <IndexGate id={s.id}>
    <div class="bar">
      <div class="field-search">
        <Icon name="search" size={20} />
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:this={input} type="search" bind:value={app.query} oninput={search} placeholder={t.searchPlaceholder} aria-label={t.searchPlaceholder} aria-describedby="search-count" autofocus />
      </div>
      <label class="scope">
        <span class="sr-only">{t.exploreScope}</span>
        <select class="select" value={app.searchChat ?? 0} onchange={(e) => { app.searchChat = Number(e.currentTarget.value) || null; search(); }}>
          <option value={0}>{t.allChats}</option>
          {#each s.chat_list as c (c.number)}<option value={c.number}>{c.number} · {chatName(c)}</option>{/each}
        </select>
      </label>
    </div>
    <fieldset class="filters">
      <legend class="sr-only">{t.filters}</legend>
      <label>
        <span>{t.filterKind}</span>
        <select class="select small" bind:value={kind} onchange={search}>
          <option value="">{t.anyKind}</option>
          {#each KINDS as [k, codes] (k)}<option value={k}>{kindLabel(codes)}</option>{/each}
        </select>
      </label>
      <label>
        <span>{t.filterWho}</span>
        <select class="select small" bind:value={who} onchange={search}>
          <option value="">{t.anyone}</option>
          <option value="me">{t.sentByMe}</option>
          <option value="them">{t.receivedByMe}</option>
          {#each senders as e (e)}<option value={e}>••••{e}</option>{/each}
        </select>
      </label>
      <label>
        <span>{t.filterAfter}</span>
        <input class="select small" type="date" placeholder="YYYY-MM-DD" bind:value={after} onchange={search} />
      </label>
      <label>
        <span>{t.filterBefore}</span>
        <input class="select small" type="date" placeholder="YYYY-MM-DD" bind:value={before} onchange={search} />
      </label>
      {#if filtered}<button type="button" class="btn small" onclick={clearFilters}>{t.clearFilters}</button>{/if}
    </fieldset>
    <p class="hint" id="search-count" aria-live="polite">
      {#if searched}
        {#if !app.query.trim()}{t.newestMatches}{/if}
        {fmt(hits.length)} {hits.length === 1 ? t.searchHit : t.searchHits} · {ms} ms {hits.length === 200 ? t.searchMore : ""}
      {:else}
        {t.screenOnly}
      {/if}
    </p>
    {#if hits.length}
      <p class="keys hint" aria-hidden="true">{t.searchKeys}</p>
      <ul class="hits" bind:this={list}>
        {#each hits as hit (hit.id)}
          {@const row = rowOf(hit.chat)}
          <li>
            <button class="hit" onclick={() => showMessage(s.id, hit.id)}>
              <span class="avatar" class:group={row && isGroup(row)} aria-hidden="true"><Icon name={row && isGroup(row) ? "users" : "user"} size={16} /></span>
              <span class="body">
                <span class="meta"><strong>{row ? chatName(row) : hit.chat}</strong><span>{hit.from_me ? t.you : t.them} · {dateTime(hit.ts)}</span></span>
                <span class="snippet">{#each parts(hit.snippet) as p, i (i)}{#if p.mark}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</span>
              </span>
              <span class="open">{t.showInChat}<Icon name="chevron" size={14} /></span>
            </button>
          </li>
        {/each}
      </ul>
    {:else if searched}
      <p class="none">{t.searchNone}</p>
    {/if}
  </IndexGate>
</div>

<style>
  .bar {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .field-search {
    flex: 1 1 320px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-m);
    background: var(--surface);
    color: var(--muted);
    box-shadow: var(--shadow-1);
  }
  .field-search:focus-within {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
  .field-search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    min-height: 46px;
    font-size: 17px;
    color: var(--text);
    outline: none;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 8px 12px;
    border: 0;
    padding: 0;
    margin: 12px 0 0;
  }
  .filters label {
    display: grid;
    gap: 3px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .scope {
    flex: 0 1 260px;
    display: flex;
  }
  .scope select {
    width: 100%;
  }
  .keys {
    text-align: right;
    margin-top: -6px;
  }
  .hits {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 6px;
  }
  .hit {
    width: 100%;
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-m);
    background: var(--surface);
    color: var(--text);
    text-align: left;
  }
  .hit:hover {
    border-color: var(--line-strong);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .meta {
    display: flex;
    gap: 10px;
    justify-content: space-between;
    flex-wrap: wrap;
  }
  .meta span {
    color: var(--muted);
    font-size: 12.5px;
  }
  .snippet {
    overflow-wrap: anywhere;
  }
  .open {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 12.5px;
    color: var(--accent-text);
    opacity: 0;
    white-space: nowrap;
    align-self: center;
  }
  .hit:hover .open,
  .hit:focus .open {
    opacity: 1;
  }
  .none {
    color: var(--muted);
    text-align: center;
    margin-top: 40px;
  }
</style>
