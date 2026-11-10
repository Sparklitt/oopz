<script lang="ts">
  // Figures about the library, or the part of it the filter picks: counts, dates, months, hours, types, apps and the
  // people written with most.
  import Bars from "../../lib/components/Bars.svelte";
  import Heatmap from "../../lib/components/Heatmap.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import LibFilterFields from "../../lib/components/LibFilterFields.svelte";
  import PageHead from "../../lib/components/PageHead.svelte";
  import type { LibStats } from "../../lib/backend/types";
  import { date, fmt, mb, year } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { appName, emptyFilter, everything, lib, openLibExport, personName } from "../../lib/library.svelte";
  import { app, be } from "../../lib/state.svelte";

  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let filter = $state(emptyFilter());
  let stats = $state<LibStats | null>(null);
  let bump = $state(0);

  $effect(() => {
    void bump;
    void lib.version;
    const f = $state.snapshot(filter);
    let live = true;
    be()
      .libStats(f)
      .then((s) => live && (stats = s))
      .catch(() => live && (stats = null));
    return () => (live = false);
  });

  let share = $derived(stats && stats.messages ? stats.from_me / stats.messages : 0);
  let topKind = $derived(Math.max(1, ...(stats?.by_kind.map(([, n]) => n) ?? [1])));
  let topPerson = $derived(Math.max(1, ...(stats?.by_person.map(([, , n]) => n) ?? [1])));

  function exportThese() {
    const f = $state.snapshot(filter);
    const empty = f.apps.length + f.sources.length + f.contacts.length + f.kinds.length === 0 && f.from_me === null && f.after_ms === null && f.before_ms === null;
    openLibExport({ filter: empty ? everything() : f, add: [], remove: [] }, l.insightsTitle);
  }
</script>

<div class="page">
  <PageHead title={l.insightsTitle} sub={l.insightsLead}>
    {#snippet actions()}
      <button class="btn" disabled={!stats?.messages} onclick={exportThese}><Icon name="download" />{t.exportShort}</button>
    {/snippet}
  </PageHead>
  <LibFilterFields bind:filter onchange={() => bump++} />
  {#if stats && stats.messages}
    <div class="tiles">
      <div class="tile"><Icon name="chats" /><span class="num">{fmt(stats.messages)}</span><span class="lbl">{t.messages}</span></div>
      <div class="tile"><Icon name="arrow" /><span class="num">{Math.round(share * 100)}%</span><span class="lbl">{t.fromMe}</span><span class="sub">{fmt(stats.from_me)} {t.fromMe}, {fmt(stats.messages - stats.from_me)} {t.fromOthers}</span></div>
      <div class="tile"><Icon name="mic" /><span class="num">{fmt(Math.round(stats.voice_secs / 60))}</span><span class="lbl">{t.audioMinutes}</span><span class="sub">{mb(stats.media_bytes)} {l.ofMedia}</span></div>
      <div class="tile"><Icon name="calendar" /><span class="num">{year(stats.first_ms)} – {year(stats.last_ms)}</span><span class="lbl">{t.dates}</span><span class="sub">{date(stats.first_ms)} – {date(stats.last_ms)}</span></div>
    </div>
    <section class="panel"><Bars values={stats.by_month} label={t.byMonth} /></section>
    {#if stats.by_day_hour.length === 7}<section class="panel"><Heatmap values={stats.by_day_hour} /></section>{/if}
    <div class="cols">
      <section class="panel"><Bars values={stats.by_hour.map((n, h): [string, number] => [`${h}:00`, n])} label={t.byHour} /></section>
      <section class="panel">
        <h2 class="h3">{t.byKind}</h2>
        <table class="table compact">
          <tbody>
            {#each stats.by_kind as [k, n, bytes] (k)}
              <tr><td><span class="kbar" style:--w="{(n / topKind) * 100}%">{l.kinds[k]}</span></td><td class="n">{fmt(n)}</td><td class="n">{bytes ? mb(bytes) : "–"}</td></tr>
            {/each}
          </tbody>
        </table>
      </section>
      {#if stats.by_app.length > 1}
        <section class="panel">
          <h2 class="h3">{l.byApp}</h2>
          <table class="table compact">
            <tbody>{#each stats.by_app as [a, n] (a)}<tr><td>{appName(a)}</td><td class="n">{fmt(n)}</td></tr>{/each}</tbody>
          </table>
        </section>
      {/if}
      {#if stats.by_person.length}
        <section class="panel">
          <h2 class="h3">{l.byPerson}</h2>
          <table class="table compact">
            <tbody>
              {#each stats.by_person.slice(0, 15) as [id, name, n], i (i)}
                <tr><td><span class="kbar" style:--w="{(n / topPerson) * 100}%">{personName(name, id)}</span></td><td class="n">{fmt(n)}</td></tr>
              {/each}
            </tbody>
          </table>
        </section>
      {/if}
    </div>
  {:else if stats}
    <p class="hint">{lib.sources.length ? t.searchNone : l.emptyTitle}</p>
  {:else}
    <p class="hint">…</p>
  {/if}
</div>

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 12px;
    margin-bottom: 16px;
  }
  .panel {
    margin-bottom: 16px;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 16px;
  }
  .cols .panel {
    margin-bottom: 0;
  }
  .kbar {
    display: inline-block;
    min-width: 100%;
    padding: 2px 6px;
    border-radius: 4px;
    background: linear-gradient(90deg, var(--accent-soft) var(--w), transparent var(--w));
  }
</style>
