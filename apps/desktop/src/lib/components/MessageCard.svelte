<script lang="ts">
  // One library message, from any app, drawn from its parts: text, media (stickers, photos and voice notes from their
  // file when the source's folder has it, else a labelled placeholder), places, contacts, polls, calls, links, cards
  // (albums, sticker packs, events…), the app's own notices, and a labelled card for a type oopz doesn't know yet;
  // with the message it answers, reactions, edits and forwards. All as text, never as HTML.
  import type { MediaPart, MessageRow } from "../backend/types";
  import { dateTime, dur, mb, time } from "../format";
  import { strings } from "../i18n";
  import { appName, messageText, senderOf } from "../library.svelte";
  import { app, be } from "../state.svelte";
  import { HAS_DURATION, KIND_ICON, SHOWN_FROM_FILE } from "../kinds";
  import FileView from "./FileView.svelte";
  import Icon from "./Icon.svelte";

  let {
    m,
    highlight = false,
    showSender = true,
    where = null,
    picked = null,
    onpick,
    onreply,
  }: {
    m: MessageRow;
    highlight?: boolean;
    showSender?: boolean;
    /** The conversation's title, for a list mixing conversations (the timeline). */
    where?: string | null;
    /** In the selection or not; null: no tick box. */
    picked?: boolean | null;
    onpick?: (on: boolean) => void;
    onreply?: (id: number) => void;
  } = $props();
  let t = $derived(strings(app.lang));
  let l = $derived(t.lib);
  let quoted = $state<MessageRow | null>(null);

  $effect(() => {
    const id = m.reply;
    if (id === null) return;
    let live = true;
    be()
      .libMessagesById([id])
      .then((rows) => live && (quoted = rows[0] ?? null))
      .catch(() => {});
    return () => (live = false);
  });

  // A sticker, a photo, a GIF or a document has no duration, whatever the source wrote (WhatsApp writes 0).
  const facts = (p: MediaPart) => [p.secs != null && HAS_DURATION[p.kind] ? dur(p.secs) : null, p.size ? mb(p.size) : null].filter(Boolean).join(" · ");
  let system = $derived(m.kind === "system" || m.kind === "call");
  // Telegram's notices carry only their detail (who, a title), not what happened: the event's label leads.
  let detailOnly = $derived(m.app === "telegram");
  let at = $derived(m.time_ms === null ? "" : m.wall_clock ? time(m.time_ms) : time(m.time_ms));
</script>

{#snippet tick()}
  {#if picked !== null}
    <label class="pick" title={l.inSelection}>
      <input type="checkbox" checked={picked} onchange={(e) => onpick?.(e.currentTarget.checked)} />
      <span class="sr-only">{l.inSelection}</span>
    </label>
  {/if}
{/snippet}

{#if system}
  <div class="row system" class:highlight>
    {@render tick()}
    <span class="chip">
      {#each m.message.parts as p, i (i)}
        {#if p.type === "call"}
          <Icon name={p.video ? "video" : "call"} size={15} />{p.video ? t.videoCall : t.voiceCall} · {l.outcomes[p.outcome]}{p.secs ? ` · ${dur(p.secs)}` : ""}{m.from_me ? ` · ${t.you}` : ""}
        {:else if p.type === "system"}
          {p.text && !detailOnly ? p.text : [l.events[p.event] ?? p.event, p.text].filter(Boolean).join(": ")}
        {/if}
      {/each}
      {#if at} · <time>{at}</time>{/if}
    </span>
  </div>
{:else}
  <div class="row msg" class:me={m.from_me}>
    {@render tick()}
    <div class="bubble" class:highlight title={m.time_ms === null ? "" : dateTime(m.time_ms)}>
      {#if where}<span class="where"><span class="app">{appName(m.app)}</span>{where}</span>{/if}
      <span class="sr-only">{senderOf(m)}:</span>
      {#if showSender && !m.from_me}<span class="sender" aria-hidden="true">{senderOf(m)}</span>{/if}
      {#if m.message.forwarded}<span class="small"><Icon name="arrow" size={13} />{t.forwarded}</span>{/if}
      {#if m.reply !== null}
        <button class="quote" title={t.replyGo} onclick={() => onreply?.(m.reply!)} disabled={!onreply}>
          <span class="sr-only">{t.replyTo}:</span>
          <strong>{quoted ? senderOf(quoted) : t.replyTo}</strong>
          <span class="q-text">{quoted ? messageText(quoted) : "…"}</span>
        </button>
      {:else if m.message.reply_to}
        <div class="quote gone"><span class="sr-only">{t.replyTo}:</span><em>{l.replyElsewhere}</em></div>
      {/if}

      {#if m.message.deleted}
        <span class="small"><Icon name="ban" size={15} />{t.deleted}</span>
      {/if}
      {#each m.message.parts as p, i (i)}
        {#if p.type === "text"}
          <p class="text">{p.text}</p>
        {:else if p.type === "media"}
          {#snippet mediaCard(missing: boolean)}
            <div class="card" class:small-card={p.kind === "sticker"}>
              <span class="card-ico"><Icon name={p.view_once ? "once" : KIND_ICON[p.kind]} size={20} /></span>
              <span class="card-text">
                <strong>{p.name ?? l.media[p.kind]}</strong>
                <span>{[l.media[p.kind], facts(p), p.view_once ? l.viewOnce : null, p.omitted ? l.omitted : null, missing ? l.noFile : null].filter(Boolean).join(" · ")}</span>
              </span>
            </div>
          {/snippet}
          {@const shown = SHOWN_FROM_FILE[p.kind]}
          {#if shown && p.file && !p.view_once && !p.omitted}
            <FileView load={() => be().libMedia(m.id, i)} as={shown} label={p.name ?? l.media[p.kind]} sticker={p.kind === "sticker"} placeholder={mediaCard} />
          {:else}
            {@render mediaCard(false)}
          {/if}
          {#if p.caption}<p class="text">{p.caption}</p>{/if}
        {:else if p.type === "location"}
          <div class="card">
            <span class="card-ico"><Icon name="pin" size={20} /></span>
            <span class="card-text"><strong>{p.name ?? (p.live ? l.liveLocation : l.kinds.location)}</strong>{#if p.lat !== null && p.lng !== null}<span class="mono">{p.lat.toFixed(5)}, {p.lng.toFixed(5)}</span>{/if}</span>
          </div>
        {:else if p.type === "contact"}
          <div class="card">
            <span class="card-ico"><Icon name="user" size={20} /></span>
            <span class="card-text"><strong>{p.name ?? l.kinds.contact}</strong>{#each p.phones as ph, j (j)}<span>{ph}</span>{/each}</span>
          </div>
        {:else if p.type === "poll"}
          {@const top = Math.max(1, ...p.options.map((o) => o.votes))}
          <div class="card"><span class="card-ico"><Icon name="poll" size={20} /></span><span class="card-text"><strong>{p.question}</strong><span>{l.kinds.poll}</span></span></div>
          <ul class="poll">
            {#each p.options as o, j (j)}
              <li><span>{o.text}</span><span class="votes">{t.votes(o.votes)}</span><span class="bar" aria-hidden="true"><i style:width="{(o.votes / top) * 100}%"></i></span></li>
            {/each}
          </ul>
        {:else if p.type === "link"}
          <div class="card"><span class="card-ico"><Icon name="globe" size={20} /></span><span class="card-text">{#if p.title}<strong>{p.title}</strong>{/if}<span class="url">{p.url}</span></span></div>
        {:else if p.type === "card"}
          <div class="card">
            <span class="card-ico"><Icon name={KIND_ICON[p.kind]} size={20} /></span>
            <span class="card-text"><strong>{p.title ?? l.cards[p.kind]}</strong><span>{l.cards[p.kind]}</span></span>
          </div>
          {#if p.text}<p class="text">{p.text}</p>{/if}
        {:else if p.type === "unknown"}
          <div class="card unknown">
            <span class="card-ico"><Icon name={KIND_ICON.unknown} size={20} /></span>
            <span class="card-text"><strong>{l.notShown(appName(m.app), p.code)}</strong>{#if p.text}<span>{p.text}</span>{/if}</span>
          </div>
        {/if}
      {/each}
      <span class="time">
        {#if m.message.starred}<Icon name="gift" size={12} />{/if}
        {#if m.message.edited}<span class="small">{t.edited}</span> · {/if}<time>{at}</time>
      </span>
    </div>
  </div>
  {#if m.message.reactions.length}
    <div class="reactions" class:me={m.from_me} aria-label={t.reactions}>
      {#each m.message.reactions as r, i (i)}<span class="reaction" class:mine={r.from_me}>{r.emoji}</span>{/each}
    </div>
  {/if}
{/if}

<style>
  .row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 2px 0;
  }
  .msg.me {
    flex-direction: row-reverse;
  }
  .pick {
    padding-top: 8px;
  }
  .bubble {
    max-width: 72%;
    padding: 7px 11px 6px;
    border-radius: 14px 14px 14px 4px;
    background: var(--bubble-in);
    color: var(--text);
    box-shadow: var(--shadow-bubble);
    display: grid;
    gap: 5px;
    min-width: 0;
  }
  .me .bubble {
    border-radius: 14px 14px 4px 14px;
    background: var(--bubble-out);
    color: var(--bubble-out-text);
  }
  @media (max-width: 599px) {
    .bubble {
      max-width: 86%;
    }
  }
  .highlight {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .where {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .app {
    font-weight: 700;
    color: var(--accent-text);
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
    unicode-bidi: plaintext;
  }
  .small {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: 12px;
    color: var(--muted);
  }
  .time {
    justify-self: end;
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: 11px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
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
  .quote.gone,
  .quote:disabled {
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
  .card {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--card-in-bubble);
    min-width: 0;
  }
  .small-card {
    max-width: 240px;
  }
  .unknown .card-ico {
    background: var(--surface-2);
    color: var(--muted);
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
    overflow-wrap: anywhere;
  }
  .poll {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
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
  .system {
    justify-content: center;
    padding: 6px 0;
  }
  .chip {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    justify-content: center;
    font-size: 12.5px;
    color: var(--muted);
    background: var(--chip);
    padding: 4px 12px;
    border-radius: 999px;
    max-width: 80%;
    text-align: center;
  }
</style>
