<script lang="ts">
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import PairPicker from "../components/PairPicker.svelte";
  import { date, fmt } from "../format";
  import { strings } from "../i18n";
  import { app, be, go, run } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let m = app.merge;
  let ready = $derived(!app.busy && !!m.sourceChat && !!m.targetChat && m.source !== m.target);
  let mi = $derived(app.missing);

  async function compare() {
    const found = await run(() => be().missing(m.source, m.sourceChat, m.target, m.targetChat));
    if (found) app.missing = found;
  }
  const span = (first: number | null, last: number | null) => (first ? `${date(first)} – ${date(last)}` : "");
</script>

<div class="page">
  <PageHead title={t.compareTitle} sub={t.compareLead} />
  {#if app.summaries.length < 2}
    <div class="note"><Icon name="info" /><span>{t.mergeNeedsTwo}</span></div>
    <button class="btn" onclick={() => go("add")}><Icon name="plus" />{t.addBackup}</button>
  {:else}
    <PairPicker />
    <div class="row-buttons">
      <button class="btn primary" disabled={!ready} onclick={compare}><Icon name="compare" />{t.compare}</button>
    </div>
    <ErrorNote error={app.error} />
    {#if app.busy}<progress aria-label={t.compare}></progress>{/if}
    {#if mi}
      <section aria-live="polite">
        {#if mi.only_a > 0}
          <div class="note warn gap">
            <Icon name="alert" />
            <span><strong>{fmt(mi.only_a)}</strong> {t.onlySource}{#if mi.only_a_first_ms}<br /><span class="muted">{span(mi.only_a_first_ms, mi.only_a_last_ms)}</span>{/if}</span>
          </div>
        {/if}
        <div class="tiles">
          <div class="tile"><span class="num">{fmt(mi.both)}</span><span class="lbl">{t.inBoth}</span></div>
          <div class="tile"><span class="num">{fmt(mi.only_a)}</span><span class="lbl">{t.onlySource}</span><span class="sub">{span(mi.only_a_first_ms, mi.only_a_last_ms)}</span></div>
          <div class="tile"><span class="num">{fmt(mi.only_b)}</span><span class="lbl">{t.onlyTarget}</span><span class="sub">{span(mi.only_b_first_ms, mi.only_b_last_ms)}</span></div>
        </div>
        <button class="btn primary" disabled={mi.only_a === 0} onclick={() => go("restore")}><Icon name="restore" />{t.restoreMissing}</button>
      </section>
    {/if}
  {/if}
</div>

<style>
  .row-buttons {
    display: flex;
    gap: 8px;
    margin: 16px 0;
  }
  progress {
    width: 100%;
  }
  .gap {
    margin-top: 8px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    margin: 16px 0;
  }
</style>
