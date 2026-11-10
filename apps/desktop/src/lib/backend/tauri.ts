// The Backend over Tauri: each call is one Rust command (apps/desktop/src-tauri/src/lib.rs). The only file that
// imports @tauri-apps.
import { getVersion } from "@tauri-apps/api/app";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Backend, OnProgress } from "./types";

/** A channel Rust sends progress on, in thousandths. */
function progress(onProgress: OnProgress): Channel<number> {
  const channel = new Channel<number>();
  channel.onmessage = onProgress;
  return channel;
}

async function pick(directory: boolean): Promise<string | null> {
  const chosen = await open({ directory, multiple: false });
  return typeof chosen === "string" ? chosen : null;
}

export const tauri: Backend = {
  pickFolder: () => pick(true),
  pickFile: () => pick(false),
  appVersion: () => getVersion(),
  findBackups: (path) => invoke("find_backups", { path }),
  checkBackup: (path, key, onProgress) => invoke("check_backup", { path, key, progress: progress(onProgress) }),
  forgetBackup: (id) => invoke("forget_backup", { id }),
  checkedBackups: () => invoke("checked_backups"),
  mergeChat: (form) => invoke("merge_chat", { form }),
  exportPage: (id, chat, labels, outDir) => invoke("export_page", { id, chat, labels, outDir }),
  showFolder: (folder) => invoke("show_folder", { folder }),
  missing: (a, chatA, b, chatB) => invoke("missing", { a, chatA, b, chatB }),
  buildIndex: (id, onProgress) => invoke("build_index", { id, progress: progress(onProgress) }),
  search: (id, query, chat) => invoke("search", { id, query, chat }),
  stats: (id, chat) => invoke("stats", { id, chat }),
  chatLen: (id, chat) => invoke("chat_len", { id, chat }),
  chatLines: (id, chat, from, count) => invoke("chat_lines", { id, chat, from, count }),
  messagePosition: (id, message) => invoke("message_position", { id, message }),
  receiveStart: (parent) => invoke("receive_start", { parent }),
  receiveStatus: () => invoke("receive_status"),
  receiveStop: () => invoke("receive_stop"),
  updateCheck: () => invoke("update_check"),
  updateInstall: (onProgress) => invoke("update_install", { progress: progress(onProgress) }),
};
