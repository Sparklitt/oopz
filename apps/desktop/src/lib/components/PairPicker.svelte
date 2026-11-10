<script lang="ts">
  // The two backups and the chat in each that Compare and Restore work on. Only one-to-one chats for now.
  import { chatName, fmt } from "../format";
  import { strings } from "../i18n";
  import { app } from "../state.svelte";
  import Icon from "./Icon.svelte";

  let t = $derived(strings(app.lang));
  let m = app.merge;
  const backups = $derived(app.summaries);

  const chatsOf = (id: number) => (backups.find((b) => b.id === id)?.chat_list ?? []).filter((c) => c.kind === "contact" || c.kind === "contact_lid");

  // Keep the picks valid as backups come and go: two different backups, a chat that exists in each.
  $effect.pre(() => {
    if (!backups.some((b) => b.id === m.source)) m.source = backups[0]?.id ?? 0;
    if (!backups.some((b) => b.id === m.target) || m.target === m.source) m.target = backups.find((b) => b.id !== m.source)?.id ?? 0;
    const sc = chatsOf(m.source);
    const tc = chatsOf(m.target);
    if (!sc.some((c) => c.number === m.sourceChat)) m.sourceChat = sc[0]?.number ?? 0;
    if (!tc.some((c) => c.number === m.targetChat)) m.targetChat = tc.find((c) => c.number === m.sourceChat)?.number ?? tc[0]?.number ?? 0;
  });

  function changed() {
    app.missing = null;
  }

  function swap() {
    [m.source, m.target, m.sourceChat, m.targetChat] = [m.target, m.source, m.targetChat, m.sourceChat];
    changed();
  }
</script>

<div class="pair">
  <fieldset class="side-pick">
    <legend class="h3">{t.source}</legend>
    <label class="field"><span class="label">{t.backup}</span>
      <select class="select" bind:value={m.source} onchange={changed}>
        {#each backups as b (b.id)}<option value={b.id}>{b.name} ({fmt(b.messages)} {t.messages})</option>{/each}
      </select>
    </label>
    <label class="field"><span class="label">{t.chat}</span>
      <select class="select" bind:value={m.sourceChat} onchange={changed}>
        {#each chatsOf(m.source) as c (c.number)}<option value={c.number}>{c.number} · {chatName(c)} · {fmt(c.messages)} {t.messages}</option>{/each}
      </select>
    </label>
  </fieldset>
  <button class="btn icon-only swap" aria-label={t.swap} title={t.swap} onclick={swap}><Icon name="swap" /></button>
  <fieldset class="side-pick">
    <legend class="h3">{t.target}</legend>
    <label class="field"><span class="label">{t.backup}</span>
      <select class="select" bind:value={m.target} onchange={changed}>
        {#each backups as b (b.id)}<option value={b.id}>{b.name} ({fmt(b.messages)} {t.messages})</option>{/each}
      </select>
    </label>
    <label class="field"><span class="label">{t.chat}</span>
      <select class="select" bind:value={m.targetChat} onchange={changed}>
        {#each chatsOf(m.target) as c (c.number)}<option value={c.number}>{c.number} · {chatName(c)} · {fmt(c.messages)} {t.messages}</option>{/each}
      </select>
    </label>
  </fieldset>
</div>

<style>
  .pair {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 14px;
    align-items: center;
  }
  .side-pick {
    border: 1px solid var(--line);
    border-radius: var(--radius-m);
    background: var(--surface);
    padding: 14px 16px 16px;
    margin: 0;
    display: grid;
    gap: 10px;
    min-width: 0;
  }
  .side-pick legend {
    padding: 0 4px;
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .field select {
    width: 100%;
  }
  .label {
    font-size: 13px;
    color: var(--muted);
  }
  @media (max-width: 820px) {
    .pair {
      grid-template-columns: 1fr;
    }
    .swap {
      justify-self: center;
      transform: rotate(90deg);
    }
  }
</style>
