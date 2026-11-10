<script lang="ts">
  import type { AppError } from "../backend/types";
  import { strings } from "../i18n";
  import { app } from "../state.svelte";
  import Icon from "./Icon.svelte";

  /** `words`: a screen's own wording for some codes (the library's), before the app's. */
  let { error, words }: { error: AppError | null; words?: Record<string, string> } = $props();
  let t = $derived(strings(app.lang));
  // Codes whose English detail adds something the person can use (a table name, a path).
  const WITH_DETAIL = ["unsupported", "merge_check", "io", "not_found", "network", "no_space", "update", "unsupported_file", "library"];
</script>

{#if error}
  <div class="note danger" role="alert">
    <Icon name="alert" />
    <div>
      {words?.[error.code] ?? t.errors[error.code] ?? error.code}
      {#if WITH_DETAIL.includes(error.code)}<div class="mono detail">{error.detail}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .detail {
    margin-top: 4px;
    opacity: 0.85;
  }
</style>
