# Every kind of data oopz handles

The catalogue every adapter maps to (`crates/oopz-model`). The kinds, the events and what each app's type codes
mean, per app version, live in the registry shared with hilo, msgkinds (decision
[0020](decisions/0020-the-shared-registry.md); its `docs/kinds.md` and `docs/apps/`): a new code or app version is
added there first. An app's type that matches none is kept as `unknown(app, code)`, shown as a labelled card
("WhatsApp: 9999, not shown yet"), counted and exported, never dropped. This page is the wider catalogue the model
grows towards. Per-app mappings and the versions checked are in `docs/adapters/<app>.md`. Apps: WA WhatsApp, TG Telegram, SG
Signal, LN LINE, VB Viber, WC WeChat, KK Kik, MS Messenger/Instagram, IM iMessage, SMS SMS/MMS/RCS, DC Discord, SL
Slack, GC Google Chat.

## 1. Who and where

| Kind | Fields | Seen in |
|---|---|---|
| account | app, own id, phone, username, display name, picture | all |
| person | app id, phone, username, email, names (app's, address book's, own "~name"), picture, about/status text, verified/business/bot flags, deleted account | all |
| contact (linked) | the people across apps the owner links into one, with vCard data | oopz |
| business profile | category, address, hours, website, catalogue | WA, TG, VB, LN, WC |
| bot | name, commands, inline mode | TG, DC, SL, LN, WC, MS |
| conversation: one-to-one, group, supergroup, community, sub-group/topic/thread, channel/newsletter, broadcast list, status/stories, own notes ("message yourself", Saved Messages, LINE Keep, Viber My Notes), secret/hidden/locked/archived chat, server/workspace + channel | id, kind, name, picture, description, created, members (role: owner, admin, member, left, removed, banned, pending), settings (disappearing timer, who may send/edit, slow mode, join approval, history visibility), pinned, muted, labels/folders | per app |

## 2. Message envelope (every message)

id (and the app's own key), conversation, sender (or "me"), sent / server / received / read / played times, edit
history (each version and time), status (pending, sent, delivered, read, played, failed), deleted for everyone /
for me (placeholder kept), view-once (opened or not), disappearing timer, forwarded (times, from where/channel), reply
to (message, quoted excerpt, quoted media), thread/topic, mentions (people, @all, roles), starred/saved/favourite,
pinned (until when), scheduled, silent, sent from (device: phone, web, desktop, companion), via bot, sponsored/ad,
translated text, original language, labels (business), read receipts per person, reactions (who, emoji or custom
emoji or sticker, when, removed), and the app's raw fields the adapter couldn't place.

## 3. Content parts (a message has one or more)

| Family | Kinds |
|---|---|
| Text | plain; formatted (bold, italic, underline, strike, spoiler, mono, code block with language, quote, lists, headings, links with text, colour/size where the app has them); emoji-only (big emoji); custom/premium emoji; animated emoji (TG dice-style and LN animated); mention; hashtag; cashtag; command (/start); phone number, email, address, date detected as links |
| Links | link with preview (url, title, description, site, image, video); rich embed (DC, SL); deep link to an app object |
| Photos | photo (JPEG, HEIC, PNG, WebP, AVIF), HD/original quality, Live Photo (photo + short video, IM), panorama, screenshot, view-once photo, self-destructing/timer photo (TG secret chats), photo with drawing/text edits, spoiler/blurred media, paid media (TG) |
| Groups of media | album/grouped media (photos and videos), media group with captions per item |
| Video | video (MP4, MOV, HEVC), HD, video note / round video (WA, TG), view-once video, streaming video, screen recording, live stream or voice chat recording (TG) |
| Animated | GIF, GIF as MP4, animated sticker (Lottie `.tgs`, WebM, APNG LINE, animated WebP), video sticker |
| Stickers | sticker (static WebP/PNG, animated, video), sticker from a photo (IM), avatar/Memoji/Bitmoji sticker, premium sticker (TG), sticker pack (id, name, publisher, stickers, tray icon), custom emoji pack, LINE sticker/emoji packs, WeChat custom emoticon |
| Sound | voice note (codec, duration, waveform, transcription), audio file/music (title, artist), round voice (TG), audio message with "raise to listen" (IM), voicemail (SMS/carrier), call recording |
| Files | document (any type: name, mime, size, pages, thumbnail), APK/executable (flagged), archive, code file, very large file (TG up to 4 GB) |
| Contacts | contact card (one or several vCards), contact request/friend request (LN, WC, KK), share of a profile |
| Places | location (lat/lng, accuracy, place name, address, map thumbnail), venue, live location (duration, path, last position, stopped), location request, check-in (IM) |
| Time | calendar event (title, time, place, attendees, responses: WA events, LN, MS), reminder, scheduled call |
| Polls and games | poll (question, options, single/multiple, anonymous, closes at, votes per person and their changes), quiz (correct answer, explanation), checklist/to-do (TG), dice/darts/slot (TG), game (TG/MS/IM apps, score), mini-program/mini-app card (WC, TG web apps, LN) |
| Money | payment sent/received/requested (WA Pay, UPI, Pix, Apple Pay Cash, Signal MobileCoin, LINE Pay, WeChat Pay transfer), red envelope (WC, LN), invoice, receipt, refund, gift (TG Stars/Premium, LN gifts), giveaway |
| Business | product, catalogue, cart/order, order status, quick reply, template message, buttons, list message, interactive reply (button or list chosen), flows/forms, appointment |
| Interactive | inline keyboard/buttons and the choice made (TG/DC/SL), slash-command result, form/modal response (SL), Digital Touch, handwriting (IM), message effects (bubble/screen effects, IM; message effects, TG) |
| Calls | voice, video, group voice, group video, call link, scheduled call, voice chat/stage (TG, DC); outcome (answered, missed, declined, busy, cancelled, failed), duration, participants, call log entries with no message |
| Stories | status/story (photo, video, text with background/font, link, music, mention, location, poll sticker), its views, replies and reactions, highlights, close-friends audience, expired |
| Posts | channel/newsletter post (views, forwards, comments thread), WeChat Moments, LINE Timeline/VOOM post, comments and likes |
| Collaborative | note/memo in a group (LN notes, WC favourites), shared album (LN, IM), group file list, pinned items, shared drive link |
| Generated | message from an AI assistant (Meta AI, bots), summary, translation, transcription |

## 4. System events

encryption notice, security code / safety number changed, number changed, device added (multi-device), account
deleted; group: created, renamed, description, picture, settings changed, member added/joined (by link, by
invite, by request approved), left, removed, banned, made/removed admin, ownership transferred, community linked,
upgraded to supergroup/community; disappearing messages on/off/timer, keep-in-chat, chat locked, screenshot taken
(TG secret, SG), message pinned/unpinned, call started/ended in group, invite link reset, join request, poll closed,
payment status change, blocked/unblocked, contact joined the app, chat history cleared, messages deleted by admin,
"this message was deleted", missed messages while offline, unknown event (code kept).

## 5. Media facts (every media part)

file name, mime type, size, width/height, rotation, duration, frame rate, codec, bitrate, hash (the app's and
oopz's SHA-256), thumbnail/blur preview, caption, waveform, transcription, whether the file is present, its path in
the source, its original/HD variant, downloaded or not, expired on the server, encrypted media key (never exported),
taken-at time and place when the app kept them.

## 6. Odd but real

a part with no file; a file with no part; two parts sharing one file; text in a media message (caption) vs a
separate text; a reply to a message not in the backup; a reaction to a missing message; times of 0, missing, in
seconds instead of milliseconds, before 1970, in the future; out-of-order ids; duplicate ids (same id, both sides);
a sender not in the members; a deleted person; mixed scripts and right-to-left; zero-width and control characters;
markup that must stay text; 64 KB+ text; a format version oopz hasn't seen (refused by name).
