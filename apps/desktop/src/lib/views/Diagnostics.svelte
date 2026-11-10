<script lang="ts">
  // How the open backup is stored, to report a problem: its tables (columns, rows), and for one chat the message
  // types and storage facts. Structure and counts only: Rust never sends text, names or numbers here. "Copy report"
  // puts the same as plain text on the clipboard; the chat is named by its number in the backup, not its name.
  import Icon from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import ErrorNote from "../components/ErrorNote.svelte";
  import type { AppError, Fact, TableInfo, TypeInfo } from "../backend/types";
  import { chatName, fmt } from "../format";
  import { strings } from "../i18n";
  import { app, be, current, toError } from "../state.svelte";

  let t = $derived(strings(app.lang));
  let s = $derived(current()!);
  let chat = $state(app.chat ?? current()?.chat_list[0]?.number ?? 0);
  let tables = $state<TableInfo[] | null>(null);
  let types = $state<TypeInfo[] | null>(null);
  let facts = $state<Fact[] | null>(null);
  let error = $state<AppError | null>(null);
  let copied = $state(false);

  $effect(() => {
    const id = s.id;
    let live = true;
    tables = null;
    be()
      .backupTables(id)
      .then((x) => live && (tables = x))
      .catch((e) => live && (error = toError(e)));
    return () => (live = false);
  });

  $effect(() => {
    const [id, c] = [s.id, chat];
    let live = true;
    types = facts = null;
    Promise.all([be().chatTypes(id, c), be().chatFacts(id, c)])
      .then(([ty, fa]) => {
        if (live) [types, facts] = [ty, fa];
      })
      .catch((e) => live && (error = toError(e)));
    return () => (live = false);
  });

  function report(): string {
    const out = [`oopz diagnostics`, `backup: ${s.name}, ${s.chats} chats, ${s.messages} messages, sound: ${s.sound}`, ""];
    out.push("tables (name: rows; columns)");
    for (const x of tables ?? []) out.push(`${x.name}: ${x.rows}; ${x.columns.join(", ")}`);
    const row = s.chat_list.find((c) => c.number === chat);
    out.push("", `chat ${chat} (${row?.kind ?? "?"}, ${row?.messages ?? "?"} messages)`, "types (code: messages, with text; linked; mimes)");
    for (const x of types ?? []) {
      out.push(`${x.code}: ${x.messages}, ${x.with_text}; ${x.linked.map(([n, c]) => `${n}=${c}`).join(" ")}; ${x.mimes.map(([m, c]) => `${m}=${c}`).join(" ")}`);
    }
    out.push("", "facts");
    for (const f of facts ?? []) out.push(`${f.label}: ${f.value}`);
    return out.join("\n");
  }

  async function copy() {
    const text = report();
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Older WebViews: a hidden text area and the old copy command.
      const area = document.createElement("textarea");
      area.value = text;
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.appendChild(area);
      area.select();
      const ok = document.execCommand("copy");
      area.remove();
      if (!ok) return;
    }
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
</script>

<div class="page">
  <PageHead title={t.navDiagnostics} sub={t.diagLead}>
    {#snippet actions()}
      <button class="btn" disabled={!tables || !types || !facts} onclick={copy}><Icon name={copied ? "check" : "paste"} />{copied ? t.copiedReport : t.copyReport}</button>
    {/snippet}
  </PageHead>
  <ErrorNote {error} />

  <section class="panel">
    <h2 class="h3">{t.diagTables}</h2>
    {#if tables}
      <table class="table compact">
        <thead><tr><th scope="col">{t.diagTables}</th><th scope="col" class="n">{t.diagRows}</th><th scope="col">{t.diagColumns}</th></tr></thead>
        <tbody>
          {#each tables as x (x.name)}
            <tr><td class="mono">{x.name}</td><td class="n">{fmt(x.rows)}</td><td class="dim cols">{x.columns.join(", ")}</td></tr>
          {/each}
        </tbody>
      </table>
    {:else}<progress aria-label={t.diagTables}></progress>{/if}
  </section>

  <section class="panel">
    <label class="pick">
      <span>{t.chat}</span>
      <select class="select" bind:value={chat}>
        {#each s.chat_list as c (c.number)}<option value={c.number}>{c.number} · {chatName(c)}</option>{/each}
      </select>
    </label>
    <h2 class="h3">{t.diagTypes}</h2>
    {#if types}
      <table class="table compact">
        <thead><tr><th scope="col">{t.diagCode}</th><th scope="col" class="n">{t.messages}</th><th scope="col" class="n">{t.diagWithText}</th><th scope="col">{t.diagLinked}</th><th scope="col">{t.diagMimes}</th></tr></thead>
        <tbody>
          {#each types as x (x.code)}
            <tr>
              <td>{x.code} <span class="dim">{t.kinds[x.code] ?? ""}</span></td>
              <td class="n">{fmt(x.messages)}</td>
              <td class="n">{fmt(x.with_text)}</td>
              <td class="dim">{x.linked.map(([n, c]) => `${n} ${fmt(c)}`).join(", ")}</td>
              <td class="dim">{x.mimes.map(([m, c]) => `${m} ${fmt(c)}`).join(", ")}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}<progress aria-label={t.diagTypes}></progress>{/if}
    <h2 class="h3">{t.diagFacts}</h2>
    {#if facts}
      <table class="table compact">
        <tbody>
          {#each facts as f, i (i)}<tr><th scope="row" class="mono">{f.label}</th><td class="mono">{f.value}</td></tr>{/each}
        </tbody>
      </table>
    {:else}<progress aria-label={t.diagFacts}></progress>{/if}
  </section>
</div>

<style>
  .panel {
    overflow-x: auto;
  }
  .cols {
    font-size: 12.5px;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .mono {
    font-size: 12.5px;
  }
</style>
