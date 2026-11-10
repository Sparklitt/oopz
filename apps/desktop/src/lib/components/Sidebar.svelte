<script lang="ts">
  import { date, mod } from "../format";
  import { strings } from "../i18n";
  import { app, current, go, onPhone, selectBackup, type View } from "../state.svelte";
  import AppMark from "./AppMark.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let t = $derived(strings(app.lang));
  let has = $derived(current() !== null);

  // Which sidebar item a screen belongs to.
  const OWNER: Partial<Record<View, View>> = { phone: "restore", done: "restore" };
  let at = $derived(OWNER[app.view] ?? app.view);

  let backupItems = $derived<[View, IconName, string][]>([
    ["overview", "grid", t.navOverview],
    ["chats", "chats", t.navChats],
    ["search", "search", t.navSearch],
    ["insights", "chart", t.navInsights],
  ]);
  let toolItems = $derived<[View, IconName, string][]>([
    ["compare", "compare", t.navCompare],
    ["restore", "restore", t.navRestore],
    ...(onPhone() ? [] : [["receive", "wifi", t.navReceive] as [View, IconName, string]]),
  ]);
</script>

<aside class="side" aria-label={t.navLabel}>
  <div class="brand">
    <AppMark size={26} />
    <span class="label wordmark">oopz</span>
  </div>

  <button class="find" disabled={!has} onclick={() => go("search")} title={t.searchAll}>
    <Icon name="search" size={16} />
    <span class="label grow">{t.searchAll}</span>
    <kbd class="label">{mod}K</kbd>
  </button>

  <nav>
    <h2 class="label">{t.navBackups}</h2>
    <ul>
      {#each app.summaries as s (s.id)}
        <li>
          <button class="item" aria-current={s.id === app.currentId ? "true" : undefined} onclick={() => selectBackup(s.id)} title={s.name}>
            <Icon name="phone" />
            <span class="label grow two">
              <span class="name">{s.name}</span>
              <span class="sub"><span class="dot" aria-hidden="true"></span>{t.opened} · {date(s.last_ms)}</span>
            </span>
          </button>
        </li>
      {/each}
      <li>
        <button class="item" aria-current={at === "add" ? "page" : undefined} onclick={() => go("add")} title={t.addBackup}>
          <Icon name="plus" /><span class="label">{t.addBackup}</span>
        </button>
      </li>
    </ul>

    <h2 class="label">{t.navThisBackup}</h2>
    <ul>
      {#each backupItems as [view, icon, label] (view)}
        <li>
          <button class="item" disabled={!has} aria-current={at === view ? "page" : undefined} onclick={() => go(view)} title={label}>
            <Icon name={icon} /><span class="label">{label}</span>
          </button>
        </li>
      {/each}
    </ul>

    <h2 class="label">{t.navTools}</h2>
    <ul>
      {#each toolItems as [view, icon, label] (view)}
        <li>
          <button class="item" aria-current={at === view ? "page" : undefined} onclick={() => go(view)} title={label}>
            <Icon name={icon} /><span class="label">{label}</span>
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="bottom">
    <button class="item" aria-current={at === "settings" ? "page" : undefined} onclick={() => go("settings")} title={t.navSettings}>
      <Icon name="settings" /><span class="label">{t.navSettings}</span>
    </button>
    <p class="local label"><Icon name="lock" size={14} />{t.staysHere}</p>
  </div>
</aside>

<style>
  .side {
    width: var(--side-w);
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 10px 12px;
    /* Translucent over the page colour: the same look with or without backdrop-filter (Chrome 76). */
    background: linear-gradient(var(--sidebar), var(--sidebar)), var(--bg);
    -webkit-backdrop-filter: blur(24px) saturate(1.4);
    backdrop-filter: blur(24px) saturate(1.4);
    border-right: 1px solid var(--line);
    overflow-y: auto;
    overflow-x: hidden;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 2px 8px 8px;
  }
  .wordmark {
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .find {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--surface-glass);
    color: var(--muted);
    font-size: 13px;
    text-align: left;
    margin-bottom: 6px;
  }
  .find:hover:not(:disabled) {
    border-color: var(--line-strong);
  }
  kbd {
    font-family: inherit;
    font-size: 11px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    border-radius: 4px;
  }
  .grow {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h2 {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
    margin: 12px 8px 4px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1px;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 32px;
    padding: 5px 8px;
    border: 0;
    border-radius: var(--radius-s);
    background: transparent;
    color: var(--text);
    font-size: 13.5px;
    text-align: left;
  }
  .item:hover:not(:disabled) {
    background: var(--hover);
  }
  .item[aria-current] {
    background: var(--selected);
    color: var(--selected-text);
    font-weight: 600;
  }
  .item:disabled {
    color: var(--faint);
  }
  .two {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
    color: var(--muted);
    font-weight: 400;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
  }
  .bottom {
    margin-top: auto;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    display: grid;
    gap: 6px;
  }
  .local {
    display: flex;
    gap: 6px;
    align-items: center;
    margin: 0;
    padding: 0 8px;
    font-size: 11.5px;
    color: var(--muted);
  }
  /* A narrow window: the sidebar keeps only its icons; the words stay for screen readers and as tooltips. */
  @media (max-width: 900px) {
    .side {
      width: 60px;
      padding: 12px 8px;
      align-items: stretch;
    }
    .label {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip: rect(0 0 0 0);
      clip-path: inset(50%);
      white-space: nowrap;
    }
    .brand,
    .item,
    .find {
      justify-content: center;
      padding-left: 0;
      padding-right: 0;
    }
    .brand {
      padding-bottom: 10px;
    }
    nav ul + .label + ul,
    nav ul {
      border-top: 1px solid var(--line);
      padding-top: 6px;
      margin-top: 6px;
    }
  }
  /* A phone: the tab bar at the bottom (TabBar.svelte) takes over. */
  @media (max-width: 599px) {
    .side {
      display: none;
    }
  }
</style>
