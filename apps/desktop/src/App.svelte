<script lang="ts">
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TabBar from "./lib/components/TabBar.svelte";
  import MobileMenu from "./lib/views/MobileMenu.svelte";
  import { app, current, go, onPhone } from "./lib/state.svelte";
  import AddBackup from "./lib/views/AddBackup.svelte";
  import Chats from "./lib/views/Chats.svelte";
  import Compare from "./lib/views/Compare.svelte";
  import Diagnostics from "./lib/views/Diagnostics.svelte";
  import Done from "./lib/views/Done.svelte";
  import ExportSheet from "./lib/views/ExportSheet.svelte";
  import FoundSheet from "./lib/views/FoundSheet.svelte";
  import Insights from "./lib/views/Insights.svelte";
  import Overview from "./lib/views/Overview.svelte";
  import Phone from "./lib/views/Phone.svelte";
  import Receive from "./lib/views/Receive.svelte";
  import Restore from "./lib/views/Restore.svelte";
  import Search from "./lib/views/Search.svelte";
  import Settings from "./lib/views/Settings.svelte";
  import UnlockSheet from "./lib/views/UnlockSheet.svelte";
  import UpdateSheet from "./lib/views/UpdateSheet.svelte";
  import WelcomeSheet from "./lib/views/WelcomeSheet.svelte";

  let has = $derived(current() !== null);
  // Screens about one backup need one open; without it they show Add a backup.
  // No receiving over Wi-Fi on a phone.
  let view = $derived((!has && ["overview", "chats", "search", "insights", "diagnostics"].includes(app.view)) || (app.view === "receive" && onPhone()) ? "add" : app.view);

  $effect(() => {
    if (app.theme === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = app.theme;
  });

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k" && has && app.sheet === null) {
      e.preventDefault();
      go("search");
      queueMicrotask(() => document.querySelector<HTMLInputElement>(".field-search input")?.focus());
    }
  }

  // Like a desktop app: no web page menu (Back, Reload…) on right-click, except where there is text to copy or paste.
  const COPYABLE = "input, textarea, code, .mono, .scroller, .hits";
  function onMenu(e: MouseEvent) {
    if (!(e.target instanceof Element && e.target.closest(COPYABLE))) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} oncontextmenu={onMenu} />

<div class="shell">
  <Sidebar />
  <main class="content" class:full={view === "chats"} data-view={view}>
    {#key view}
      {#if view === "add"}<AddBackup />
      {:else if view === "receive"}<Receive />
      {:else if view === "overview"}<Overview />
      {:else if view === "chats"}<Chats />
      {:else if view === "search"}<Search />
      {:else if view === "insights"}<Insights />
      {:else if view === "compare"}<Compare />
      {:else if view === "restore"}<Restore />
      {:else if view === "phone"}<Phone />
      {:else if view === "done"}<Done />
      {:else if view === "settings"}<Settings />
      {:else if view === "diagnostics"}<Diagnostics />
      {:else if view === "backups" || view === "tools"}<MobileMenu section={view} />
      {/if}
    {/key}
  </main>
  <TabBar />
</div>

{#if app.sheet === "welcome"}<WelcomeSheet />
{:else if app.sheet === "found"}<FoundSheet />
{:else if app.sheet === "unlock" && app.unlocking}<UnlockSheet />
{:else if app.sheet === "export" && has}<ExportSheet />
{:else if app.sheet === "update"}<UpdateSheet />
{/if}

<style>
  .shell {
    display: flex;
    height: calc(100vh - var(--top-inset, 0px));
    min-height: 0;
  }
  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    background: var(--bg);
  }
  .content.full {
    overflow: hidden;
  }
  @media (max-width: 599px) {
    .shell {
      flex-direction: column;
    }
    .content {
      min-height: 0;
    }
  }
</style>
