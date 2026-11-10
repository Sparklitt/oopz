<script lang="ts">
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon from "../components/Icon.svelte";
  import IndexGate from "../components/IndexGate.svelte";
  import PathRow from "../components/PathRow.svelte";
  import Sheet from "../components/Sheet.svelte";
  import { chatName } from "../format";
  import { strings } from "../i18n";
  import { app, be, current, run } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let s = $derived(current()!);
  let chatRow = $derived(app.exportChat === null ? null : (s.chat_list.find((c) => c.number === app.exportChat) ?? null));
  let withChat = $state(app.exportChat !== null);
  let saved = $state("");

  async function save() {
    const dir = await be().pickFolder();
    if (!dir) return;
    const chat = withChat && chatRow ? chatRow.number : null;
    const heading = chat !== null && chatRow ? `${chatRow.number} · ${chatName(chatRow)}` : t.wholeBackup;
    const path = await run(() =>
      be().exportPage(
        s.id,
        chat,
        {
          lang: app.lang, heading, messages: t.messages, sent: t.fromMe, received: t.fromOthers, dates: t.dates,
          by_month: t.byMonth, by_hour: t.byHour, by_kind: t.byKind, audio_minutes: t.audioMinutes, senders: t.senders,
          you: t.you, note: t.exportNote, kinds: t.kinds,
        },
        dir,
      ),
    );
    if (path) saved = path;
  }

  function close() {
    app.error = null;
    app.sheet = null;
  }
</script>

<Sheet label={t.exportTitle} size="medium" onclose={close}>
  <h1 class="sheet-title">{t.exportTitle}</h1>
  <p class="muted">{t.exportLead}</p>
  <IndexGate id={s.id}>
    <fieldset class="choices">
      <legend class="h3">{t.exportWhat}</legend>
      <label class="check"><input type="radio" name="export" checked={!withChat} onchange={() => (withChat = false)} /><span>{t.exportWhole}</span></label>
      {#if chatRow}
        <label class="check"><input type="radio" name="export" checked={withChat} onchange={() => (withChat = true)} /><span>{t.exportThisChat} ({chatName(chatRow)})</span></label>
      {/if}
    </fieldset>
    <div class="note warn"><Icon name="unlock" /><span>{t.exportNote}</span></div>
    <ErrorNote error={app.error} />
    {#if saved}
      <div class="note ok" role="status"><Icon name="check" /><span>{t.exportSaved}:</span></div>
      <PathRow folder={saved.replace(/[\\/][^\\/]*$/, "")} shown={saved} />
    {/if}
  </IndexGate>
  {#snippet footer()}
    <button class="btn" onclick={close}>{saved ? t.close : t.cancel}</button>
    <button class="btn primary" disabled={app.busy || !app.index[s.id]?.built} onclick={save}><Icon name="download" />{t.exportButton}</button>
  {/snippet}
</Sheet>

<style>
  .choices {
    border: 0;
    padding: 0;
    margin: 16px 0;
    display: grid;
    gap: 8px;
  }
  .choices legend {
    padding: 0;
    margin-bottom: 6px;
  }
</style>
