// What the UI knows about each kind of message (msgkinds' kinds: crates/oopz-model's Kind). Every table is a
// Record<Kind, …>, so a kind added to backend/types.ts doesn't build until each one says what to do with it.
import type { IconName } from "./components/Icon.svelte";
import type { Kind, MediaKind } from "./backend/types";

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
  system: "info",
  deleted: "ban",
  unknown: "alert",
};

/** It plays for a while (msgkinds' `Kind::has_duration`): a sticker, a photo, a GIF, a document or an album never
 *  shows one, whatever the source wrote. */
export const HAS_DURATION: Record<Kind, boolean> = {
  text: false,
  link: false,
  photo: false,
  video: true,
  video_note: true,
  gif: false,
  voice: true,
  audio: true,
  sticker: false,
  document: false,
  location: false,
  contact: false,
  poll: false,
  call: true,
  album: false,
  sticker_pack: false,
  event: false,
  payment: false,
  game: false,
  business: false,
  story: false,
  system: false,
  deleted: false,
  unknown: false,
};

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

/** A file's type from its first bytes, for a blob the webview shows (WebP stickers, PNG, JPEG, Ogg voice notes). */
export function sniff(bytes: ArrayBuffer, fallback: string): string {
  const b = new Uint8Array(bytes, 0, Math.min(12, bytes.byteLength));
  const ascii = (from: number, to: number) => String.fromCharCode(...b.slice(from, to));
  if (ascii(0, 4) === "RIFF" && ascii(8, 12) === "WEBP") return "image/webp";
  if (b[0] === 0x89 && ascii(1, 4) === "PNG") return "image/png";
  if (b[0] === 0xff && b[1] === 0xd8) return "image/jpeg";
  if (ascii(0, 4) === "OggS") return "audio/ogg";
  return fallback;
}
