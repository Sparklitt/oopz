<script lang="ts">
  // A modal panel for a one-off task. A <dialog> opened with showModal(): the rest of the window is inert, focus
  // stays inside, and Escape closes it unless `onclose` is missing (a step that can't be left half-done).
  import type { Snippet } from "svelte";

  let {
    label,
    size = "medium",
    onclose,
    children,
    footer,
  }: { label: string; size?: "small" | "medium" | "large"; onclose?: () => void; children: Snippet; footer?: Snippet } = $props();

  let dialog: HTMLDialogElement;
  $effect(() => {
    dialog.showModal();
    return () => dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  class="sheet {size}"
  aria-label={label}
  oncancel={(e) => {
    e.preventDefault();
    onclose?.();
  }}
>
  <div class="body">{@render children()}</div>
  {#if footer}<div class="foot">{@render footer()}</div>{/if}
</dialog>

<style>
  .sheet {
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-sheet);
    padding: 0;
    width: var(--w);
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 32px);
    display: flex;
    flex-direction: column;
  }
  .sheet:not([open]) {
    display: none;
  }
  .small {
    --w: 460px;
  }
  .medium {
    --w: 580px;
  }
  .large {
    --w: 860px;
  }
  .sheet::backdrop {
    background: var(--backdrop);
    -webkit-backdrop-filter: blur(6px);
    backdrop-filter: blur(6px);
  }
  .body {
    padding: 28px 28px 20px;
    overflow: auto;
    min-height: 0;
    position: relative;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 8px;
    padding: 14px 20px;
    border-top: 1px solid var(--line);
    background: var(--surface-2);
    border-radius: 0 0 var(--radius-l) var(--radius-l);
  }
  @media (max-width: 700px) {
    .body {
      padding: 20px 18px 16px;
    }
  }
  /* A phone: the sheet fills the screen. */
  @media (max-width: 599px) {
    .sheet {
      width: 100%;
      max-width: 100%;
      height: 100%;
      max-height: 100%;
      margin: 0;
      border: 0;
      border-radius: 0;
    }
    .body {
      flex: 1;
    }
    .foot {
      border-radius: 0;
    }
  }
</style>
