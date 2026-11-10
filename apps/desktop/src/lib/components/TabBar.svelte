<script lang="ts">
  // On a phone-wide window, the sidebar becomes four tabs at the bottom; Backups and Tools open a list page.
  import { strings } from "../i18n";
  import { app, current, go, type View } from "../state.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let t = $derived(strings(app.lang));
  let has = $derived(current() !== null);
  const TAB_OF: Record<View, View> = {
    backups: "backups", add: "backups", overview: "backups", insights: "backups",
    chats: "chats", search: "search",
    tools: "tools", receive: "tools", compare: "tools", restore: "tools", phone: "tools", done: "tools", settings: "tools",
  };
  let at = $derived(TAB_OF[app.view]);
  let tabs = $derived<[View, IconName, string, boolean][]>([
    ["backups", "phone", t.navBackups, true],
    ["chats", "chats", t.navChats, has],
    ["search", "search", t.navSearch, has],
    ["tools", "grid", t.navTools, true],
  ]);
</script>

<nav class="tabbar" aria-label={t.navLabel}>
  {#each tabs as [view, icon, label, on] (view)}
    <button class="tab" disabled={!on} aria-current={at === view ? "page" : undefined} onclick={() => go(view)}>
      <Icon name={icon} size={22} /><span>{label}</span>
    </button>
  {/each}
</nav>

<style>
  .tabbar {
    display: none;
  }
  @media (max-width: 599px) {
    .tabbar {
      display: grid;
      grid-template-columns: repeat(4, 1fr);
      flex: none;
      border-top: 1px solid var(--line);
      background: var(--surface);
      padding: 4px 4px 6px;
    }
  }
  .tab {
    display: grid;
    justify-items: center;
    gap: 2px;
    min-height: 52px;
    padding: 4px 2px;
    border: 0;
    border-radius: var(--radius-m);
    background: transparent;
    color: var(--muted);
    font-size: 12px;
  }
  .tab span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tab[aria-current] {
    color: var(--selected-text);
    background: var(--selected);
    font-weight: 600;
  }
  .tab:disabled {
    color: var(--faint);
  }
</style>
