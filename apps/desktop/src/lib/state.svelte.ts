// The app's state and the actions that change it. Keys are never kept here: each key field holds its own value
// and empties it as soon as it is sent.
import type { AdbDevice, AppError, Backend, BackupFile, BuildInfo, MergeOutcome, Missing, PullInfo, PullStatus, ReceiveInfo, ReceiveStatus, Step, Summary, UpdateFound } from "./backend/types";
import { isAppError } from "./backend/types";
import { initialLang, usePhoneWords, type Lang } from "./i18n";

export type View = "add" | "receive" | "adb" | "overview" | "chats" | "search" | "insights" | "compare" | "restore" | "phone" | "done" | "settings" | "backups" | "tools" | "diagnostics" | "library" | "lchats" | "lsearch" | "linsights" | "contacts" | "select" | "timeline" | "librarymenu";
export type SheetName = "welcome" | "found" | "unlock" | "export" | "update" | "libexport";
export type Theme = "system" | "light" | "dark";

interface IndexState {
  built: BuildInfo | null;
  /** The last progress report, or null before the first one. */
  progress: Step | null;
  error: AppError | null;
  /** The person cancelled the build: shown with a Try again, not as an error. */
  stopped: boolean;
}

function load(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function save(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage blocked: the choice lasts until the app closes.
  }
}

const savedTheme = load("oopz.theme");

export const app = $state({
  lang: initialLang() as Lang,
  theme: (savedTheme === "light" || savedTheme === "dark" ? savedTheme : "system") as Theme,
  /** Chats shown by kind and masked id instead of their names (off unless switched on, remembered). */
  hideNames: load("oopz.hideNames") === "on",
  view: "add" as View,
  // The welcome sheet until the person has agreed once; if storage fails, it asks again next time.
  sheet: (load("oopz.consent") === "yes" ? null : "welcome") as SheetName | null,
  /** Error of the current screen's last action, shown on that screen. */
  error: null as AppError | null,
  busy: false,
  files: null as BackupFile[] | null,
  /** The file the unlock sheet opens. */
  unlocking: null as BackupFile | null,
  summaries: [] as Summary[],
  currentId: null as number | null,
  /** The chat open in Chats, and the message to show in it (a search result), if any. */
  chat: null as number | null,
  jump: null as { chat: number; pos: number } | null,
  /** What Search and Insights look at: a chat, or null for the whole backup. */
  searchChat: null as number | null,
  query: "",
  index: {} as Record<number, IndexState>,
  /** The Wi-Fi page: `parent` is the folder picked (to start again on another address), `started` when (for tips). */
  receive: null as { info: ReceiveInfo; status: ReceiveStatus; parent: string; started: number } | null,
  /** Copying with adb: whether adb is on this computer, the phones it sees (null until asked), the one picked. */
  adb: { found: false, devices: null as AdbDevice[] | null, serial: "", pull: null as { info: PullInfo; status: PullStatus } | null },
  /** Android: the private folder the last picked backup was copied into; results are made there, then saved. */
  privateDir: null as string | null,
  /** The Media (or WhatsApp) folder photos are shown from, picked by the person; null: placeholders. */
  mediaFolder: null as string | null,
  /** The chat the export sheet saves, or null for the whole backup. */
  exportChat: null as number | null,
  merge: { source: 0, sourceChat: 0, target: 0, targetChat: 0, out: "", media: "" },
  missing: null as Missing | null,
  outcome: null as MergeOutcome | null,
  phoneChecks: [] as boolean[],
  updates: {
    auto: load("oopz.updates") === "on",
    status: "idle" as "idle" | "checking" | "none" | "found" | "installing" | "failed",
    found: null as UpdateFound | null,
    /** When the last check happened, in any run (Rust keeps it), or null if never. */
    lastChecked: null as number | null,
    error: null as AppError | null,
    progress: null as Step | null,
  },
});

// The page's language from the start, so the browser doesn't offer to translate a page it thinks is English.
document.documentElement.lang = app.lang;

let backend: Backend;
export function setBackend(b: Backend) {
  backend = b;
  usePhoneWords(onPhone());
  // For the CSS: on Android the page starts below the status bar.
  document.documentElement.dataset.platform = b.platform;
}
/** The app runs on the phone itself: no receiving over Wi-Fi from a phone. */
export const onPhone = () => backend.platform === "android";
/** The backend, for components that call it directly (reads that change no state). */
export const be = () => backend;

export const current = () => app.summaries.find((s) => s.id === app.currentId) ?? null;

export function toError(e: unknown, code = "io"): AppError {
  return isAppError(e) ? e : { code, detail: String(e) };
}

/** The person pressed Cancel: the work stopped as asked, which is not an error to show. */
export const isCancelled = (e: unknown) => isAppError(e) && e.code === "cancelled";

/** Asks Rust to stop the check or index build that is running. */
export const cancelWork = () => backend.cancelWork().catch(() => {});

export function go(view: View) {
  app.view = view;
  app.error = null;
}

/** Runs one action of the screen: busy while it runs, its error shown on the screen. */
export async function run<T>(work: () => Promise<T>): Promise<T | undefined> {
  app.busy = true;
  app.error = null;
  try {
    return await work();
  } catch (e) {
    app.error = toError(e);
    return undefined;
  } finally {
    app.busy = false;
  }
}

export function setLang(lang: Lang) {
  app.lang = lang;
  document.documentElement.lang = lang;
  save("oopz.lang", lang);
}

/** The person ticked "these are my own backups": not asked again at the next start. */
export function giveConsent() {
  save("oopz.consent", "yes");
}

export function setHideNames(on: boolean) {
  app.hideNames = on;
  save("oopz.hideNames", on ? "on" : "off");
}

export function setTheme(theme: Theme) {
  app.theme = theme;
  save("oopz.theme", theme);
}

export async function listBackups(path: string) {
  const files = await run(() => backend.findBackups(path));
  if (files) {
    app.files = files;
    app.sheet = "found";
  }
}

/** Android: the picked files (content:// references: the backup, and WhatsApp's contacts file for names) are copied
 *  into one private folder, then the largest, the backup, is opened like a path; Rust finds the contacts beside it. */
export async function importPicked() {
  const uris = await backend.pickFiles();
  if (uris.length === 0) return;
  const paths = await run(() => backend.importBackup(uris));
  const path = paths?.[0];
  if (!path) return;
  app.privateDir = path.replace(/[\\/][^\\/]*$/, "");
  await dropBackup(path);
}

/** A file or folder dropped on Add backup: one crypt15 file goes straight to its key; anything else is listed. */
export async function dropBackup(path: string) {
  if (!/\.crypt15$/i.test(path)) return listBackups(path);
  const files = await run(() => backend.findBackups(path));
  if (!files) return;
  app.files = files;
  const only = files.length === 1 && files[0].kind === "crypt15" ? files[0] : null;
  app.unlocking = only;
  app.sheet = only ? "unlock" : "found";
}

/** After the unlock sheet opened a backup: it becomes the current one. */
export async function opened(summary: Summary) {
  app.summaries = await backend.checkedBackups();
  app.currentId = summary.id;
  app.chat = app.searchChat = null;
  app.sheet = null;
  go("overview");
  // Chats, Search and Insights need the index: start it now, in the background, so it is ready sooner.
  void ensureIndex(summary.id);
}

export function selectBackup(id: number) {
  if (app.currentId !== id) app.chat = app.searchChat = null;
  app.currentId = id;
  if (!["overview", "chats", "search", "insights"].includes(app.view)) go("overview");
}

export async function closeBackup(id: number) {
  await run(() => backend.forgetBackup(id));
  delete app.index[id];
  app.summaries = app.summaries.filter((s) => s.id !== id);
  if (app.currentId === id) {
    app.currentId = app.summaries[0]?.id ?? null;
    app.chat = app.searchChat = null;
    go(app.currentId === null ? "add" : "overview");
  }
}

/** Builds a backup's search index once; it doesn't make the app busy, so other screens stay usable meanwhile. */
export async function ensureIndex(id: number) {
  const known = app.index[id];
  if (known && !known.error && !known.stopped) return;
  app.index[id] = { built: null, progress: null, error: null, stopped: false };
  try {
    const built = await backend.buildIndex(id, (step) => {
      if (app.index[id]) app.index[id].progress = step;
    });
    if (app.index[id]) app.index[id].built = built;
  } catch (e) {
    if (!app.index[id]) return;
    if (isCancelled(e)) app.index[id].stopped = true;
    else app.index[id].error = toError(e);
  }
}

/** Opens a message in Chats (from a search result). */
export async function showMessage(id: number, message: number) {
  const at = await backend.messagePosition(id, message);
  if (!at) return;
  app.chat = at[0];
  app.jump = { chat: at[0], pos: at[1] };
  go("chats");
}

export function openExport(chat: number | null) {
  app.exportChat = chat;
  app.sheet = "export";
}

// ---- receiving over Wi-Fi ----

let poll: number | undefined;
export async function receiveStart() {
  const parent = await backend.pickFolder();
  if (parent) await receiveOn(parent, null);
}
/** The page on another of this computer's addresses (before anything arrived): same folder, new QR code. */
export async function receiveAddress(ip: string) {
  if (app.receive) await receiveOn(app.receive.parent, ip);
}
async function receiveOn(parent: string, ip: string | null) {
  const info = await run(() => backend.receiveStart(parent, ip));
  if (!info) return;
  app.receive = { info, status: await backend.receiveStatus(), parent, started: Date.now() };
  window.clearInterval(poll);
  poll = window.setInterval(async () => {
    if (!app.receive) return;
    const status = await backend.receiveStatus();
    app.receive.status = status;
    if (!status.running) window.clearInterval(poll);
  }, 500);
}
export async function receiveStop() {
  const status = await backend.receiveStop();
  if (app.receive) app.receive.status = status;
}
export async function receiveUse() {
  if (!app.receive) return;
  await receiveStop();
  await listBackups(app.receive.info.dest);
}

// ---- copying with adb (USB or wireless debugging) ----

/** Whether adb is on this computer; asked once when the "get the backup" screen opens. */
export async function adbCheck() {
  app.adb.found = !onPhone() && (await backend.adbFound().catch(() => false));
}
/** The phones adb sees; the only ready one is picked. */
export async function adbRefresh() {
  const devices = await run(() => backend.adbDevices());
  if (!devices) return;
  app.adb.devices = devices;
  const ready = devices.filter((d) => d.state === "device");
  if (!devices.some((d) => d.serial === app.adb.serial)) app.adb.serial = ready.length === 1 ? ready[0].serial : "";
}
/** Wireless debugging: pairs (when a code is given), connects, and picks that phone. */
export async function adbWireless(pairHost: string, code: string, host: string) {
  const ok = await run(async () => {
    if (pairHost && code) await backend.adbPair(pairHost, code);
    if (host) await backend.adbConnect(host);
    return true;
  });
  if (!ok) return;
  await adbRefresh();
  if (host && app.adb.devices?.some((d) => d.serial === host)) app.adb.serial = host;
}
let pullPoll: number | undefined;
export async function adbPullStart(business: boolean, media: boolean) {
  const parent = await backend.pickFolder();
  if (!parent) return;
  const info = await run(() => backend.adbPullStart(parent, app.adb.serial, business, media));
  if (!info) return;
  app.adb.pull = { info, status: await backend.adbPullStatus() };
  window.clearInterval(pullPoll);
  pullPoll = window.setInterval(async () => {
    if (!app.adb.pull) return;
    const status = await backend.adbPullStatus();
    app.adb.pull.status = status;
    if (!status.running) window.clearInterval(pullPoll);
  }, 500);
}
export async function adbPullStop() {
  const status = await backend.adbPullStop();
  if (app.adb.pull) app.adb.pull.status = status;
}
export async function adbPullUse() {
  if (app.adb.pull) await listBackups(app.adb.pull.info.dest);
}

// ---- updates (decision 0011): off unless switched on; nothing installs without a click ----

export function setAutoUpdates(on: boolean) {
  app.updates.auto = on;
  save("oopz.updates", on ? "on" : "off");
}

export async function checkUpdates(showSheet: boolean) {
  const u = app.updates;
  u.status = "checking";
  u.error = null;
  try {
    const checked = await backend.updateCheck();
    u.found = checked.found;
    u.lastChecked = checked.checked_ms;
    u.status = u.found ? "found" : "none";
    if (u.found && showSheet && app.sheet === null) app.sheet = "update";
  } catch (e) {
    u.error = toError(e, "update");
    u.status = "failed";
  }
}

export async function loadLastChecked() {
  if (app.updates.lastChecked === null) app.updates.lastChecked = await backend.updateLastChecked().catch(() => null);
}

export async function installUpdate() {
  const u = app.updates;
  u.status = "installing";
  u.progress = null;
  try {
    await backend.updateInstall((step) => (u.progress = step));
    // The app restarts itself on success: nothing to do here.
  } catch (e) {
    u.error = toError(e, "update");
    u.status = "failed";
  }
}

/** Forgets every open backup in the UI, after Rust closed them all. */
function closedAll() {
  Object.assign(app, { summaries: [], currentId: null, chat: null, searchChat: null, query: "", jump: null, index: {}, files: null, unlocking: null, outcome: null, missing: null, phoneChecks: [], receive: null, privateDir: null });
  app.adb.pull = null;
  window.clearInterval(poll);
  window.clearInterval(pullPoll);
}

/** Wipes every decrypted copy now; Rust closes every backup too. Returns the files left (0: all gone). */
export async function wipeNow(): Promise<number | undefined> {
  const left = await run(() => backend.wipeNow());
  if (left !== undefined) closedAll();
  return left;
}

/** Closes every backup (their decrypted copies are wiped) and starts from the welcome sheet. */
export async function startOver() {
  for (const s of app.summaries) await backend.forgetBackup(s.id);
  Object.assign(app, { summaries: [], currentId: null, chat: null, searchChat: null, query: "", jump: null, index: {}, files: null, unlocking: null, outcome: null, missing: null, phoneChecks: [] });
  app.view = "add";
  app.sheet = "welcome";
}
