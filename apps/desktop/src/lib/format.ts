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

/** A chat as the backend knows it: its kind and the masked end of its id. There are no names in a backup's index. */
export const chatName = (c: ChatRow) => `${strings(app.lang).who[c.kind]} ••••${c.ending}`;

export const isGroup = (c: ChatRow) => c.kind === "group" || c.kind === "broadcast";

/** The Ctrl or ⌘ key's name on this system. */
export const mod = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl ";
