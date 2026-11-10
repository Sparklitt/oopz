<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import { chatName, date, fmt, isGroup, mb, year } from "../format";
  import { strings } from "../i18n";
  import { app, closeBackup, current, go, openExport } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let s = $derived(current()!);
  let limit = $state(200);
  let sorted = $derived([...s.chat_list].sort((a, b) => b.messages - a.messages));
  let top = $derived(Math.max(1, sorted[0]?.messages ?? 1));
  let built = $derived(app.index[s.id]?.built);

  function openChat(n: number) {
    app.chat = n;
    app.jump = null;
    go("chats");
  }
</script>

<div class="page">
  <PageHead title={s.name} sub="{t.lastMessage} {date(s.last_ms)} · {mb(s.bytes)}">
    {#snippet badge()}
      {#if s.sound}
        <span class="badge ok"><Icon name="check" size={14} />{t.healthy}</span>
      {:else}
        <span class="badge danger"><Icon name="alert" size={14} />{t.problems}</span>
      {/if}
    {/snippet}
    {#snippet actions()}
      <button class="btn" onclick={() => go("search")}><Icon name="search" />{t.navSearch}</button>
      <button class="btn" onclick={() => openExport(null)}><Icon name="download" />{t.exportShort}</button>
      <button class="btn" onclick={() => closeBackup(s.id)}><Icon name="close" />{t.forget}</button>
    {/snippet}
  </PageHead>

  {#if s.sound}
    <div class="note ok"><Icon name="check" /><span>{t.sound}</span></div>
  {:else}
    <div class="note danger" role="alert"><Icon name="alert" /><span>{t.unsound} {s.problems}</span></div>
  {/if}

  <div class="tiles">
    <div class="tile"><Icon name="chats" /><span class="num">{fmt(s.messages)}</span><span class="lbl">{t.messages}</span></div>
    <div class="tile">
      <Icon name="users" /><span class="num">{fmt(s.chats)}</span><span class="lbl">{t.chats}</span>
      <span class="sub">{fmt(s.one_to_one)} {t.oneToOne}, {fmt(s.groups)} {s.groups === 1 ? t.group : t.groups}</span>
    </div>
    <div class="tile">
      <Icon name="calendar" /><span class="num">{year(s.first_ms)} – {year(s.last_ms)}</span><span class="lbl">{t.period}</span>
      <span class="sub">{date(s.first_ms)} – {date(s.last_ms)}</span>
    </div>
    <div class="tile"><Icon name="file" /><span class="num">{mb(s.bytes)}</span><span class="lbl">{t.fileSize}</span></div>
  </div>

  <section>
    <h2 class="h2">{t.mostActive}</h2>
    <p class="hint">{t.masked}</p>
    <table class="table chats">
      <thead>
        <tr>
          {#each t.chatTable as c, i (i)}<th class:n={i === 2} scope="col">{c}</th>{/each}
          <th class="share"><span class="sr-only">{t.share}</span></th>
        </tr>
      </thead>
      <tbody>
        {#each sorted.slice(0, limit) as c (c.number)}
          <tr>
            <td class="dim">{c.number}</td>
            <td>
              <button class="link plain" onclick={() => openChat(c.number)}>
                <span class="avatar" class:group={isGroup(c)} aria-hidden="true"><Icon name={isGroup(c) ? "users" : "user"} size={14} /></span>{chatName(c)}
              </button>
            </td>
            <td class="n">{fmt(c.messages)}</td>
            <td>{date(c.last_ms)}</td>
            <td class="share"><span class="bar" style:width="{Math.max(2, (c.messages / top) * 100)}%"></span></td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if sorted.length > limit}
      <button class="btn small more" onclick={() => (limit += 500)}>{t.showMore}</button>
    {/if}
  </section>

  <details class="fingerprint">
    <summary>{t.fingerprint}</summary>
    <code class="mono">{s.sha256}</code>
  </details>

  {#if built}
    <p class="hint"><Icon name="check" size={14} /> {t.exploreBuilt} ({fmt(built.messages)} {t.messages}, {(built.millis / 1000).toFixed(1)} s)</p>
  {/if}
  {#if app.summaries.length < 2}
    <div class="note"><Icon name="info" /><span>{t.mergeNeedsTwo}</span></div>
  {/if}
  <div class="row-buttons">
    <button class="btn" onclick={() => go("add")}><Icon name="plus" />{t.checkAnother}</button>
    <button class="btn" disabled={app.summaries.length < 2} onclick={() => go("restore")}><Icon name="restore" />{t.toMerge}</button>
  </div>
</div>

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 12px;
    margin: 18px 0 28px;
  }
  .chats {
    margin-top: 8px;
  }
  .chats td {
    vertical-align: middle;
    padding-top: 5px;
    padding-bottom: 5px;
  }
  .chats .avatar {
    width: 26px;
    height: 26px;
  }
  .plain {
    display: inline-flex;
    gap: 8px;
    align-items: center;
    color: var(--text);
    text-align: left;
  }
  .share {
    width: 22%;
  }
  .bar {
    display: block;
    height: 6px;
    border-radius: 3px;
    background: var(--accent);
    opacity: 0.75;
  }
  .more {
    margin-top: 10px;
  }
  .fingerprint {
    margin: 22px 0 12px;
  }
  .fingerprint summary {
    cursor: pointer;
    color: var(--muted);
  }
  .fingerprint code {
    display: block;
    margin-top: 6px;
    overflow-wrap: anywhere;
  }
  .row-buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 16px;
  }
  @media (max-width: 760px) {
    .share {
      display: none;
    }
  }
</style>
