// What the UI needs from the backend, and the data it gets back. The shapes mirror crates/oopz-app's serialized
// structs; tauri.ts maps each call to a Rust command (apps/desktop/src-tauri/src/lib.rs), mock.ts invents the data.

import type { Strings } from "../i18n/en";

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
  /** A group's subject, or the contact's name from wa.db beside the backup; hidden when "Hide names" is on. */
  name: string | null;
  /** The chat's last message. */
  preview: Preview | null;
}

export interface Preview {
  kind: number;
  from_me: boolean;
  text: string | null;
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

/** The export page's words, from the language files (src/lib/i18n/). */
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
  current: { name: string; done: number; total: number; bytes_per_sec: number; secs_left: number | null } | null;
  refused: number;
  stopped: "done" | "stopped" | "idle" | "refused" | null;
  /** Seconds until the page stops by itself if nothing arrives (0 once stopped). */
  idle_secs_left: number;
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
  /** The message this one answers, as WhatsApp kept a copy of it. */
  reply: Reply | null;
  /** [emoji, from me], oldest first. */
  reactions: [string, boolean][];
  edited: boolean;
  /** WhatsApp's forward score: 0 not forwarded, 1 to 4 forwarded, 5 or more "forwarded many times". */
  forwarded: number;
  /** A poll's options: [name, votes], in their order. */
  poll: [string, number][];
  call: Call | null;
  media: Media | null;
}

export interface Reply {
  /** The answered message, when it's still in the backup (the viewer can jump to it). */
  id: number | null;
  from_me: boolean;
  kind: number;
  text: string | null;
}

export interface Call {
  video: boolean;
  /** WhatsApp's call result code: 5 answered, 2 missed, 3 declined; anything else unknown. */
  result: number | null;
  secs: number | null;
}

/** What the backup says about a file, whether or not the file is at hand. */
export interface Media {
  mime: string | null;
  /** A document's file name. */
  name: string | null;
  size: number | null;
  secs: number | null;
  width: number | null;
  height: number | null;
}

/** What a search is narrowed to; every part optional. No words and a filter: the newest messages that match it. */
export interface Filter {
  chat?: number | null;
  /** WhatsApp message type codes; empty means any. */
  kinds: number[];
  from_me?: boolean | null;
  /** The last 3 characters of a sender's id. */
  sender?: string | null;
  after_ms?: number | null;
  before_ms?: number | null;
}

export interface Stats {
  messages: number;
  from_me: number;
  from_others: number;
  first_ms: number | null;
  last_ms: number | null;
  by_month: [string, number][];
  by_hour: number[];
  /** Messages per day of the week (Sunday first) and hour: 7 rows of 24. */
  by_day_hour: number[][];
  by_kind: [number, number, number][];
  audio_seconds: number;
  senders: [string, number][];
}

export interface UpdateFound {
  version: string;
  /** The release notes, plain text. */
  notes: string | null;
  date_ms: number | null;
}

export interface UpdateChecked {
  found: UpdateFound | null;
  checked_ms: number;
}

/** Diagnostics: a table of the decrypted database, with its columns and row count. */
export interface TableInfo {
  name: string;
  columns: string[];
  rows: number;
}

/** Diagnostics: one WhatsApp message type in a chat. */
export interface TypeInfo {
  code: number;
  messages: number;
  with_text: number;
  /** [table, messages of this type with a row in it] */
  linked: [string, number][];
  /** [media type, messages], most common first */
  mimes: [string, number][];
}

/** Diagnostics: a count or range about how a chat is stored (never text, ids or numbers). */
export interface Fact {
  label: string;
  value: string;
}

// ---- the library (decision 0016): every app's sources in one place; shapes from crates/oopz-core ----

export type AppCode = "whatsapp" | "telegram" | "sms" | "line" | "viber" | "signal" | "imessage";
export type Kind = "text" | "link" | "photo" | "video" | "gif" | "sticker" | "voice" | "audio" | "document" | "location" | "contact" | "poll" | "call" | "system" | "deleted" | "unknown";
export type Order = "conversation" | "time" | "newest";

export interface LibraryInfo {
  dir: string;
  /** Kept between runs (Settings); otherwise removed when the app closes. */
  kept: boolean;
  sources: number;
  messages: number;
  /** The library's files on disk. */
  bytes: number;
  /** PDF exports are in this build. */
  pdf: boolean;
}

export interface AdapterInfo {
  id: string;
  app: AppCode;
  secret: "none" | "key" | "password";
}

export interface SourceRow {
  id: number;
  app: AppCode;
  adapter: string;
  /** The file or folder's name. */
  name: string;
  format: string | null;
  added_ms: number;
  notes: string[];
  /** Times are the phone's clock, with no time zone. */
  wall_clock: boolean;
  has_media: boolean;
  conversations: number;
  people: number;
  messages: number;
  first_ms: number | null;
  last_ms: number | null;
  /** False: the export doesn't say which person is the owner; ask (libSetMe). */
  owner_known: boolean;
}

export type ConversationKind = "direct" | "group" | "channel" | "broadcast" | "notes" | "unknown";

export interface ConversationRow {
  id: number;
  source: number;
  app: AppCode;
  kind: ConversationKind;
  name: string | null;
  /** The other members' names, for a one-to-one conversation's title. */
  members: string[];
  messages: number;
  first_ms: number | null;
  last_ms: number | null;
  wall_clock: boolean;
  preview: string | null;
}

export type CallOutcome = "answered" | "missed" | "declined" | "cancelled" | "busy" | "failed" | "unknown";

export interface MediaPart {
  type: "media";
  kind: "photo" | "video" | "video_note" | "gif" | "sticker" | "voice" | "audio" | "document";
  file: string | null;
  name: string | null;
  mime: string | null;
  size: number | null;
  width: number | null;
  height: number | null;
  secs: number | null;
  caption: string | null;
  view_once: boolean;
  omitted: boolean;
}

export type Part =
  | { type: "text"; text: string }
  | MediaPart
  | { type: "location"; lat: number | null; lng: number | null; name: string | null; live: boolean }
  | { type: "contact"; name: string | null; phones: string[]; vcard: string | null }
  | { type: "poll"; question: string; options: { text: string; voters: string[]; votes: number }[]; multiple: boolean }
  | { type: "call"; video: boolean; outcome: CallOutcome; secs: number | null }
  | { type: "link"; url: string; title: string | null }
  | { type: "system"; event: string; text: string | null }
  | { type: "unknown"; code: string; text: string | null };

/** A message as its app's adapter read it (crates/oopz-model). */
export interface LibMessage {
  id: string;
  conversation: string;
  sender: string | null;
  from_me: boolean;
  time_ms: number | null;
  edited_ms: number | null;
  edited: boolean;
  deleted: boolean;
  forwarded: boolean;
  starred: boolean;
  reply_to: string | null;
  parts: Part[];
  reactions: { by: string | null; from_me: boolean; emoji: string }[];
}

export interface MessageRow {
  id: number;
  conversation: number;
  source: number;
  app: AppCode;
  pos: number;
  time_ms: number | null;
  wall_clock: boolean;
  from_me: boolean;
  sender: number | null;
  /** The contact's name, else the app's name for them, else their id in the app. */
  sender_name: string | null;
  kind: Kind;
  /** The library id of the message this one answers, when it's in the library. */
  reply: number | null;
  message: LibMessage;
}

export interface LibHit {
  id: number;
  conversation: number;
  app: AppCode;
  time_ms: number | null;
  from_me: boolean;
  sender_name: string | null;
  /** Matched words sit between \u0002 and \u0003. */
  snippet: string;
}

/** One filter for everything; empty lists mean "any". */
export interface LibFilter {
  apps: AppCode[];
  sources: number[];
  conversations: number[];
  contacts: number[];
  people: number[];
  kinds: Kind[];
  from_me: boolean | null;
  after_ms: number | null;
  before_ms: number | null;
  words: string;
}

/** A filter, plus messages added or taken out by hand. */
export interface Selection {
  filter: LibFilter;
  add: number[];
  remove: number[];
}

export interface SavedSelection {
  id: number;
  name: string;
  selection: Selection;
  saved_ms: number;
}

export interface PersonRow {
  id: number;
  source: number;
  app: AppCode;
  key: string;
  name: string | null;
  phone: string | null;
  username: string | null;
  email: string | null;
  me: boolean;
  contact: number | null;
  messages: number;
}

export interface ContactRow {
  id: number;
  name: string;
  phones: string[];
  emails: string[];
  /** Linked people: [person id, app]. */
  people: [number, AppCode][];
}

export interface Suggestion {
  reason: "phone" | "email";
  value: string;
  people: number[];
  contact: number | null;
  name: string | null;
}

export interface LibStats {
  messages: number;
  from_me: number;
  first_ms: number | null;
  last_ms: number | null;
  by_month: [string, number][];
  by_hour: number[];
  /** Sunday first, 7 rows of 24. */
  by_day_hour: number[][];
  /** [kind, messages, media bytes] */
  by_kind: [Kind, number, number][];
  by_app: [AppCode, number][];
  /** [person id, name, messages], most first. */
  by_person: [number | null, string | null, number][];
  media_bytes: number;
  voice_secs: number;
}

export type ExportFormat = "html" | "json" | "csv" | "txt" | "vcard" | "media-zip" | "pdf";

/** A library export's words (oopz_export::Words), from the language files (src/lib/i18n/). */
export type ExportWords = Strings["exportWords"] & {
  /** A call's outcome and a notice with no text of its own, by code. */
  outcomes: Record<string, string>;
  events: Record<string, string>;
};

export interface ExportOptions {
  order: Order;
  /** An IANA zone; null for this device's. */
  time_zone: string | null;
  words: ExportWords;
  title: string | null;
  skip_system: boolean;
  /** PDF only. */
  page?: "a4" | "letter";
  media?: "thumbnails" | "none";
  cover?: boolean;
  contents?: boolean;
}

export interface Exported {
  path: string;
  /** The folder it's in: the one picked, or a private one to save from (Android). */
  folder: string;
  messages: number;
  bytes: number;
  missing_files: number;
  pages: number;
  missing_glyphs: number;
}

export type LinkName ="source" | "report" | "security" | "licences" | "releases";

/** An error from Rust: a stable code the UI words, and an English detail. */
export interface AppError {
  code: string;
  detail: string;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

/** Where a long piece of work is: its step, thousandths done, and the seconds left once they can be told. */
export interface Step {
  step: "fingerprint" | "read" | "unpack" | "prepare" | "index" | "download" | "import" | "export";
  per_mille: number;
  secs_left: number | null;
}

export type OnProgress = (step: Step) => void;

export interface Backend {
  /** Where the app runs, fixed for the run: on a phone the words say "this phone" and Wi-Fi receiving is hidden. */
  platform: "android" | "desktop";
  /** The file and folder pickers: a path, or null when the person cancels. */
  pickFolder(): Promise<string | null>;
  pickFile(): Promise<string | null>;
  /** The system's save picker, for a file named `name`: where to write it (on Android a content:// reference). */
  pickSave(name: string): Promise<string | null>;
  /** Several files from the system's picker (Android: the backup and, for names, WhatsApp's contacts file). */
  pickFiles(): Promise<string[]>;
  /** Android: copies the picked content:// files into one private folder (free space checked first) and returns the
   *  copies' paths, largest first (the message backup, named msgstore.db.crypt15), which the other commands open
   *  like any path. Fails with
   *  no_space, not_crypt15 (an empty file) or io. */
  importBackup(uris: string[]): Promise<string[]>;
  /** Copies a file inside a result folder this run made to where the person picked; returns the bytes written. */
  saveResult(from: string, to: string): Promise<number>;
  appVersion(): Promise<string>;

  findBackups(path: string): Promise<BackupFile[]>;
  /** The key goes to Rust for this one call; the caller clears its input right away. */
  checkBackup(path: string, key: string, onProgress: OnProgress): Promise<Summary>;
  forgetBackup(id: number): Promise<void>;
  checkedBackups(): Promise<Summary[]>;
  /** Stops the check or index build that is running; it then fails with the code "cancelled". */
  cancelWork(): Promise<void>;
  /** Wipes every decrypted copy now and closes every backup; returns the files it couldn't remove (0: all gone). */
  wipeNow(): Promise<number>;
  openLink(which: LinkName): Promise<void>;
  mergeChat(form: MergeForm): Promise<MergeOutcome>;
  /** Saves stats (and a chat's messages) as a web page in a new file in `outDir`; returns its path. */
  exportPage(id: number, chat: number | null, labels: ExportLabels, outDir: string): Promise<string>;
  /** Opens a folder this run of the app made (a result, or where a page was saved) in the file manager. */
  showFolder(folder: string): Promise<void>;
  missing(a: number, chatA: number, b: number, chatB: number): Promise<Missing>;
  buildIndex(id: number, onProgress: OnProgress): Promise<BuildInfo>;
  search(id: number, query: string, filter: Filter): Promise<Hit[]>;
  stats(id: number, chat: number | null): Promise<Stats>;
  chatLen(id: number, chat: number): Promise<number>;
  /** `count` messages of a chat from position `from`, oldest first. */
  chatLines(id: number, chat: number, from: number, count: number): Promise<Line[]>;
  /** The chat and position of a message, for jumping from a search result to it. */
  messagePosition(id: number, message: number): Promise<[number, number] | null>;
  /** The position of a chat's first message at or after `ts`, for "jump to date". */
  positionAt(id: number, chat: number, ts: number): Promise<number | null>;
  /** A photo's thumbnail as JPEG bytes, from the Media (or WhatsApp) folder the person picked; empty when none. */
  thumbnail(id: number, message: number, media: string, max: number): Promise<ArrayBuffer>;
  backupTables(id: number): Promise<TableInfo[]>;
  chatTypes(id: number, chat: number): Promise<TypeInfo[]>;
  chatFacts(id: number, chat: number): Promise<Fact[]>;
  receiveStart(parent: string): Promise<ReceiveInfo>;
  receiveStatus(): Promise<ReceiveStatus>;
  receiveStop(): Promise<ReceiveStatus>;
  /** Asks GitHub for a newer oopz (decision 0011); null when this one is the newest. */
  updateCheck(): Promise<UpdateChecked>;
  /** When the last check happened, in any run; null if never. */
  updateLastChecked(): Promise<number | null>;
  /** Installs what the last check found, after checking its signature, and restarts the app. */
  updateInstall(onProgress: OnProgress): Promise<void>;
  // ---- the library ----
  libraryInfo(): Promise<LibraryInfo>;
  /** Keeps the library between runs (moved to the app's data folder), or not (removed when the app closes). */
  librarySetKeep(keep: boolean): Promise<LibraryInfo>;
  libAdapters(): Promise<AdapterInfo[]>;
  /** Adds a file or folder (Android: a content:// reference): its app detected unless `adapter` names it; the secret
   *  only when the adapter asks for one. Stops on cancelWork ("cancelled"), leaving the library as it was. */
  libImport(path: string, adapter: string | null, secret: string | null, onProgress: OnProgress): Promise<SourceRow>;
  libSources(): Promise<SourceRow[]>;
  libRemoveSource(id: number): Promise<void>;
  /** Marks a person as the owner of their source: their messages become "from me". */
  libSetMe(person: number): Promise<void>;
  libConversations(filter: LibFilter): Promise<ConversationRow[]>;
  libConversationLen(conversation: number): Promise<number>;
  /** `count` messages from position `from`, in the conversation's order (at most 500). */
  libMessages(conversation: number, from: number, count: number): Promise<MessageRow[]>;
  libPositionAt(conversation: number, ts: number): Promise<number | null>;
  /** A message's [conversation, position]. */
  libPosition(message: number): Promise<[number, number] | null>;
  libSearch(filter: LibFilter, limit: number): Promise<LibHit[]>;
  libFind(filter: LibFilter, order: Order, offset: number, limit: number): Promise<MessageRow[]>;
  libCount(filter: LibFilter): Promise<number>;
  /** The ids of a selection's messages. */
  libSelected(selection: Selection, order: Order): Promise<number[]>;
  libMessagesById(ids: number[]): Promise<MessageRow[]>;
  libStats(filter: LibFilter): Promise<LibStats>;
  libPeople(source: number | null): Promise<PersonRow[]>;
  libContacts(): Promise<ContactRow[]>;
  /** Links people (from any apps) into one contact; returns its id. */
  libLink(people: number[], name: string | null): Promise<number>;
  libUnlink(person: number): Promise<void>;
  libRenameContact(contact: number, name: string): Promise<void>;
  libDeleteContact(contact: number): Promise<void>;
  libSuggestions(): Promise<Suggestion[]>;
  libAccept(reason: string, value: string): Promise<number>;
  /** Reads a .vcf file (Android: a content:// reference) into contacts; returns how many cards it held. */
  libImportVcards(path: string): Promise<number>;
  libSaveSelection(name: string, selection: Selection): Promise<number>;
  libSelections(): Promise<SavedSelection[]>;
  libDeleteSelection(id: number): Promise<void>;
  /** Writes a selection into a new file in `folder` (null: a private folder to save from, on Android). */
  libExport(selection: Selection, format: ExportFormat, options: ExportOptions, folder: string | null, onProgress: OnProgress): Promise<Exported>;
  libExportContacts(contacts: number[], folder: string | null): Promise<Exported>;

  /** Files dropped on the window; returns the function that stops listening. */
  onFileDrop(cb: (paths: string[]) => void): () => void;
}
