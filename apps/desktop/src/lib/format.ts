import type { ChatRow } from "./backend/types";
import { app } from "./state.svelte";
import { strings } from "./i18n";

export const fmt = (n: number) => n.toLocaleString(app.lang);
export const date = (ms: number | null) => (ms ? new Date(ms).toLocaleDateString(app.lang) : "–");
export const longDate = (ms: number) => new Date(ms).toLocaleDateString(app.lang, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
export const time = (ms: number) => new Date(ms).toLocaleTimeString(app.lang, { hour: "2-digit", minute: "2-digit" });
export const dateTime = (ms: number) => new Date(ms).toLocaleString(app.lang);
export const year = (ms: number | null) => (ms ? String(new Date(ms).getFullYear()) : "–");
// Under 1 MB in kB, so a small file doesn't read "0 MB"; from 1 GB in GB.
export const mb = (bytes: number) =>
  bytes > 0 && bytes < 1e6
    ? `${Math.max(1, Math.round(bytes / 1e3)).toLocaleString(app.lang)} kB`
    : bytes >= 1e9
      ? `${(bytes / 1e9).toLocaleString(app.lang, { maximumFractionDigits: 1 })} GB`
      : `${(bytes / 1e6).toLocaleString(app.lang, { maximumFractionDigits: 1 })} MB`;

/** A chat's name (a group's subject, or the contact's name from wa.db), or its kind and the masked end of its id when
 *  it has none or names are hidden. */
export const chatName = (c: ChatRow) => (c.name && !app.hideNames ? c.name : `${strings(app.lang).who[c.kind]} ••••${c.ending}`);

/** The chat's last message in a few words: its text, or its kind; "You: " first when it was sent. */
export function previewText(c: ChatRow): string | null {
  const p = c.preview;
  if (!p) return null;
  const t = strings(app.lang);
  const body = p.text ?? `[${t.kinds[p.kind] ?? `#${p.kind}`}]`;
  return p.from_me ? `${t.you}: ${body}` : body;
}

export const isGroup = (c: ChatRow) => c.kind === "group" || c.kind === "broadcast";

/** The Ctrl or ⌘ key's name on this system. */
export const mod = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl ";
