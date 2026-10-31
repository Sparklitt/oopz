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

/** Choices that only change what the screen shows: how the backup arrives, and which listed file is picked. */
const ui = { method: "folder" as "folder" | "wifi", pick: -1 };

let t: Strings = strings(state.lang);
let poll: number | undefined;

// ---- small DOM helpers ----

type Child = Node | string | null | undefined | false;
/** A screen: its content, plus the action bar under it (Back on the left, the main action last on the right). */
type Screen = { body: Child[]; back?: Child; actions?: Child[] };

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

/** Line icons, 24×24, drawn with the text colour. Built as SVG nodes, not parsed from a string. */
const ICONS = {
  folder: ["M3 7.5A1.5 1.5 0 0 1 4.5 6h4.4l2 2h8.6A1.5 1.5 0 0 1 21 9.5v8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z"],
  file: ["M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z", "M14 3v5h5"],
  wifi: ["M2.5 9a14 14 0 0 1 19 0", "M5.5 12.5a9.5 9.5 0 0 1 13 0", "M8.7 15.9a5 5 0 0 1 6.6 0", "M12 19.5h.01"],
  key: ["M3.5 15.5a4 4 0 1 0 8 0a4 4 0 1 0-8 0", "M10.4 12.6 20 3", "M16.5 6.5l3 3", "M13.8 9.2l2 2"],
  check: ["M5 12.5l4.5 4.5L19 7.5"],
  shield: ["M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z", "M9 12l2 2 4-4"],
  lock: ["M6.5 11h11a1 1 0 0 1 1 1v7.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1V12a1 1 0 0 1 1-1z", "M8.5 11V8a3.5 3.5 0 0 1 7 0v3"],
  search: ["M4 10.5a6.5 6.5 0 1 0 13 0a6.5 6.5 0 1 0-13 0", "M15.5 15.5 20 20"],
  chart: ["M4 20h16", "M7 16v-4", "M12 16V6", "M17 16V9"],
  merge: ["M6 4v4a6 6 0 0 0 6 6h6", "M15 11l3 3-3 3", "M6 20v-3"],
  phone: ["M8 2.5h8a1.5 1.5 0 0 1 1.5 1.5v16a1.5 1.5 0 0 1-1.5 1.5H8A1.5 1.5 0 0 1 6.5 20V4A1.5 1.5 0 0 1 8 2.5z", "M11 18.5h2"],
  download: ["M12 4v11", "M7.5 10.5 12 15l4.5-4.5", "M5 20h14"],
  alert: ["M10.3 4.2 2.6 18a2 2 0 0 0 1.7 3h15.4a2 2 0 0 0 1.7-3L13.7 4.2a2 2 0 0 0-3.4 0z", "M12 9.5v4", "M12 17h.01"],
  info: ["M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0", "M12 11v5", "M12 8h.01"],
  chevron: ["M9 6l6 6-6 6"],
  arrow: ["M5 12h14", "M13 6l6 6-6 6"],
  close: ["M6 6l12 12", "M18 6 6 18"],
} as const;
type IconName = keyof typeof ICONS;

function icon(name: IconName, cls = "icon"): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  for (const [k, v] of Object.entries({ class: cls, viewBox: "0 0 24 24", "aria-hidden": "true", focusable: "false" })) svg.setAttribute(k, v);
  for (const d of ICONS[name]) {
    const p = document.createElementNS(ns, "path");
    p.setAttribute("d", d);
    svg.append(p);
  }
  return svg;
}

const fmt = (n: number) => n.toLocaleString(state.lang);
const date = (ms: number | null) => (ms ? new Date(ms).toLocaleDateString(state.lang) : "–");
// Under 1 MB in kB, so a small file doesn't read "0 MB".
const mb = (bytes: number) =>
  bytes > 0 && bytes < 1e6 ? `${Math.max(1, Math.round(bytes / 1e3)).toLocaleString(state.lang)} kB` : `${(bytes / 1e6).toLocaleString(state.lang, { maximumFractionDigits: 1 })} MB`;

/** The screen's title, its one-line explanation, and (optionally) the file it is about. */
function head(title: string, lead?: Child, about?: string): Node {
  return h(
    "header",
    { class: "head" },
    h("h1", {}, title),
    about && h("p", { class: "about" }, icon("file"), h("span", {}, about)),
    lead && h("p", { class: "lead" }, lead),
  );
}

/** A titled group of content on the page itself (no box): a heading, then whatever follows. */
function section(title: string | null, ...children: Child[]): HTMLElement {
  return h("section", { class: "sec" }, title && h("h2", {}, title), ...children);
}

/** A big number with its small label: the numbers in a row read at a glance, without boxes. */
function stat(value: string, label: string, sub?: string): Node {
  return h("div", { class: "stat" }, h("span", { class: "num" }, value), h("span", { class: "lbl" }, label), sub && h("span", { class: "sub" }, sub));
}

function notice(kind: "note" | "ok", ...children: Child[]): Node {
  return h("p", { class: kind }, icon(kind === "ok" ? "check" : "info"), h("span", {}, ...children));
}

/** A folder path shown as text, with a folder icon in front. */
function pathBox(path: string, empty = false): Node {
  return h("div", { class: `path${empty ? " empty" : ""}` }, icon("folder"), h("code", {}, path));
}

/** A label tied to its control by id, the control, and an optional line of help under it. */
function field(id: string, label: Child, control: Node, hint?: Child): Node {
  if (control instanceof HTMLElement) control.id = id;
  const hintId = `${id}-hint`;
  if (hint && control instanceof HTMLElement) control.setAttribute("aria-describedby", hintId);
  return h("div", { class: "field" }, h("label", { for: id }, label), control, hint && h("p", { class: "hint", id: hintId }, hint));
}

/** The merge form's key field outlives a render (picking a folder re-renders the form), never the screen: it is
 *  emptied when sent and when the person leaves Merge. The key stays in this field only, never in `state`. */
let mergeKey: HTMLInputElement | undefined;

function go(step: Step) {
  if (step !== "merge" && mergeKey) mergeKey.value = "";
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
  return h("div", { class: "progress" }, h("p", {}, label), bar);
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
  return h("div", { class: "error", role: "alert" }, icon("alert"), h("div", {}, words, withDetail ? h("div", { class: "mono" }, e.detail) : null));
}

async function pickFolder(): Promise<string | null> {
  const chosen = await open({ directory: true, multiple: false });
  return typeof chosen === "string" ? chosen : null;
}

// ---- screens ----

const RULE_ICONS: IconName[] = ["key", "shield", "file", "lock"];

function welcome(): Screen {
  return {
    body: [
      head(t.welcomeTitle, t.welcomeLead),
      section(t.rulesTitle, h("ul", { class: "rules" }, ...t.rules.map((r, i) => h("li", {}, h("span", { class: "badge" }, icon(RULE_ICONS[i] ?? "check")), h("span", {}, r))))),
      h(
        "label",
        { class: "check consent" },
        h("input", {
          type: "checkbox",
          checked: state.consent,
          onchange: (e) => {
            state.consent = (e.target as HTMLInputElement).checked;
            render();
          },
        }),
        h("span", {}, t.consent),
      ),
    ],
    actions: [h("button", { class: "primary", disabled: !state.consent, onclick: () => go("get") }, t.next)],
  };
}

async function listBackups(path: string) {
  const files = await run(() => api.findBackups(path));
  if (files) {
    state.files = files;
    ui.pick = files.findIndex((f) => f.kind === "crypt15");
  }
  render();
}

/** A choice between ways or files: a radio button dressed as a row or a tile, so arrows and Space work as usual. */
function option(name: string, checked: boolean, disabled: boolean, onpick: () => void, ...content: Child[]): Node {
  return h(
    "label",
    { class: `option${checked ? " on" : ""}${disabled ? " off" : ""}` },
    h("input", { type: "radio", name, class: "sr-only", checked, disabled, onchange: onpick }),
    ...content,
  );
}

function get(): Screen {
  const rx = state.receive;
  const method = (m: typeof ui.method, ico: IconName, title: string, hint: string) =>
    option("method", ui.method === m, false, () => { ui.method = m; render(); }, h("span", { class: "badge" }, icon(ico)), h("span", { class: "option-text" }, h("strong", {}, title), h("span", {}, hint)));
  const folderPanel = h(
    "div",
    { class: "panel" },
    h("p", {}, t.fromFolderText),
    h(
      "div",
      { class: "row" },
      h("button", { class: "secondary", disabled: state.busy, onclick: async () => { const p = await pickFolder(); if (p) await listBackups(p); } }, icon("folder"), t.pickFolder),
      h(
        "button",
        {
          class: "secondary",
          disabled: state.busy,
          onclick: async () => {
            const p = await open({ multiple: false, directory: false });
            if (typeof p === "string") await listBackups(p);
          },
        },
        icon("file"),
        t.pickFile,
      ),
    ),
  );
  const wifiPanel = h(
    "div",
    { class: "panel" },
    !rx && h("p", {}, t.fromWifiText),
    !rx && h("p", { class: "hint" }, t.wifiWhere),
    !rx &&
      h(
        "div",
        { class: "row" },
        h(
          "button",
          {
            class: "secondary",
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
          icon("wifi"),
          t.startWifi,
        ),
      ),
    rx && receivePanel(rx.info, rx.status),
  );
  const files = state.files;
  const picked = files?.[ui.pick];
  return {
    body: [
      head(t.getTitle, t.getLead),
      errorBox(),
      h(
        "div",
        { class: "options", role: "radiogroup", "aria-label": t.getTitle },
        method("folder", "folder", t.fromFolder, t.fromFolderHint),
        method("wifi", "wifi", t.fromWifi, t.fromWifiHint),
      ),
      ui.method === "folder" ? folderPanel : wifiPanel,
      notice("note", t.driveNote),
      files && filesList(files),
    ],
    back: h("button", { class: "secondary", onclick: () => go("welcome") }, t.back),
    actions: [
      h(
        "button",
        { class: "primary", disabled: !picked || picked.kind !== "crypt15", onclick: () => { if (picked) { state.selected = picked; go("key"); } } },
        t.choose,
      ),
    ],
  };
}

function receivePanel(info: ReceiveInfo, status: ReceiveStatus): Node {
  const qr = h("div", { class: "qr", "aria-label": "QR code" });
  qr.innerHTML = info.qr_svg; // Generated by our own Rust code from our own address: no outside text in it.
  const cur = status.current;
  return h(
    "div",
    { class: "receive" },
    status.running &&
      h(
        "div",
        { class: "qr-row" },
        qr,
        h(
          "div",
          {},
          h("p", { class: "strong" }, t.wifiScan),
          h("p", {}, t.wifiOr),
          h("p", {}, h("code", {}, info.url)),
          h("p", { class: "hint" }, `${t.wifiFree}: ${mb(info.free)}`),
        ),
      ),
    status.running && notice("note", t.wifiWarn),
    cur && h("div", { class: "progress" }, h("p", {}, `${t.wifiReceiving}: `, cur.name), h("progress", { max: String(cur.total || 1), value: String(cur.done) })),
    status.files.length > 0 &&
      h(
        "div",
        {},
        h("h3", {}, t.wifiReceived),
        h("ul", { class: "plain" }, ...status.files.map((f) => h("li", {}, icon("check", "icon ok-ico"), h("span", {}, f.name), h("span", { class: "n" }, mb(f.bytes))))),
      ),
    status.stopped && h("p", {}, t.wifiStopped[status.stopped] ?? status.stopped),
    h(
      "div",
      { class: "row" },
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
      status.running &&
        h("button", { class: "secondary", onclick: async () => { const s = await api.receiveStop(); if (state.receive) state.receive.status = s; render(); } }, t.wifiStop),
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
    "fieldset",
    { class: "sec files" },
    h("legend", {}, t.found),
    h("p", { class: "hint" }, t.foundHint),
    h(
      "div",
      { class: "list-options" },
      ...files.map((f, i) => {
        const opens = f.kind === "crypt15";
        return option(
          "backup",
          i === ui.pick,
          !opens,
          () => { ui.pick = i; render(); },
          icon("file"),
          h(
            "span",
            { class: "option-text" },
            h("strong", {}, f.name, i === 0 && opens && h("span", { class: "tag" }, t.newest)),
            h("span", {}, t.kind[f.kind]),
          ),
          h("span", { class: "n" }, mb(f.bytes)),
        );
      }),
    ),
  );
}

function key(): Screen {
  const f = state.selected;
  if (!f) return { body: [h("p", {}, "–")], back: h("button", { class: "secondary", onclick: () => go("get") }, t.back) };
  const count = h("span", {}, `0/64 ${t.keyDigits}`);
  const input = h("input", {
    class: "key",
    type: "password",
    autocomplete: "off",
    spellcheck: "false",
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
  return {
    body: [
      head(t.keyTitle, t.keyLead, f.name),
      errorBox(),
      h(
        "form",
        { onsubmit: (e) => { e.preventDefault(); void submit(); } },
        field("key-input", t.keyTitle, input, h("span", { class: "hint-row" }, count, h("span", { class: "safe" }, icon("shield"), t.keyNever))),
      ),
      state.busy && progressNode("open", t.keyOpening),
      h(
        "div",
        { class: "disclosure" },
        h(
          "button",
          { class: "link", "aria-expanded": state.showPassword ? "true" : "false", onclick: () => { state.showPassword = !state.showPassword; render(); } },
          icon("chevron", "icon chev"),
          t.passwordQ,
        ),
        state.showPassword &&
          h(
            "div",
            { class: "disclosed" },
            h("h3", {}, t.passwordTitle),
            h("p", {}, t.passwordText),
            h("ol", { class: "numbered" }, ...t.passwordSteps.map((s) => h("li", {}, s))),
            notice("note", t.passwordEdge),
          ),
      ),
    ],
    back: h("button", { class: "secondary", onclick: () => go("get") }, t.back),
    actions: [h("button", { class: "primary", disabled: state.busy, onclick: () => void submit() }, icon("key"), t.keyOpen)],
  };
}

function check(): Screen {
  const s = state.current;
  if (!s) return { body: [] };
  const chats = s.chat_list.slice(0, state.chatLimit);
  return {
    body: [
      head(t.checkTitle, null, s.name),
      s.sound ? notice("ok", t.sound) : h("div", { class: "error", role: "alert" }, icon("alert"), h("div", {}, t.unsound, " ", s.problems)),
      h(
        "div",
        { class: "stats" },
        stat(fmt(s.messages), t.messages),
        stat(fmt(s.chats), t.chats, `${fmt(s.one_to_one)} ${t.oneToOne}, ${fmt(s.groups)} ${s.groups === 1 ? t.group : t.groups}`),
        stat(date(s.last_ms), t.lastMessage, `${t.from} ${date(s.first_ms)}`),
        stat(mb(s.bytes), t.fileSize),
      ),
      section(
        t.chatsTitle,
        h("p", { class: "hint" }, t.masked),
        h(
          "div",
          { class: "list" },
          h(
            "table",
            {},
            h("thead", {}, h("tr", {}, ...t.chatTable.map((c, i) => h("th", { class: i === 2 ? "n" : "" }, c)))),
            h(
              "tbody",
              {},
              ...chats.map((c) =>
                h(
                  "tr",
                  {},
                  h("td", { class: "dim" }, String(c.number)),
                  h("td", {}, `${t.who[c.kind]} ••••${c.ending}`),
                  h("td", { class: "n" }, fmt(c.messages)),
                  h("td", {}, date(c.last_ms)),
                ),
              ),
            ),
          ),
        ),
        s.chat_list.length > state.chatLimit &&
          h("div", { class: "row" }, h("button", { class: "secondary small", onclick: () => { state.chatLimit += 500; render(); } }, t.showMore)),
      ),
      h("dl", { class: "meta" }, h("dt", {}, t.fingerprint), h("dd", {}, h("code", {}, s.sha256))),
      state.summaries.length < 2 && notice("note", t.mergeNeedsTwo),
    ],
    back: h("button", { class: "secondary", onclick: () => { state.files = null; go("get"); } }, t.checkAnother),
    actions: [
      h("button", { class: "secondary", onclick: () => go("explore") }, icon("search"), t.toExplore),
      h("button", { class: "primary", disabled: state.summaries.length < 2, onclick: () => go("merge") }, icon("merge"), t.toMerge),
    ],
  };
}

// ---- Explore: stats, search and a virtualized chat viewer over the backup's local index ----

const explorer = { built: null as BuildInfo | null, forId: 0, chat: null as number | null, building: false, saved: "" };

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
  return h("div", { class: "chart" }, h("h3", {}, label), box, ends);
}

function statsView(s: Stats): Node {
  const kinds = s.by_kind.map(([k, n, bytes]) =>
    h("tr", {}, h("td", {}, t.kinds[k] ?? `#${k}`), h("td", { class: "n" }, fmt(n)), h("td", { class: "n" }, bytes ? mb(bytes) : "–")),
  );
  return section(
    t.statsTitle,
    h(
      "div",
      { class: "stats" },
      stat(fmt(s.messages), t.messages),
      stat(fmt(s.from_me), t.fromMe, `${fmt(s.from_others)} ${t.fromOthers}`),
      stat(date(s.last_ms), t.lastMessage, `${t.from} ${date(s.first_ms)}`),
      stat(fmt(Math.round(s.audio_seconds / 60)), t.audioMinutes),
    ),
    bars(s.by_month, t.byMonth),
    bars(s.by_hour.map((n, hr) => [`${hr}:00`, n]), t.byHour),
    h(
      "div",
      { class: "cols" },
      h(
        "div",
        {},
        h("h3", {}, t.byKind),
        h(
          "table",
          {},
          h("thead", {}, h("tr", {}, ...t.kindTable.map((c, i) => h("th", { class: i > 0 ? "n" : "" }, c)))),
          h("tbody", {}, ...kinds),
        ),
      ),
      s.senders.length > 0 &&
        h(
          "div",
          {},
          h("h3", {}, t.senders),
          h("table", {}, h("tbody", {}, ...s.senders.map(([ending, n]) => h("tr", {}, h("td", {}, `••••${ending}`), h("td", { class: "n" }, fmt(n)))))),
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

function explore(): Screen {
  const s = state.current;
  if (!s) return { body: [] };
  const statsBox = h("div", {});
  const results = h("div", { class: "results" });
  const viewerBox = h("div", {});
  const status = explorer.built && explorer.forId === s.id ? notice("ok", `${t.exploreBuilt} (${fmt(explorer.built.messages)} ${t.messages}, ${(explorer.built.millis / 1000).toFixed(1)} s)`) : progressNode("index", t.exploreBuilding);
  const chats: [number, string][] = [[0, t.wholeBackup], ...s.chat_list.map((c): [number, string] => [c.number, `${c.number} · ${t.who[c.kind]} ••••${c.ending} · ${fmt(c.messages)}`])];
  const showStats = async () => {
    const st = await api.stats(s.id, explorer.chat).catch(() => null);
    if (st) statsBox.replaceChildren(statsView(st));
  };
  const openViewer = async (chat: number, at: number) => {
    const len = await api.chatLen(s.id, chat);
    viewerBox.replaceChildren(section(`${t.viewer} · ${chat}`, viewer(s.id, chat, len, at)));
    viewerBox.scrollIntoView({ behavior: "smooth" });
  };
  const saveExport = async () => {
    const dir = await pickFolder();
    if (!dir) return;
    const chat = explorer.chat;
    const heading = chats.find(([n]) => n === (chat ?? 0))?.[1] ?? t.wholeBackup;
    const saved = await run(() =>
      api.exportPage(s.id, chat, {
        lang: state.lang, heading, messages: t.messages, sent: t.fromMe, received: t.fromOthers, dates: t.dates,
        by_month: t.byMonth, by_hour: t.byHour, by_kind: t.byKind, audio_minutes: t.audioMinutes, senders: t.senders,
        you: t.you, note: t.exportNote, kinds: t.kinds,
      }, dir),
    );
    if (saved) explorer.saved = saved;
    render();
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
          search.value.trim() ? h("p", { class: "hint" }, `${fmt(hits.length)} ${hits.length === 1 ? t.searchHit : t.searchHits} · ${ms} ms ${hits.length === 200 ? t.searchMore : ""}`) : "",
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
  // Filled here, not by render(): picking a chat mustn't rebuild the search box under the person's typing.
  const viewerRow = h("div", { class: "row" });
  const showViewerButton = () =>
    viewerRow.replaceChildren(...(explorer.chat === null ? [] : [h("button", { class: "secondary small", onclick: () => void openViewer(explorer.chat!, 0) }, t.openViewer)]));
  showViewerButton();
  const scope = select(explorer.chat ?? 0, chats, (v) => {
    explorer.chat = v || null;
    showViewerButton();
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
  const ready = explorer.built && explorer.forId === s.id;
  return {
    body: [
      head(t.exploreTitle, null, s.name),
      errorBox(),
      status,
      h("p", { class: "safe" }, icon("shield"), t.screenOnly),
      section(
        null,
        h(
          "div",
          { class: "toolbar" },
          field("explore-scope", t.exploreScope, scope),
          h("div", { class: "field grow" }, h("span", { class: "label", "aria-hidden": "true" }, t.searchLabel), h("div", { class: "search" }, icon("search"), search)),
        ),
        results,
        viewerRow,
      ),
      viewerBox,
      statsBox,
      ready &&
        section(
          t.exportTitle,
          h("p", {}, t.exportLead),
          notice("note", t.exportNote),
          h("div", { class: "row" }, h("button", { class: "secondary", disabled: state.busy, onclick: () => void saveExport() }, icon("download"), t.exportButton)),
          explorer.saved && h("p", { class: "ok" }, icon("check"), h("span", {}, `${t.exportSaved}: `, h("code", {}, explorer.saved))),
        ),
    ],
    back: h("button", { class: "secondary", onclick: () => go("check") }, t.back),
  };
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

function merge(): Screen {
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
  const keyInput = (mergeKey ??= h("input", { class: "key", type: "password", autocomplete: "off", spellcheck: "false" }));
  const folderField = (id: string, label: string, hint: string, value: string, set: (v: string) => void, optional: boolean) =>
    h(
      "div",
      { class: "field" },
      h("span", { class: "label", id }, label),
      h(
        "div",
        { class: "row tight" },
        pathBox(value || (optional ? t.none : t.noFolder), !value),
        h("button", { class: "secondary", "aria-describedby": id, onclick: async () => { const p = await pickFolder(); if (p) { set(p); render(); } } }, icon("folder"), t.chooseFolder),
        optional && value && h("button", { class: "secondary icon-only", "aria-label": t.clear, title: t.clear, onclick: () => { set(""); render(); } }, icon("close")),
      ),
      h("p", { class: "hint" }, hint),
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
    if (outcome) window.scrollTo(0, 0); // The outcome is shown above the form.
  };
  const compare = async () => {
    const found = await run(() => api.missing(m.source, m.sourceChat, m.target, m.targetChat));
    if (found) state.missing = found;
    render();
  };
  const span = (first: number | null, last: number | null) => (first ? `${date(first)} – ${date(last)}` : undefined);
  const mi = state.missing;
  const o = state.outcome;
  const ready = !state.busy && !!m.out && !!m.sourceChat && !!m.targetChat && m.source !== m.target;
  const side = (id: string, title: string, backup: number, chat: number, chats: [number, string][], setBackup: (v: number) => void, setChat: (v: number) => void) =>
    h(
      "div",
      { class: "side-pick" },
      h("h3", {}, title),
      field(`${id}-backup`, t.backup, select(backup, names, (v) => { setBackup(v); state.missing = null; render(); })),
      field(`${id}-chat`, t.chat, select(chat, chats, (v) => { setChat(v); state.missing = null; render(); })),
    );
  return {
    body: [
      head(t.mergeTitle, t.mergeLead),
      o &&
        section(
          null,
          notice("ok", t.mergeDone),
          h(
            "div",
            { class: "stats" },
            stat(fmt(o.added), t.added),
            stat(fmt(o.already_there), t.alreadyThere),
            o.media_copied !== null && stat(fmt(o.media_copied), t.mediaCopied, `${fmt(o.media_missing ?? 0)} ${t.mediaMissing}`),
          ),
          o.original_untouched && notice("ok", t.untouched),
          pathBox(o.backup),
          h("details", {}, h("summary", {}, `${o.checks.length} checks`), h("ul", {}, ...o.checks.map((c) => h("li", { class: "mono" }, c)))),
        ),
      section(
        t.mergeWhich,
        h(
          "div",
          { class: "pair" },
          side("source", t.source, m.source, m.sourceChat, sourceChats, (v) => (m.source = v), (v) => (m.sourceChat = v)),
          h("span", { class: "pair-arrow" }, icon("arrow")),
          side("target", t.target, m.target, m.targetChat, targetChats, (v) => (m.target = v), (v) => (m.targetChat = v)),
        ),
        h(
          "div",
          { class: "row" },
          h("button", { class: "secondary", disabled: state.busy || !m.sourceChat || !m.targetChat || m.source === m.target, onclick: () => void compare() }, icon("search"), t.compare),
        ),
        mi &&
          h(
            "div",
            { class: "stats" },
            stat(fmt(mi.only_a), t.onlySource, span(mi.only_a_first_ms, mi.only_a_last_ms)),
            stat(fmt(mi.only_b), t.onlyTarget, span(mi.only_b_first_ms, mi.only_b_last_ms)),
            stat(fmt(mi.both), t.inBoth),
          ),
      ),
      section(
        t.mergeWhere,
        field("target-key", t.targetKey, keyInput, t.targetKeyHint),
        folderField("out-label", t.outFolder, t.outFolderHint, m.out, (v) => (m.out = v), false),
        folderField("media-label", t.mediaFolder, t.mediaFolderHint, m.media, (v) => (m.media = v), true),
      ),
      errorBox(),
      state.busy && h("div", { class: "progress" }, h("p", {}, t.merging), h("progress", {})),
    ],
    back: h("button", { class: "secondary", onclick: () => go("check") }, t.back),
    actions: [
      h("button", { class: o ? "secondary" : "primary", disabled: !ready, onclick: () => void runMerge() }, icon("merge"), t.runMerge),
      o && h("button", { class: "primary", onclick: () => go("phone2") }, icon("phone"), t.toPhone2),
    ],
  };
}

function phone2(): Screen {
  const checks = state.phoneChecks;
  const ticked = t.phoneSteps.filter((_, i) => checks[i]).length;
  return {
    body: [
      head(t.phoneTitle, t.phoneLead),
      state.outcome && pathBox(state.outcome.folder),
      h(
        "ol",
        { class: "checklist" },
        ...t.phoneSteps.map(([title, text], i) =>
          h(
            "li",
            { class: checks[i] ? "done" : "" },
            h(
              "label",
              { class: "check" },
              h("input", { type: "checkbox", checked: !!checks[i], onchange: (e) => { checks[i] = (e.target as HTMLInputElement).checked; render(); } }),
              h("span", { class: "check-text" }, h("strong", {}, `${i + 1}. ${title}`), h("span", {}, text)),
            ),
          ),
        ),
      ),
    ],
    back: h("button", { class: "secondary", onclick: () => go("merge") }, t.back),
    actions: [
      h("span", { class: "count" }, `${ticked}/${t.phoneSteps.length} ${t.ticked}`),
      h("button", { class: "primary", disabled: t.phoneSteps.some((_, i) => !checks[i]), onclick: () => go("done") }, t.phoneDone),
    ],
  };
}

function done(): Screen {
  return {
    body: [
      h("div", { class: "done-mark" }, icon("check")),
      head(t.doneTitle, t.doneLead),
      state.outcome && pathBox(state.outcome.folder),
    ],
    actions: [
      h(
        "button",
        {
          class: "secondary",
          onclick: async () => {
            for (const s of state.summaries) await api.forgetBackup(s.id);
            Object.assign(state, { summaries: [], current: null, files: null, selected: null, outcome: null, phoneChecks: [] });
            go("welcome");
          },
        },
        t.startOver,
      ),
    ],
  };
}

const SCREENS: Record<Step, () => Screen> = { welcome, get, key, check, explore, merge, phone2, done };
const LANGS: [Lang, string][] = [["en", "English"], ["es", "Español"]];

function render() {
  t = strings(state.lang);
  document.documentElement.lang = state.lang;
  const at = STEPS.indexOf(state.step);
  document.getElementById("steps")!.replaceChildren(
    ...t.steps.map((name, i) =>
      h(
        "li",
        { class: i === at ? "on" : i < at ? "past" : "", "aria-current": i === at ? "step" : undefined },
        h("span", { class: "mark", "aria-hidden": "true" }, i < at ? icon("check") : String(i + 1)),
        h("span", {}, name),
        i < at && h("span", { class: "sr-only" }, ` (${t.stepDone})`),
      ),
    ),
  );
  document.getElementById("lang")!.replaceChildren(
    ...LANGS.map(([lang, name]) =>
      h("button", { type: "button", lang, "aria-pressed": state.lang === lang ? "true" : "false", onclick: () => setLang(lang) }, name),
    ),
  );
  const screen = SCREENS[state.step]();
  const app = document.getElementById("app")!;
  app.dataset.screen = state.step;
  app.replaceChildren(...screen.body.filter((n): n is Node => !!n));
  const bar = document.getElementById("actions")!;
  const right = (screen.actions ?? []).filter((n): n is Node => !!n);
  bar.hidden = !screen.back && right.length === 0;
  bar.replaceChildren(h("div", { class: "actions-in" }, screen.back || h("span", {}), h("div", { class: "right" }, ...right)));
}

function setLang(lang: Lang) {
  state.lang = lang;
  try {
    localStorage.setItem("oopz.lang", state.lang);
  } catch {
    // Storage blocked: the choice lasts until the app closes.
  }
  render();
}

render();
