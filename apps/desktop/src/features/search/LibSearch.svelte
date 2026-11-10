<script lang="ts">
  // Search across the whole library (or one conversation): words with accents and case ignored, narrowed by the
  // filter fields. Matches are marked from the snippet's \u0002 and \u0003, as text, never as HTML.
  import { onMount } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import LibFilterFields from "../../lib/components/LibFilterFields.svelte";
  import PageHead from "../../lib/components/PageHead.svelte";
  import type { ConversationRow, LibHit } from "../../lib/backend/types";
  import { dateTime, fmt } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { appName, convTitle, emptyFilter, lib, personName, showLibMessage } from "../../lib/library.svelte";
  import { app, be } from "../../lib/state.svelte";

  const LIMIT = 200;
  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let filter = $state(emptyFilter());
  let hits = $state<LibHit[]>([]);
  let convs = $state<ConversationRow[]>([]);
  let searched = $state(false);
  let ms = $state(0);
  let seq = 0;
  let timer: number | undefined;

  let byId = $derived(new Map(convs.map((c) => [c.id, c])));
  let narrowed = $derived(filter.apps.length + filter.sources.length + filter.contacts.length + filter.kinds.length > 0 || filter.from_me !== null || filter.after_ms !== null || filter.before_ms !== null);

  function search() {
    window.clearTimeout(timer);
    timer = window.setTimeout(async () => {
      const mine = ++seq;
      const f = { ...filter, words: lib.query, conversations: lib.searchConv === null ? [] : [lib.searchConv] };
      if (!f.words.trim() && !narrowed && lib.searchConv === null) {
        hits = [];
        searched = false;
        return;
      }
      const started = performance.now();
      const found = await be().libSearch(f, LIMIT).catch(() => [] as LibHit[]);
      if (mine !== seq) return;
      ms = Math.round(performance.now() - started);
      hits = found;
      searched = true;
    }, 120);
  }

  const parts = (snippet: string) => snippet.split(/[\u0002\u0003]/).map((text, i) => ({ text, mark: i % 2 === 1 }));

  onMount(() => {
    be()
      .libConversations(emptyFilter())
      .then((c) => (convs = c))
      .catch(() => {});
    search();
  });
</script>

<div class="page">
  <PageHead title={l.searchTitle} sub={l.searchLead} />
  <div class="field-search">
    <Icon name="search" size={20} />
    <!-- svelte-ignore a11y_autofocus -->
    <input type="search" bind:value={lib.query} oninput={search} placeholder={l.searchPlaceholder} aria-label={l.searchPlaceholder} aria-describedby="lib-search-count" autofocus />
  </div>
  {#if lib.searchConv !== null}
    {@const c = byId.get(lib.searchConv)}
    <p class="scope">{l.onlyIn} <strong>{c ? convTitle(c) : `#${lib.searchConv}`}</strong> <button class="btn small" onclick={() => { lib.searchConv = null; search(); }}>{l.searchEverywhere}</button></p>
  {/if}
  <LibFilterFields bind:filter onchange={search} />
  <p class="hint" id="lib-search-count" aria-live="polite">
    {#if searched}
      {fmt(hits.length)} {hits.length === 1 ? t.searchHit : t.searchHits} · {ms} ms {hits.length === LIMIT ? t.searchMore : ""}
    {:else}
      {t.screenOnly}
    {/if}
  </p>
  {#if hits.length}
    <ul class="hits">
      {#each hits as hit (hit.id)}
        {@const c = byId.get(hit.conversation)}
        <li>
          <button class="hit" onclick={() => showLibMessage(hit.id)}>
            <span class="body">
              <span class="meta">
                <strong><span class="app">{appName(hit.app)}</span> {c ? convTitle(c) : `#${hit.conversation}`}</strong>
                <span>{hit.from_me ? t.you : personName(hit.sender_name, null)} · {hit.time_ms === null ? "" : dateTime(hit.time_ms)}</span>
              </span>
              <span class="snippet">{#each parts(hit.snippet) as p, i (i)}{#if p.mark}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</span>
            </span>
            <Icon name="chevron" size={14} />
          </button>
        </li>
      {/each}
    </ul>
  {:else if searched}
    <p class="none">{t.searchNone}</p>
  {/if}
</div>

<style>
  .field-search {
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
  .scope {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
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
    align-items: center;
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
  .app {
    color: var(--accent-text);
  }
  .snippet {
    overflow-wrap: anywhere;
    unicode-bidi: plaintext;
  }
  .none {
    color: var(--muted);
    text-align: center;
    margin-top: 40px;
  }
</style>
