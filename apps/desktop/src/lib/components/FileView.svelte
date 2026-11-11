<script lang="ts">
  // A message's file shown as it is: a sticker or a photo as a picture (an animated WebP keeps moving), a voice note
  // as a player. Fetched only once the row is on screen (or at once where IntersectionObserver is missing), from a
  // blob URL revoked when the row leaves the page. Until then, or when the folder doesn't have the file, the
  // placeholder the caller gives is shown.
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { sniff } from "../kinds";

  let {
    load,
    as,
    label,
    sticker = false,
    placeholder,
  }: {
    load: () => Promise<ArrayBuffer>;
    as: "image" | "audio";
    label: string;
    /** Small, with no frame: a sticker. */
    sticker?: boolean;
    placeholder: Snippet<[missing: boolean]>;
  } = $props();
  let url = $state<string | null>(null);
  let missing = $state(false);
  let box: HTMLDivElement;

  onMount(() => {
    let gone = false;
    let made: string | null = null;
    async function fetchIt() {
      try {
        const bytes = await load();
        if (gone) return;
        if (bytes.byteLength === 0) missing = true;
        else url = made = URL.createObjectURL(new Blob([bytes], { type: sniff(bytes, as === "audio" ? "audio/ogg" : "image/jpeg") }));
      } catch {
        if (!gone) missing = true;
      }
    }
    let seen: IntersectionObserver | null = null;
    if (typeof IntersectionObserver === "function") {
      seen = new IntersectionObserver((entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          seen?.disconnect();
          void fetchIt();
        }
      });
      seen.observe(box);
    } else {
      void fetchIt();
    }
    return () => {
      gone = true;
      seen?.disconnect();
      if (made) URL.revokeObjectURL(made);
    };
  });
</script>

<div class="file" bind:this={box}>
  {#if url && as === "image"}
    <img src={url} alt={label} class:sticker />
  {:else if url}
    <audio controls preload="metadata" src={url} aria-label={label}></audio>
  {:else}
    {@render placeholder(missing)}
  {/if}
</div>

<style>
  .file {
    min-width: 0;
  }
  img {
    display: block;
    width: 240px;
    max-width: 100%;
    height: auto;
    border-radius: 10px;
  }
  img.sticker {
    width: 128px;
    height: 128px;
    object-fit: contain;
    border-radius: 0;
  }
  audio {
    display: block;
    width: 260px;
    max-width: 100%;
    height: 36px;
  }
</style>
