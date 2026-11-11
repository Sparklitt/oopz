<script lang="ts">
  // A message's file shown as it is: a sticker or a photo as a picture (an animated WebP keeps moving; a WebM sticker
  // loops as a muted video; a Lottie `.tgs` sticker is unzipped here and drawn by lottie-web's light player, SVG only,
  // loaded only when one is shown), a voice note as a player. Fetched only once the row is on screen (or at once where IntersectionObserver is missing), from a
  // blob URL revoked when the row leaves the page. Until then, or when the folder doesn't have the file, the
  // placeholder the caller gives is shown.
  import type { Snippet } from "svelte";
  import { onMount, tick } from "svelte";
  import { sniff, TGS } from "../kinds";

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
  let type = $state("");
  let lottieBox = $state<HTMLDivElement | null>(null);
  let missing = $state(false);
  let box: HTMLDivElement;

  onMount(() => {
    let gone = false;
    let made: string | null = null;
    let stop: (() => void) | null = null;
    /** Draws a `.tgs` sticker; its stop function, or null when the file isn't Lottie this player reads. */
    async function lottie(bytes: ArrayBuffer): Promise<(() => void) | null> {
      try {
        const json = await new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"))).text();
        const { default: player } = await import("lottie-web/build/player/lottie_light");
        type = TGS;
        await tick();
        if (gone || !lottieBox) return null;
        const anim = player.loadAnimation({ container: lottieBox, renderer: "svg", loop: true, autoplay: true, animationData: JSON.parse(json) });
        return () => anim.destroy();
      } catch {
        return null;
      }
    }
    async function fetchIt() {
      try {
        const bytes = await load();
        if (gone) return;
        if (bytes.byteLength === 0) missing = true;
        else {
          type = sniff(bytes, as === "audio" ? "audio/ogg" : "image/jpeg");
          if (type === TGS) {
            stop = await lottie(bytes);
            if (!stop && !gone) missing = true;
          } else url = made = URL.createObjectURL(new Blob([bytes], { type }));
        }
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
      stop?.();
    };
  });
</script>

<div class="file" bind:this={box}>
  {#if type === TGS && !missing}
    <div class="lottie" class:sticker role="img" aria-label={label} bind:this={lottieBox}></div>
  {:else if url && type === "video/webm"}
    <video src={url} class:sticker autoplay loop muted playsinline aria-label={label}></video>
  {:else if url && as === "image"}
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
  video {
    display: block;
    width: 240px;
    max-width: 100%;
    border-radius: 10px;
  }
  .lottie {
    width: 128px;
    height: 128px;
  }
  img.sticker,
  video.sticker {
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
