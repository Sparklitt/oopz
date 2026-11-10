// The Backend over Tauri: each call is one Rust command (apps/desktop/src-tauri/src/lib.rs). The only file that
// imports @tauri-apps.
import { getVersion } from "@tauri-apps/api/app";
import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Backend, OnProgress, Step } from "./types";

/** A channel Rust sends progress on: the step, thousandths done, seconds left. */
function progress(onProgress: OnProgress): Channel<Step> {
  const channel = new Channel<Step>();
  channel.onmessage = onProgress;
  return channel;
}

async function pick(directory: boolean): Promise<string | null> {
  const chosen = await open({ directory, multiple: false });
  return typeof chosen === "string" ? chosen : null;
}

export const tauri: Backend = {
  // Android's WebView says so in its user agent; no plugin needed for one fact.
  platform: /Android/.test(navigator.userAgent) ? "android" : "desktop",
  pickFolder: () => pick(true),
  pickFile: () => pick(false),
  pickSave: async (name) => (await save({ defaultPath: name })) ?? null,
  pickFiles: async () => {
    const chosen = await open({ multiple: true });
    return Array.isArray(chosen) ? chosen : typeof chosen === "string" ? [chosen] : [];
  },
  importBackup: (uris) => invoke("import_backup", { uris }),
  saveResult: (from, to) => invoke("save_result", { from, to }),
  appVersion: () => getVersion(),
  findBackups: (path) => invoke("find_backups", { path }),
  checkBackup: (path, key, onProgress) => invoke("check_backup", { path, key, progress: progress(onProgress) }),
  forgetBackup: (id) => invoke("forget_backup", { id }),
  checkedBackups: () => invoke("checked_backups"),
  cancelWork: () => invoke("cancel_work"),
  wipeNow: () => invoke("wipe_now"),
  openLink: (which) => invoke("open_link", { which }),
  mergeChat: (form) => invoke("merge_chat", { form }),
  exportPage: (id, chat, labels, outDir) => invoke("export_page", { id, chat, labels, outDir }),
  showFolder: (folder) => invoke("show_folder", { folder }),
  missing: (a, chatA, b, chatB) => invoke("missing", { a, chatA, b, chatB }),
  buildIndex: (id, onProgress) => invoke("build_index", { id, progress: progress(onProgress) }),
  search: (id, query, filter) => invoke("search", { id, query, filter }),
  stats: (id, chat) => invoke("stats", { id, chat }),
  chatLen: (id, chat) => invoke("chat_len", { id, chat }),
  chatLines: (id, chat, from, count) => invoke("chat_lines", { id, chat, from, count }),
  messagePosition: (id, message) => invoke("message_position", { id, message }),
  positionAt: (id, chat, ts) => invoke("position_at", { id, chat, ts }),
  // Raw bytes: Rust answers with an ipc Response, which arrives as an ArrayBuffer.
  thumbnail: (id, message, media, max) => invoke("thumbnail", { id, message, media, max }),
  backupTables: (id) => invoke("backup_tables", { id }),
  chatTypes: (id, chat) => invoke("chat_types", { id, chat }),
  chatFacts: (id, chat) => invoke("chat_facts", { id, chat }),
  receiveStart: (parent) => invoke("receive_start", { parent }),
  receiveStatus: () => invoke("receive_status"),
  receiveStop: () => invoke("receive_stop"),
  updateCheck: () => invoke("update_check"),
  updateLastChecked: () => invoke("update_last_checked"),
  updateInstall: (onProgress) => invoke("update_install", { progress: progress(onProgress) }),
  onFileDrop(cb) {
    let stop: (() => void) | null = null;
    let gone = false;
    void getCurrentWebview()
      .onDragDropEvent((e) => {
        if (e.payload.type === "drop") cb(e.payload.paths);
      })
      .then((unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      });
    return () => {
      gone = true;
      stop?.();
    };
  },
};
