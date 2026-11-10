<script lang="ts">
  import AppMark from "../components/AppMark.svelte";
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon from "../components/Icon.svelte";
  import { strings } from "../i18n";
  import { app, be, go, listBackups } from "../state.svelte";

  let t = $derived(strings(app.lang));

  async function pick(folder: boolean) {
    const path = folder ? await be().pickFolder() : await be().pickFile();
    if (path) await listBackups(path);
  }
</script>

<div class="page narrow">
  <h1 class="sr-only">{t.addTitle}</h1>
  <div class="zone">
    <svg class="art" viewBox="0 0 260 110" aria-hidden="true">
      <rect x="22" y="14" width="46" height="84" rx="9" class="stroke" />
      <path d="M40 88h10" class="stroke" />
      <path d="M78 58h44" class="dash" />
      <rect x="132" y="18" width="104" height="66" rx="6" class="stroke" />
      <path d="M118 94h132l-8-10H126z" class="stroke" />
      <foreignObject x="168" y="34" width="34" height="34"><AppMark size={34} /></foreignObject>
    </svg>
    <h2>{t.addTitle}</h2>
    <p>{t.addLead}</p>
  </div>

  <ErrorNote error={app.error} />

  <p class="get-lead">{t.getLead}</p>
  <div class="cards">
    <button class="card" disabled={app.busy} onclick={() => pick(true)}>
      <Icon name="folder" size={26} />
      <strong>{t.pickFolder}</strong>
      <span>{t.fromFolderHint}</span>
    </button>
    <button class="card" disabled={app.busy} onclick={() => pick(false)}>
      <Icon name="file" size={26} />
      <strong>{t.pickFile}</strong>
      <span>{t.pickFileHint}</span>
    </button>
    <button class="card" disabled={app.busy} onclick={() => go("receive")}>
      <Icon name="wifi" size={26} />
      <strong>{t.fromWifi}</strong>
      <span>{t.fromWifiHint}</span>
    </button>
  </div>

  {#if app.files?.length}
    <p class="again"><button class="link" onclick={() => (app.sheet = "found")}>{t.foundAgain}</button></p>
  {/if}

  <details class="where">
    <summary>{t.whereQ}</summary>
    <p>{t.fromFolderText}</p>
  </details>
  <div class="note"><Icon name="info" /><span>{t.driveNote}</span></div>
</div>

<style>
  .zone {
    border: 1.5px dashed var(--line-strong);
    border-radius: var(--radius-l);
    padding: 28px 24px 26px;
    text-align: center;
    background: var(--surface-glass);
  }
  .art {
    width: 240px;
    max-width: 100%;
    height: auto;
    color: var(--muted);
  }
  .stroke {
    fill: var(--surface);
    stroke: currentColor;
    stroke-width: 2.5;
    stroke-linejoin: round;
  }
  .dash {
    stroke: currentColor;
    stroke-width: 2;
    stroke-dasharray: 4 5;
  }
  h2 {
    font-size: 24px;
    margin: 14px 0 4px;
  }
  .zone p {
    margin: 0;
    color: var(--muted);
  }
  .get-lead {
    margin: 18px 0 0;
    color: var(--muted);
    text-align: center;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    margin: 12px 0 16px;
  }
  .card {
    display: grid;
    justify-items: center;
    align-content: start;
    gap: 6px;
    padding: 20px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-m);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-1);
    text-align: center;
  }
  .card :global(.icon) {
    color: var(--accent);
  }
  .card:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .card span {
    color: var(--muted);
    font-size: 13px;
  }
  .again {
    text-align: center;
    margin: 0 0 8px;
  }
  .where {
    margin: 8px 0 14px;
  }
  .where summary {
    color: var(--accent-text);
    cursor: pointer;
    width: fit-content;
    margin: 0 auto;
  }
  .where p {
    color: var(--muted);
  }
</style>
