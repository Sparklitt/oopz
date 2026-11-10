<script lang="ts">
  // What the sidebar holds, as a page: the Backups tab and the Tools tab on a phone-wide window.
  import Icon, { type IconName } from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import { date } from "../format";
  import { strings } from "../i18n";
  import { app, current, go, onPhone, selectBackup, type View } from "../state.svelte";

  let { section }: { section: "backups" | "tools" | "library" } = $props();
  let t = $derived(strings(app.lang));
  let has = $derived(current() !== null);
  let rows = $derived<[View, IconName, string, boolean][]>(
    section === "library"
      ? [
          ["library", "folder", t.lib.navSources, true],
          ["lchats", "chats", t.navChats, true],
          ["lsearch", "search", t.navSearch, true],
          ["timeline", "calendar", t.lib.timelineTitle, true],
          ["linsights", "chart", t.navInsights, true],
          ["contacts", "users", t.lib.contactsTitle, true],
          ["select", "check", t.lib.selectTitle, true],
        ]
      : section === "backups"
      ? [
          ["overview", "grid", t.navOverview, has],
          ["insights", "chart", t.navInsights, has],
        ]
      : [
          ["compare", "compare", t.navCompare, true],
          ["restore", "restore", t.navRestore, true],
          ...(onPhone() ? [] : [["receive", "wifi", t.navReceive, true] as [View, IconName, string, boolean]]),
          ["diagnostics", "fileCheck", t.navDiagnostics, has],
          ["settings", "settings", t.navSettings, true],
        ],
  );
</script>

<div class="page">
  <PageHead title={section === "backups" ? t.navBackups : section === "library" ? t.lib.nav : t.navTools} />
  {#if section === "backups"}
    <ul class="inset menu">
      {#each app.summaries as s (s.id)}
        <li>
          <button class="row" aria-current={s.id === app.currentId ? "true" : undefined} onclick={() => { selectBackup(s.id); go("overview"); }}>
            <Icon name="phone" /><span class="grow"><strong>{s.name}</strong><span class="muted">{t.opened} · {date(s.last_ms)}</span></span><Icon name="chevron" size={16} />
          </button>
        </li>
      {/each}
      <li><button class="row" onclick={() => go("add")}><Icon name="plus" /><span class="grow">{t.addBackup}</span><Icon name="chevron" size={16} /></button></li>
    </ul>
    {#if has}<h2 class="h3">{t.navThisBackup}</h2>{/if}
  {/if}
  {#if section !== "backups" || has}
    <ul class="inset menu">
      {#each rows as [view, icon, label, on] (view)}
        <li><button class="row" disabled={!on} onclick={() => go(view)}><Icon name={icon} /><span class="grow">{label}</span><Icon name="chevron" size={16} /></button></li>
      {/each}
    </ul>
  {/if}
  <p class="hint local"><Icon name="lock" size={14} />{t.staysHere}</p>
</div>

<style>
  .menu > li {
    padding: 0;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 8px 16px;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
  }
  .row[aria-current] {
    background: var(--selected);
  }
  .row:disabled {
    color: var(--faint);
  }
  .grow {
    flex: 1;
    min-width: 0;
    display: grid;
    overflow-wrap: anywhere;
  }
  .grow .muted {
    font-size: 12.5px;
  }
  .local {
    display: flex;
    gap: 6px;
    align-items: center;
  }
</style>
