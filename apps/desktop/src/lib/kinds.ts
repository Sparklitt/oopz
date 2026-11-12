// What the UI knows about each kind of message (msgkinds' kinds: crates/oopz-model's Kind). Every table is a
// Record<Kind, …>, so a kind added to backend/types.ts doesn't build until each one says what to do with it.
import type { IconName } from "./components/Icon.svelte";
import type { Kind, MediaKind } from "./backend/types";
import { FORMATS, HEAD_LEN, MIME, sniff as sniffHead } from "./generated/msgkinds";

export const KIND_ICON: Record<Kind, IconName> = {
  text: "chats",
  link: "globe",
  photo: "image",
  video: "video",
  video_note: "video",
  gif: "video",
  voice: "mic",
  audio: "mic",
  sticker: "smile",
  document: "file",
  location: "pin",
  contact: "user",
  poll: "poll",
  call: "call",
  album: "grid",
  sticker_pack: "smile",
  event: "calendar",
  payment: "gift",
  game: "play",
  business: "megaphone",
  story: "eye",
  reaction: "smile",
  system: "info",
  deleted: "ban",
  unknown: "alert",
};

/** It plays for a while (msgkinds' `Kind::has_duration`): a sticker, a photo, a GIF, a document or an album never
 *  shows one, whatever the source wrote. */
export { HAS_DURATION } from "./generated/msgkinds";

/** Every kind, in msgkinds' order. */
export const KINDS = Object.keys(KIND_ICON) as Kind[];

/** Media kinds the app shows from their file when the source's folder has it. */
export const SHOWN_FROM_FILE: Record<MediaKind, "image" | "audio" | null> = {
  photo: "image",
  sticker: "image",
  voice: "audio",
  video: null,
  video_note: null,
  gif: null,
  audio: null,
  document: null,
};

/** Telegram's animated sticker: Lottie JSON, gzipped. */
export const TGS = MIME.TGS;

/** A file's type from its first bytes (msgkinds' signatures), for a blob the webview shows; `fallback`, the type the
 *  source gave, when the bytes say nothing or only show its container (a `.tgs` is gzip, an `.m4a` an MP4). A gzip
 *  otherwise is a Lottie sticker, the only one the app shows. */
export function sniff(bytes: ArrayBuffer, fallback: string): string {
  const found = sniffHead(new Uint8Array(bytes, 0, Math.min(HEAD_LEN, bytes.byteLength)));
  if (!found) return fallback;
  if (FORMATS.some((f) => f.mime === fallback && f.container === found)) return fallback;
  return found === MIME.GZIP ? TGS : found;
}
