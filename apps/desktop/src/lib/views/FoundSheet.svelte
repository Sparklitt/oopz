<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import Sheet from "../components/Sheet.svelte";
  import { mb } from "../format";
  import { strings } from "../i18n";
  import { app } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let files = $derived(app.files ?? []);
  let pick = $state((app.files ?? []).findIndex((f) => f.kind === "crypt15"));
  let picked = $derived(files[pick]);

  function open() {
    if (!picked || picked.kind !== "crypt15") return;
    app.unlocking = picked;
    app.sheet = "unlock";
  }
</script>

<Sheet label={t.found} size="medium" onclose={() => (app.sheet = null)}>
  <h1 class="sheet-title">{t.found}</h1>
  <p class="muted">{t.foundHint}</p>
  {#if files.length === 0}
    <div class="note"><Icon name="info" /><span>{t.foundNone}</span></div>
  {:else}
    <fieldset class="list">
      <legend class="sr-only">{t.found}</legend>
      {#each files as f, i (f.path)}
        {@const opens = f.kind === "crypt15"}
        <label class="option" class:on={i === pick} class:off={!opens}>
          <input type="radio" name="backup" class="sr-only" checked={i === pick} disabled={!opens} onchange={() => (pick = i)} />
          <Icon name="file" size={22} />
          <span class="text">
            <strong>{f.name}{#if i === files.findIndex((x) => x.kind === "crypt15")}<span class="muted">{` · ${t.newest}`}</span>{/if}</strong>
            <span class="muted">{mb(f.bytes)}{#if !opens} · {t.kind[f.kind]}{/if}</span>
          </span>
          {#if opens}<span class="pill ok">{t.kind[f.kind]}</span>{:else}<Icon name="info" size={18} />{/if}
        </label>
      {/each}
    </fieldset>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => (app.sheet = null)}>{t.cancel}</button>
    <button class="btn primary" disabled={!picked || picked.kind !== "crypt15"} onclick={open}>{t.choose}</button>
  {/snippet}
</Sheet>

<style>
  .list {
    border: 0;
    margin: 16px 0 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .option {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-m);
    cursor: pointer;
    color: var(--muted);
  }
  .option.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
    background: var(--accent-wash);
  }
  .option.off {
    cursor: default;
    opacity: 0.75;
  }
  .option:focus-within {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .text strong {
    color: var(--text);
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .pill {
    flex: none;
    font-size: 12px;
    padding: 3px 9px;
    border-radius: 999px;
    white-space: nowrap;
  }
  .pill.ok {
    background: var(--ok-bg);
    color: var(--ok-text);
  }
</style>
