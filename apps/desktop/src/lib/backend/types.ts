// What the UI needs from the backend, and the data it gets back. The shapes mirror crates/oopz-app's serialized
// structs; tauri.ts maps each call to a Rust command (apps/desktop/src-tauri/src/lib.rs), mock.ts invents the data.

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

export interface MergeForm {
  source: number;
  source_chat: number;
  target: number;
  target_chat: number;
  target_key: string;
  out_parent: string;
  media: string | null;
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

/** The export page's words, from i18n.ts. */
export interface ExportLabels {
  lang: string;
  heading: string;
  messages: string;
  sent: string;
  received: string;
  dates: string;
  by_month: string;
  by_hour: string;
  by_kind: string;
  audio_minutes: string;
  senders: string;
  you: string;
  note: string;
  kinds: Record<number, string>;
}

/** Two backups' copies of a chat, matched by WhatsApp's message id. */
export interface Missing {
  both: number;
  only_a: number;
  only_a_first_ms: number | null;
  only_a_last_ms: number | null;
  only_b: number;
  only_b_first_ms: number | null;
  only_b_last_ms: number | null;
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
  /** WhatsApp's message type code (0 text, 1 photo, 2 audio, 3 video, 9 document, 20 sticker, …). */
  kind: number;
  text: string | null;
  /** The last 3 characters of the sender's id, in a group. */
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

export interface UpdateFound {
  version: string;
}

/** An error from Rust: a stable code the UI words, and an English detail. */
export interface AppError {
  code: string;
  detail: string;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

/** Thousandths done of a long step. */
export type OnProgress = (perMille: number) => void;

export interface Backend {
  /** Where the app runs, fixed for the run: on a phone the words say "this phone" and Wi-Fi receiving is hidden. */
  platform: "android" | "desktop";
  /** The file and folder pickers: a path, or null when the person cancels. */
  pickFolder(): Promise<string | null>;
  pickFile(): Promise<string | null>;
  appVersion(): Promise<string>;

  findBackups(path: string): Promise<BackupFile[]>;
  /** The key goes to Rust for this one call; the caller clears its input right away. */
  checkBackup(path: string, key: string, onProgress: OnProgress): Promise<Summary>;
  forgetBackup(id: number): Promise<void>;
  checkedBackups(): Promise<Summary[]>;
  mergeChat(form: MergeForm): Promise<MergeOutcome>;
  /** Saves stats (and a chat's messages) as a web page in a new file in `outDir`; returns its path. */
  exportPage(id: number, chat: number | null, labels: ExportLabels, outDir: string): Promise<string>;
  /** Opens a folder this run of the app made (a result, or where a page was saved) in the file manager. */
  showFolder(folder: string): Promise<void>;
  missing(a: number, chatA: number, b: number, chatB: number): Promise<Missing>;
  buildIndex(id: number, onProgress: OnProgress): Promise<BuildInfo>;
  search(id: number, query: string, chat: number | null): Promise<Hit[]>;
  stats(id: number, chat: number | null): Promise<Stats>;
  chatLen(id: number, chat: number): Promise<number>;
  /** `count` messages of a chat from position `from`, oldest first. */
  chatLines(id: number, chat: number, from: number, count: number): Promise<Line[]>;
  /** The chat and position of a message, for jumping from a search result to it. */
  messagePosition(id: number, message: number): Promise<[number, number] | null>;
  receiveStart(parent: string): Promise<ReceiveInfo>;
  receiveStatus(): Promise<ReceiveStatus>;
  receiveStop(): Promise<ReceiveStatus>;
  /** Asks GitHub for a newer oopz (decision 0011); null when this one is the newest. */
  updateCheck(): Promise<UpdateFound | null>;
  /** Installs what the last check found, after checking its signature, and restarts the app. */
  updateInstall(onProgress: OnProgress): Promise<void>;
}
