// The app's state and the actions that change it. Keys are never kept here: each key field holds its own value
// and empties it as soon as it is sent.
import type { AppError, Backend, BackupFile, BuildInfo, MergeOutcome, Missing, ReceiveInfo, ReceiveStatus, Summary, UpdateFound } from "./backend/types";
import { isAppError } from "./backend/types";
import { initialLang, usePhoneWords, type Lang } from "./i18n";

export type View = "add" | "receive" | "overview" | "chats" | "search" | "insights" | "compare" | "restore" | "phone" | "done" | "settings" | "backups" | "tools";
export type SheetName = "welcome" | "found" | "unlock" | "export" | "update";
export type Theme = "system" | "light" | "dark";

interface IndexState {
  built: BuildInfo | null;
  /** Thousandths done, or null before the first report. */
  progress: number | null;
  error: AppError | null;
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
  view: "add" as View,
  sheet: "welcome" as SheetName | null,
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
  receive: null as { info: ReceiveInfo; status: ReceiveStatus } | null,
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
    error: null as AppError | null,
    progress: null as number | null,
  },
});

// The page's language from the start, so the browser doesn't offer to translate a page it thinks is English.
document.documentElement.lang = app.lang;

let backend: Backend;
export function setBackend(b: Backend) {
  backend = b;
  usePhoneWords(onPhone());
}
/** The app runs on the phone itself: no receiving over Wi-Fi from a phone. */
export const onPhone = () => backend.platform === "android";
/** The backend, for components that call it directly (reads that change no state). */
export const be = () => backend;

export const current = () => app.summaries.find((s) => s.id === app.currentId) ?? null;

export function toError(e: unknown, code = "io"): AppError {
  return isAppError(e) ? e : { code, detail: String(e) };
}

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
  if (known && !known.error) return;
  app.index[id] = { built: null, progress: null, error: null };
  try {
    const built = await backend.buildIndex(id, (perMille) => {
      if (app.index[id]) app.index[id].progress = perMille;
    });
    if (app.index[id]) app.index[id].built = built;
  } catch (e) {
    if (app.index[id]) app.index[id].error = toError(e);
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
  if (!parent) return;
  const info = await run(() => backend.receiveStart(parent));
  if (!info) return;
  app.receive = { info, status: await backend.receiveStatus() };
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
    u.found = await backend.updateCheck();
    u.status = u.found ? "found" : "none";
    if (u.found && showSheet && app.sheet === null) app.sheet = "update";
  } catch (e) {
    u.error = toError(e, "update");
    u.status = "failed";
  }
}

export async function installUpdate() {
  const u = app.updates;
  u.status = "installing";
  u.progress = null;
  try {
    await backend.updateInstall((perMille) => (u.progress = perMille));
    // The app restarts itself on success: nothing to do here.
  } catch (e) {
    u.error = toError(e, "update");
    u.status = "failed";
  }
}

/** Closes every backup (their decrypted copies are wiped) and starts from the welcome sheet. */
export async function startOver() {
  for (const s of app.summaries) await backend.forgetBackup(s.id);
  Object.assign(app, { summaries: [], currentId: null, chat: null, searchChat: null, query: "", jump: null, index: {}, files: null, unlocking: null, outcome: null, missing: null, phoneChecks: [] });
  app.view = "add";
  app.sheet = "welcome";
}
