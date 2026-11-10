<script lang="ts">
  import ErrorNote from "../components/ErrorNote.svelte";
  import Icon from "../components/Icon.svelte";
  import KeyBoxes from "../components/KeyBoxes.svelte";
  import Ring from "../components/Ring.svelte";
  import Sheet from "../components/Sheet.svelte";
  import StepLine from "../components/StepLine.svelte";
  import type { AppError, Step } from "../backend/types";
  import { strings } from "../i18n";
  import { app, be, cancelWork, isCancelled, opened, toError } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let file = $derived(app.unlocking);
  let key = $state("");
  let opening = $state(false);
  let progress = $state<Step | null>(null);
  let error = $state<AppError | null>(null);
  let showPassword = $state(false);
  let boxes = $state<KeyBoxes>();

  async function submit(e: Event) {
    e.preventDefault();
    if (!file || opening) return;
    const value = key;
    key = ""; // The key leaves the page now; Rust wipes it after this one call.
    opening = true;
    progress = null;
    error = null;
    try {
      const summary = await be().checkBackup(file.path, value, (p) => (progress = p));
      await opened(summary);
    } catch (err) {
      // Cancelled: back to the key, quietly.
      error = isCancelled(err) ? null : toError(err);
      opening = false;
      queueMicrotask(() => boxes?.focus());
    }
  }

  function cancel() {
    if (opening) return;
    key = "";
    app.sheet = "found";
  }
</script>

<Sheet label={t.keyTitle} size="medium" onclose={opening ? undefined : cancel} footer={opening ? undefined : buttons}>
  {#if opening}
    <div class="opening" role="status">
      <Ring value={progress?.per_mille ?? null} size={120} label={t.keyOpening} />
      <h1 class="sheet-title">{t.keyOpeningTitle}</h1>
      <p class="muted">{t.keyOpening}</p>
      <StepLine step={progress} oncancel={cancelWork} />
      <p class="file"><Icon name="file" size={16} />{file?.name}</p>
    </div>
  {:else}
    <form id="unlock" onsubmit={submit}>
      <div class="top">
        <span class="lock"><Icon name="lock" size={30} /></span>
        <h1 class="sheet-title">{t.keyTitle}</h1>
        <p class="muted">{t.keyLead}</p>
        <p class="file"><Icon name="file" size={16} />{file?.name}</p>
      </div>
      <ErrorNote {error} />
      <KeyBoxes bind:this={boxes} bind:value={key} id="unlock-key" label="{t.keyTitle}: {file?.name ?? ''}" describedby="unlock-never" />
      <p class="safe" id="unlock-never"><Icon name="shield" size={16} />{t.keyNever}</p>
    </form>
    <div class="disclosure">
      <button type="button" class="link" aria-expanded={showPassword} aria-controls="password-help" onclick={() => (showPassword = !showPassword)}>
        <Icon name={showPassword ? "chevronDown" : "chevron"} size={16} />{t.passwordQ}
      </button>
      {#if showPassword}
        <div class="note warn help" id="password-help">
          <Icon name="alert" />
          <div>
            <h2 class="h3">{t.passwordTitle}</h2>
            <p>{t.passwordText}</p>
            <ol>
              {#each t.passwordSteps as step, i (i)}<li>{step}</li>{/each}
            </ol>
            <p>{t.passwordEdge}</p>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</Sheet>

{#snippet buttons()}
  <button type="button" class="btn" onclick={cancel}>{t.back}</button>
  <button type="submit" form="unlock" class="btn primary"><Icon name="unlock" />{t.keyOpen}</button>
{/snippet}

<style>
  .top {
    text-align: center;
    display: grid;
    justify-items: center;
    gap: 4px;
    margin-bottom: 18px;
  }
  .top p {
    margin: 0;
    max-width: 50ch;
  }
  .lock {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent-text);
    margin-bottom: 6px;
  }
  .file {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: 13px;
    color: var(--muted);
    margin-top: 6px !important;
    overflow-wrap: anywhere;
  }
  .safe {
    display: flex;
    gap: 6px;
    align-items: center;
    justify-content: center;
    color: var(--muted);
    font-size: 13px;
    margin: 14px 0 6px;
    text-align: center;
  }
  .disclosure {
    text-align: center;
  }
  .help {
    text-align: left;
    margin-top: 10px;
  }
  .help p,
  .help ol {
    margin: 6px 0;
  }
  .help ol {
    padding-left: 20px;
  }
  .opening {
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 6px;
    padding: 18px 0 8px;
  }
  .opening p {
    margin: 0;
  }
</style>
