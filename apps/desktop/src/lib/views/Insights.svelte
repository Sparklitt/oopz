<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import IndexGate from "../components/IndexGate.svelte";
  import PageHead from "../components/PageHead.svelte";
  import Bars from "../components/Bars.svelte";
  import Heatmap from "../components/Heatmap.svelte";
  import type { Stats } from "../backend/types";
  import { chatName, date, fmt, mb, year } from "../format";
  import { strings } from "../i18n";
  import { app, be, current, openExport } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let s = $derived(current()!);
  let stats = $state<Stats | null>(null);
  let ready = $derived(!!app.index[s.id]?.built);

  $effect(() => {
    if (!ready) return;
    const chat = app.searchChat;
    let live = true;
    be()
      .stats(s.id, chat)
      .then((st) => live && (stats = st))
      .catch(() => live && (stats = null));
    return () => (live = false);
  });

  let sentShare = $derived(stats && stats.messages ? stats.from_me / stats.messages : 0);
  let topKind = $derived(Math.max(1, ...(stats?.by_kind.map(([, n]) => n) ?? [1])));
  let topSender = $derived(Math.max(1, ...(stats?.senders.map(([, n]) => n) ?? [1])));
  const R = 15.9;
  const C = 2 * Math.PI * R;
</script>

<div class="page">
  <PageHead title={t.navInsights}>
    {#snippet actions()}
      <label class="scope">
        <span class="sr-only">{t.exploreScope}</span>
        <select class="select" value={app.searchChat ?? 0} onchange={(e) => (app.searchChat = Number(e.currentTarget.value) || null)}>
          <option value={0}>{t.wholeBackup}</option>
          {#each s.chat_list as c (c.number)}<option value={c.number}>{c.number} · {chatName(c)}</option>{/each}
        </select>
      </label>
      <button class="btn" disabled={!stats} onclick={() => openExport(app.searchChat)}><Icon name="download" />{t.exportShort}</button>
    {/snippet}
  </PageHead>
  <IndexGate id={s.id}>
    {#if stats}
      <div class="tiles">
        <div class="tile"><Icon name="chats" /><span class="num">{fmt(stats.messages)}</span><span class="lbl">{t.messages}</span></div>
        <div class="tile ring-tile">
          <svg class="ring" viewBox="0 0 36 36" aria-hidden="true">
            <circle cx="18" cy="18" r={R} class="track" />
            <circle cx="18" cy="18" r={R} class="part" stroke-dasharray="{C * sentShare} {C}" />
          </svg>
          <span class="num">{Math.round(sentShare * 100)}%</span>
          <span class="lbl">{t.fromMe}</span>
          <span class="sub">{fmt(stats.from_me)} {t.fromMe}, {fmt(stats.from_others)} {t.fromOthers}</span>
        </div>
        <div class="tile"><Icon name="mic" /><span class="num">{fmt(Math.round(stats.audio_seconds / 60))}</span><span class="lbl">{t.audioMinutes}</span></div>
        <div class="tile">
          <Icon name="calendar" /><span class="num">{year(stats.first_ms)} – {year(stats.last_ms)}</span><span class="lbl">{t.dates}</span>
          <span class="sub">{date(stats.first_ms)} – {date(stats.last_ms)}</span>
        </div>
      </div>

      <section class="panel">
        <Bars values={stats.by_month} label={t.byMonth} />
      </section>
      {#if stats.by_day_hour.length === 7}
        <section class="panel"><Heatmap values={stats.by_day_hour} /></section>
      {/if}
      <div class="cols">
        <section class="panel">
          <Bars values={stats.by_hour.map((n, h): [string, number] => [`${h}:00`, n])} label={t.byHour} />
        </section>
        <section class="panel">
          <h2 class="h3">{t.byKind}</h2>
          <table class="table compact">
            <thead><tr>{#each t.kindTable as c, i (i)}<th class:n={i > 0} scope="col">{c}</th>{/each}</tr></thead>
            <tbody>
              {#each stats.by_kind as [k, n, bytes] (k)}
                <tr>
                  <td><span class="kbar" style:--w="{(n / topKind) * 100}%">{t.kinds[k] ?? `#${k}`}</span></td>
                  <td class="n">{fmt(n)}</td>
                  <td class="n">{bytes ? mb(bytes) : "–"}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>
      </div>
      {#if stats.senders.length > 0}
        <section class="panel">
          <h2 class="h3">{t.senders}</h2>
          <table class="table compact">
            <tbody>
              {#each stats.senders as [ending, n] (ending)}
                <tr><td><span class="kbar" style:--w="{(n / topSender) * 100}%">••••{ending}</span></td><td class="n">{fmt(n)}</td></tr>
              {/each}
            </tbody>
          </table>
        </section>
      {/if}
    {:else}
      <p class="hint">…</p>
    {/if}
  </IndexGate>
</div>

<style>
  .scope select {
    max-width: 260px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 12px;
    margin-bottom: 16px;
  }
  .ring {
    width: 26px;
    height: 26px;
    transform: rotate(-90deg);
  }
  .ring circle {
    fill: none;
    stroke-width: 5;
  }
  .ring .track {
    stroke: var(--surface-3);
  }
  .ring .part {
    stroke: var(--accent);
  }
  .panel {
    margin-bottom: 16px;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 16px;
  }
  .cols .panel {
    margin-bottom: 0;
  }
  .cols + .panel {
    margin-top: 16px;
  }
  .kbar {
    display: inline-block;
    min-width: 100%;
    padding: 2px 6px;
    border-radius: 4px;
    background: linear-gradient(90deg, var(--accent-soft) var(--w), transparent var(--w));
  }
</style>
