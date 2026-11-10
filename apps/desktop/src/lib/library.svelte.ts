// The library's state, shared by its screens (src/features/*): what's in it, the conversation open, the selection
// being built, and the export sheet's subject. Names and text stay on screen only.
import type { AdapterInfo, AppCode, ConversationRow, LibFilter, LibraryInfo, MessageRow, Selection, SourceRow } from "./backend/types";
import { strings } from "./i18n";
import { app, be, go, run } from "./state.svelte";

export const emptyFilter = (): LibFilter => ({ apps: [], sources: [], conversations: [], contacts: [], people: [], kinds: [], from_me: null, after_ms: null, before_ms: null, words: "" });
export const emptySelection = (): Selection => ({ filter: emptyFilter(), add: [], remove: [] });

export const lib = $state({
  info: null as LibraryInfo | null,
  sources: [] as SourceRow[],
  adapters: [] as AdapterInfo[],
  /** The conversation open in Library chats, and the position to show in it (a search result), if any. */
  conv: null as number | null,
  jump: null as { conv: number; pos: number } | null,
  /** Library chats narrowed to these apps / this source. */
  chatApps: [] as AppCode[],
  chatSource: null as number | null,
  query: "",
  /** Library search narrowed to this conversation. */
  searchConv: null as number | null,
  /** The selection Select builds; chats add messages to it by hand. */
  selection: emptySelection(),
  /** What the export sheet writes. */
  exporting: null as { selection: Selection; title: string } | null,
  /** Bumped when sources change, so screens reload what they show. */
  version: 0,
});

/** A filter that matches everything in the library (every app in it), as the CLI's: an empty one selects nothing. */
export function everything(): LibFilter {
  return { ...emptyFilter(), apps: [...new Set(lib.sources.map((s) => s.app))] };
}

export async function refreshLibrary() {
  const [info, sources] = await Promise.all([be().libraryInfo(), be().libSources()]);
  lib.info = info;
  lib.sources = sources;
  if (lib.adapters.length === 0) lib.adapters = await be().libAdapters();
  lib.version++;
}

export async function removeSource(id: number) {
  await run(() => be().libRemoveSource(id));
  if (lib.conv !== null) lib.conv = null;
  await refreshLibrary();
}

export async function setKeep(keep: boolean) {
  const info = await run(() => be().librarySetKeep(keep));
  if (info) lib.info = info;
}

/** After "Wipe now": the library is gone, kept or not. */
export function libraryWiped() {
  Object.assign(lib, { sources: [], conv: null, jump: null, chatSource: null, query: "", selection: emptySelection(), exporting: null });
  lib.version++;
  void be()
    .libraryInfo()
    .then((i) => (lib.info = i))
    .catch(() => {});
}

export const appName = (a: AppCode) => ({ whatsapp: "WhatsApp", telegram: "Telegram", sms: "SMS", line: "LINE", viber: "Viber", signal: "Signal", imessage: "iMessage" })[a];

/** A conversation's title: its name, else its members' names; its kind and number when names are hidden. */
export function convTitle(c: ConversationRow): string {
  const t = strings(app.lang).lib;
  if (app.hideNames) return `${t.convKinds[c.kind]} #${c.id}`;
  return c.name || c.members.join(", ") || `${t.convKinds[c.kind]} #${c.id}`;
}

/** A person's name as the library has it; a number when names are hidden. */
export function personName(name: string | null, id: number | null): string {
  const t = strings(app.lang).lib;
  if (app.hideNames || !name) return id === null ? t.someone : `${t.person} #${id}`;
  return name;
}

export function senderOf(m: MessageRow): string {
  const t = strings(app.lang);
  return m.from_me ? t.you : personName(m.sender_name, m.sender);
}

/** A message's text for a list line: its text parts, else its kind in brackets. */
export function messageText(m: MessageRow): string {
  const t = strings(app.lang).lib;
  const words: string[] = [];
  for (const p of m.message.parts) {
    if (p.type === "text") words.push(p.text);
    else if (p.type === "media" && p.caption) words.push(p.caption);
    else if (p.type === "link") words.push(p.title ?? p.url);
    else if ((p.type === "system" || p.type === "unknown") && p.text) words.push(p.text);
    else if (p.type === "poll") words.push(p.question);
  }
  return words.join(" ") || `[${t.kinds[m.kind]}]`;
}

/** A yyyy-mm-dd field as the start of that day, this device's time; `next` gives the start of the day after. */
export function dayStart(value: string, next = false): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  return m ? new Date(+m[1], +m[2] - 1, +m[3] + (next ? 1 : 0)).getTime() : null;
}

/** Opens a message in Library chats. */
export async function showLibMessage(message: number) {
  const at = await be().libPosition(message);
  if (!at) return;
  lib.conv = at[0];
  lib.jump = { conv: at[0], pos: at[1] };
  go("lchats");
}

export function openLibExport(selection: Selection, title: string) {
  lib.exporting = { selection, title };
  app.sheet = "libexport";
}

/** A tick in a conversation: the message is added by hand, or no longer. */
export function handPick(id: number, on: boolean) {
  const s = lib.selection;
  s.add = s.add.filter((x) => x !== id);
  s.remove = s.remove.filter((x) => x !== id);
  if (on) s.add.push(id);
}

/** A tick in the selection's own list: the message stays in, or is taken out by hand. */
export function toggleInSelection(id: number, on: boolean) {
  const s = lib.selection;
  s.add = s.add.filter((x) => x !== id);
  s.remove = s.remove.filter((x) => x !== id);
  if (on) s.add.push(id);
  else s.remove.push(id);
}
