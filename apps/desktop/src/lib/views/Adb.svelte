<script lang="ts">
  import { onMount } from "svelte";
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import { mb } from "../format";
  import { strings } from "../i18n";
  import type { AppError, ToolsOffer } from "../backend/types";
  import { adbCheck, adbPullStart, adbPullStop, adbPullUse, adbRefresh, adbWireless, app, be, toError } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let pull = $derived(app.adb.pull);
  let devices = $derived(app.adb.devices ?? []);
  let ready = $derived(devices.some((d) => d.serial === app.adb.serial && d.state === "device"));
  let business = $state(false);
  let media = $state(false);
  // Wireless debugging: the pairing screen's address and code, then the address to connect to.
  let pairHost = $state("");
  let code = $state("");
  let host = $state("");

  // No adb here: Google's platform-tools, downloaded only after the person reads and accepts the licence (0019).
  let offer = $state<ToolsOffer | null>(null);
  let toolsBusy = $state(false);
  let toolsDone = $state<number | null>(null);
  let toolsError = $state<AppError | null>(null);
  async function showOffer() {
    toolsBusy = true;
    toolsError = null;
    try {
      offer = await be().toolsOffer();
    } catch (e) {
      toolsError = toError(e);
    } finally {
      toolsBusy = false;
    }
  }
  async function download() {
    toolsBusy = true;
    toolsError = null;
    toolsDone = 0;
    try {
      await be().toolsDownload((s) => (toolsDone = s.per_mille));
      offer = null;
      await adbCheck();
      if (app.adb.found) await adbRefresh();
    } catch (e) {
      toolsError = toError(e);
    } finally {
      toolsBusy = false;
      toolsDone = null;
    }
  }

  onMount(async () => {
    await adbCheck();
    if (app.adb.found && app.adb.devices === null) void adbRefresh();
  });

  async function connect(e: SubmitEvent) {
    e.preventDefault();
    await adbWireless(pairHost.trim(), code.trim(), host.trim());
    // The code works once: never kept.
    code = "";
  }
</script>

<div class="page">
  <PageHead title={t.fromAdb} sub={t.fromAdbText} />
  <ErrorNote error={app.error} />

  {#if !app.adb.found}
    <section class="panel block" aria-live="polite">
      <h2 class="h3">{t.toolsTitle}</h2>
      <p>{t.toolsLead}</p>
      {#if !offer}
        <div class="row-buttons">
          <button class="btn primary" disabled={toolsBusy} onclick={showOffer}><Icon name="download" />{t.toolsShow}</button>
        </div>
      {:else}
        <p class="muted">{t.toolsLicence(offer.version, mb(offer.bytes))}</p>
        <textarea class="licence" readonly rows="10" aria-label={t.toolsLicence(offer.version, mb(offer.bytes))} value={offer.license}></textarea>
        {#if toolsDone !== null}
          <div class="file-progress">
            <p><span>{t.toolsDownloading}</span><span class="pct">{Math.round(toolsDone / 10)}%</span></p>
            <progress max="1000" value={toolsDone} aria-label={t.toolsDownloading}></progress>
          </div>
        {:else}
          <div class="row-buttons">
            <button class="btn primary" disabled={toolsBusy} onclick={download}><Icon name="download" />{t.toolsAccept}</button>
          </div>
        {/if}
      {/if}
      <ErrorNote error={toolsError} />
      <p class="muted">{t.toolsOr} <span class="mono">developer.android.com/tools/releases/platform-tools</span></p>
    </section>
  {:else if !pull}
    <section class="panel block">
      <div class="head">
        <h2 class="h3">{t.adbPhones}</h2>
        <button class="btn small" disabled={app.busy} onclick={adbRefresh}><Icon name="refresh" size={16} />{t.adbLook}</button>
      </div>
      {#if app.adb.devices !== null && devices.length === 0}
        <p class="muted">{t.adbNone}</p>
      {/if}
      {#if devices.length > 0}
        <div class="devices" role="radiogroup" aria-label={t.adbPhones}>
          {#each devices as d (d.serial)}
            <label class="device" class:off={d.state !== "device"}>
              <input type="radio" name="phone" value={d.serial} bind:group={app.adb.serial} disabled={d.state !== "device"} />
              <span class="grow"><strong>{d.model ?? d.serial}</strong>{#if d.model}{" "}<span class="muted mono">{d.serial}</span>{/if}</span>
              <span class="muted">{d.wireless ? t.adbWireless : t.adbUsb} · {t.adbStates[d.state] ?? d.state}</span>
            </label>
          {/each}
        </div>
      {/if}
      <label class="check"><input type="checkbox" bind:checked={business} />{t.adbBusiness}</label>
      <label class="check"><input type="checkbox" bind:checked={media} />{t.adbMedia}</label>
      <div class="row-buttons">
        <button class="btn primary" disabled={app.busy || !ready} onclick={() => adbPullStart(business, media)}><Icon name="folder" />{t.adbStart}</button>
      </div>
    </section>

    <details class="panel block wireless">
      <summary>{t.adbWirelessTitle}</summary>
      <ol>
        {#each t.adbWirelessSteps as step (step)}<li>{step}</li>{/each}
      </ol>
      <form onsubmit={connect}>
        <label>{t.adbPairHost}<input class="select" bind:value={pairHost} placeholder="192.168.1.20:37099" autocomplete="off" spellcheck="false" /></label>
        <label>{t.adbCode}<input class="select" bind:value={code} inputmode="numeric" maxlength="6" autocomplete="off" /></label>
        <label>{t.adbHost}<input class="select" bind:value={host} placeholder="192.168.1.20:41235" autocomplete="off" spellcheck="false" /></label>
        <div class="row-buttons">
          <button class="btn" type="submit" disabled={app.busy || !(host.trim() || (pairHost.trim() && code.trim()))}>{t.adbConnect}</button>
        </div>
      </form>
    </details>
  {:else}
    {@const s = pull.status}
    <section class="panel block" aria-live="polite">
      <p class="muted where"><span class="mono">{pull.info.root}</span> → <span class="mono">{pull.info.dest}</span></p>
      {#if s.running}
        <div class="file-progress">
          <p><span>{t.adbCopying}{s.folder ? `: ${s.folder}` : ""} · {mb(s.total)}</span><span class="pct">{Math.round((s.done / (s.total || 1)) * 100)}%</span></p>
          <progress max={s.total || 1} value={s.done} aria-label={t.adbCopying}></progress>
          <!-- Changes every half second: not read out. -->
          <p class="pace" aria-live="off">
            {#if s.bytes_per_sec > 0}<span>{t.wifiPace}: {mb(s.bytes_per_sec)}/s</span>{/if}
            {#if s.secs_left !== null}<span>{t.secsLeft(s.secs_left)}</span>{/if}
          </p>
        </div>
      {/if}
      {#if s.copied.length > 0}
        <h2 class="h3">{t.adbCopied}</h2>
        <ul class="inset">
          {#each s.copied as c (c.name)}
            <li class="row"><Icon name="check" class="ok-ico" /><span class="grow">{c.name}</span><span class="muted">{t.adbFiles(c.files)} · {mb(c.bytes)}</span></li>
          {/each}
        </ul>
      {/if}
      <p class="free"><Icon name="download" />{t.wifiFree}: {mb(pull.info.free)}</p>
      {#if s.stopped}<p>{t.adbStopped[s.stopped] ?? s.stopped}</p>{/if}
      <ErrorNote error={s.error} />
      <div class="row-buttons">
        {#if s.copied.some((c) => c.name === "Databases")}<button class="btn primary" onclick={adbPullUse}>{t.adbUse}</button>{/if}
        {#if s.running}<button class="btn" onclick={adbPullStop}>{t.adbStop}</button>{/if}
        {#if !s.running}<button class="btn" onclick={() => (app.adb.pull = null)}>{t.adbAgain}</button>{/if}
      </div>
    </section>
  {/if}
</div>

<style>
  .licence {
    width: 100%;
    resize: vertical;
    font: inherit;
    font-size: 0.85rem;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    margin: 0;
  }
  .block {
    display: grid;
    gap: 12px;
    padding: 18px;
    margin-bottom: 16px;
    max-width: 720px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .head h2 {
    margin: 0;
  }
  .devices {
    display: grid;
    gap: 6px;
  }
  .device {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--surface-2);
  }
  .device.off {
    opacity: 0.75;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .wireless summary {
    cursor: pointer;
    font-weight: 600;
  }
  .wireless ol {
    margin: 0;
    padding-inline-start: 20px;
    display: grid;
    gap: 6px;
  }
  form {
    display: grid;
    gap: 10px;
  }
  form label {
    display: grid;
    gap: 4px;
    font-size: 13px;
    color: var(--muted);
  }
  .where {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .file-progress p {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin: 0 0 6px;
  }
  .pct {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  progress {
    width: 100%;
  }
  .pace {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 13px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .free {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 0;
    color: var(--muted);
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .grow {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .row-buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
</style>
