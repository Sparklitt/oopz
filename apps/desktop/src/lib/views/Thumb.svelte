<script lang="ts">
  // A photo's thumbnail from the Media folder the person picked. Fetched only once the row is on screen (or at once
  // where IntersectionObserver is missing), shown from a blob URL that is revoked when the row leaves the page.
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { strings } from "../i18n";
  import { app, be } from "../state.svelte";

  let { id, message, folder, label }: { id: number; message: number; folder: string; label: string } = $props();
  let t = $derived(strings(app.lang));
  let url = $state<string | null>(null);
  let missing = $state(false);
  let box: HTMLDivElement;

  onMount(() => {
    let gone = false;
    let made: string | null = null;
    async function load() {
      try {
        const bytes = await be().thumbnail(id, message, folder, 320);
        if (gone) return;
        if (bytes.byteLength === 0) missing = true;
        else url = made = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
      } catch {
        if (!gone) missing = true;
      }
    }
    let seen: IntersectionObserver | null = null;
    if (typeof IntersectionObserver === "function") {
      seen = new IntersectionObserver((entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          seen?.disconnect();
          void load();
        }
      });
      seen.observe(box);
    } else {
      void load();
    }
    return () => {
      gone = true;
      seen?.disconnect();
      if (made) URL.revokeObjectURL(made);
    };
  });
</script>

<div class="thumb" bind:this={box}>
  {#if url}
    <img src={url} alt={label} />
  {:else}
    <div class="media" title={missing ? t.photoMissing : undefined}>
      <Icon name="image" size={26} />
      <span>{missing ? t.photoMissing : label}</span>
    </div>
  {/if}
</div>

<style>
  .thumb img {
    display: block;
    width: 240px;
    max-width: 100%;
    height: auto;
    border-radius: 10px;
  }
  .media {
    width: 240px;
    max-width: 100%;
    height: 180px;
    border-radius: 10px;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 4px;
    padding: 8px;
    text-align: center;
    background: linear-gradient(135deg, var(--ph-1), var(--ph-2));
    color: var(--ph-ink);
    font-size: 12.5px;
  }
</style>
