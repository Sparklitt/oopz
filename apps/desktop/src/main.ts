// The oopz desktop UI: one screen per step, rendered from `state`. It only shows results and sends commands; the
// work happens in Rust (crates/oopz-app). Text from files (names) is always set as text, never as HTML.
import { open } from "@tauri-apps/plugin-dialog";
import { api, isAppError, type AppError, type BackupFile, type BuildInfo, type Hit, type Line, type MergeOutcome, type Missing, type ReceiveInfo, type ReceiveStatus, type Stats, type Summary } from "./api";
import { initialLang, strings, type Lang, type Strings } from "./i18n";

type Work = "open" | "index";
type Step = "welcome" | "get" | "key" | "check" | "explore" | "merge" | "phone2" | "done";
const STEPS: Step[] = ["welcome", "get", "key", "check", "explore", "merge", "phone2", "done"];

const state = {
  lang: initialLang() as Lang,
  step: "welcome" as Step,
  consent: false,
  files: null as BackupFile[] | null,
  selected: null as BackupFile | null,
  summaries: [] as Summary[],
  current: null as Summary | null,
  error: null as AppError | null,
  busy: false,
  /** Thousandths done of each long step (opening, indexing), or null before its first report. They can overlap. */
  progress: { open: null, index: null } as Record<Work, number | null>,
  showPassword: false,
  chatLimit: 200,
  receive: null as { info: ReceiveInfo; status: ReceiveStatus } | null,
  merge: { source: 0, sourceChat: 0, target: 0, targetChat: 0, out: "", media: "" },
  outcome: null as MergeOutcome | null,
  /** "What's missing" for the chats picked on the merge screen; cleared when the picks change. */
  missing: null as Missing | null,
  phoneChecks: [] as boolean[],
};

let t: Strings = strings(state.lang);
let poll: number | undefined;

// ---- small DOM helpers ----

type Child = Node | string | null | undefined | false;

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<Record<string, string | boolean | ((e: Event) => void)>> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (typeof v === "function") el.addEventListener(k.replace(/^on/, ""), v);
    else if (typeof v === "boolean") {
      if (v) el.setAttribute(k, "");
    } else if (v !== undefined) el.setAttribute(k, v);
  }
  for (const c of children) if (c) el.append(c);
  return el;
}

const fmt = (n: number) => n.toLocaleString(state.lang);
const date = (ms: number | null) => (ms ? new Date(ms).toLocaleDateString(state.lang) : "–");
const mb = (bytes: number) => `${(bytes / 1e6).toLocaleString(state.lang, { maximumFractionDigits: 1 })} MB`;

function go(step: Step) {
  state.step = step;
  state.error = null;
  render();
  window.scrollTo(0, 0);
}

/** A bar per long step; `onProgress(work)` moves its bar without re-rendering the screen. */
const progressBars: Record<Work, HTMLProgressElement | null> = { open: null, index: null };
function progressNode(work: Work, label: string): Node {
  const bar = h("progress", { max: "1000", "aria-label": label });
  const value = state.progress[work];
  if (value !== null) bar.value = value;
  progressBars[work] = bar;
  return h("p", {}, label, bar);
}
function onProgress(work: Work): (perMille: number) => void {
  state.progress[work] = null;
  progressBars[work]?.removeAttribute("value"); // Indeterminate until the first report, not the last run's value.
  return (perMille) => {
    state.progress[work] = perMille;
    const bar = progressBars[work];
    if (bar) bar.value = perMille;
  };
}

async function run<T>(work: () => Promise<T>): Promise<T | undefined> {
  state.busy = true;
  state.error = null;
  render();
  try {
    return await work();
  } catch (e) {
    state.error = isAppError(e) ? e : { code: "io", detail: String(e) };
    return undefined;
  } finally {
    state.busy = false;
    render();
  }
}

function errorBox(): Node | null {
  const e = state.error;
  if (!e) return null;
  const words = t.errors[e.code] ?? e.code;
  // Codes whose English detail adds something the person can use (a table name, a path).
  const withDetail = ["unsupported", "merge_check", "io", "not_found", "network"].includes(e.code);
  return h("div", { class: "error", role: "alert" }, words, withDetail ? h("div", { class: "mono" }, e.detail) : null);
}

async function pickFolder(): Promise<string | null> {
  const chosen = await open({ directory: true, multiple: false });
  return typeof chosen === "string" ? chosen : null;
}

// ---- screens ----

function welcome(): Child[] {
  return [
    h("h1", {}, t.welcomeTitle),
    h("p", { class: "lead" }, t.welcomeLead),
    h("div", { class: "card" }, h("h2", {}, t.rulesTitle), h("ul", {}, ...t.rules.map((r) => h("li", {}, r)))),
    h(
      "label",
      { class: "check" },
      h("input", {
        type: "checkbox",
        checked: state.consent,
        onchange: (e) => {
          state.consent = (e.target as HTMLInputElement).checked;
          render();
        },
      }),
      t.consent,
    ),
    h("div", { class: "row" }, h("button", { class: "primary", disabled: !state.consent, onclick: () => go("get") }, t.next)),
  ];
}

async function listBackups(path: string) {
  const files = await run(() => api.findBackups(path));
  if (files) state.files = files;
  render();
}

function get(): Child[] {
  const rx = state.receive;
  const wifi = h(
    "div",
    { class: "card" },
    h("h2", {}, t.fromWifi),
    h("p", {}, t.fromWifiText),
    !rx &&
      h("p", {}, t.wifiWhere),
    !rx &&
      h(
        "button",
        {
          class: "ghost",
          disabled: state.busy,
          onclick: async () => {
            const parent = await pickFolder();
            if (!parent) return;
            const info = await run(() => api.receiveStart(parent));
            if (info) {
              state.receive = { info, status: await api.receiveStatus() };
              startPolling();
              render();
            }
          },
        },
        t.startWifi,
      ),
    rx && receivePanel(rx.info, rx.status),
  );
  return [
    h("h1", {}, t.getTitle),
    h("p", { class: "lead" }, t.getLead),
    errorBox(),
    h(
      "div",
      { class: "grid" },
      h(
        "div",
        { class: "card" },
        h("h2", {}, t.fromFolder),
        h("p", {}, t.fromFolderText),
        h(
          "div",
          { class: "row" },
          h("button", { class: "ghost", disabled: state.busy, onclick: async () => { const p = await pickFolder(); if (p) await listBackups(p); } }, t.pickFolder),
          h(
            "button",
            {
              class: "ghost",
              disabled: state.busy,
              onclick: async () => {
                const p = await open({ multiple: false, directory: false });
                if (typeof p === "string") await listBackups(p);
              },
            },
            t.pickFile,
          ),
        ),
      ),
      wifi,
    ),
    h("p", { class: "note" }, t.driveNote),
    state.files && filesList(state.files),
    h("div", { class: "row" }, h("button", { class: "ghost", onclick: () => go("welcome") }, t.back)),
  ];
}

function receivePanel(info: ReceiveInfo, status: ReceiveStatus): Node {
  const qr = h("div", { class: "qr", "aria-label": "QR code" });
  qr.innerHTML = info.qr_svg; // Generated by our own Rust code from our own address: no outside text in it.
  const cur = status.current;
  return h(
    "div",
    {},
    status.running && h("p", {}, t.wifiScan),
    status.running && qr,
    status.running && h("p", {}, t.wifiOr, " ", h("code", {}, info.url)),
    status.running && h("p", { class: "note" }, t.wifiWarn),
    status.running && h("p", {}, `${t.wifiFree}: ${mb(info.free)}`),
    cur && h("div", {}, `${t.wifiReceiving}: `, cur.name, h("progress", { max: String(cur.total || 1), value: String(cur.done) })),
    status.files.length > 0 &&
      h("div", {}, h("strong", {}, t.wifiReceived), h("ul", {}, ...status.files.map((f) => h("li", {}, `${f.name} · ${mb(f.bytes)}`)))),
    status.stopped && h("p", {}, t.wifiStopped[status.stopped] ?? status.stopped),
    h(
      "div",
      { class: "row" },
      status.running &&
        h("button", { class: "ghost", onclick: async () => { const s = await api.receiveStop(); if (state.receive) state.receive.status = s; render(); } }, t.wifiStop),
      status.files.length > 0 &&
        h(
          "button",
          {
            class: "primary",
            onclick: async () => {
              const s = await api.receiveStop();
              if (state.receive) state.receive.status = s;
              await listBackups(info.dest);
            },
          },
          t.wifiUseThese,
        ),
    ),
  );
}

function startPolling() {
  window.clearInterval(poll);
  poll = window.setInterval(async () => {
    if (!state.receive) return;
    const status = await api.receiveStatus();
    state.receive.status = status;
    if (state.step === "get") render();
    if (!status.running) window.clearInterval(poll);
  }, 500);
}

function filesList(files: BackupFile[]): Node {
  return h(
    "div",
    { class: "card" },
    h("h2", {}, t.found),
    h(
      "table",
      {},
      ...files.map((f, i) =>
        h(
          "tr",
          {},
          h("td", {}, f.name, i === 0 && f.kind === "crypt15" ? ` (${t.newest})` : ""),
          h("td", { class: "n" }, mb(f.bytes)),
          h("td", {}, t.kind[f.kind]),
          h(
            "td",
            {},
            f.kind === "crypt15" &&
              h("button", { class: "primary small", onclick: () => { state.selected = f; go("key"); } }, t.choose),
          ),
        ),
      ),
    ),
  );
}

function key(): Child[] {
  const f = state.selected;
  if (!f) return [h("p", {}, "–")];
  const count = h("span", {}, `0/64 ${t.keyDigits}`);
  const input = h("input", {
    class: "key",
    type: "password",
    autocomplete: "off",
    spellcheck: "false",
    "aria-label": t.keyTitle,
    oninput: (e) => {
      const n = (e.target as HTMLInputElement).value.replace(/\s/g, "").length;
      count.textContent = `${n}/64 ${t.keyDigits}`;
    },
  });
  const submit = async () => {
    const value = input.value;
    input.value = ""; // The key leaves the page now; Rust wipes it after this one call.
    const summary = await run(() => api.checkBackup(f.path, value, onProgress("open")));
    if (summary) {
      state.summaries = await api.checkedBackups();
      state.current = summary;
      state.chatLimit = 200;
      go("check");
    }
  };
  return [
    h("h1", {}, t.keyTitle),
    h("p", { class: "lead" }, t.keyLead),
    errorBox(),
    h(
      "div",
      { class: "card" },
      h("label", {}, `${t.keyLabel} `, h("strong", {}, f.name)),
      h("form", { onsubmit: (e) => { e.preventDefault(); void submit(); } }, input),
      h("p", {}, count),
      h("p", {}, t.keyNever),
      state.busy && progressNode("open", t.keyOpening),
      h(
        "div",
        { class: "row" },
        h("button", { class: "primary", disabled: state.busy, onclick: () => void submit() }, t.keyOpen),
        h("button", { class: "ghost", onclick: () => go("get") }, t.back),
      ),
    ),
    h(
      "div",
      { class: "card" },
      h("button", { class: "ghost small", onclick: () => { state.showPassword = !state.showPassword; render(); } }, t.passwordQ),
      state.showPassword &&
        h(
          "div",
          {},
          h("h2", {}, t.passwordTitle),
          h("p", {}, t.passwordText),
          h("ol", { class: "steps" }, ...t.passwordSteps.map((s) => h("li", {}, s))),
          h("p", { class: "note" }, t.passwordEdge),
        ),
    ),
  ];
}

function check(): Child[] {
  const s = state.current;
  if (!s) return [];
  const chats = s.chat_list.slice(0, state.chatLimit);
  return [
    h("h1", {}, t.checkTitle, " · ", s.name),
    s.sound ? h("p", { class: "ok" }, "✓ ", t.sound) : h("p", { class: "error" }, t.unsound, " ", s.problems),
    h(
      "div",
      { class: "grid" },
      h("div", { class: "card stat" }, fmt(s.messages), h("small", {}, t.messages)),
      h("div", { class: "card stat" }, fmt(s.chats), h("small", {}, `${t.chats}: ${fmt(s.one_to_one)} ${t.oneToOne}, ${fmt(s.groups)} ${t.groups}`)),
      h("div", { class: "card stat" }, date(s.last_ms), h("small", {}, `${t.from} ${date(s.first_ms)}`)),
      h("div", { class: "card stat" }, mb(s.bytes), h("small", {}, s.name)),
    ),
    h(
      "div",
      { class: "card" },
      h("p", {}, t.masked),
      h(
        "div",
        { class: "list" },
        h(
          "table",
          {},
          h("tr", {}, ...t.chatTable.map((c) => h("th", {}, c))),
          ...chats.map((c) =>
            h(
              "tr",
              {},
              h("td", {}, String(c.number)),
              h("td", {}, `${t.who[c.kind]} ••••${c.ending}`),
              h("td", { class: "n" }, fmt(c.messages)),
              h("td", {}, date(c.last_ms)),
            ),
          ),
        ),
      ),
      s.chat_list.length > state.chatLimit &&
        h("button", { class: "ghost small", onclick: () => { state.chatLimit += 500; render(); } }, t.showMore),
      h("p", {}, t.fingerprint, " ", h("code", {}, s.sha256)),
    ),
    state.summaries.length < 2 && h("p", { class: "note" }, t.mergeNeedsTwo),
    h(
      "div",
      { class: "row" },
      h("button", { class: "ghost", onclick: () => { state.files = null; go("get"); } }, t.checkAnother),
      h("button", { class: "ghost", onclick: () => go("explore") }, t.toExplore),
      h("button", { class: "primary", disabled: state.summaries.length < 2, onclick: () => go("merge") }, t.toMerge),
    ),
  ];
}

// ---- Explore: stats, search and a virtualized chat viewer over the backup's local index ----

const explorer = { built: null as BuildInfo | null, forId: 0, chat: null as number | null, building: false };

/** Builds a snippet's text nodes, matched words in <mark>; never parsed as HTML. */
function snippetNodes(snippet: string): Node[] {
  const out: Node[] = [];
  for (const [i, part] of snippet.split(/[\u0002\u0003]/).entries()) {
    if (part) out.push(i % 2 === 1 ? h("mark", {}, part) : document.createTextNode(part));
  }
  return out;
}

/** One series of bars (no legend: the heading names it), a tooltip per bar, labels at the ends. */
function bars(values: [string, number][], label: string): Node {
  const max = Math.max(1, ...values.map(([, n]) => n));
  const box = h("div", { class: "bars", role: "img", "aria-label": label });
  for (const [name, n] of values) {
    const bar = h("div", { class: "bar", title: `${name}: ${fmt(n)}` });
    bar.style.height = `${Math.max(1, (n / max) * 100)}%`;
    box.append(bar);
  }
  const ends = values.length
    ? h("div", { class: "bar-ends" }, h("span", {}, values[0][0]), h("span", {}, `max ${fmt(max)}`), h("span", {}, values[values.length - 1][0]))
    : null;
  return h("div", {}, h("h2", {}, label), box, ends);
}

function statsView(s: Stats): Node {
  const kinds = s.by_kind.map(([k, n, bytes]) =>
    h("tr", {}, h("td", {}, t.kinds[k] ?? `#${k}`), h("td", { class: "n" }, fmt(n)), h("td", { class: "n" }, bytes ? mb(bytes) : "–")),
  );
  return h(
    "div",
    {},
    h(
      "div",
      { class: "grid" },
      h("div", { class: "card stat" }, fmt(s.messages), h("small", {}, t.messages)),
      h("div", { class: "card stat" }, fmt(s.from_me), h("small", {}, `${t.fromMe} · ${fmt(s.from_others)} ${t.fromOthers}`)),
      h("div", { class: "card stat" }, date(s.last_ms), h("small", {}, `${t.from} ${date(s.first_ms)}`)),
      h("div", { class: "card stat" }, fmt(Math.round(s.audio_seconds / 60)), h("small", {}, t.audioMinutes)),
    ),
    h("div", { class: "card" }, bars(s.by_month, t.byMonth)),
    h("div", { class: "card" }, bars(s.by_hour.map((n, hr) => [`${hr}:00`, n]), t.byHour)),
    h(
      "div",
      { class: "grid" },
      h("div", { class: "card" }, h("h2", {}, t.byKind), h("table", {}, h("tr", {}, ...t.kindTable.map((c) => h("th", {}, c))), ...kinds)),
      s.senders.length > 0 &&
        h(
          "div",
          { class: "card" },
          h("h2", {}, t.senders),
          h("table", {}, ...s.senders.map(([ending, n]) => h("tr", {}, h("td", {}, `••••${ending}`), h("td", { class: "n" }, fmt(n))))),
        ),
    ),
  );
}

const ROW = 64;

/** A list that only has the rows on screen in the page: a chat of 100,000 messages scrolls like one of 100. */
function viewer(id: number, chat: number, len: number, startAt: number): Node {
  const box = h("div", { class: "viewer", tabindex: "0", "aria-label": t.viewer });
  const spacer = h("div", { class: "spacer" });
  spacer.style.height = `${len * ROW}px`;
  box.append(spacer);
  const pages = new Map<number, Line[] | "loading">();
  const PAGE = 200;
  const draw = () => {
    const first = Math.max(0, Math.floor(box.scrollTop / ROW) - 10);
    const last = Math.min(len, first + Math.ceil(box.clientHeight / ROW) + 20);
    const rows: Node[] = [];
    for (let i = first; i < last; i++) {
      const page = Math.floor(i / PAGE);
      const lines = pages.get(page);
      if (lines === undefined) {
        pages.set(page, "loading");
        void api.chatLines(id, chat, page * PAGE, PAGE).then((l) => {
          pages.set(page, l);
          draw();
        });
      }
      const line = Array.isArray(lines) ? lines[i - page * PAGE] : undefined;
      const row = h(
        "div",
        { class: `line ${line?.from_me ? "me" : ""}` },
        line
          ? h(
              "div",
              { class: "bubble" },
              h("small", {}, `${line.from_me ? t.you : line.sender ? `••••${line.sender}` : t.them} · ${new Date(line.ts).toLocaleString(state.lang)}`),
              h("div", { class: "text" }, line.text ?? `[${t.kinds[line.kind] ?? `#${line.kind}`}]`),
            )
          : "…",
      );
      row.style.top = `${i * ROW}px`;
      rows.push(row);
    }
    spacer.replaceChildren(...rows);
  };
  box.addEventListener("scroll", () => requestAnimationFrame(draw));
  requestAnimationFrame(() => {
    box.scrollTop = Math.max(0, startAt * ROW - box.clientHeight / 2);
    draw();
  });
  return box;
}

function explore(): Child[] {
  const s = state.current;
  if (!s) return [];
  const statsBox = h("div", {});
  const results = h("div", { class: "list" });
  const viewerBox = h("div", {});
  const status = explorer.built && explorer.forId === s.id ? h("p", {}, `${t.exploreBuilt} (${fmt(explorer.built.messages)} ${t.messages}, ${(explorer.built.millis / 1000).toFixed(1)} s)`) : progressNode("index", t.exploreBuilding);
  const chats: [number, string][] = [[0, t.wholeBackup], ...s.chat_list.map((c): [number, string] => [c.number, `${c.number} · ${t.who[c.kind]} ••••${c.ending} · ${fmt(c.messages)}`])];
  const showStats = async () => {
    const st = await api.stats(s.id, explorer.chat).catch(() => null);
    if (st) statsBox.replaceChildren(statsView(st));
  };
  const openViewer = async (chat: number, at: number) => {
    const len = await api.chatLen(s.id, chat);
    viewerBox.replaceChildren(h("h2", {}, `${t.viewer} · ${chat}`), viewer(s.id, chat, len, at));
    viewerBox.scrollIntoView({ behavior: "smooth" });
  };
  let seq = 0;
  let timer: number | undefined;
  const search = h("input", {
    type: "search",
    placeholder: t.searchPlaceholder,
    "aria-label": t.searchPlaceholder,
    oninput: () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(async () => {
        const mine = ++seq;
        const started = performance.now();
        const hits = await api.search(s.id, search.value, explorer.chat).catch(() => [] as Hit[]);
        if (mine !== seq) return; // A newer search has started.
        const ms = Math.round(performance.now() - started);
        results.replaceChildren(
          search.value.trim() ? h("p", {}, `${fmt(hits.length)} ${t.searchHits} · ${ms} ms ${hits.length === 200 ? t.searchMore : ""}`) : "",
          ...hits.map((hit) =>
            h(
              "div",
              { class: "hit", role: "button", tabindex: "0", onclick: async () => { const p = await api.messagePosition(s.id, hit.id); if (p) await openViewer(p[0], p[1]); } },
              h("small", {}, `${hit.chat} · ${hit.from_me ? t.you : t.them} · ${new Date(hit.ts).toLocaleString(state.lang)}`),
              h("div", {}, ...snippetNodes(hit.snippet)),
            ),
          ),
        );
      }, 80);
    },
  });
  const scope = select(explorer.chat ?? 0, chats, (v) => {
    explorer.chat = v || null;
    void showStats();
    search.dispatchEvent(new Event("input"));
  });
  void (async () => {
    if (!(explorer.built && explorer.forId === s.id)) {
      if (explorer.building) return; // run() re-renders while it builds: start it once.
      explorer.building = true;
      // Not run(): the build doesn't make the app busy, so other screens stay usable while it runs.
      const built = await api
        .buildIndex(s.id, onProgress("index"))
        .catch((e: unknown) => {
          state.error = isAppError(e) ? e : { code: "io", detail: String(e) };
          return undefined;
        })
        .finally(() => (explorer.building = false));
      if (!built) return render();
      explorer.built = built;
      explorer.forId = s.id;
      render(); // Again, now with the index: shows the stats.
      return;
    }
    await showStats();
  })();
  return [
    h("h1", {}, t.exploreTitle, " · ", s.name),
    errorBox(),
    status,
    h("p", { class: "note" }, t.screenOnly),
    h("div", { class: "card" }, h("label", {}, t.exploreScope, " ", scope), h("div", { class: "row" }, search), results,
      explorer.chat !== null && h("div", { class: "row" }, h("button", { class: "ghost small", onclick: () => void openViewer(explorer.chat!, 0) }, t.openViewer))),
    viewerBox,
    statsBox,
    h("div", { class: "row" }, h("button", { class: "ghost", onclick: () => go("check") }, t.back)),
  ];
}

function select(value: number, options: [number, string][], onchange: (v: number) => void): HTMLSelectElement {
  const el = h("select", { onchange: (e) => onchange(Number((e.target as HTMLSelectElement).value)) });
  for (const [v, label] of options) {
    const o = h("option", { value: String(v) }, label);
    o.selected = v === value;
    el.append(o);
  }
  return el;
}

function merge(): Child[] {
  const m = state.merge;
  const backups = state.summaries;
  if (!backups.some((b) => b.id === m.source)) m.source = backups[0]?.id ?? 0;
  if (!backups.some((b) => b.id === m.target)) m.target = backups.find((b) => b.id !== m.source)?.id ?? 0;
  const chatsOf = (id: number): [number, string][] =>
    (backups.find((b) => b.id === id)?.chat_list ?? [])
      .filter((c) => c.kind === "contact" || c.kind === "contact_lid")
      .map((c) => [c.number, `${c.number} · ${t.who[c.kind]} ••••${c.ending} · ${fmt(c.messages)} ${t.messages}`]);
  const sourceChats = chatsOf(m.source);
  const targetChats = chatsOf(m.target);
  if (!sourceChats.some(([n]) => n === m.sourceChat)) m.sourceChat = sourceChats[0]?.[0] ?? 0;
  if (!targetChats.some(([n]) => n === m.targetChat)) m.targetChat = targetChats[0]?.[0] ?? 0;
  const names: [number, string][] = backups.map((b) => [b.id, `${b.name} (${fmt(b.messages)} ${t.messages})`]);
  const keyInput = h("input", { class: "key", type: "password", autocomplete: "off", spellcheck: "false", "aria-label": t.targetKey });
  const field = (label: string, ...control: Node[]) => h("div", { class: "card" }, h("label", {}, h("strong", {}, label)), ...control);
  const folderField = (label: string, value: string, set: (v: string) => void, optional: boolean) =>
    field(
      label,
      h(
        "div",
        { class: "row" },
        h("code", {}, value || (optional ? t.none : "–")),
        h("button", { class: "ghost small", onclick: async () => { const p = await pickFolder(); if (p) { set(p); render(); } } }, t.chooseFolder),
        optional && value && h("button", { class: "ghost small", onclick: () => { set(""); render(); } }, "×"),
      ),
    );
  const runMerge = async () => {
    const targetKey = keyInput.value;
    keyInput.value = "";
    const outcome = await run(() =>
      api.mergeChat({
        source: m.source,
        source_chat: m.sourceChat,
        target: m.target,
        target_chat: m.targetChat,
        target_key: targetKey,
        out_parent: m.out,
        media: m.media || null,
      }),
    );
    if (outcome) state.outcome = outcome;
    render();
  };
  const compare = async () => {
    const found = await run(() => api.missing(m.source, m.sourceChat, m.target, m.targetChat));
    if (found) state.missing = found;
    render();
  };
  const span = (first: number | null, last: number | null) => (first ? ` (${date(first)} – ${date(last)})` : "");
  const mi = state.missing;
  const o = state.outcome;
  return [
    h("h1", {}, t.mergeTitle),
    h("p", { class: "lead" }, t.mergeLead),
    field(t.source, select(m.source, names, (v) => { m.source = v; state.missing = null; render(); }), h("div", {}, t.chat), select(m.sourceChat, sourceChats, (v) => { m.sourceChat = v; state.missing = null; render(); })),
    field(t.target, select(m.target, names, (v) => { m.target = v; state.missing = null; render(); }), h("div", {}, t.chat), select(m.targetChat, targetChats, (v) => { m.targetChat = v; state.missing = null; render(); })),
    h(
      "div",
      { class: "card" },
      h("button", { class: "ghost small", disabled: state.busy || !m.sourceChat || !m.targetChat || m.source === m.target, onclick: () => void compare() }, t.compare),
      mi && h("p", {}, `${fmt(mi.both)} ${t.inBoth}`),
      mi && h("p", {}, `${fmt(mi.only_a)} ${t.onlySource}${span(mi.only_a_first_ms, mi.only_a_last_ms)}`),
      mi && h("p", {}, `${fmt(mi.only_b)} ${t.onlyTarget}${span(mi.only_b_first_ms, mi.only_b_last_ms)}`),
    ),
    field(t.targetKey, keyInput),
    folderField(t.outFolder, m.out, (v) => (m.out = v), false),
    folderField(t.mediaFolder, m.media, (v) => (m.media = v), true),
    errorBox(),
    state.busy && h("p", {}, t.merging, h("progress", {})),
    o &&
      h(
        "div",
        { class: "card" },
        h("p", { class: "ok" }, "✓ ", t.mergeDone),
        h("p", {}, `${fmt(o.added)} ${t.added}, ${fmt(o.already_there)} ${t.alreadyThere}`),
        o.media_copied !== null && h("p", {}, `${fmt(o.media_copied)} ${t.mediaCopied}, ${fmt(o.media_missing ?? 0)} ${t.mediaMissing}`),
        o.original_untouched && h("p", { class: "ok" }, "✓ ", t.untouched),
        h("p", {}, h("code", {}, o.backup)),
        h("details", {}, h("summary", {}, `${o.checks.length} checks`), h("ul", {}, ...o.checks.map((c) => h("li", { class: "mono" }, c)))),
      ),
    h(
      "div",
      { class: "row" },
      h("button", { class: "ghost", onclick: () => go("check") }, t.back),
      h("button", { class: "primary", disabled: state.busy || !m.out || !m.sourceChat || !m.targetChat || m.source === m.target, onclick: () => void runMerge() }, t.runMerge),
      o && h("button", { class: "primary", onclick: () => go("phone2") }, t.toPhone2),
    ),
  ];
}

function phone2(): Child[] {
  const checks = state.phoneChecks;
  return [
    h("h1", {}, t.phoneTitle),
    h("p", { class: "lead" }, t.phoneLead),
    state.outcome && h("p", {}, h("code", {}, state.outcome.folder)),
    h(
      "ol",
      { class: "steps" },
      ...t.phoneSteps.map(([title, text], i) =>
        h(
          "li",
          {},
          h(
            "label",
            { class: "check" },
            h("input", { type: "checkbox", checked: !!checks[i], onchange: (e) => { checks[i] = (e.target as HTMLInputElement).checked; render(); } }),
            h("span", {}, h("strong", {}, title), h("p", {}, text)),
          ),
        ),
      ),
    ),
    h(
      "div",
      { class: "row" },
      h("button", { class: "ghost", onclick: () => go("merge") }, t.back),
      h("button", { class: "primary", disabled: t.phoneSteps.some((_, i) => !checks[i]), onclick: () => go("done") }, t.phoneDone),
    ),
  ];
}

function done(): Child[] {
  return [
    h("h1", {}, t.doneTitle),
    h("p", { class: "lead" }, t.doneLead),
    state.outcome && h("p", {}, h("code", {}, state.outcome.folder)),
    h(
      "div",
      { class: "row" },
      h(
        "button",
        {
          class: "ghost",
          onclick: async () => {
            for (const s of state.summaries) await api.forgetBackup(s.id);
            Object.assign(state, { summaries: [], current: null, files: null, selected: null, outcome: null, phoneChecks: [] });
            go("welcome");
          },
        },
        t.startOver,
      ),
    ),
  ];
}

const SCREENS: Record<Step, () => Child[]> = { welcome, get, key, check, explore, merge, phone2, done };

function render() {
  t = strings(state.lang);
  document.documentElement.lang = state.lang;
  const nav = document.getElementById("steps")!;
  const at = STEPS.indexOf(state.step);
  nav.replaceChildren(...t.steps.map((name, i) => h("span", { class: i === at ? "on" : i < at ? "past" : "" }, name)));
  document.getElementById("lang")!.textContent = state.lang === "en" ? "ES" : "EN";
  document.getElementById("app")!.replaceChildren(...SCREENS[state.step]().filter((n): n is Node => !!n));
}

document.getElementById("lang")!.addEventListener("click", () => {
  state.lang = state.lang === "en" ? "es" : "en";
  try {
    localStorage.setItem("oopz.lang", state.lang);
  } catch {
    // Storage blocked: the choice lasts until the app closes.
  }
  render();
});

render();
