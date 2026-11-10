<script lang="ts">
  // One message, drawn by its kind. A backup's index has no pictures or sound, so media show as labelled placeholders.
  import Icon from "../components/Icon.svelte";
  import type { Line } from "../backend/types";
  import { dateTime, time } from "../format";
  import { strings } from "../i18n";
  import { app } from "../state.svelte";

  let { line, highlight = false }: { line: Line; highlight?: boolean } = $props();
  let t = $derived(strings(app.lang));
  let kindName = $derived(t.kinds[line.kind] ?? `#${line.kind}`);
  let who = $derived(line.from_me ? t.you : line.sender ? `••••${line.sender}` : t.them);
  // A fixed waveform per message, so it doesn't change as the list scrolls.
  let wave = $derived(Array.from({ length: 28 }, (_, i) => 25 + ((line.id * 31 + i * 17) % 75)));
</script>

{#if line.kind === 7}
  <div class="system"><span>{line.text ?? kindName}</span></div>
{:else if line.kind === 90}
  <div class="system call" class:highlight>
    <span><Icon name="call" size={15} />{kindName}{line.from_me ? ` · ${t.you}` : ""} · <time datetime={new Date(line.ts).toISOString()}>{time(line.ts)}</time></span>
  </div>
{:else}
  <div class="msg" class:me={line.from_me}>
    <div class="bubble" class:highlight class:bare={line.kind === 20} title={dateTime(line.ts)}>
      <span class="sr-only">{who}:</span>
      {#if !line.from_me && line.sender}<span class="sender" aria-hidden="true">••••{line.sender}</span>{/if}

      {#if line.kind === 1 || line.kind === 3 || line.kind === 81 || line.kind === 13 || line.kind === 42}
        <div class="media" class:round={line.kind === 81}>
          <Icon name={line.kind === 1 ? "image" : line.kind === 42 ? "once" : "video"} size={26} />
          <span>{kindName}</span>
        </div>
      {:else if line.kind === 99}
        <div class="album">
          {#each [0, 1, 2, 3] as i (i)}<div class="media small"><Icon name="image" size={18} /></div>{/each}
        </div>
        <span class="label"><Icon name="image" size={14} />{kindName}</span>
      {:else if line.kind === 2}
        <div class="voice">
          <span class="play"><Icon name="play" size={14} /></span>
          <span class="wave" aria-hidden="true">{#each wave as h, i (i)}<i style:height="{h}%"></i>{/each}</span>
          <Icon name="mic" size={16} />
        </div>
        <span class="label">{kindName}</span>
      {:else if line.kind === 9}
        <div class="card"><span class="card-ico"><Icon name="file" size={20} /></span><span class="card-text"><strong>{line.text ?? kindName}</strong><span>{kindName}</span></span></div>
      {:else if line.kind === 5 || line.kind === 16}
        <div class="map" aria-hidden="true">
          <svg viewBox="0 0 200 90" preserveAspectRatio="none"><path d="M0 60 C40 40 70 75 110 50 S170 20 200 35" /><path d="M30 0 L60 90" /><path d="M140 0 L120 90" /></svg>
          <span class="pin"><Icon name="pin" size={22} /></span>
        </div>
        <span class="label"><Icon name="pin" size={14} />{kindName}</span>
      {:else if line.kind === 46}
        <div class="card"><span class="card-ico"><Icon name="poll" size={20} /></span><span class="card-text"><strong>{line.text ?? kindName}</strong><span>{kindName}</span></span></div>
      {:else if line.kind === 4}
        <div class="card"><span class="card-ico"><Icon name="user" size={20} /></span><span class="card-text"><strong>{kindName}</strong></span></div>
      {:else if line.kind === 20}
        <span class="sticker"><Icon name="smile" size={44} /><span class="sr-only">{kindName}</span></span>
      {:else if line.kind === 15}
        <span class="deleted"><Icon name="ban" size={15} />{t.deleted}</span>
      {/if}

      {#if line.text && ![7, 9, 46].includes(line.kind)}
        <p class="text" class:caption={line.kind !== 0}>{line.text}</p>
      {:else if line.kind === 0}
        <p class="text">[{kindName}]</p>
      {/if}
      <time class="time" datetime={new Date(line.ts).toISOString()}>{time(line.ts)}</time>
    </div>
  </div>
{/if}

<style>
  .msg {
    display: flex;
    padding: 2px 0;
  }
  .msg.me {
    justify-content: flex-end;
  }
  .bubble {
    position: relative;
    max-width: 72%;
    padding: 7px 11px 6px;
    border-radius: 14px 14px 14px 4px;
    background: var(--bubble-in);
    color: var(--text);
    box-shadow: var(--shadow-bubble);
    display: grid;
    gap: 5px;
  }
  .me .bubble {
    border-radius: 14px 14px 4px 14px;
    background: var(--bubble-out);
    color: var(--bubble-out-text);
  }
  .bubble.bare {
    background: transparent;
    box-shadow: none;
  }
  .highlight {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .sender {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent-text);
  }
  .text {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .time {
    justify-self: end;
    font-size: 11px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    margin-top: -2px;
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
    background: linear-gradient(135deg, var(--ph-1), var(--ph-2));
    color: var(--ph-ink);
    font-size: 12.5px;
  }
  .media.round {
    width: 180px;
    height: 180px;
    border-radius: 50%;
  }
  .album {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 3px;
    width: 220px;
  }
  .media.small {
    width: 100%;
    height: 108px;
    border-radius: 6px;
  }
  .label {
    display: inline-flex;
    gap: 5px;
    align-items: center;
    font-size: 12.5px;
    color: var(--muted);
  }
  .voice {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 220px;
    color: var(--muted);
  }
  .play {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--on-accent);
  }
  .play :global(.icon) {
    fill: currentColor;
  }
  .wave {
    flex: 1;
    height: 24px;
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .wave i {
    flex: 1;
    border-radius: 1px;
    background: currentColor;
    opacity: 0.55;
  }
  .card {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--card-in-bubble);
    min-width: 200px;
  }
  .card-ico {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent-text);
    flex: none;
  }
  .card-text {
    display: grid;
    min-width: 0;
  }
  .card-text strong {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .card-text span {
    font-size: 12.5px;
    color: var(--muted);
  }
  .map {
    position: relative;
    width: 240px;
    max-width: 100%;
    height: 110px;
    border-radius: 10px;
    background: var(--map-bg);
    overflow: hidden;
  }
  .map svg {
    width: 100%;
    height: 100%;
  }
  .map path {
    fill: none;
    stroke: var(--map-road);
    stroke-width: 6;
  }
  .pin {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -70%);
    color: var(--danger);
  }
  .sticker {
    color: var(--muted);
  }
  .deleted {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-style: italic;
    color: var(--muted);
  }
  .system {
    display: flex;
    justify-content: center;
    padding: 6px 0;
  }
  .system span {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: 12.5px;
    color: var(--muted);
    background: var(--chip);
    padding: 4px 12px;
    border-radius: 999px;
    max-width: 80%;
    text-align: center;
  }
</style>
