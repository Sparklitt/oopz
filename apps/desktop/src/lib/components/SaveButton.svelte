<script lang="ts">
  // On Android a result stays in the app's private folder until the person saves it: "Save…" opens the system's save
  // picker and Rust copies the file there (save_result). The private copy goes on close or Wipe now.
  import type { AppError } from "../backend/types";
  import { mb } from "../format";
  import { strings } from "../i18n";
  import { app, be, toError } from "../state.svelte";
  import ErrorNote from "./ErrorNote.svelte";
  import Icon from "./Icon.svelte";

  let { from, name }: { from: string; name: string } = $props();
  let t = $derived(strings(app.lang));
  let saving = $state(false);
  let saved = $state<number | null>(null);
  let error = $state<AppError | null>(null);

  async function save() {
    error = null;
    const to = await be().pickSave(name).catch(() => null);
    if (!to) return;
    saving = true;
    try {
      saved = await be().saveResult(from, to);
    } catch (e) {
      error = toError(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="save">
  <button class="btn primary" disabled={saving} onclick={save}><Icon name="download" />{t.saveAs}</button>
  {#if saved !== null}<span class="ok-line" role="status"><Icon name="check" size={16} />{t.savedBytes(mb(saved))}</span>{/if}
</div>
<ErrorNote {error} />

<style>
  .save {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
    margin: 8px 0;
  }
  .ok-line {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
