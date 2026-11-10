<script lang="ts">
  // The library's sources: what's in it, from which app, the owner when an export doesn't say, and adding or
  // removing one. Where the library lives (wiped on close, or kept) is said up front; Settings changes it.
  import { onMount } from "svelte";
  import { spans } from "../../lib/notes";
  import ErrorNote from "../../lib/components/ErrorNote.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import PageHead from "../../lib/components/PageHead.svelte";
  import type { PersonRow, SourceRow } from "../../lib/backend/types";
  import { date, fmt, mb } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { appName, lib, personName, refreshLibrary, removeSource } from "../../lib/library.svelte";
  import { app, be, go, run } from "../../lib/state.svelte";
  import AddSource from "./AddSource.svelte";

  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let adding = $state(false);
  let removing = $state<number | null>(null);
  let asking = $state<number | null>(null);
  let people = $state<PersonRow[]>([]);
  let me = $state(0);

  onMount(() => {
    void run(refreshLibrary);
  });

  async function ask(s: SourceRow) {
    asking = s.id;
    me = 0;
    const list = await run(() => be().libPeople(s.id));
    people = (list ?? []).sort((a, b) => b.messages - a.messages);
  }

  async function setMe() {
    if (!me) return;
    await run(() => be().libSetMe(me));
    asking = null;
    await refreshLibrary();
  }
</script>

<div class="page">
  <PageHead title={l.title} sub={l.lead}>
    {#snippet actions()}
      <button class="btn primary" onclick={() => (adding = true)}><Icon name="plus" />{l.add}</button>
    {/snippet}
  </PageHead>

  {#if lib.info}
    <div class="note" class:ok={lib.info.kept}>
      <Icon name={lib.info.kept ? "lock" : "info"} />
      <span>
        {lib.info.kept ? l.keptNote : l.notKeptNote}
        <button class="link" onclick={() => go("settings")}>{l.changeInSettings}</button>
        {#if lib.info.sources}<span class="muted"> · {fmt(lib.info.messages)} {(lib.info.messages) === 1 ? t.message : t.messages} · {mb(lib.info.bytes)}</span>{/if}
      </span>
    </div>
  {/if}
  <ErrorNote error={app.error} words={l.errors} />

  {#if lib.sources.length === 0}
    <div class="empty">
      <Icon name="folder" size={40} />
      <h2 class="h3">{l.emptyTitle}</h2>
      <p class="muted">{l.emptyLead}</p>
      <ul class="apps">
        {#each lib.adapters as a (a.id)}<li><strong>{appName(a.app)}</strong> <span class="muted">{l.adapterNames[a.id] ?? a.id}</span></li>{/each}
      </ul>
      <button class="btn primary" onclick={() => (adding = true)}><Icon name="plus" />{l.add}</button>
    </div>
  {:else}
    <ul class="sources">
      {#each lib.sources as s (s.id)}
        <li class="panel source">
          <div class="top">
            <span class="badge">{appName(s.app)}</span>
            <strong class="name">{s.name}</strong>
            <span class="muted">{l.adapterNames[s.adapter] ?? s.adapter}</span>
          </div>
          <p class="counts">
            {fmt(s.conversations)} {l.conversations} · {fmt(s.people)} {l.people} · {fmt(s.messages)} {(s.messages) === 1 ? t.message : t.messages}
            · {date(s.first_ms)} – {date(s.last_ms)}
          </p>
          <p class="muted small">{l.added} {date(s.added_ms)}{#if s.wall_clock} · {l.wallClock}{/if}{#if !s.has_media} · {l.noMedia}{/if}</p>
          {#if s.notes.length}
            <ul class="notes">{#each s.notes as n, i (i)}<li>{#each spans(n) as sp, j (j)}{#if sp.code}<code>{sp.text}</code>{:else}{sp.text}{/if}{/each}</li>{/each}</ul>
          {/if}
          {#if !s.owner_known}
            <div class="note warn">
              <Icon name="user" />
              <div class="grow">
                <p>{l.ownerUnknown}</p>
                {#if asking === s.id}
                  <div class="row">
                    <label class="sr-only" for="me-{s.id}">{l.whoAreYou}</label>
                    <select id="me-{s.id}" class="select" bind:value={me}>
                      <option value={0}>{l.pickYourself}</option>
                      {#each people as p (p.id)}<option value={p.id}>{personName(p.name ?? p.username ?? p.phone, p.id)} · {fmt(p.messages)} {(p.messages) === 1 ? t.message : t.messages}</option>{/each}
                    </select>
                    <button class="btn primary small" disabled={!me || app.busy} onclick={setMe}>{l.thatsMe}</button>
                    <button class="btn small" onclick={() => (asking = null)}>{t.cancel}</button>
                  </div>
                {:else}
                  <button class="btn small" onclick={() => ask(s)}>{l.whoAreYou}</button>
                {/if}
              </div>
            </div>
          {/if}
          <div class="actions">
            <button class="btn small" onclick={() => { lib.chatSource = s.id; lib.conv = null; go("lchats"); }}><Icon name="chats" size={16} />{l.openChats}</button>
            {#if removing === s.id}
              <span class="confirm" role="alert">{l.removeConfirm}</span>
              <button class="btn small danger" disabled={app.busy} onclick={async () => { await removeSource(s.id); removing = null; }}>{l.remove}</button>
              <button class="btn small" onclick={() => (removing = null)}>{t.cancel}</button>
            {:else}
              <button class="btn small" onclick={() => (removing = s.id)}><Icon name="ban" size={16} />{l.remove}</button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
  <p class="hint local"><Icon name="lock" size={14} />{l.staysHere}</p>
</div>

{#if adding}<AddSource onclose={() => (adding = false)} />{/if}

<style>
  .note {
    margin-bottom: 16px;
  }
  .empty {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 6px;
    padding: 32px 16px;
    color: var(--muted);
  }
  .empty p {
    max-width: 52ch;
    margin: 0;
  }
  .apps {
    list-style: none;
    padding: 0;
    margin: 8px 0 14px;
    display: grid;
    gap: 4px;
    color: var(--text);
  }
  .sources {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 12px;
  }
  .source p {
    margin: 4px 0;
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    align-items: center;
  }
  .name {
    overflow-wrap: anywhere;
  }
  .small {
    font-size: 12.5px;
  }
  .notes {
    margin: 6px 0;
    padding-left: 18px;
    font-size: 13px;
    color: var(--muted);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .grow p {
    margin: 0 0 6px;
  }
  .row,
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .row select {
    max-width: 100%;
  }
  .actions {
    margin-top: 10px;
  }
  .confirm {
    color: var(--danger);
    font-size: 13px;
  }
  .danger {
    color: var(--danger);
  }
  .local {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 18px;
  }
</style>
