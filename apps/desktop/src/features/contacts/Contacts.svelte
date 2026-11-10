<script lang="ts">
  // Who's who across apps: each app's people stay per app, and the person links them into contacts. Suggestions
  // (the same number or email in different sources) are only applied on a click. Contacts can come from a .vcf file
  // and go out as one.
  import { onMount } from "svelte";
  import ErrorNote from "../../lib/components/ErrorNote.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import PageHead from "../../lib/components/PageHead.svelte";
  import PathRow from "../../lib/components/PathRow.svelte";
  import SaveButton from "../../lib/components/SaveButton.svelte";
  import type { ContactRow, Exported, PersonRow, Suggestion } from "../../lib/backend/types";
  import { fmt } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { appName, lib, personName } from "../../lib/library.svelte";
  import { app, be, onPhone, run } from "../../lib/state.svelte";

  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let contacts = $state<ContactRow[]>([]);
  let people = $state<PersonRow[]>([]);
  let suggestions = $state<Suggestion[]>([]);
  let picked = $state<number[]>([]);
  let linking = $state<number[]>([]);
  let linkName = $state("");
  let renaming = $state<number | null>(null);
  let newName = $state("");
  let deleting = $state<number | null>(null);
  let filter = $state("");
  let imported = $state<number | null>(null);
  let exported = $state<Exported | null>(null);

  let byId = $derived(new Map(people.map((p) => [p.id, p])));
  let unlinked = $derived(
    people
      .filter((p) => !p.me && p.contact === null)
      .filter((p) => !filter.trim() || `${p.name ?? ""} ${p.username ?? ""} ${p.phone ?? ""}`.toLowerCase().includes(filter.trim().toLowerCase()))
      .sort((a, b) => b.messages - a.messages),
  );
  const who = (p: PersonRow | undefined) => (p ? personName(p.name ?? p.username ?? p.phone ?? p.key, p.id) : "?");
  const contactName = (c: ContactRow) => (app.hideNames ? `${l.contact} #${c.id}` : c.name);

  async function load() {
    const [c, p, s] = await Promise.all([be().libContacts(), be().libPeople(null), be().libSuggestions()]);
    contacts = c.sort((a, b) => a.name.localeCompare(b.name));
    people = p;
    suggestions = s;
  }

  onMount(() => {
    void run(load);
  });

  async function act(work: () => Promise<unknown>) {
    const ok = await run(async () => {
      await work();
      return true;
    });
    if (ok) await run(load);
  }

  async function importVcf() {
    const path = await be().pickFile();
    if (!path) return;
    imported = null;
    const n = await run(() => be().libImportVcards(path));
    if (n !== undefined) {
      imported = n;
      await run(load);
    }
  }

  async function exportPicked() {
    const folder = onPhone() ? null : await be().pickFolder();
    if (!onPhone() && !folder) return;
    exported = (await run(() => be().libExportContacts(picked, folder))) ?? null;
  }

  const toggle = (list: number[], id: number, on: boolean) => (on ? [...list, id] : list.filter((x) => x !== id));
</script>

<div class="page">
  <PageHead title={l.contactsTitle} sub={l.contactsLead}>
    {#snippet actions()}
      <button class="btn" onclick={importVcf}><Icon name="plus" />{l.importVcf}</button>
      <button class="btn" disabled={picked.length === 0} onclick={exportPicked}><Icon name="download" />{l.exportVcf} ({fmt(picked.length)})</button>
    {/snippet}
  </PageHead>
  <ErrorNote error={app.error} words={l.errors} />
  {#if imported !== null}<div class="note ok" role="status"><Icon name="check" /><span>{l.vcfImported(imported)}</span></div>{/if}
  {#if exported}
    <div class="note ok" role="status"><Icon name="check" /><span>{t.exportSaved}</span></div>
    {#if onPhone()}<SaveButton from={exported.path} name={exported.path.replace(/^.*[\\/]/, "")} />{:else}<PathRow folder={exported.folder} shown={exported.path} />{/if}
  {/if}

  {#if suggestions.length}
    <h2 class="h3">{l.suggestions}</h2>
    <ul class="inset list">
      {#each suggestions as s (`${s.reason}:${s.value}`)}
        <li class="row">
          <span class="grow">
            <strong>{s.name && !app.hideNames ? s.name : l.samePerson}</strong>
            <span class="muted">{s.reason === "phone" ? l.samePhone : l.sameEmail}: {app.hideNames ? "••••" : s.value}</span>
            <span class="muted">{s.people.map((id) => `${who(byId.get(id))} (${byId.get(id) ? appName(byId.get(id)!.app) : "?"})`).join(" · ")}</span>
          </span>
          <button class="btn small primary" disabled={app.busy} onclick={() => act(() => be().libAccept(s.reason, s.value))}>{l.linkThem}</button>
        </li>
      {/each}
    </ul>
  {/if}

  <h2 class="h3">{l.contactsTitle} ({fmt(contacts.length)})</h2>
  {#if contacts.length === 0}
    <p class="hint">{l.noContacts}</p>
  {:else}
    <ul class="inset list">
      {#each contacts as c (c.id)}
        <li class="row top">
          <input type="checkbox" aria-label="{l.pickForVcf}: {contactName(c)}" checked={picked.includes(c.id)} onchange={(e) => (picked = toggle(picked, c.id, e.currentTarget.checked))} />
          <span class="grow">
            {#if renaming === c.id}
              <form class="inline" onsubmit={(e) => { e.preventDefault(); void act(() => be().libRenameContact(c.id, newName)).then(() => (renaming = null)); }}>
                <input class="select" bind:value={newName} aria-label={l.rename} />
                <button class="btn small primary" disabled={!newName.trim()}>{l.save}</button>
                <button type="button" class="btn small" onclick={() => (renaming = null)}>{t.cancel}</button>
              </form>
            {:else}
              <strong>{contactName(c)}</strong>
            {/if}
            {#if !app.hideNames && (c.phones.length || c.emails.length)}<span class="muted">{[...c.phones, ...c.emails].join(" · ")}</span>{/if}
            <span class="people">
              {#each c.people as [pid, a] (pid)}
                <span class="person"><span class="badge">{appName(a)}</span>{who(byId.get(pid))}<button class="link" title={l.unlink} aria-label="{l.unlink}: {who(byId.get(pid))}" onclick={() => act(() => be().libUnlink(pid))}><Icon name="close" size={14} /></button></span>
              {/each}
              {#if c.people.length === 0}<span class="muted">{l.cardOnly}</span>{/if}
            </span>
          </span>
          <span class="buttons">
            {#if deleting === c.id}
              <button class="btn small danger" onclick={() => act(() => be().libDeleteContact(c.id)).then(() => (deleting = null))}>{l.delete}</button>
              <button class="btn small" onclick={() => (deleting = null)}>{t.cancel}</button>
            {:else}
              <button class="btn small" onclick={() => { renaming = c.id; newName = c.name; }}>{l.rename}</button>
              <button class="btn small" onclick={() => (deleting = c.id)}>{l.delete}</button>
            {/if}
          </span>
        </li>
      {/each}
    </ul>
  {/if}

  <h2 class="h3">{l.notLinked}</h2>
  <p class="muted">{l.notLinkedLead}</p>
  <div class="bar">
    <input class="select" type="search" placeholder={l.filterPeople} aria-label={l.filterPeople} bind:value={filter} />
    {#if linking.length}
      <input class="select" placeholder={l.contactName} aria-label={l.contactName} bind:value={linkName} />
      <button class="btn primary small" disabled={app.busy} onclick={() => act(() => be().libLink(linking, linkName.trim() || null)).then(() => { linking = []; linkName = ""; })}>{l.linkPicked(linking.length)}</button>
    {/if}
  </div>
  <ul class="inset list">
    {#each unlinked.slice(0, 300) as p (p.id)}
      <li class="row">
        <input type="checkbox" aria-label="{l.pickToLink}: {who(p)}" checked={linking.includes(p.id)} onchange={(e) => (linking = toggle(linking, p.id, e.currentTarget.checked))} />
        <span class="grow"><strong><span class="badge">{appName(p.app)}</span> {who(p)}</strong><span class="muted">{app.hideNames ? "" : [p.phone, p.username && `@${p.username}`, p.email].filter(Boolean).join(" · ")} {fmt(p.messages)} {(p.messages) === 1 ? t.message : t.messages}</span></span>
      </li>
    {/each}
    {#if unlinked.length === 0}<li class="row muted">{lib.sources.length ? l.allLinked : l.emptyTitle}</li>{/if}
  </ul>
</div>

<style>
  .list {
    margin-bottom: 20px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .row.top {
    align-items: flex-start;
  }
  .grow {
    flex: 1 1 220px;
    min-width: 0;
    display: grid;
    gap: 2px;
    overflow-wrap: anywhere;
  }
  .grow .muted {
    font-size: 12.5px;
  }
  .people {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .person {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: 13px;
  }
  .buttons,
  .inline,
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .bar {
    margin-bottom: 10px;
  }
  .bar input {
    max-width: 100%;
  }
  .danger {
    color: var(--danger);
  }
</style>
