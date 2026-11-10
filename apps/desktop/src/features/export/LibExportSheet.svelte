<script lang="ts">
  // Writes a library selection to a new file: HTML, PDF, JSON, CSV, TXT, vCard or a zip of its media. On a computer
  // into a folder the person picks; on a phone into the app's private folder, then saved where they pick. The file
  // is not encrypted: said before it's written.
  import ErrorNote from "../../lib/components/ErrorNote.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import PathRow from "../../lib/components/PathRow.svelte";
  import Ring from "../../lib/components/Ring.svelte";
  import SaveButton from "../../lib/components/SaveButton.svelte";
  import Sheet from "../../lib/components/Sheet.svelte";
  import StepLine from "../../lib/components/StepLine.svelte";
  import type { AppError, ExportFormat, Exported, Order, Step } from "../../lib/backend/types";
  import { fmt, mb } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { lib } from "../../lib/library.svelte";
  import { app, be, cancelWork, isCancelled, onPhone, toError } from "../../lib/state.svelte";

  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let job = $derived(lib.exporting);
  let format = $state<ExportFormat>("html");
  let order = $state<Order>("conversation");
  let title = $state(lib.exporting?.title ?? "");
  let skipSystem = $state(false);
  let page = $state<"a4" | "letter">(app.lang === "en" && /US|CA/.test(navigator.language) ? "letter" : "a4");
  let thumbs = $state(true);
  let cover = $state(true);
  let contents = $state(true);
  let busy = $state(false);
  let progress = $state<Step | null>(null);
  let error = $state<AppError | null>(null);
  let done = $state<Exported | null>(null);
  let stopped = $state(false);
  let count = $state<number | null>(null);

  let formats = $derived<ExportFormat[]>(["html", ...(lib.info?.pdf ? (["pdf"] as ExportFormat[]) : []), "json", "csv", "txt", "vcard", "media-zip"]);

  $effect(() => {
    const sel = job?.selection;
    if (!sel) return;
    let live = true;
    be()
      .libSelected(sel, "time")
      .then((ids) => live && (count = ids.length))
      .catch(() => {});
    return () => (live = false);
  });

  async function write() {
    if (!job || busy) return;
    const folder = onPhone() ? null : await be().pickFolder();
    if (!onPhone() && !folder) return;
    busy = true;
    error = null;
    stopped = false;
    progress = null;
    try {
      const options = {
        order, time_zone: null, lang: app.lang, title: title.trim() || null, skip_system: skipSystem,
        ...(format === "pdf" ? { page, media: thumbs ? ("thumbnails" as const) : ("none" as const), cover, contents } : {}),
      };
      done = await be().libExport(job.selection, format, options, folder, (p) => (progress = p));
    } catch (e) {
      if (isCancelled(e)) stopped = true;
      else error = toError(e);
    } finally {
      busy = false;
    }
  }

  function close() {
    if (busy) return;
    lib.exporting = null;
    app.sheet = null;
  }
</script>

<Sheet label={l.exportTitle} size="medium" onclose={busy ? undefined : close} footer={busy ? undefined : buttons}>
  {#if busy}
    <div class="busy" role="status">
      <Ring value={progress?.per_mille ?? null} size={110} label={l.exporting} />
      <h1 class="sheet-title">{l.exporting}</h1>
      <StepLine step={progress} oncancel={cancelWork} />
    </div>
  {:else}
    <h1 class="sheet-title">{l.exportTitle}</h1>
    <p class="muted">{count === null ? "…" : `${fmt(count)} ${t.messages}`}</p>
    <fieldset class="choices">
      <legend class="h3">{l.format}</legend>
      <div class="formats">
        {#each formats as f (f)}
          <label class="check"><input type="radio" name="lib-format" checked={format === f} onchange={() => (format = f)} /><span><strong>{l.formats[f]}</strong> <span class="muted">{l.formatHints[f]}</span></span></label>
        {/each}
      </div>
    </fieldset>
    {#if format !== "vcard" && format !== "media-zip"}
      <div class="grid">
        <label class="field"><span>{l.docTitle}</span><input class="select" bind:value={title} /></label>
        <label class="field">
          <span>{l.order}</span>
          <select class="select" bind:value={order}>
            <option value="conversation">{l.byConversation}</option>
            <option value="time">{l.oneTimeline}</option>
          </select>
        </label>
      </div>
      <label class="check"><input type="checkbox" bind:checked={skipSystem} /><span>{l.skipSystem}</span></label>
    {/if}
    {#if format === "pdf"}
      <div class="grid">
        <label class="field">
          <span>{l.pageSize}</span>
          <select class="select" bind:value={page}><option value="a4">A4</option><option value="letter">Letter</option></select>
        </label>
      </div>
      <label class="check"><input type="checkbox" bind:checked={thumbs} /><span>{l.thumbnails}</span></label>
      <label class="check"><input type="checkbox" bind:checked={cover} /><span>{l.cover}</span></label>
      <label class="check"><input type="checkbox" bind:checked={contents} /><span>{l.contents}</span></label>
    {/if}
    <div class="note warn"><Icon name="unlock" /><span>{l.exportNote}</span></div>
    {#if stopped}<p class="muted" role="status">{l.exportStopped}</p>{/if}
    <ErrorNote {error} words={l.errors} />
    {#if done}
      <div class="note ok" role="status">
        <Icon name="check" />
        <span>
          {t.exportSaved}: {fmt(done.messages)} {t.messages} · {mb(done.bytes)}{#if done.pages} · {l.pages(done.pages)}{/if}
          {#if done.missing_files}<br />{l.missingFiles(done.missing_files)}{/if}
          {#if done.missing_glyphs}<br />{l.missingGlyphs}{/if}
        </span>
      </div>
      {#if onPhone()}<SaveButton from={done.path} name={done.path.replace(/^.*[\\/]/, "")} />{:else}<PathRow folder={done.folder} shown={done.path} />{/if}
    {/if}
  {/if}
</Sheet>

{#snippet buttons()}
  <button class="btn" onclick={close}>{done ? t.close : t.cancel}</button>
  <button class="btn primary" disabled={!job || count === 0} onclick={write}><Icon name="download" />{onPhone() ? l.exportMake : l.exportChooseFolder}</button>
{/snippet}

<style>
  .busy {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 6px;
    padding: 18px 0 8px;
  }
  .choices {
    border: 0;
    padding: 0;
    margin: 12px 0;
  }
  .choices legend {
    padding: 0;
    margin-bottom: 6px;
  }
  .formats {
    display: grid;
    gap: 6px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 10px;
    margin: 10px 0;
  }
  .field {
    display: grid;
    gap: 4px;
    font-size: 13px;
    color: var(--muted);
  }
  .field input,
  .field select {
    width: 100%;
    box-sizing: border-box;
  }
  .check {
    margin: 4px 0;
  }
  .note {
    margin-top: 12px;
  }
</style>
