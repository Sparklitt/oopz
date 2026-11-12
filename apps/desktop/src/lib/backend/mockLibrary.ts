// The library's half of the mock (mock.ts): an invented library in memory, with three sources to start from
// (?library=empty starts with none). Adding a file picks its app by the file's name: .json Telegram, .xml SMS,
// .csv Viber, .crypt15 a WhatsApp backup (the mock's right key opens it), anything else a WhatsApp chat export, whose
// owner the person is asked to pick; a folder, an iPhone backup (any password but "wrong" opens it). Every phone
// number starts with 9999; every name is made up.
import { mockSticker } from "./mockMedia";
import type {
  AdapterInfo,
  AppCode,
  Backend,
  ContactRow,
  ConversationKind,
  ConversationRow,
  Kind,
  LibFilter,
  LibHit,
  LibraryInfo,
  LibStats,
  MessageRow,
  OnProgress,
  Part,
  PersonRow,
  SavedSelection,
  Selection,
  SourceRow,
  Step,
} from "./types";
import { MIME, extension } from "../generated/msgkinds";
/** A format's extension with its dot (`.json`). */
const dot = (mime: string) => `.${extension(mime)}`;

type LibraryBackend = Pick<Backend, Extract<keyof Backend, `lib${string}` | `library${string}`>>;

interface Source extends Omit<SourceRow, "conversations" | "people" | "messages" | "first_ms" | "last_ms" | "owner_known"> {}
interface Conv {
  id: number;
  source: number;
  kind: ConversationKind;
  name: string | null;
  members: number[];
}

const DAY = 86_400_000;
const NOW = Date.UTC(2026, 10, 9, 16, 24);

const WORDS = [
  "¿Quedamos mañana para comer?",
  "Te llamo luego",
  "Did you see the photos from the trip?",
  "Perfecto, gracias",
  "The train is late again",
  "¿Has visto a Ana Prueba?",
  "Happy birthday! 🎉",
  "Nos vemos en la playa",
  "I'll send you the document",
  "Buenas noches 🌙",
  "مرحبا، كيف حالك؟",
  "Can you call me back?",
  "Mi número nuevo es +9999 555 0199",
];

function hash(a: number, b: number): number {
  let h = (a * 0x9e3779b1) ^ (b * 0x85ebca77);
  h = Math.imul(h ^ (h >>> 16), 0x7feb352d);
  return (h ^ (h >>> 15)) >>> 0;
}

/** Accents and case ignored, as the library's index does. */
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export function libraryMock(opts: {
  sleep: (ms: number) => Promise<unknown>;
  slowly: (ms: number, onProgress: OnProgress, name: () => Step["step"]) => Promise<void>;
  rightKey: string;
  android: boolean;
  empty: boolean;
}): LibraryBackend & { wipe(): void } {
  const { sleep, slowly } = opts;
  let kept = false;
  let ids = 1;
  const sources: Source[] = [];
  const people: PersonRow[] = [];
  const convs: Conv[] = [];
  const msgs: MessageRow[] = [];
  const contacts: (ContactRow & { cardOnly: boolean })[] = [];
  const selections: SavedSelection[] = [];
  const imported = new Set<string>();

  const ADAPTERS: AdapterInfo[] = [
    { id: "whatsapp-android", app: "whatsapp", secret: "key" },
    { id: "whatsapp-ios", app: "whatsapp", secret: "password" },
    { id: "whatsapp-business-ios", app: "whatsapp", secret: "password" },
    { id: "whatsapp-export", app: "whatsapp", secret: "none" },
    { id: "telegram-json", app: "telegram", secret: "none" },
    { id: "sms-xml", app: "sms", secret: "none" },
    { id: "line-export", app: "line", secret: "none" },
    { id: "viber-csv", app: "viber", secret: "none" },
  ];

  const NAMES = ["Ana Prueba", "Ben Ejemplo", "Carla Muestra", "Dani Ficticio", "Eva Demo", "Fede Inventado"];

  /** One invented source: its people, conversations and messages. */
  function addSource(app: AppCode, adapter: string, name: string, ownerKnown: boolean, seed: number): Source {
    const sid = ids++;
    const src: Source = { id: sid, app, adapter, name, format: null, added_ms: NOW, notes: app === "whatsapp" && adapter === "whatsapp-export" ? ["Times are the phone's clock: the export has no time zone."] : [], wall_clock: adapter === "whatsapp-export", has_media: app !== "sms" };
    sources.push(src);
    const me: PersonRow = { id: ids++, source: sid, app, key: "me", name: "Yo", phone: "+9999 555 0100", username: null, email: null, me: ownerKnown, contact: null, messages: 0 };
    people.push(me);
    const others = NAMES.filter((_, i) => (i + seed) % 2 === 0 || i < 3).map((n, i): PersonRow => ({
      id: ids++, source: sid, app, key: `p${i}`, name: n, phone: app === "telegram" ? null : `+9999 555 01${String(10 + i)}`,
      username: app === "telegram" ? n.split(" ")[0].toLowerCase() : null, email: null, me: false, contact: null, messages: 0,
    }));
    people.push(...others);
    const list: Conv[] = others.slice(0, 4).map((p) => ({ id: ids++, source: sid, kind: "direct", name: null, members: [me.id, p.id] }));
    if (app !== "sms") list.push({ id: ids++, source: sid, kind: "group", name: app === "telegram" ? "Viaje a Cádiz 2026" : "Familia 🏡", members: [me.id, ...others.map((p) => p.id)] });
    convs.push(...list);
    for (const c of list) {
      const n = 60 + (hash(c.id, seed) % 400);
      const start = NOW - (300 + (hash(c.id, 3) % 900)) * DAY;
      for (let i = 0; i < n; i++) {
        const fromMe = hash(c.id, i) % 3 === 0;
        const other = c.members[1 + (hash(i, c.id) % (c.members.length - 1))];
        const sender = fromMe ? me.id : other;
        const ts = start + Math.floor(((NOW - start) * i) / n) + (hash(i, 7) % 3600e3);
        const r = hash(c.id * 31, i) % 40;
        let kind: Kind = "text";
        let parts: Part[] = [{ type: "text", text: WORDS[hash(c.id, i * 3) % WORDS.length] }];
        if (r === 0) (kind = "photo"), (parts = [{ type: "media", kind: "photo", file: `IMG-${i}.jpg`, name: null, mime: MIME.JPEG, size: 240_000 + i, width: 1200, height: 900, secs: null, caption: i % 2 ? "Mira qué vistas" : null, view_once: false, omitted: false }]);
        else if (r === 1) (kind = "voice"), (parts = [{ type: "media", kind: "voice", file: null, name: null, mime: MIME.OGG_AUDIO, size: 31_000, width: null, height: null, secs: 4 + (i % 50), caption: null, view_once: false, omitted: false }]);
        else if (r === 2) (kind = "location"), (parts = [{ type: "location", lat: 36.5298, lng: -6.2923, name: "Playa de la Caleta", live: false }]);
        else if (r === 3) (kind = "call"), (parts = [{ type: "call", video: i % 2 === 0, outcome: i % 3 ? "answered" : "missed", secs: i % 3 ? 120 + i : null }]);
        else if (r === 4 && c.kind === "group") (kind = "poll"), (parts = [{ type: "poll", question: "¿Qué día quedamos?", options: [{ text: "Sábado", voters: [], votes: 3 }, { text: "Domingo", voters: [], votes: 1 }], multiple: false }]);
        else if (r === 5) (kind = "link"), (parts = [{ type: "text", text: "Look: https://example.com/viaje" }, { type: "link", url: "https://example.com/viaje", title: "Example trip" }]);
        else if (r === 6) (kind = "document"), (parts = [{ type: "media", kind: "document", file: null, name: "Billetes tren.pdf", mime: MIME.PDF, size: 88_000, width: null, height: null, secs: null, caption: null, view_once: false, omitted: true }]);
        else if (r === 7 && i % 4 === 0) (kind = "system"), (parts = [{ type: "system", event: "member_added", text: null }]);
        else if (r === 8) (kind = "sticker"), (parts = [{ type: "media", kind: "sticker", file: i % 3 ? `STK-${i}.webp` : null, name: null, mime: MIME.WEBP, size: 18_000, width: 512, height: 512, secs: 0, caption: null, view_once: false, omitted: false }]);
        else if (r === 9 && c.kind === "group") (kind = "album"), (parts = [{ type: "card", kind: "album", title: null, text: null }]);
        else if (r === 10 && i % 3 === 0) (kind = "unknown"), (parts = [{ type: "unknown", code: "9999", text: null }]);
        const row: MessageRow = {
          id: ids++, conversation: c.id, source: sid, app, pos: i, time_ms: ts, wall_clock: src.wall_clock, from_me: fromMe && ownerKnown, sender,
          sender_name: people.find((p) => p.id === sender)?.name ?? null, kind, reply: null,
          message: { id: String(i), conversation: String(c.id), sender: String(sender), from_me: fromMe, time_ms: ts, edited_ms: null, edited: i % 23 === 0, deleted: false, forwarded: i % 29 === 0, starred: false, reply_to: null, parts, reactions: i % 11 === 0 ? [{ by: null, from_me: !fromMe, emoji: "❤️" }] : [] },
        };
        if (i % 13 === 5 && i > 0) {
          row.reply = msgs[msgs.length - 1].id;
          row.message.reply_to = "x";
        }
        msgs.push(row);
      }
    }
    return src;
  }

  if (!opts.empty) {
    addSource("whatsapp", "whatsapp-export", "WhatsApp Chat with Ana Prueba.zip", false, 1);
    addSource("telegram", "telegram-json", "result.json", true, 2);
    addSource("sms", "sms-xml", "sms-20261101.xml", true, 3);
    contacts.push({ id: ids++, name: "Gema Ensayo", phones: ["+9999 555 0177"], emails: [], people: [], cardOnly: true });
  }

  const sourceRow = (s: Source): SourceRow => {
    const mine = msgs.filter((m) => m.source === s.id);
    return {
      ...s,
      conversations: convs.filter((c) => c.source === s.id).length,
      people: people.filter((p) => p.source === s.id).length,
      messages: mine.length,
      first_ms: mine.length ? Math.min(...mine.map((m) => m.time_ms ?? NOW)) : null,
      last_ms: mine.length ? Math.max(...mine.map((m) => m.time_ms ?? 0)) : null,
      owner_known: people.some((p) => p.source === s.id && p.me),
    };
  };
  const source = (id: number) => sources.find((s) => s.id === id) ?? fail("not_found", `no source ${id}`);
  const convOf = (id: number) => convs.find((c) => c.id === id) ?? fail("not_found", `no conversation ${id}`);
  const nameOf = (pid: number | null) => {
    const p = people.find((x) => x.id === pid);
    if (!p) return null;
    return contacts.find((c) => c.id === p.contact)?.name ?? p.name ?? p.username ?? p.phone ?? p.key;
  };
  const withNames = (m: MessageRow): MessageRow => ({ ...m, sender_name: nameOf(m.sender) });

  function matches(f: LibFilter, m: MessageRow): boolean {
    const c = convOf(m.conversation);
    const who = (ids: number[]) =>
      (m.sender !== null && ids.includes(m.sender)) || (c.kind === "direct" && c.members.some((p) => ids.includes(p) && !people.find((x) => x.id === p)?.me));
    if (f.apps.length && !f.apps.includes(m.app)) return false;
    if (f.sources.length && !f.sources.includes(m.source)) return false;
    if (f.conversations.length && !f.conversations.includes(m.conversation)) return false;
    if (f.contacts.length && !who(people.filter((p) => p.contact !== null && f.contacts.includes(p.contact)).map((p) => p.id))) return false;
    if (f.people.length && !who(f.people)) return false;
    if (f.kinds.length && !f.kinds.includes(m.kind)) return false;
    if (f.from_me !== null && m.from_me !== f.from_me) return false;
    if (f.after_ms !== null && (m.time_ms ?? 0) < f.after_ms) return false;
    if (f.before_ms !== null && (m.time_ms ?? 0) >= f.before_ms) return false;
    if (f.words.trim()) {
      const text = fold(textOf(m));
      if (!fold(f.words).split(/\s+/).filter(Boolean).every((w) => text.includes(w))) return false;
    }
    return true;
  }
  const isEmpty = (f: LibFilter) =>
    !f.apps.length && !f.sources.length && !f.conversations.length && !f.contacts.length && !f.people.length && !f.kinds.length && f.from_me === null && f.after_ms === null && f.before_ms === null && !f.words;
  const textOf = (m: MessageRow) =>
    m.message.parts.map((p) => (p.type === "text" ? p.text : p.type === "media" ? (p.caption ?? "") : p.type === "poll" ? p.question : p.type === "location" ? (p.name ?? "") : "")).join(" ");

  function ordered(list: MessageRow[], order: "conversation" | "time" | "newest") {
    const by = order === "conversation" ? (a: MessageRow, b: MessageRow) => a.conversation - b.conversation || a.pos - b.pos : (a: MessageRow, b: MessageRow) => (a.time_ms ?? 0) - (b.time_ms ?? 0) || a.id - b.id;
    const out = [...list].sort(by);
    return order === "newest" ? out.reverse() : out;
  }

  function selected(s: Selection) {
    return msgs.filter((m) => ((!isEmpty(s.filter) && matches(s.filter, m)) || s.add.includes(m.id)) && !s.remove.includes(m.id));
  }

  function info(): LibraryInfo {
    return {
      dir: kept ? "/home/you/.local/share/app.oopz/library" : "/home/you/.cache/app.oopz/library",
      kept,
      sources: sources.length,
      messages: msgs.length,
      bytes: msgs.length * 900 + sources.length * 40_000,
      pdf: !opts.android,
    };
  }

  function fail(code: string, detail: string): never {
    throw { code, detail };
  }

  function contactRows(): ContactRow[] {
    return contacts.map((c) => ({
      id: c.id,
      name: c.name,
      phones: c.phones.length ? c.phones : people.filter((p) => p.contact === c.id && p.phone).map((p) => p.phone!),
      emails: c.emails,
      people: people.filter((p) => p.contact === c.id).map((p): [number, AppCode] => [p.id, p.app]),
    }));
  }

  function suggestions() {
    const byPhone = new Map<string, PersonRow[]>();
    for (const p of people) if (p.phone && !p.me) byPhone.set(p.phone, [...(byPhone.get(p.phone) ?? []), p]);
    const out = [];
    for (const [value, list] of byPhone) {
      const card = contacts.find((c) => c.phones.includes(value));
      const unlinked = list.filter((p) => p.contact === null || (card && p.contact !== card.id));
      const sourcesOf = new Set(list.map((p) => p.source));
      if (unlinked.length && (sourcesOf.size > 1 || card)) {
        out.push({ reason: "phone" as const, value, people: list.map((p) => p.id), contact: card?.id ?? list.find((p) => p.contact !== null)?.contact ?? null, name: card?.name ?? list[0].name });
      }
    }
    return out;
  }

  return {
    /** "Wipe now": the library goes, kept or not. */
    wipe() {
      for (const list of [sources, people, convs, msgs, contacts, selections] as unknown[][]) list.length = 0;
      imported.clear();
    },
    async libraryInfo() {
      return info();
    },
    async librarySetKeep(keep) {
      await sleep(300);
      kept = keep;
      return info();
    },
    async libAdapters() {
      return ADAPTERS;
    },
    async libImport(path, adapter, secret, onProgress) {
      const name = path.replace(/^.*[\\/]/, "").replace(/^.*%2F/, "");
      const lower = name.toLowerCase();
      const pickedId =
        adapter ?? (lower.endsWith(".crypt15") ? "whatsapp-android" : lower.endsWith(dot(MIME.JSON)) ? "telegram-json" : lower.endsWith(".xml") ? "sms-xml" : lower.endsWith(dot(MIME.CSV)) ? "viber-csv" : lower.endsWith(dot(MIME.PLAIN_TEXT)) || lower.endsWith(dot(MIME.ZIP)) ? "whatsapp-export" : !name.includes(".") ? "whatsapp-ios" : null);
      const a = ADAPTERS.find((x) => x.id === pickedId) ?? fail("unsupported_file", `${name} isn't a file oopz can open`);
      if (a.secret !== "none") {
        if (!secret) fail(a.secret === "key" ? "needs_key" : "needs_password", "this file needs its owner's secret");
        if (a.secret === "key" && secret.replace(/\s/g, "").toLowerCase() !== opts.rightKey) fail("wrong_key", "the key doesn't open it");
        if (a.secret === "password" && secret === "wrong") fail("wrong_key", "the password doesn't open it");
      }
      if (imported.has(path)) fail("exists", `${name} is already in the library (source 1)`);
      await slowly(1800, onProgress, () => "import");
      imported.add(path);
      return sourceRow(addSource(a.app, a.id, name, a.id !== "whatsapp-export", sources.length + 4));
    },
    async libSources() {
      return sources.map(sourceRow);
    },
    async libRemoveSource(id) {
      source(id);
      sources.splice(sources.indexOf(source(id)), 1);
      const gone = (x: { source: number }) => x.source === id;
      for (const list of [people, convs, msgs] as { source: number }[][]) for (let i = list.length - 1; i >= 0; i--) if (gone(list[i])) list.splice(i, 1);
      for (let i = contacts.length - 1; i >= 0; i--) if (!contacts[i].cardOnly && !people.some((p) => p.contact === contacts[i].id)) contacts.splice(i, 1);
    },
    async libSetMe(person) {
      const p = people.find((x) => x.id === person) ?? fail("not_found", `no person ${person}`);
      for (const x of people) if (x.source === p.source) x.me = x.id === person;
      for (const m of msgs) if (m.source === p.source && m.sender !== null) m.from_me = m.sender === person;
    },
    async libConversations(filter) {
      await sleep(60);
      const hit = isEmpty(filter) ? null : new Set(msgs.filter((m) => matches(filter, m)).map((m) => m.conversation));
      return convs
        .filter((c) => !hit || hit.has(c.id))
        .map((c): ConversationRow => {
          const mine = msgs.filter((m) => m.conversation === c.id);
          const last = mine[mine.length - 1];
          return {
            id: c.id, source: c.source, app: source(c.source).app, kind: c.kind, name: c.name,
            members: c.members.filter((p) => !people.find((x) => x.id === p)?.me).map((p) => nameOf(p) ?? "?"),
            messages: mine.length, first_ms: mine[0]?.time_ms ?? null, last_ms: last?.time_ms ?? null, wall_clock: source(c.source).wall_clock,
            preview: last ? textOf(last).slice(0, 80) || null : null,
          };
        })
        .sort((a, b) => (b.last_ms ?? 0) - (a.last_ms ?? 0));
    },
    async libConversationLen(conversation) {
      return msgs.filter((m) => m.conversation === conversation).length;
    },
    async libMessages(conversation, from, count) {
      await sleep(40);
      return msgs.filter((m) => m.conversation === conversation && m.pos >= from).slice(0, Math.min(count, 500)).map(withNames);
    },
    async libPositionAt(conversation, ts) {
      const m = msgs.find((x) => x.conversation === conversation && (x.time_ms ?? 0) >= ts);
      return m ? m.pos : null;
    },
    async libPosition(message) {
      const m = msgs.find((x) => x.id === message);
      return m ? [m.conversation, m.pos] : null;
    },
    async libSearch(filter, limit) {
      await sleep(80);
      const words = fold(filter.words).split(/\s+/).filter(Boolean);
      return ordered(msgs.filter((m) => matches(filter, m)), "newest")
        .slice(0, limit)
        .map((m): LibHit => {
          let snippet = textOf(m);
          for (const w of words) {
            const at = fold(snippet).indexOf(w);
            if (at >= 0) snippet = `${snippet.slice(0, at)}\u0002${snippet.slice(at, at + w.length)}\u0003${snippet.slice(at + w.length)}`;
          }
          return { id: m.id, conversation: m.conversation, app: m.app, time_ms: m.time_ms, from_me: m.from_me, sender_name: nameOf(m.sender), snippet: snippet || `[${m.kind}]` };
        });
    },
    async libFind(filter, order, offset, limit) {
      await sleep(60);
      return ordered(msgs.filter((m) => matches(filter, m)), order).slice(offset, offset + Math.min(limit, 500)).map(withNames);
    },
    async libCount(filter) {
      return msgs.filter((m) => matches(filter, m)).length;
    },
    async libSelected(selection, order) {
      await sleep(60);
      return ordered(selected(selection), order).map((m) => m.id);
    },
    async libMessagesById(list) {
      return list.map((id) => msgs.find((m) => m.id === id)).filter((m): m is MessageRow => !!m).map(withNames);
    },
    async libMedia(message, part) {
      await sleep(80);
      const p = msgs.find((m) => m.id === message)?.message.parts[part];
      if (p?.type !== "media" || !p.file) return new ArrayBuffer(0);
      return p.kind === "sticker" ? mockSticker(message) : new ArrayBuffer(0);
    },
    async libStats(filter) {
      await sleep(120);
      const list = isEmpty(filter) ? msgs : msgs.filter((m) => matches(filter, m));
      const s: LibStats = { messages: list.length, from_me: 0, first_ms: null, last_ms: null, by_month: [], by_hour: Array(24).fill(0), by_day_hour: Array.from({ length: 7 }, () => Array(24).fill(0)), by_kind: [], by_app: [], by_person: [], media_bytes: 0, voice_secs: 0 };
      const months = new Map<string, number>();
      const kinds = new Map<Kind, [number, number]>();
      const apps = new Map<AppCode, number>();
      const who = new Map<number | null, number>();
      for (const m of list) {
        const d = new Date(m.time_ms ?? 0);
        if (m.from_me) s.from_me++;
        s.first_ms = Math.min(s.first_ms ?? Infinity, m.time_ms ?? Infinity);
        s.last_ms = Math.max(s.last_ms ?? 0, m.time_ms ?? 0);
        const month = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
        months.set(month, (months.get(month) ?? 0) + 1);
        s.by_hour[d.getHours()]++;
        s.by_day_hour[d.getDay()][d.getHours()]++;
        let bytes = 0;
        for (const p of m.message.parts) if (p.type === "media") (bytes += p.size ?? 0), p.kind === "voice" && (s.voice_secs += p.secs ?? 0);
        s.media_bytes += bytes;
        const k = kinds.get(m.kind) ?? [0, 0];
        kinds.set(m.kind, [k[0] + 1, k[1] + bytes]);
        apps.set(m.app, (apps.get(m.app) ?? 0) + 1);
        if (!m.from_me) who.set(m.sender, (who.get(m.sender) ?? 0) + 1);
      }
      s.by_month = [...months].sort((a, b) => (a[0] < b[0] ? -1 : 1));
      s.by_kind = [...kinds].map(([k, [n, b]]): [Kind, number, number] => [k, n, b]).sort((a, b) => b[1] - a[1]);
      s.by_app = [...apps].sort((a, b) => b[1] - a[1]);
      s.by_person = [...who].map(([p, n]): [number | null, string | null, number] => [p, nameOf(p), n]).sort((a, b) => b[2] - a[2]).slice(0, 50);
      return s;
    },
    async libPeople(src) {
      return people.filter((p) => src === null || p.source === src).map((p) => ({ ...p, messages: msgs.filter((m) => m.sender === p.id).length }));
    },
    async libContacts() {
      return contactRows();
    },
    async libLink(list, name) {
      let id = people.find((p) => list.includes(p.id) && p.contact !== null)?.contact ?? null;
      if (id === null) {
        id = ids++;
        contacts.push({ id, name: name ?? people.find((p) => list.includes(p.id))?.name ?? "?", phones: [], emails: [], people: [], cardOnly: false });
      } else if (name) contacts.find((c) => c.id === id)!.name = name;
      for (const p of people) if (list.includes(p.id)) p.contact = id;
      return id;
    },
    async libUnlink(person) {
      const p = people.find((x) => x.id === person) ?? fail("not_found", `no person ${person}`);
      p.contact = null;
    },
    async libRenameContact(contact, name) {
      if (!name.trim()) fail("not_found", "a contact needs a name");
      (contacts.find((c) => c.id === contact) ?? fail("not_found", `no contact ${contact}`)).name = name.trim();
    },
    async libDeleteContact(contact) {
      const i = contacts.findIndex((c) => c.id === contact);
      if (i < 0) fail("not_found", `no contact ${contact}`);
      contacts.splice(i, 1);
      for (const p of people) if (p.contact === contact) p.contact = null;
    },
    async libSuggestions() {
      return suggestions();
    },
    async libAccept(reason, value) {
      const s = suggestions().find((x) => x.reason === reason && x.value === value) ?? fail("not_found", "that suggestion is gone");
      let id = s.contact;
      if (id === null) {
        id = ids++;
        contacts.push({ id, name: s.name ?? value, phones: [], emails: [], people: [], cardOnly: false });
      }
      for (const p of people) if (s.people.includes(p.id)) p.contact = id;
      return id;
    },
    async libImportVcards(path) {
      await sleep(300);
      if (!/\.vcf$/i.test(path) && !path.startsWith("content://")) fail("unsupported_file", "not a .vcf file");
      contacts.push({ id: ids++, name: "Hugo Inventado", phones: ["+9999 555 0111"], emails: ["hugo@example.com"], people: [], cardOnly: true });
      return 1;
    },
    async libSaveSelection(name, selection) {
      if (selections.some((s) => s.name === name)) fail("exists", `a selection named ${name} exists`);
      const id = ids++;
      selections.push({ id, name, selection: JSON.parse(JSON.stringify(selection)), saved_ms: Date.now() });
      return id;
    },
    async libSelections() {
      return selections;
    },
    async libDeleteSelection(id) {
      selections.splice(selections.findIndex((s) => s.id === id), 1);
    },
    async libExport(selection, format, options, folder, onProgress) {
      const n = selected(selection).length;
      if (n === 0) fail("not_found", "the selection is empty");
      if (format === "pdf" && opts.android) fail("unsupported", "PDF exports aren't in this build");
      await slowly(1500, onProgress, () => "export");
      const dir = folder ?? "/data/user/0/app.oopz/cache/decrypted/session-x1y2/export-a1b2";
      const ext = { html: "html", json: "json", csv: "csv", txt: "txt", vcard: "vcf", "media-zip": "zip", pdf: "pdf" }[format];
      return { path: `${dir}/oopz-${format}-1.${ext}`, folder: dir, messages: n, bytes: n * 420, missing_files: format === "media-zip" ? 3 : 0, pages: format === "pdf" ? Math.ceil(n / 30) + (options.cover ? 1 : 0) : 0, missing_glyphs: 0 };
    },
    async libExportContacts(list, folder) {
      if (list.length === 0) fail("not_found", "no contacts picked");
      const dir = folder ?? "/data/user/0/app.oopz/cache/decrypted/session-x1y2/export-c3d4";
      return { path: `${dir}/oopz-contacts-1.vcf`, folder: dir, messages: 0, bytes: list.length * 120, missing_files: 0, pages: 0, missing_glyphs: 0 };
    },
  };
}
