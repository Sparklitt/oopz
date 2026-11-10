<script lang="ts">
  // A modal panel for a one-off task. A <dialog> opened with showModal(): the rest of the window is inert, focus
  // stays inside, and Escape closes it unless `onclose` is missing (a step that can't be left half-done).
  // Safari before 15.4 has no showModal(): there the same element is a fixed panel over a backdrop that takes the
  // clicks, and Escape and keeping focus inside are done here.
  import type { Snippet } from "svelte";

  let {
    label,
    size = "medium",
    onclose,
    children,
    footer,
  }: { label: string; size?: "small" | "medium" | "large"; onclose?: () => void; children: Snippet; footer?: Snippet } = $props();

  const native = typeof HTMLDialogElement === "function" && typeof HTMLDialogElement.prototype.showModal === "function";

  let dialog: HTMLDialogElement;
  $effect(() => {
    if (native) {
      dialog.showModal();
      return () => dialog.close();
    }
    const before = document.activeElement;
    dialog.setAttribute("open", "");
    (focusables()[0] ?? dialog).focus();
    return () => {
      dialog.removeAttribute("open");
      if (before instanceof HTMLElement) before.focus();
    };
  });

  function focusables(): HTMLElement[] {
    const all = dialog.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    return Array.prototype.filter.call(all, (el: HTMLElement) => el.offsetParent !== null) as HTMLElement[];
  }

  function onkeydown(e: KeyboardEvent) {
    if (native) return;
    if (e.key === "Escape" || e.key === "Esc") {
      e.preventDefault();
      onclose?.();
    } else if (e.key === "Tab") {
      const list = focusables();
      if (list.length === 0) return e.preventDefault();
      const first = list[0];
      const last = list[list.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  /** Focus that gets out (a click on the page behind, a screen reader) comes back to the sheet. */
  function onfocusin(e: FocusEvent) {
    if (!native && dialog && e.target instanceof Node && !dialog.contains(e.target)) (focusables()[0] ?? dialog).focus();
  }
</script>

<svelte:document {onfocusin} />

{#if !native}<div class="backdrop" aria-hidden="true"></div>{/if}

<dialog
  bind:this={dialog}
  class="sheet {size}"
  class:shim={!native}
  role={native ? undefined : "dialog"}
  aria-modal={native ? undefined : "true"}
  tabindex="-1"
  aria-label={label}
  {onkeydown}
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
  .shim {
    position: fixed;
    top: 50%;
    left: 50%;
    -webkit-transform: translate(-50%, -50%);
    transform: translate(-50%, -50%);
    margin: 0;
    z-index: 101;
  }
  .backdrop {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 100;
    background: var(--backdrop);
    -webkit-backdrop-filter: blur(6px);
    backdrop-filter: blur(6px);
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
      box-sizing: border-box;
      padding-top: var(--top-inset, 0px);
      margin: 0;
      border: 0;
      border-radius: 0;
    }
    .shim {
      top: 0;
      left: 0;
      -webkit-transform: none;
      transform: none;
    }
    .body {
      flex: 1;
    }
    .foot {
      border-radius: 0;
    }
  }
</style>
