// Invented data for working on the UI without Rust or a real backup (VITE_BACKEND=mock, or a development page in a
// browser). Everything is generated from the chat and position, so the same message always looks the same, and a
// chat of a million messages costs nothing until a window of it is asked for.
//
// To reach each case: the key 0123456789abcdef repeated four times opens every crypt15 backup, anything else of
// 64 hex characters is the wrong key. "Choose a backup file" picks a crypt14 file. The 2025 backup opens with a
// damaged database. Update checks answer in turn: a new version, none, a failure.
import pkg from "../../../package.json";
import type { Backend, BackupFile, ChatRow, Hit, Line, Missing, OnProgress, ReceiveStatus, Stats, Summary } from "./types";

const RIGHT_KEY = "0123456789abcdef".repeat(4);
const FOLDER = "/home/you/Phone/Databases";
const NOW = Date.UTC(2026, 10, 9, 16, 24);
const FIRST = Date.UTC(2019, 2, 3, 9, 0);

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Reports progress in steps over `ms`, like a slow disk would. */
async function slowly(ms: number, onProgress: OnProgress) {
  const steps = 40;
  for (let i = 0; i <= steps; i++) {
    onProgress(Math.round((i / steps) * 1000));
    await sleep(ms / steps);
  }
}

function fail(code: string, detail: string): never {
  throw { code, detail };
}

/** A small integer hash: the same inputs always give the same "random" number. */
function hash(a: number, b: number): number {
  let h = (a * 0x9e3779b1) ^ (b * 0x85ebca77);
  h = Math.imul(h ^ (h >>> 16), 0x7feb352d);
  h = Math.imul(h ^ (h >>> 15), 0x846ca68b);
  return (h ^ (h >>> 16)) >>> 0;
}

const PHRASES = [
  "¿Cómo estás?",
  "¡Muy bien! ¿Y tú?",
  "See you at the beach on Saturday?",
  "¿Vamos a la playa este fin de semana?",
  "Ben Ejemplo says hi",
  "Perfecto, nos vemos mañana",
  "Can you send me the address?",
  "Ana Prueba llega a las ocho",
  "I left the keys with the neighbour",
  "Qué ganas de playa",
  "Happy birthday! 🎉",
  "Gracias por todo",
  "On my way, ten minutes",
  "¿Has visto las fotos del viaje?",
  "Dinner at Ana's tonight",
  "Llámame cuando puedas",
  "The train is late again",
  "Buenas noches",
  "Did you book the flights?",
  "Te mando el documento ahora",
];
const DOCS = ["Contrato alquiler.pdf", "Billetes tren.pdf", "Receta paella.docx", "Seguro coche.pdf"];

/** The invented chats of the first backup. The second one is older and lacks part of chat 4. */
const CHATS: ChatRow[] = [
  { number: 1, kind: "contact", ending: "001", messages: 1_000_000, last_ms: NOW },
  { number: 2, kind: "group", ending: "002", messages: 48_210, last_ms: NOW - 2 * 3600e3 },
  { number: 3, kind: "contact", ending: "003", messages: 12_034, last_ms: NOW - 4 * 3600e3 },
  { number: 4, kind: "contact_lid", ending: "004", messages: 4_612, last_ms: NOW - 26 * 3600e3 },
  { number: 5, kind: "group", ending: "005", messages: 9_876, last_ms: NOW - 3 * 86400e3 },
  { number: 6, kind: "broadcast", ending: "006", messages: 120, last_ms: NOW - 40 * 86400e3 },
  ...Array.from({ length: 54 }, (_, i): ChatRow => {
    const n = i + 7;
    return { number: n, kind: n % 9 === 0 ? "group" : "contact", ending: String(n).padStart(3, "0"), messages: 20 + (hash(n, 1) % 3000), last_ms: NOW - (hash(n, 2) % 400) * 86400e3 };
  }),
];
const GROUP_SENDERS = ["011", "023", "047", "058", "072"];

function chatsOf(older: boolean): ChatRow[] {
  return CHATS.map((c) => (older && c.number === 4 ? { ...c, messages: 1_200, last_ms: Date.UTC(2024, 2, 1) } : c));
}

function rowOf(rows: ChatRow[], chat: number): ChatRow {
  return rows.find((c) => c.number === chat) ?? fail("not_found", `chat ${chat}`);
}

// The newest messages of chat 1: one of each kind, so the viewer shows them all on opening.
const SCRIPT: { kind: number; me: boolean; text: string | null }[] = [
  { kind: 0, me: false, text: PHRASES[0] },
  { kind: 0, me: true, text: PHRASES[1] },
  { kind: 1, me: false, text: "Mira qué vistas" },
  { kind: 99, me: false, text: null },
  { kind: 2, me: true, text: null },
  { kind: 9, me: false, text: DOCS[0] },
  { kind: 5, me: true, text: null },
  { kind: 46, me: false, text: "¿Cena el viernes?" },
  { kind: 90, me: false, text: null },
  { kind: 15, me: true, text: null },
  { kind: 3, me: false, text: null },
  { kind: 20, me: true, text: null },
  { kind: 7, me: false, text: "The security code changed." },
  { kind: 0, me: false, text: "Last message: Great!" },
];

interface Msg {
  kind: number;
  me: boolean;
  /** Index into PHRASES when the text is one, for search. */
  phrase: number | null;
  text: string | null;
}

function kindFor(h: number): number {
  const k = h % 100;
  if (k < 70) return 0;
  const rest = [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 4, 5, 7, 9, 9, 13, 15, 16, 20, 20, 42, 46, 81, 90, 90, 99, 0, 0];
  return rest[k - 70];
}

function message(chat: number, len: number, pos: number): Msg {
  if (chat === 1 && pos >= len - SCRIPT.length) {
    const s = SCRIPT[pos - (len - SCRIPT.length)];
    const phrase = s.text === null ? -1 : PHRASES.indexOf(s.text);
    return { kind: s.kind, me: s.me, phrase: phrase >= 0 ? phrase : null, text: s.text };
  }
  const h = hash(chat, pos);
  const kind = kindFor(h);
  const me = (h >>> 8) % 2 === 0;
  const phrase = (h >>> 12) % PHRASES.length;
  if (kind === 0 || ((kind === 1 || kind === 3) && h % 3 === 0)) return { kind, me, phrase, text: PHRASES[phrase] };
  if (kind === 9) return { kind, me, phrase: null, text: DOCS[h % DOCS.length] };
  if (kind === 46) return { kind, me, phrase: null, text: "¿Cena el viernes?" };
  if (kind === 7) return { kind, me: false, phrase: null, text: "The security code changed." };
  return { kind, me, phrase: null, text: null };
}

function timeOf(row: ChatRow, pos: number): number {
  const last = row.last_ms ?? NOW;
  const step = (last - FIRST) / Math.max(1, row.messages - 1);
  return Math.round(FIRST + step * pos + (hash(row.number, pos) % Math.max(1, Math.floor(step / 2))));
}

function lineOf(row: ChatRow, pos: number): Line {
  const m = message(row.number, row.messages, pos);
  const group = row.kind === "group";
  return {
    id: row.number * 10_000_000 + pos,
    ts: timeOf(row, pos),
    from_me: m.me && m.kind !== 7,
    kind: m.kind,
    text: m.text,
    sender: group && !m.me && m.kind !== 7 ? GROUP_SENDERS[hash(row.number, pos) % GROUP_SENDERS.length] : null,
  };
}

const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
const words = (s: string) => s.split(/([\p{L}\p{N}]+)/u);

/** The phrase with the words the query starts matched between \u0002 and \u0003, or null when it doesn't match. */
function snippet(phrase: string, terms: string[]): string | null {
  const parts = words(phrase);
  const folded = parts.map(fold);
  if (!terms.every((t) => folded.some((w) => w.startsWith(t)))) return null;
  return parts.map((p, i) => (terms.some((t) => folded[i].startsWith(t)) && /\p{L}|\p{N}/u.test(p) ? `\u0002${p}\u0003` : p)).join("");
}

function kindBytes(kind: number, h: number): number {
  const base: Record<number, number> = { 1: 1.4e6, 2: 60e3, 3: 12e6, 9: 2.3e6, 13: 800e3, 20: 40e3, 81: 3e6, 99: 5e6 };
  return base[kind] ? Math.round(base[kind] * (0.5 + (h % 100) / 100)) : 0;
}

function emptyStats(): Stats {
  return { messages: 0, from_me: 0, from_others: 0, first_ms: null, last_ms: null, by_month: [], by_hour: Array(24).fill(0), by_kind: [], audio_seconds: 0, senders: [] };
}

function chatStats(row: ChatRow): Stats {
  const s = emptyStats();
  const months = new Map<string, number>();
  const kinds = new Map<number, [number, number]>();
  const senders = new Map<string, number>();
  const offset = new Date(NOW).getTimezoneOffset() * 60e3;
  let monthEnd = 0;
  let month = "";
  for (let pos = 0; pos < row.messages; pos++) {
    const line = lineOf(row, pos);
    if (line.ts >= monthEnd) {
      const d = new Date(line.ts);
      month = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
      monthEnd = new Date(d.getFullYear(), d.getMonth() + 1, 1).getTime();
    }
    months.set(month, (months.get(month) ?? 0) + 1);
    s.by_hour[Math.floor((line.ts - offset) / 3600e3) % 24]++;
    const h = hash(row.number, pos);
    const k = kinds.get(line.kind) ?? [0, 0];
    k[0]++;
    k[1] += kindBytes(line.kind, h);
    kinds.set(line.kind, k);
    if (line.kind === 2) s.audio_seconds += 5 + (h % 115);
    if (line.from_me) s.from_me++;
    if (line.sender) senders.set(line.sender, (senders.get(line.sender) ?? 0) + 1);
  }
  s.messages = row.messages;
  s.from_others = row.messages - s.from_me;
  s.first_ms = row.messages ? lineOf(row, 0).ts : null;
  s.last_ms = row.messages ? lineOf(row, row.messages - 1).ts : null;
  s.by_month = [...months];
  s.by_kind = [...kinds].map(([k, [n, b]]): [number, number, number] => [k, n, b]).sort((a, b) => b[1] - a[1]);
  s.senders = [...senders].sort((a, b) => b[1] - a[1]);
  return s;
}

function addStats(all: Stats[]): Stats {
  const s = emptyStats();
  const months = new Map<string, number>();
  const kinds = new Map<number, [number, number]>();
  const senders = new Map<string, number>();
  for (const x of all) {
    s.messages += x.messages;
    s.from_me += x.from_me;
    s.from_others += x.from_others;
    s.audio_seconds += x.audio_seconds;
    if (x.first_ms !== null && (s.first_ms === null || x.first_ms < s.first_ms)) s.first_ms = x.first_ms;
    if (x.last_ms !== null && (s.last_ms === null || x.last_ms > s.last_ms)) s.last_ms = x.last_ms;
    x.by_hour.forEach((n, i) => (s.by_hour[i] += n));
    for (const [m, n] of x.by_month) months.set(m, (months.get(m) ?? 0) + n);
    for (const [k, n, b] of x.by_kind) {
      const v = kinds.get(k) ?? [0, 0];
      kinds.set(k, [v[0] + n, v[1] + b]);
    }
    for (const [e, n] of x.senders) senders.set(e, (senders.get(e) ?? 0) + n);
  }
  s.by_month = [...months].sort((a, b) => a[0].localeCompare(b[0]));
  s.by_kind = [...kinds].map(([k, [n, b]]): [number, number, number] => [k, n, b]).sort((a, b) => b[1] - a[1]);
  s.senders = [...senders].sort((a, b) => b[1] - a[1]).slice(0, 20);
  return s;
}

/** A QR-looking picture: finder squares and a fixed pattern. It encodes nothing. */
function fakeQr(): string {
  const n = 29;
  const cells: string[] = [];
  const finder = (x: number, y: number) => {
    const d = Math.max(Math.abs(x - 3), Math.abs(y - 3));
    return d !== 2 && d <= 3;
  };
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      const inFinder = (x < 7 && y < 7) || (x >= n - 7 && y < 7) || (x < 7 && y >= n - 7);
      const on = inFinder ? finder(x < 7 ? x : x - (n - 7), y < 7 ? y : y - (n - 7)) : x > 7 || y > 7 ? hash(x, y) % 2 === 0 : false;
      if (on) cells.push(`<rect x="${x + 2}" y="${y + 2}" width="1" height="1"/>`);
    }
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${n + 4} ${n + 4}" shape-rendering="crispEdges"><rect width="100%" height="100%" fill="#fff"/><g fill="#000">${cells.join("")}</g></svg>`;
}

export function createMock(): Backend {
  const FILES: BackupFile[] = [
    { path: `${FOLDER}/msgstore.db.crypt15`, name: "msgstore.db.crypt15", bytes: 1_204_000_000, kind: "crypt15" },
    { path: `${FOLDER}/msgstore-2026-11-08.1.db.crypt15`, name: "msgstore-2026-11-08.1.db.crypt15", bytes: 1_131_000_000, kind: "crypt15" },
    { path: `${FOLDER}/msgstore-2025-06-01.1.db.crypt15`, name: "msgstore-2025-06-01.1.db.crypt15", bytes: 984_000_000, kind: "crypt15" },
    { path: `${FOLDER}/msgstore-2026-11-07.1.db.crypt15`, name: "msgstore-2026-11-07.1.db.crypt15", bytes: 4_096, kind: "not_crypt15" },
    { path: `${FOLDER}/msgstore-2021-01-01.1.db.crypt14`, name: "msgstore-2021-01-01.1.db.crypt14", bytes: 412_000_000, kind: "crypt14" },
  ];
  const CRYPT14 = "/home/you/Downloads/msgstore-2021-01-01.1.db.crypt14";
  const opened = new Map<number, Summary & { path: string; older: boolean }>();
  const statsCache = new Map<string, Stats>();
  let nextId = 1;
  let updateChecks = 0;
  let receive: { started: number; dest: string; stoppedAt: number | null; how: ReceiveStatus["stopped"] } | null = null;

  const backup = (id: number) => opened.get(id) ?? fail("not_found", `backup ${id}`);
  const rowsOf = (id: number) => backup(id).chat_list;

  function statsFor(id: number, chat: number | null): Stats {
    const rows = rowsOf(id).filter((c) => chat === null || c.number === chat);
    const parts = rows.map((row) => {
      const key = `${backup(id).older}:${row.number}`;
      let s = statsCache.get(key);
      if (!s) statsCache.set(key, (s = chatStats(row)));
      return s;
    });
    return chat === null ? addStats(parts) : (parts[0] ?? emptyStats());
  }

  function receiveStatus(): ReceiveStatus {
    if (!receive) return { running: false, files: [], current: null, refused: 0, stopped: null };
    const at = (receive.stoppedAt ?? Date.now()) - receive.started;
    const plan = [
      { name: "msgstore.db.crypt15", bytes: 412_000_000, from: 2500, to: 8000 },
      { name: "msgstore-2026-11-08.1.db.crypt15", bytes: 409_000_000, from: 8500, to: 13000 },
    ];
    const files = plan.filter((f) => at >= f.to).map((f) => ({ name: f.name, bytes: f.bytes, sha256: "9f2c…e41a" }));
    const cur = plan.find((f) => at >= f.from && at < f.to);
    const done = at >= 14000;
    const stopped = receive.stoppedAt !== null ? receive.how : done ? "done" : null;
    return {
      running: stopped === null,
      files,
      current: stopped === null && cur ? { name: cur.name, done: Math.round(((at - cur.from) / (cur.to - cur.from)) * cur.bytes), total: cur.bytes } : null,
      refused: 0,
      stopped,
    };
  }

  const missing = (a: number, chatA: number, b: number, chatB: number): Missing => {
    const ra = rowOf(rowsOf(a), chatA);
    const rb = rowOf(rowsOf(b), chatB);
    if (chatA !== chatB) return { both: 0, only_a: ra.messages, only_a_first_ms: lineOf(ra, 0).ts, only_a_last_ms: ra.last_ms, only_b: rb.messages, only_b_first_ms: lineOf(rb, 0).ts, only_b_last_ms: rb.last_ms };
    const both = Math.min(ra.messages, rb.messages);
    const gap = (r: ChatRow, from: number) => (r.messages > from ? [lineOf(r, from).ts, lineOf(r, r.messages - 1).ts] : [null, null]);
    const [af, al] = gap(ra, both);
    const [bf, bl] = gap(rb, both);
    return { both, only_a: ra.messages - both, only_a_first_ms: af, only_a_last_ms: al, only_b: rb.messages - both, only_b_first_ms: bf, only_b_last_ms: bl };
  };

  function checkKey(key: string) {
    const k = key.replace(/\s/g, "").toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(k)) fail("key_format", "expected 64 hex characters");
    if (k !== RIGHT_KEY) fail("wrong_key", "the key doesn't decrypt this file");
  }

  return {
    pickFolder: async () => FOLDER,
    pickFile: async () => CRYPT14,
    appVersion: async () => pkg.version,

    async findBackups(path) {
      await sleep(300);
      if (path === CRYPT14) return [{ path, name: "msgstore-2021-01-01.1.db.crypt14", bytes: 412_000_000, kind: "crypt14" }];
      if (receive && path === receive.dest) return receiveStatus().files.map((f): BackupFile => ({ path: `${path}/${f.name}`, name: f.name, bytes: f.bytes, kind: "crypt15" }));
      return FILES;
    },

    async checkBackup(path, key, onProgress) {
      await sleep(200);
      if (path.endsWith(".crypt14")) fail("crypt14", "crypt14 needs the key file");
      const file = FILES.find((f) => f.path === path) ?? FILES.find((f) => path.endsWith(`/${f.name}`));
      if (!file) fail("not_found", path);
      if (file.kind === "not_crypt15") fail("not_crypt15", "the header is cut short");
      checkKey(key);
      await slowly(1800, onProgress);
      for (const s of opened.values()) if (s.path === path) return s;
      const older = file.name !== "msgstore.db.crypt15";
      const unsound = file.name.startsWith("msgstore-2025");
      const chat_list = chatsOf(older);
      const messages = chat_list.reduce((n, c) => n + c.messages, 0);
      const summary = {
        id: nextId++,
        name: file.name,
        bytes: file.bytes,
        sha256: (hash(file.bytes, 7).toString(16).padStart(8, "0") + "c0ffee").repeat(4).slice(0, 64),
        sound: !unsound,
        problems: unsound ? "the integrity check failed on 2 of 41 tables (message_media, message_quoted)" : "",
        chats: chat_list.length,
        messages,
        first_ms: FIRST,
        last_ms: older ? Date.UTC(2026, 10, 8, 2, 0) : NOW,
        one_to_one: chat_list.filter((c) => c.kind === "contact" || c.kind === "contact_lid").length,
        groups: chat_list.filter((c) => c.kind === "group").length,
        chat_list,
        path,
        older,
      };
      opened.set(summary.id, summary);
      return summary;
    },

    async forgetBackup(id) {
      opened.delete(id);
    },
    async checkedBackups() {
      return [...opened.values()];
    },

    async mergeChat(form) {
      await sleep(300);
      if (form.source === form.target) fail("unsupported", "the two backups are the same");
      checkKey(form.target_key);
      if (!form.out_parent) fail("io", "no folder");
      const m = missing(form.source, form.source_chat, form.target, form.target_chat);
      await sleep(1800);
      const folder = `${form.out_parent}/oopz-merged-2026-11-10`;
      return {
        folder,
        backup: `${folder}/Databases/msgstore.db.crypt15`,
        sha256: "4be1".repeat(16),
        target_before: backup(form.target).messages,
        already_there: m.both,
        added: m.only_a,
        checks: ["integrity_check: ok", "foreign_key_check: ok", "message count matches", "re-encrypted file opens with the key", "original fingerprints unchanged"],
        media_copied: form.media ? 214 : null,
        media_missing: form.media ? 3 : null,
        original_untouched: true,
      };
    },

    async exportPage(_id, chat, _labels, outDir) {
      await sleep(700);
      return `${outDir}/oopz-${chat === null ? "backup" : `chat-${chat}`}-2026-11-10.html`;
    },
    async showFolder() {},

    async missing(a, chatA, b, chatB) {
      await sleep(500);
      return missing(a, chatA, b, chatB);
    },

    async buildIndex(id, onProgress) {
      await slowly(2200, onProgress);
      const s = backup(id);
      return { messages: s.messages, with_text: Math.round(s.messages * 0.72), millis: 2214 };
    },

    async search(id, query, chat) {
      const terms = fold(query).split(/[^\p{L}\p{N}]+/u).filter(Boolean);
      if (!terms.length) return [];
      const matching = new Map<number, string>();
      PHRASES.forEach((p, i) => {
        const s = snippet(p, terms);
        if (s) matching.set(i, s);
      });
      const hits: Hit[] = [];
      if (!matching.size) return hits;
      for (const row of rowsOf(id)) {
        if (chat !== null && row.number !== chat) continue;
        let found = 0;
        for (let pos = row.messages - 1; pos >= 0 && found < 200; pos--) {
          const m = message(row.number, row.messages, pos);
          if (m.phrase === null || !matching.has(m.phrase)) continue;
          const line = lineOf(row, pos);
          hits.push({ id: line.id, chat: row.number, ts: line.ts, from_me: line.from_me, snippet: matching.get(m.phrase)! });
          found++;
        }
      }
      return hits.sort((a, b) => b.ts - a.ts).slice(0, 200);
    },

    async stats(id, chat) {
      return statsFor(id, chat);
    },
    async chatLen(id, chat) {
      return rowOf(rowsOf(id), chat).messages;
    },
    async chatLines(id, chat, from, count) {
      await sleep(40);
      const row = rowOf(rowsOf(id), chat);
      const out: Line[] = [];
      for (let p = from; p < Math.min(row.messages, from + count); p++) out.push(lineOf(row, p));
      return out;
    },
    async messagePosition(id, message) {
      const chat = Math.floor(message / 10_000_000);
      const pos = message % 10_000_000;
      return rowsOf(id).some((c) => c.number === chat) ? [chat, pos] : null;
    },

    async receiveStart(parent) {
      await sleep(300);
      receive = { started: Date.now(), dest: `${parent}/oopz-received-2026-11-10`, stoppedAt: null, how: null };
      return { url: "http://192.168.1.24:8752/", qr_svg: fakeQr(), dest: receive.dest, free: 120_400_000_000 };
    },
    async receiveStatus() {
      return receiveStatus();
    },
    async receiveStop() {
      if (receive && receive.stoppedAt === null && receiveStatus().running) {
        receive.stoppedAt = Date.now();
        receive.how = "stopped";
      }
      return receiveStatus();
    },

    async updateCheck() {
      await sleep(900);
      const turn = updateChecks++ % 3;
      if (turn === 1) return null;
      if (turn === 2) fail("update", "couldn't reach github.com (timed out)");
      return { version: "0.1.2" };
    },
    async updateInstall(onProgress) {
      await slowly(2500, onProgress);
      // The real app restarts here; the mock stays on the last screen.
    },
  };
}
