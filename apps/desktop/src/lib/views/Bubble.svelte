<script lang="ts">
  import { APP_NAME } from "../generated/msgkinds";
  // One message, drawn by its kind, with what WhatsApp kept around it: the message it answers, reactions, edits,
  // forwards, poll votes, call results and file facts. Photos, stickers and voice notes show from the Media folder
  // when the person picked one; other media are labelled placeholders. A type oopz doesn't know is a labelled card.
  import FileView from "../components/FileView.svelte";
  import Icon from "../components/Icon.svelte";
  import type { Line } from "../backend/types";
  import { dateTime, dur, mb, time } from "../format";
  import { strings } from "../i18n";
  import { app, be, showMessage } from "../state.svelte";
  import Thumb from "./Thumb.svelte";

  let { id, line, highlight = false }: { id: number; line: Line; highlight?: boolean } = $props();
  let t = $derived(strings(app.lang));
  let mediaFacts = $derived(
    [line.media?.secs != null ? dur(line.media.secs) : null, line.media?.size ? mb(line.media.size) : null].filter(Boolean).join(" · "),
  );
  let pollMax = $derived(Math.max(1, ...line.poll.map(([, v]) => v)));
  let callText = $derived(
    line.call
      ? [
          line.call.video ? t.videoCall : t.voiceCall,
          line.call.result !== null && t.callResult[line.call.result] ? t.callResult[line.call.result] : t.callUnknown,
          line.call.secs ? dur(line.call.secs) : null,
        ]
          .filter(Boolean)
          .join(" · ")
      : t.kinds[90],
  );
  // A code oopz doesn't know: "WhatsApp: 9999, not shown yet".
  let known = $derived(t.kinds[line.kind] !== undefined);
  let kindName = $derived(t.kinds[line.kind] ?? t.lib.notShown(APP_NAME.whatsapp, String(line.kind)));
  let who = $derived(line.from_me ? t.you : line.sender ? `••••${line.sender}` : t.them);
  // A fixed waveform per message, so it doesn't change as the list scrolls.
  let wave = $derived(Array.from({ length: 28 }, (_, i) => 25 + ((line.id * 31 + i * 17) % 75)));
</script>

{#if line.kind === 7}
  <div class="system"><span>{line.text ?? kindName}</span></div>
{:else if line.kind === 90}
  <div class="system call" class:highlight class:missed={line.call?.result === 2 || line.call?.result === 3}>
    <span><Icon name={line.call?.video ? "video" : "call"} size={15} />{callText}{line.from_me ? ` · ${t.you}` : ""} · <time datetime={new Date(line.ts).toISOString()}>{time(line.ts)}</time></span>
  </div>
{:else}
  <div class="msg" class:me={line.from_me}>
    <div class="bubble" class:highlight class:bare={line.kind === 20} title={dateTime(line.ts)}>
      <span class="sr-only">{who}:</span>
      {#if !line.from_me && line.sender}<span class="sender" aria-hidden="true">••••{line.sender}</span>{/if}
      {#if line.forwarded > 0}
        <span class="forwarded"><Icon name="arrow" size={13} />{line.forwarded >= 5 ? t.forwardedMany : t.forwarded}</span>
      {/if}
      {#if line.reply}
        {@const r = line.reply}
        {@const quoted = r.text ?? `[${t.kinds[r.kind] ?? `#${r.kind}`}]`}
        {#if r.id !== null}
          <button class="quote" title={t.replyGo} onclick={() => showMessage(id, r.id!)}>
            <span class="sr-only">{t.replyTo}:</span><strong>{r.from_me ? t.you : t.them}</strong><span class="q-text">{quoted}</span>
          </button>
        {:else}
          <div class="quote gone"><span class="sr-only">{t.replyTo}:</span><strong>{r.from_me ? t.you : t.them}</strong><span class="q-text">{quoted}</span><em>{t.replyGone}</em></div>
        {/if}
      {/if}

      {#if line.kind === 1 && app.mediaFolder}
        <Thumb {id} message={line.id} folder={app.mediaFolder} label={line.text ?? kindName} />
      {:else if line.kind === 1 || line.kind === 3 || line.kind === 81 || line.kind === 13 || line.kind === 42}
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
        {#snippet voiceBar()}
          <div class="voice">
            <span class="play"><Icon name="play" size={14} /></span>
            <span class="wave" aria-hidden="true">{#each wave as h, i (i)}<i style:height="{h}%"></i>{/each}</span>
            <Icon name="mic" size={16} />
          </div>
        {/snippet}
        {#if app.mediaFolder}
          {@const folder = app.mediaFolder}
          <FileView load={() => be().mediaFile(id, line.id, folder)} as="audio" label={kindName} placeholder={voiceBar} />
        {:else}
          {@render voiceBar()}
        {/if}
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
        {#if line.poll.length}
          <ul class="poll">
            {#each line.poll as [name, votes], i (i)}
              <li><span class="opt">{name}</span><span class="votes">{t.votes(votes)}</span><span class="bar" aria-hidden="true"><i style:width="{(votes / pollMax) * 100}%"></i></span></li>
            {/each}
          </ul>
        {/if}
      {:else if line.kind === 4}
        <div class="card"><span class="card-ico"><Icon name="user" size={20} /></span><span class="card-text"><strong>{kindName}</strong></span></div>
      {:else if line.kind === 20}
        {#snippet stickerMark()}
          <span class="sticker"><Icon name="smile" size={44} /><span class="sr-only">{kindName}</span></span>
        {/snippet}
        {#if app.mediaFolder}
          {@const folder = app.mediaFolder}
          <FileView load={() => be().mediaFile(id, line.id, folder)} as="image" label={kindName} sticker placeholder={stickerMark} />
        {:else}
          {@render stickerMark()}
        {/if}
      {:else if line.kind === 15}
        <span class="deleted"><Icon name="ban" size={15} />{t.deleted}</span>
      {:else if !known}
        <div class="card"><span class="card-ico"><Icon name="alert" size={20} /></span><span class="card-text"><strong>{kindName}</strong></span></div>
      {/if}

      {#if line.text && ![7, 9, 46].includes(line.kind)}
        <p class="text" class:caption={line.kind !== 0}>{line.text}</p>
      {:else if line.kind === 0}
        <p class="text">[{kindName}]</p>
      {/if}
      {#if mediaFacts && line.kind !== 0}<span class="facts">{mediaFacts}</span>{/if}
      <span class="time">{#if line.edited}<span class="edited">{t.edited}</span> · {/if}<time datetime={new Date(line.ts).toISOString()}>{time(line.ts)}</time></span>
    </div>
  </div>
  {#if line.reactions.length}
    <div class="reactions" class:me={line.from_me} aria-label={t.reactions}>
      {#each line.reactions as [emoji, mine], i (i)}<span class="reaction" class:mine title={mine ? t.you : t.them}>{emoji}</span>{/each}
    </div>
  {/if}
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
  .forwarded,
  .facts,
  .edited {
    font-size: 12px;
    color: var(--muted);
  }
  .forwarded {
    display: flex;
    align-items: center;
    gap: 4px;
    font-style: italic;
  }
  .quote {
    display: grid;
    gap: 1px;
    text-align: left;
    font: inherit;
    font-size: 13px;
    color: inherit;
    padding: 5px 9px;
    border: 0;
    border-left: 3px solid var(--accent);
    border-radius: 6px;
    background: var(--surface-2);
    max-width: 100%;
    cursor: pointer;
  }
  .quote.gone {
    cursor: default;
    border-left-color: var(--line-strong);
  }
  .quote strong {
    color: var(--accent-text);
    font-size: 12px;
  }
  .q-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .quote em {
    font-size: 11.5px;
    color: var(--muted);
  }
  .poll {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
    min-width: 200px;
  }
  .poll li {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2px 8px;
    font-size: 13.5px;
  }
  .votes {
    color: var(--muted);
    font-size: 12px;
  }
  .bar {
    grid-column: 1 / -1;
    height: 4px;
    border-radius: 2px;
    background: var(--line);
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
  }
  .reactions {
    display: flex;
    gap: 3px;
    margin: -6px 0 4px 10px;
  }
  .reactions.me {
    justify-content: flex-end;
    margin: -6px 10px 4px 0;
  }
  .reaction {
    font-size: 14px;
    line-height: 1;
    padding: 3px 6px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .reaction.mine {
    border-color: var(--accent);
  }
  .call.missed span {
    color: var(--danger);
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
