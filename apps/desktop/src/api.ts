// Typed calls to the Rust commands (apps/desktop/src-tauri/src/lib.rs). The shapes mirror crates/oopz-app.
import { Channel, invoke } from "@tauri-apps/api/core";

export type BackupKind = "crypt15" | "crypt14" | "crypt12" | "not_crypt15";

export interface BackupFile {
  path: string;
  name: string;
  bytes: number;
  kind: BackupKind;
}

export interface ChatRow {
  number: number;
  kind: "contact" | "contact_lid" | "group" | "broadcast" | "other";
  ending: string;
  messages: number;
  last_ms: number | null;
}

export interface Summary {
  id: number;
  name: string;
  bytes: number;
  sha256: string;
  sound: boolean;
  problems: string;
  chats: number;
  messages: number;
  first_ms: number | null;
  last_ms: number | null;
  one_to_one: number;
  groups: number;
  chat_list: ChatRow[];
}

export interface MergeOutcome {
  folder: string;
  backup: string;
  sha256: string;
  target_before: number;
  already_there: number;
  added: number;
  checks: string[];
  media_copied: number | null;
  media_missing: number | null;
  original_untouched: boolean;
}

export interface ReceiveInfo {
  url: string;
  qr_svg: string;
  dest: string;
  /** Bytes free where the files go. */
  free: number;
}

export interface ReceiveStatus {
  running: boolean;
  files: { name: string; bytes: number; sha256: string }[];
  current: { name: string; done: number; total: number } | null;
  refused: number;
  stopped: "done" | "stopped" | "idle" | "refused" | null;
}

export interface BuildInfo {
  messages: number;
  with_text: number;
  millis: number;
}

export interface Hit {
  id: number;
  chat: number;
  ts: number;
  from_me: boolean;
  /** Matched words sit between \u0002 and \u0003. */
  snippet: string;
}

export interface Line {
  id: number;
  ts: number;
  from_me: boolean;
  kind: number;
  text: string | null;
  sender: string | null;
}

export interface Stats {
  messages: number;
  from_me: number;
  from_others: number;
  first_ms: number | null;
  last_ms: number | null;
  by_month: [string, number][];
  by_hour: number[];
  by_kind: [number, number, number][];
  audio_seconds: number;
  senders: [string, number][];
}

/** An error from Rust: a stable code the UI words, and an English detail. */
export interface AppError {
  code: string;
  detail: string;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

/** A channel Rust sends progress on, in thousandths. */
function progress(onProgress: (perMille: number) => void): Channel<number> {
  const channel = new Channel<number>();
  channel.onmessage = onProgress;
  return channel;
}

export const api = {
  findBackups: (path: string) => invoke<BackupFile[]>("find_backups", { path }),
  /** The key goes to Rust for this one call; the caller clears its input right away. */
  checkBackup: (path: string, key: string, onProgress: (perMille: number) => void) =>
    invoke<Summary>("check_backup", { path, key, progress: progress(onProgress) }),
  forgetBackup: (id: number) => invoke<void>("forget_backup", { id }),
  checkedBackups: () => invoke<Summary[]>("checked_backups"),
  mergeChat: (form: {
    source: number;
    source_chat: number;
    target: number;
    target_chat: number;
    target_key: string;
    out_parent: string;
    media: string | null;
  }) => invoke<MergeOutcome>("merge_chat", { form }),
  buildIndex: (id: number, onProgress: (perMille: number) => void) =>
    invoke<BuildInfo>("build_index", { id, progress: progress(onProgress) }),
  search: (id: number, query: string, chat: number | null) => invoke<Hit[]>("search", { id, query, chat }),
  stats: (id: number, chat: number | null) => invoke<Stats>("stats", { id, chat }),
  chatLen: (id: number, chat: number) => invoke<number>("chat_len", { id, chat }),
  chatLines: (id: number, chat: number, from: number, count: number) =>
    invoke<Line[]>("chat_lines", { id, chat, from, count }),
  messagePosition: (id: number, message: number) =>
    invoke<[number, number] | null>("message_position", { id, message }),
  receiveStart: (parent: string) => invoke<ReceiveInfo>("receive_start", { parent }),
  receiveStatus: () => invoke<ReceiveStatus>("receive_status"),
  receiveStop: () => invoke<ReceiveStatus>("receive_stop"),
};
