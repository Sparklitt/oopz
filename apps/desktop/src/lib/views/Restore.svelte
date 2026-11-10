<script lang="ts">
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon from "../components/Icon.svelte";
  import KeyBoxes from "../components/KeyBoxes.svelte";
  import PageHead from "../components/PageHead.svelte";
  import PairPicker from "../components/PairPicker.svelte";
  import PathRow from "../components/PathRow.svelte";
  import SaveButton from "../components/SaveButton.svelte";
  import { date, fmt } from "../format";
  import { strings } from "../i18n";
  import { app, be, go, onPhone, run } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let m = app.merge;
  // The key stays in this screen only, and is emptied when sent.
  let key = $state("");
  let page: HTMLDivElement;
  let mi = $derived(app.missing);
  let o = $derived(app.outcome);
  let pairOk = $derived(!!m.sourceChat && !!m.targetChat && m.source !== m.target);
  // On Android the result is made in the private folder and saved from there; no Media folder can be picked.
  let out = $derived(onPhone() ? (app.privateDir ?? "") : m.out);
  let ready = $derived(!app.busy && !!out && pairOk);
  const span = (first: number | null, last: number | null) => (first ? `${date(first)} – ${date(last)}` : undefined);

  async function folder(set: (v: string) => void) {
    const p = await be().pickFolder();
    if (p) set(p);
  }

  async function compare() {
    const found = await run(() => be().missing(m.source, m.sourceChat, m.target, m.targetChat));
    if (found) app.missing = found;
  }

  async function merge() {
    const targetKey = key;
    key = "";
    const outcome = await run(() =>
      be().mergeChat({ source: m.source, source_chat: m.sourceChat, target: m.target, target_chat: m.targetChat, target_key: targetKey, out_parent: out, media: onPhone() ? null : m.media || null }),
    );
    if (outcome) {
      app.outcome = outcome;
      page.closest(".content")?.scrollTo(0, 0); // The outcome is shown above the form.
    }
  }
</script>

<div class="page" bind:this={page}>
  <PageHead title={t.mergeTitle} sub={t.mergeLead} />
  {#if app.summaries.length < 2}
    <div class="note"><Icon name="info" /><span>{t.mergeNeedsTwo}</span></div>
    <button class="btn" onclick={() => go("add")}><Icon name="plus" />{t.addBackup}</button>
  {:else}
    {#if o}
      <section class="done panel" aria-live="polite">
        <div class="done-top">
          <span class="big-check"><Icon name="check" size={28} /></span>
          <div>
            <h2 class="h2">{t.mergeDone}</h2>
            <p class="muted">{fmt(o.added)} {t.added} · {fmt(o.already_there)} {t.alreadyThere}</p>
          </div>
        </div>
        <div class="tiles">
          <div class="tile"><span class="num">{fmt(o.added)}</span><span class="lbl">{t.added}</span></div>
          <div class="tile"><span class="num">{fmt(o.already_there)}</span><span class="lbl">{t.alreadyThere}</span></div>
          {#if o.media_copied !== null}
            <div class="tile"><span class="num">{fmt(o.media_copied)}</span><span class="lbl">{t.mediaCopied}</span><span class="sub">{fmt(o.media_missing ?? 0)} {t.mediaMissing}</span></div>
          {/if}
        </div>
        {#if o.original_untouched}<div class="note ok"><Icon name="shield" /><span>{t.untouched}</span></div>{/if}
        {#if onPhone()}
          <SaveButton from="{o.folder}/Databases/msgstore.db.crypt15" name="msgstore.db.crypt15" />
        {:else}
          <PathRow folder={o.folder} shown={o.backup} />
        {/if}
        <details class="checks">
          <summary>{t.checksDone(o.checks.length)}</summary>
          <ul>{#each o.checks as c, i (i)}<li class="mono">{c}</li>{/each}</ul>
        </details>
        <button class="btn primary" onclick={() => go("phone")}><Icon name="phone" />{t.toPhone2}</button>
      </section>
    {/if}

    <ol class="steps">
      <li>
        <h2 class="h2"><span class="n">1</span>{t.mergeWhich}</h2>
        <PairPicker />
        <div class="row-buttons">
          <button class="btn" disabled={app.busy || !pairOk} onclick={compare}><Icon name="search" />{t.compare}</button>
        </div>
        {#if mi}
          <div class="tiles">
            <div class="tile"><span class="num">{fmt(mi.only_a)}</span><span class="lbl">{t.onlySource}</span><span class="sub">{span(mi.only_a_first_ms, mi.only_a_last_ms)}</span></div>
            <div class="tile"><span class="num">{fmt(mi.only_b)}</span><span class="lbl">{t.onlyTarget}</span><span class="sub">{span(mi.only_b_first_ms, mi.only_b_last_ms)}</span></div>
            <div class="tile"><span class="num">{fmt(mi.both)}</span><span class="lbl">{t.inBoth}</span></div>
          </div>
        {/if}
      </li>
      <li>
        <h2 class="h2"><span class="n">2</span>{t.mergeWhere}</h2>
        <div class="field">
          <span class="label" id="target-key-label">{t.targetKey}</span>
          <KeyBoxes bind:value={key} id="target-key" label={t.targetKey} describedby="target-key-hint" compact disabled={app.busy} />
          <p class="hint" id="target-key-hint">{t.targetKeyHint}</p>
        </div>
        {#if !onPhone()}
        <div class="field">
          <span class="label" id="out-label">{t.outFolder}</span>
          <div class="folder-row">
            <div class="path" class:empty={!m.out}><Icon name="folder" /><code>{m.out || t.noFolder}</code></div>
            <button class="btn" aria-describedby="out-label" onclick={() => folder((v) => (m.out = v))}><Icon name="folder" />{t.chooseFolder}</button>
          </div>
          <p class="hint">{t.outFolderHint}</p>
        </div>
        <div class="field">
          <span class="label" id="media-label">{t.mediaFolder}</span>
          <div class="folder-row">
            <div class="path" class:empty={!m.media}><Icon name="folder" /><code>{m.media || t.none}</code></div>
            <button class="btn" aria-describedby="media-label" onclick={() => folder((v) => (m.media = v))}><Icon name="folder" />{t.chooseFolder}</button>
            {#if m.media}<button class="btn icon-only" aria-label={t.clear} title={t.clear} onclick={() => (m.media = "")}><Icon name="close" /></button>{/if}
          </div>
          <p class="hint">{t.mediaFolderHint}</p>
        </div>
        {/if}
      </li>
      <li>
        <h2 class="h2"><span class="n">3</span>{t.mergeRun}</h2>
        <ErrorNote error={app.error} />
        {#if app.busy}
          <div class="progress" role="status"><p>{t.merging}</p><progress aria-label={t.merging}></progress></div>
        {/if}
        <button class="btn" class:primary={!o} disabled={!ready} onclick={merge}><Icon name="restore" />{t.runMerge}</button>
      </li>
    </ol>
  {/if}
</div>

<style>
  .steps {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 28px;
  }
  .n {
    display: inline-grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 13px;
    margin-right: 10px;
    vertical-align: 2px;
  }
  .row-buttons {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 12px;
    margin: 14px 0;
  }
  .field {
    display: grid;
    gap: 6px;
    margin-bottom: 18px;
    max-width: 620px;
  }
  .label {
    font-weight: 600;
  }
  .folder-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .path {
    flex: 1 1 240px;
    min-width: 0;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--surface-2);
  }
  .path code {
    overflow-wrap: anywhere;
  }
  .path.empty code {
    color: var(--muted);
    font-family: inherit;
  }
  .progress progress {
    width: 100%;
  }
  .done {
    margin-bottom: 28px;
    display: grid;
    gap: 12px;
  }
  .done-top {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .done-top p {
    margin: 0;
  }
  .big-check {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--ok);
    color: #fff;
    flex: none;
  }
  .checks summary {
    cursor: pointer;
    color: var(--muted);
  }
  .done > .btn {
    justify-self: start;
  }
</style>
