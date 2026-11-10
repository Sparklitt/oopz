<script lang="ts">
  import AppMark from "../components/AppMark.svelte";
  import Icon, { type IconName } from "../components/Icon.svelte";
  import Sheet from "../components/Sheet.svelte";
  import { LANGS, strings, type Lang } from "../i18n";
  import { app, giveConsent, go, setLang } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let consent = $state(false);
  const ICONS: IconName[] = ["key", "shield", "fileCheck", "cloudOff"];

  function start() {
    giveConsent();
    app.sheet = app.updates.status === "found" ? "update" : null;
    go("add");
  }
</script>

<Sheet label={t.welcomeTitle} size="medium">
  <div class="lang">
    <label class="sr-only" for="welcome-lang">{t.language}</label>
    <select id="welcome-lang" class="select small" value={app.lang} onchange={(e) => setLang(e.currentTarget.value as Lang)}>
      {#each LANGS as [code, name] (code)}<option value={code} lang={code}>{name}</option>{/each}
    </select>
  </div>
  <div class="top">
    <AppMark size={64} />
    <h1>{t.welcomeTitle}</h1>
    <p class="lead">{t.welcomeLead}</p>
  </div>
  <h2 class="sr-only">{t.rulesTitle}</h2>
  <ul class="inset rules">
    {#each t.rules as rule, i (i)}
      <li>
        <span class="badge"><Icon name={ICONS[i]} /></span>
        <span><strong>{t.rulesShort[i]}</strong><span class="more">{rule}</span></span>
      </li>
    {/each}
  </ul>
  {#snippet footer()}
    <div class="go">
      <label class="check">
        <input type="checkbox" bind:checked={consent} />
        <span>{t.consent}</span>
      </label>
      <button class="btn primary wide" disabled={!consent} onclick={start}>{t.getStarted}</button>
    </div>
  {/snippet}
</Sheet>

<style>
  .lang {
    position: absolute;
    top: 14px;
    right: 14px;
  }
  .top {
    text-align: center;
    display: grid;
    justify-items: center;
    gap: 6px;
    margin: 12px 0 20px;
  }
  h1 {
    font-size: 24px;
    margin: 12px 0 0;
    max-width: 22ch;
    line-height: 1.2;
  }
  .lead {
    color: var(--muted);
    margin: 0;
    max-width: 52ch;
  }
  .rules li {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .rules strong {
    display: block;
    font-weight: 600;
  }
  .more {
    display: block;
    color: var(--muted);
    font-size: 13px;
    margin-top: 2px;
  }
  .badge {
    color: var(--accent);
    padding-top: 1px;
  }
  .go {
    width: 100%;
    display: grid;
    gap: 12px;
    padding: 4px 8px;
  }
  .wide {
    width: 100%;
  }
</style>
