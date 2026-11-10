<script lang="ts">
  import { onMount } from "svelte";
  import AppMark from "../components/AppMark.svelte";
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon, { type IconName } from "../components/Icon.svelte";
  import { LANGS, strings, type Lang } from "../i18n";
  import type { LinkName } from "../backend/types";
  import { dateTime, fmt, mb } from "../format";
  import { lib, libraryWiped, refreshLibrary, setKeep } from "../library.svelte";
  import { app, be, checkUpdates, loadLastChecked, setAutoUpdates, setHideNames, setLang, setTheme, wipeNow, type Theme } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let wiped = $state<number | null>(null);
  let links = $derived<[LinkName, string][]>([
    ["source", t.sourceCode],
    ["releases", t.linkReleases],
    ["report", t.linkReport],
    ["security", t.linkSecurity],
    ["licences", t.linkLicences],
  ]);

  let confirmWipe = $state(false);
  async function wipe() {
    confirmWipe = false;
    wiped = null;
    wiped = (await wipeNow()) ?? null;
    if (wiped !== null) libraryWiped();
  }
  let tab = $state<"general" | "updates" | "privacy" | "about">("general");
  let version = $state("");
  let u = $derived(app.updates);
  let tabs = $derived<[typeof tab, IconName, string][]>([
    ["general", "settings", t.setGeneral],
    ["updates", "refresh", t.updatesTitle],
    ["privacy", "shield", t.setPrivacy],
    ["about", "info", t.setAbout],
  ]);
  let themes = $derived<[Theme, IconName, string][]>([
    ["system", "settings", t.themeSystem],
    ["light", "sun", t.themeLight],
    ["dark", "moon", t.themeDark],
  ]);

  onMount(() => {
    void loadLastChecked();
    refreshLibrary().catch(() => {});
    be()
      .appVersion()
      .then((v) => (version = v))
      .catch(() => {});
  });
</script>

<div class="settings">
  <nav class="tabs" aria-label={t.navSettings}>
    <h1 class="sr-only">{t.navSettings}</h1>
    {#each tabs as [id, icon, label] (id)}
      <button class="tab" aria-current={tab === id ? "page" : undefined} onclick={() => (tab = id)}><Icon name={icon} size={16} />{label}</button>
    {/each}
  </nav>

  <div class="pane">
    {#if tab === "general"}
      <h2 class="h2">{t.setGeneral}</h2>
      <div class="inset">
        <div class="set-row">
          <label for="set-lang">{t.language}</label>
          <select id="set-lang" class="select" value={app.lang} onchange={(e) => setLang(e.currentTarget.value as Lang)}>
            {#each LANGS as [code, name] (code)}<option value={code} lang={code}>{name}</option>{/each}
          </select>
        </div>
        <div class="set-row">
          <span id="theme-label">{t.theme}</span>
          <div class="segmented" role="radiogroup" aria-labelledby="theme-label">
            {#each themes as [id, icon, label] (id)}
              <label class:on={app.theme === id}>
                <input type="radio" class="sr-only" name="theme" checked={app.theme === id} onchange={() => setTheme(id)} />
                <Icon name={icon} size={15} />{label}
              </label>
            {/each}
          </div>
        </div>
        <div class="set-row">
          <label for="hide-names">{t.hideNames}<span class="muted hint-line">{t.hideNamesHint}</span></label>
          <input id="hide-names" class="switch" type="checkbox" role="switch" checked={app.hideNames} onchange={(e) => setHideNames(e.currentTarget.checked)} />
        </div>
      </div>
    {:else if tab === "updates"}
      <h2 class="h2">{t.updatesTitle}</h2>
      <p class="muted">{t.updatesLead}</p>
      <div class="inset">
        <div class="set-row">
          <label for="auto-updates">{t.updatesAuto}</label>
          <input id="auto-updates" class="switch" type="checkbox" role="switch" checked={u.auto} onchange={(e) => setAutoUpdates(e.currentTarget.checked)} />
        </div>
        <div class="set-row">
          <span aria-live="polite">
            {#if u.status === "checking"}{t.updateChecking}
            {:else if u.status === "none"}<span class="ok-line"><Icon name="check" size={16} />{t.updateNone}</span>
            {:else if (u.status === "found" || u.status === "installing") && u.found}<strong>{t.updateFound(u.found.version)}</strong>
            {/if}
          </span>
          <span class="buttons">
            {#if (u.status === "found" || u.status === "installing") && u.found}
              <button class="btn primary" onclick={() => (app.sheet = "update")}>{t.updateInstall}</button>
            {/if}
            <button class="btn" disabled={u.status === "checking" || u.status === "installing"} onclick={() => checkUpdates(false)}>{t.updateCheck}</button>
          </span>
        </div>
      </div>
      <p class="muted">{t.lastChecked}: {u.lastChecked ? dateTime(u.lastChecked) : t.never}</p>
      {#if u.status === "failed"}<ErrorNote error={u.error} />{/if}
    {:else if tab === "privacy"}
      <h2 class="h2">{t.setPrivacy}</h2>
      <ul class="inset rules">
        {#each t.rules as rule, i (i)}<li><strong>{t.rulesShort[i]}</strong><span class="muted">{rule}</span></li>{/each}
        <li><strong>{t.keyTitle}</strong><span class="muted">{t.keyNever}</span></li>
        <li><strong>{t.navChats}</strong><span class="muted">{t.screenOnly}</span></li>
      </ul>
      <h3 class="h3">{t.lib.keepTitle}</h3>
      <div class="inset">
        <div class="set-row">
          <label for="keep-library">{t.lib.keepLabel}<span class="muted hint-line">{lib.info?.kept ? t.lib.keepOn : t.lib.keepOff}</span></label>
          <input id="keep-library" class="switch" type="checkbox" role="switch" disabled={app.busy || !lib.info} checked={lib.info?.kept ?? false} onchange={(e) => setKeep(e.currentTarget.checked)} />
        </div>
      </div>
      {#if lib.info}<p class="muted">{t.lib.keepWhere}: <code>{lib.info.dir}</code> · {fmt(lib.info.sources)} {t.lib.sources} · {fmt(lib.info.messages)} {t.messages} · {mb(lib.info.bytes)}</p>{/if}
      <h3 class="h3">{t.wipeTitle}</h3>
      <p class="muted">{t.wipeText} {t.lib.wipeAlso}</p>
      <ErrorNote error={app.error} />
      <p class="wipe-row">
        {#if confirmWipe}
          <span role="alert">{t.lib.wipeConfirm}</span>
          <button class="btn danger" disabled={app.busy} onclick={wipe}><Icon name="ban" />{t.wipeNow}</button>
          <button class="btn" onclick={() => (confirmWipe = false)}>{t.cancel}</button>
        {:else}
          <button class="btn" disabled={app.busy} onclick={() => (confirmWipe = true)}><Icon name="ban" />{t.wipeNow}</button>
        {/if}
        {#if wiped !== null}<span role="status" class:ok-line={wiped === 0}>{#if wiped === 0}<Icon name="check" size={16} />{t.wipeDone}{:else}{t.wipeLeft(wiped)}{/if}</span>{/if}
      </p>
    {:else}
      <h2 class="h2">{t.setAbout}</h2>
      <div class="inset about">
        <div class="who">
          <AppMark size={48} />
          <div>
            <strong>oopz {version}</strong>
            <p class="muted">{t.aboutLicence}</p>
          </div>
        </div>
      </div>
      <h3 class="h3">{t.links}</h3>
      <ul class="inset link-list">
        {#each links as [which, label] (which)}
          <li><button class="link" onclick={() => be().openLink(which).catch(() => {})}><Icon name="globe" size={16} />{label}</button></li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .settings {
    display: flex;
    gap: 28px;
    max-width: 900px;
    padding: 28px 32px;
  }
  .tabs {
    width: 180px;
    flex: none;
    display: grid;
    align-content: start;
    gap: 2px;
  }
  .tab {
    display: flex;
    gap: 8px;
    align-items: center;
    min-height: 32px;
    padding: 4px 10px;
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text);
    text-align: left;
  }
  .tab:hover {
    background: var(--hover);
  }
  .tab[aria-current] {
    background: var(--selected);
    color: var(--selected-text);
    font-weight: 600;
  }
  .pane {
    flex: 1;
    min-width: 0;
  }
  .pane > .h2 {
    margin-top: 2px;
  }
  .wipe-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
  }
  .link-list {
    list-style: none;
  }
  .link-list li {
    padding: 6px 0;
  }
  .hint-line {
    display: block;
    font-size: 13px;
  }
  .set-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .ok-line {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    color: var(--ok-text);
  }
  .segmented {
    display: inline-flex;
    padding: 2px;
    border-radius: var(--radius-s);
    background: var(--surface-3);
    flex-wrap: wrap;
  }
  .segmented label {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    padding: 5px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13.5px;
  }
  .segmented label.on {
    background: var(--surface);
    box-shadow: var(--shadow-1);
    font-weight: 600;
  }
  .segmented label:focus-within {
    outline: 2px solid var(--focus);
  }
  .rules li {
    display: grid;
    gap: 2px;
  }
  .who {
    display: flex;
    gap: 14px;
    align-items: center;
  }
  .who p {
    margin: 2px 0 0;
  }
  @media (max-width: 760px) {
    .settings {
      flex-direction: column;
      padding: 20px 16px;
      gap: 12px;
    }
    .tabs {
      width: auto;
      display: flex;
      flex-wrap: wrap;
    }
  }
</style>
