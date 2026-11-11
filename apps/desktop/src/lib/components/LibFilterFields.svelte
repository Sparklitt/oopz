<script lang="ts">
  // The library's filter as form fields: app, source, contact, type, who sent it and a range of days. Each change
  // goes into `filter` and calls `onchange`. Shared by search, insights, the timeline and Select.
  import { onMount } from "svelte";
  import type { AppCode, ContactRow, Kind, LibFilter } from "../backend/types";
  import { strings } from "../i18n";
  import { KINDS } from "../kinds";
  import { appName, dayStart, lib } from "../library.svelte";
  import { app, be } from "../state.svelte";

  let { filter = $bindable(), onchange }: { filter: LibFilter; onchange?: () => void } = $props();
  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let contacts = $state<ContactRow[]>([]);
  let apps = $derived([...new Set(lib.sources.map((s) => s.app))] as AppCode[]);

  const day = (ms: number | null, before = false) => {
    if (ms === null) return "";
    const d = new Date(before ? ms - 1 : ms);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };
  let filtered = $derived(filter.apps.length + filter.sources.length + filter.contacts.length + filter.kinds.length > 0 || filter.from_me !== null || filter.after_ms !== null || filter.before_ms !== null);

  onMount(() => {
    be()
      .libContacts()
      .then((c) => (contacts = c.sort((a, b) => a.name.localeCompare(b.name))))
      .catch(() => {});
  });

  function set(change: (f: LibFilter) => void) {
    change(filter);
    onchange?.();
  }
  const one = <T,>(v: string, parse: (s: string) => T): T[] => (v ? [parse(v)] : []);
</script>

<fieldset class="filters">
  <legend class="sr-only">{t.filters}</legend>
  {#if apps.length > 1}
    <label>
      <span>{l.app}</span>
      <select class="select small" value={filter.apps[0] ?? ""} onchange={(e) => set((f) => (f.apps = one(e.currentTarget.value, (s) => s as AppCode)))}>
        <option value="">{l.anyApp}</option>
        {#each apps as a (a)}<option value={a}>{appName(a)}</option>{/each}
      </select>
    </label>
  {/if}
  {#if lib.sources.length > 1}
    <label>
      <span>{l.source}</span>
      <select class="select small" value={filter.sources.length === 1 ? filter.sources[0] : 0} onchange={(e) => set((f) => (f.sources = one(e.currentTarget.value === "0" ? "" : e.currentTarget.value, Number)))}>
        <option value={0}>{l.allSources}</option>
        {#each lib.sources as s (s.id)}<option value={s.id}>{appName(s.app)} · {s.name}</option>{/each}
      </select>
    </label>
  {/if}
  {#if contacts.length}
    <label>
      <span>{l.contact}</span>
      <select class="select small" value={filter.contacts[0] ?? 0} onchange={(e) => set((f) => (f.contacts = one(e.currentTarget.value === "0" ? "" : e.currentTarget.value, Number)))}>
        <option value={0}>{l.anyContact}</option>
        {#each contacts as c (c.id)}<option value={c.id}>{app.hideNames ? `${l.contact} #${c.id}` : c.name}</option>{/each}
      </select>
    </label>
  {/if}
  <label>
    <span>{t.filterKind}</span>
    <select class="select small" value={filter.kinds[0] ?? ""} onchange={(e) => set((f) => (f.kinds = one(e.currentTarget.value, (s) => s as Kind)))}>
      <option value="">{t.anyKind}</option>
      {#each KINDS as k (k)}<option value={k}>{l.kinds[k]}</option>{/each}
    </select>
  </label>
  <label>
    <span>{t.filterWho}</span>
    <select class="select small" value={filter.from_me === null ? "" : filter.from_me ? "me" : "them"} onchange={(e) => set((f) => (f.from_me = e.currentTarget.value === "" ? null : e.currentTarget.value === "me"))}>
      <option value="">{t.anyone}</option>
      <option value="me">{t.sentByMe}</option>
      <option value="them">{t.receivedByMe}</option>
    </select>
  </label>
  <label>
    <span>{t.filterAfter}</span>
    <input class="select small" type="date" placeholder="YYYY-MM-DD" value={day(filter.after_ms)} onchange={(e) => set((f) => (f.after_ms = dayStart(e.currentTarget.value)))} />
  </label>
  <label>
    <span>{t.filterBefore}</span>
    <input class="select small" type="date" placeholder="YYYY-MM-DD" value={day(filter.before_ms, true)} onchange={(e) => set((f) => (f.before_ms = dayStart(e.currentTarget.value, true)))} />
  </label>
  {#if filtered}
    <button type="button" class="btn small" onclick={() => set((f) => Object.assign(f, { apps: [], sources: [], contacts: [], kinds: [], from_me: null, after_ms: null, before_ms: null }))}>{t.clearFilters}</button>
  {/if}
</fieldset>

<style>
  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 8px 12px;
    border: 0;
    padding: 0;
    margin: 12px 0;
  }
  label {
    display: grid;
    gap: 3px;
    font-size: 12.5px;
    color: var(--muted);
    min-width: 0;
  }
  select {
    max-width: 220px;
  }
</style>
