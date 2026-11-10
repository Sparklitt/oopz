<script lang="ts">
  // Adding a file or folder to the library: pick it, let oopz tell the app (or say which), give the key or password
  // only when that app's file needs one, and watch it come in, with a way to stop. The key is used for this one
  // import and wiped; it is never kept.
  import ErrorNote from "../../lib/components/ErrorNote.svelte";
  import { spans } from "../../lib/notes";
  import Icon from "../../lib/components/Icon.svelte";
  import KeyBoxes from "../../lib/components/KeyBoxes.svelte";
  import Ring from "../../lib/components/Ring.svelte";
  import Sheet from "../../lib/components/Sheet.svelte";
  import StepLine from "../../lib/components/StepLine.svelte";
  import type { AppError, SourceRow, Step } from "../../lib/backend/types";
  import { fmt } from "../../lib/format";
  import { strings } from "../../lib/i18n";
  import { appName, lib, refreshLibrary } from "../../lib/library.svelte";
  import { app, be, cancelWork, isCancelled, onPhone, toError } from "../../lib/state.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let path = $state("");
  let adapter = $state("");
  let secret = $state("");
  /** What the picked file asked for, once an import said so: an iPhone backup wants its password only when it's
   *  encrypted, so nothing is asked before. */
  let secretKind = $state<"none" | "key" | "password">("none");
  let importing = $state(false);
  let progress = $state<Step | null>(null);
  let error = $state<AppError | null>(null);
  let done = $state<SourceRow | null>(null);
  let stopped = $state(false);

  let shown = $derived(decodeURIComponent(path.replace(/^.*[\\/]/, "")).replace(/^.*[/:]/, ""));

  async function pick(folder: boolean) {
    const p = folder ? await be().pickFolder() : await be().pickFile();
    if (p) {
      path = p;
      secretKind = "none";
      error = null;
      done = null;
    }
  }

  async function add(e: Event) {
    e.preventDefault();
    if (!path || importing) return;
    const value = secretKind === "none" ? null : secret;
    secret = ""; // The key leaves the page now; Rust wipes it after this one call.
    importing = true;
    stopped = false;
    progress = null;
    error = null;
    try {
      done = await be().libImport(path, adapter || null, value, (p) => (progress = p));
      await refreshLibrary();
    } catch (err) {
      if (isCancelled(err)) stopped = true;
      else {
        error = toError(err);
        // The file needs a key or password: ask for it now.
        if (error.code === "needs_key") secretKind = "key";
        else if (error.code === "needs_password" || (error.code === "needs_secret" && secretKind === "none")) secretKind = "password";
      }
    } finally {
      importing = false;
    }
  }
</script>

<Sheet label={l.addTitle} size="medium" onclose={importing ? undefined : onclose} footer={importing ? undefined : buttons}>
  {#if importing}
    <div class="busy" role="status">
      <Ring value={progress?.per_mille ?? null} size={110} label={l.importing} />
      <h1 class="sheet-title">{l.importing}</h1>
      <p class="muted">{shown}</p>
      <StepLine step={progress} oncancel={cancelWork} />
    </div>
  {:else if done}
    <div class="busy">
      <span class="ok-ico"><Icon name="check" size={30} /></span>
      <h1 class="sheet-title">{l.added}: {appName(done.app)}</h1>
      <p class="muted">{done.name}</p>
      <p>{fmt(done.conversations)} {l.conversations} · {fmt(done.people)} {l.people} · {fmt(done.messages)} {(done.messages) === 1 ? t.message : t.messages}</p>
      {#if !done.owner_known}<div class="note warn"><Icon name="user" /><span>{l.ownerUnknown}</span></div>{/if}
      {#each done.notes as n, i (i)}<p class="muted small">{#each spans(n) as sp, j (j)}{#if sp.code}<code>{sp.text}</code>{:else}{sp.text}{/if}{/each}</p>{/each}
    </div>
  {:else}
    <form id="add-source" onsubmit={add}>
      <h1 class="sheet-title">{l.addTitle}</h1>
      <p class="muted">{l.addLead}</p>
      <div class="pickers">
        <button type="button" class="btn" onclick={() => pick(false)}><Icon name="file" />{l.pickFile}</button>
        {#if !onPhone()}<button type="button" class="btn" onclick={() => pick(true)}><Icon name="folder" />{l.pickFolder}</button>{/if}
      </div>
      {#if path}<p class="file"><Icon name="file" size={16} /><code>{shown}</code></p>{/if}

      <label class="field">
        <span>{l.whichApp}</span>
        <select class="select" bind:value={adapter} onchange={() => (secretKind = "none")}>
          <option value="">{l.detect}</option>
          {#each lib.adapters as a (a.id)}<option value={a.id}>{appName(a.app)} · {l.adapterNames[a.id] ?? a.id}</option>{/each}
        </select>
      </label>

      {#if stopped}<p class="muted" role="status">{t.stopped}</p>{/if}
      <!-- Asking for the key or password is said beside its box: not twice. -->
      {#if !(error && ["needs_key", "needs_password", "needs_secret"].includes(error.code))}<ErrorNote {error} words={l.errors} />{/if}

      {#if secretKind === "key"}
        <p class="muted">{l.keyNeeded}</p>
        <KeyBoxes bind:value={secret} id="lib-key" label={t.keyTitle} describedby="lib-never" compact />
        <p class="safe" id="lib-never"><Icon name="shield" size={16} />{l.keyOnce}</p>
      {:else if secretKind === "password"}
        <label class="field">
          <span>{l.passwordNeeded}</span>
          <input class="select" type="password" autocomplete="off" bind:value={secret} />
        </label>
        <p class="safe"><Icon name="shield" size={16} />{l.keyOnce}</p>
      {/if}
      <p class="hint">{onPhone() ? l.supportedPhone : l.supported}</p>
    </form>
  {/if}
</Sheet>

{#snippet buttons()}
  {#if done}
    <button type="button" class="btn" onclick={() => { done = null; path = ""; adapter = ""; }}><Icon name="plus" />{l.addAnother}</button>
    <button type="button" class="btn primary" onclick={onclose}>{t.close}</button>
  {:else}
    <button type="button" class="btn" onclick={onclose}>{t.cancel}</button>
    <button type="submit" form="add-source" class="btn primary" disabled={!path || (secretKind !== "none" && !secret.trim())}><Icon name="plus" />{l.addButton}</button>
  {/if}
{/snippet}

<style>
  .busy {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 6px;
    padding: 18px 0 8px;
  }
  .busy p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .pickers {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 14px 0 8px;
  }
  .file {
    display: flex;
    gap: 6px;
    align-items: center;
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .field {
    display: grid;
    /* A long option name mustn't widen the column past the screen (it did on a phone). */
    grid-template-columns: minmax(0, 1fr);
    gap: 4px;
    margin: 12px 0;
    font-size: 13px;
    color: var(--muted);
  }
  .field select,
  .field input {
    max-width: 100%;
    min-width: 0;
  }
  .safe {
    display: flex;
    gap: 6px;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    font-size: 13px;
  }
  .small {
    font-size: 12.5px;
  }
</style>
