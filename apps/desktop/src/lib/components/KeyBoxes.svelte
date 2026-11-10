<script lang="ts">
  // The 64-character key as 16 boxes of 4. One real input lies over the boxes, so typing, pasting, editing and
  // screen readers work as in any field; the boxes only draw what it holds. Its value lives here and in the parent's
  // local variable, never in the app state.
  import { strings } from "../i18n";
  import { app } from "../state.svelte";
  import Icon from "./Icon.svelte";

  let {
    value = $bindable(""),
    id,
    label,
    describedby,
    compact = false,
    disabled = false,
  }: { value?: string; id: string; label: string; describedby?: string; compact?: boolean; disabled?: boolean } = $props();

  let t = $derived(strings(app.lang));
  let show = $state(false);
  let focused = $state(false);
  let caret = $state(0);
  let input: HTMLInputElement;
  let pasteHint = $state(false);

  let chars = $derived(value.replace(/\s/g, ""));
  let active = $derived(Math.min(15, Math.floor(caret / 4)));

  function track() {
    if (!input) return; // An event that arrives as the field goes away (Enter opened the backup).
    const at = input.selectionStart ?? value.length;
    caret = value.slice(0, at).replace(/\s/g, "").length;
  }

  async function paste() {
    try {
      const text = await navigator.clipboard.readText();
      value = text.trim();
      input.focus();
      input.setSelectionRange(value.length, value.length);
      track();
    } catch {
      // The webview may not allow reading the clipboard: the field still takes Ctrl+V.
      pasteHint = true;
      input.focus();
    }
  }

  export function focus() {
    input.focus();
  }
</script>

<div class="keybox" class:compact>
  <div class="grid" class:focused class:disabled>
    {#each Array(16) as _, g (g)}
      {@const group = chars.slice(g * 4, g * 4 + 4)}
      <div class="box" class:on={focused && g === active} class:filled={group.length === 4} aria-hidden="true">
        {show ? group : "•".repeat(group.length)}{#if focused && g === active && group.length < 4}<span class="caret"></span>{/if}
      </div>
    {/each}
    <input
      bind:this={input}
      bind:value
      {id}
      {disabled}
      class="real"
      type={show ? "text" : "password"}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label={label}
      aria-describedby={describedby ? `${describedby} ${id}-count` : `${id}-count`}
      oninput={track}
      onkeyup={track}
      onclick={() => {
        input.setSelectionRange(value.length, value.length);
        track();
      }}
      onselect={track}
      onfocus={() => {
        focused = true;
        track();
      }}
      onblur={() => (focused = false)}
    />
  </div>
  <div class="tools">
    <button type="button" class="btn small" {disabled} onclick={paste}><Icon name="paste" size={16} />{t.keyPaste}</button>
    <button type="button" class="btn small" {disabled} aria-pressed={show} onclick={() => (show = !show)}>
      <Icon name={show ? "eyeOff" : "eye"} size={16} />{show ? t.keyHide : t.keyShow}
    </button>
    <span class="count" id="{id}-count" class:over={chars.length > 64}>{chars.length} / 64 <span class="sr-only">{t.keyDigits}</span></span>
  </div>
  {#if pasteHint}<p class="hint">{t.keyPasteHint}</p>{/if}
</div>

<style>
  .grid {
    position: relative;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    padding: 4px;
    border-radius: var(--radius-m);
  }
  .grid.focused {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .grid.disabled {
    opacity: 0.55;
  }
  .box {
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-s);
    background: var(--surface-2);
    font-family: var(--mono);
    font-size: 17px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text);
  }
  .box.filled {
    background: var(--surface);
  }
  .box.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  @media (forced-colors: active) {
    .box.on {
      outline: 2px solid Highlight;
    }
  }
  .compact .box {
    height: 32px;
    font-size: 14px;
  }
  .caret {
    width: 1.5px;
    height: 1.1em;
    background: var(--accent);
    margin-left: 1px;
    animation: blink 1s steps(1) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .caret {
      animation: none;
    }
  }
  .real {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    font-size: 16px;
    cursor: text;
    border: 0;
  }
  .tools {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 10px;
    flex-wrap: wrap;
  }
  .count {
    margin-left: auto;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    font-size: 13px;
  }
  .count.over {
    color: var(--danger);
  }
</style>
