# oopz for every messaging app: the plan

Status: proposed (2026-11-10). Each phase below gets its own decision record before it is built.

## Purpose

A person's own chat history, from any messaging app, on their own devices: open it, search it, select from it, merge
it, export it, put it back. **Offline-first** (nothing leaves the device), **fast** (a million messages in seconds),
**light** (old phones: Android 7+, WebView 67+, measured on a 2018 Galaxy S9+ on Android 8), **secure** (keys used once
and wiped, originals only read, results checked before they're written), and **free** (AGPL), doing at least what the
paid tools do (Dr.Fone WhatsApp Transfer, iMazing, Backuptrans, WazzapMigrator and the like).

How the paid tools reach these apps (iPhone local backups; Android APK downgrades, derived keys, cloud logins) and
which of those oopz uses: [how-paid-tools-work.md](how-paid-tools-work.md).

## The rule every adapter follows

oopz opens only what an app gives its owner: an **official export** (a chat exported to text, a data download), or a
**backup opened with the owner's own key or password**. It never cracks, guesses or derives another way in, never
roots a phone for the person, never imitates an app's servers to fetch a key (the same line as decision 0007's
"never gets around encryption"). Where an app gives its owner no way out, oopz says so plainly instead of pretending.

## What each app lets its owner take out (to verify on real files, one adapter at a time)

| App | What the owner can get, without root | oopz | Notes |
|---|---|---|---|
| WhatsApp Android | `msgstore.db.crypt15` + 64-digit key | **done** (read, merge) | crypt14/12 need a key file only a rooted phone has: refused (0007) |
| WhatsApp Android | "Export chat" (`.txt` + media, per chat) | phase 2 | no key needed; text and media, no reactions or ids |
| WhatsApp iPhone | Finder/iTunes backup (unencrypted, or with the backup password) → `ChatStorage.sqlite` | phase 3 | backup password is the owner's; the backup format is documented |
| Telegram | Desktop "Export chat history" (JSON or HTML) | phase 2 | the richest official export: ids, replies, reactions, media |
| Signal Android | `.backup` file + its 30-digit passphrase | phase 3 | the format is open (protobuf frames, AES-CTR + HMAC) |
| LINE | "Export chat history" (`.txt` per chat) | phase 2 | the full backup is encrypted by LINE's servers: not openable |
| Viber | "Export message history" (CSV in a zip, by email) on the phone; Viber Desktop's local database | phase 2 / 3 | desktop database is plain SQLite on the owner's computer |
| Messenger, Instagram | Meta "Download your information" (JSON or HTML) | phase 3 | |
| SMS / MMS, call log | "SMS Backup & Restore" XML; Android's own export where present | phase 2 | the most common old-phone history |
| iMessage / SMS on iPhone | the same iPhone backup's `sms.db`; on a Mac `~/Library/Messages/chat.db` | phase 3 | |
| Google Chat, Discord, Slack | their data downloads (JSON) | later | |
| WeChat | on iPhone: the device's local backup (owner's backup password) holds its files | phase 3 (iPhone) | if a version encrypts its database with an app-held key, refused by name; on Android: root or derived keys only, **not supported** |
| Kik | on iPhone: the local backup holds `kik.sqlite` | phase 3 (iPhone) | on Android: root or an old-APK downgrade only, **not supported** |
| LINE, Viber (full history) | on iPhone: the local backup | phase 3 (iPhone) | |

Writing back into an app is possible only where the app restores from a file the owner controls: **WhatsApp Android
(done)**, and later WhatsApp iPhone through a computer backup. For every other app, "import" means into oopz's own
library and its exports.

## One model for every app (`crates/oopz-model`)

Adapters turn each app's data into one model; everything after that (search, select, merge, export) works on the
model only, so a new app is one new adapter and nothing else.

- **Account**: an app and the owner's id in it.
- **Person**: someone in a conversation, per app (`app`, the app's id, phone number when the app has one, name as the
  app or the address book gives it). People in different apps are **linked** into one **Contact** by the owner
  (suggested by a shared phone number or email, never silently).
- **Conversation**: one-to-one, group, broadcast/channel, status/story, own notes; its people, name, picture, and
  events (created, renamed, members added, left, removed, admins, description, settings such as disappearing
  messages).
- **Message**: the app's id, conversation, sender, times (sent, received, read, edited, deleted), status, and one or
  more **parts** (below); plus reply-to, forwarded (and how often), starred/pinned, edited history, deleted for
  everyone / for me, view-once, disappearing, reactions (who, which, when, taken back), poll votes, read receipts per
  person, mentions, and what the adapter couldn't place (kept, never dropped: an `unknown` part with the app's own
  type code).

### Every kind of data

The full catalogue (people, conversations, the message envelope, every content part, system events, media facts and
the odd-but-real cases) is [data-types.md](data-types.md). Each adapter maps its app's types to it, and an app's type
with no match is kept as `unknown` with its code, shown as "#code" and counted, so nothing silently disappears. A
table per adapter in `docs/adapters/<app>.md` records the mapping and which app versions were checked.

## What people can do with it

1. **Open** any supported source (file, folder, export zip; from the phone over Wi-Fi or USB).
2. **Select**: by app, account, contact (linked across apps), conversation, date range, sender, and kind (photos,
   stickers, sticker packs, voice notes, documents, links, text only, …), and by search.
3. **Export** a selection: HTML (one file, works offline, printable), **PDF** (customisable: cover, which parts,
   media inline or as thumbnails, page size, language, time zone, one conversation per chapter), JSON (the model, for
   other tools), CSV (tables), TXT (plain), media folder or zip, vCard (contacts).
4. **Merge for reading**: the same two people's conversations from several apps (WhatsApp + Telegram + SMS…) in one
   timeline, each message marked with its app, exported as one (or several) documents.
5. **Merge back into an app** (where the app allows, today WhatsApp Android): a chat from an old phone or another
   person's backup into a newer database (done for one-to-one, decision 0006); next, groups (needs its decision),
   selected messages only, and an older own backup (0008).
6. **Contacts**: import and export selected contacts (vCard), linked across apps.
7. **Compare**: what's missing between two backups or two apps.

## The MVP (everything in it ships together, each feature in the CLI and the app)

1. **Library**: open WhatsApp Android (crypt15), WhatsApp "Export chat" zips, Telegram JSON exports, SMS Backup &
   Restore XML, LINE and Viber text/CSV exports; a list of sources with health, counts and dates; remove a source and
   its private copies.
2. **Read**: conversations list (all apps together or per app), a fast viewer of every part kind (with media from
   the source folder, view-once never shown), jump to date, message details (times, receipts, edits, reactions).
3. **Search**: across everything or one conversation; filters by app, person, kind, date; accents and case ignored;
   results in milliseconds on a million messages.
4. **Insights**: per conversation and overall: by month, hour, weekday, kind, person, media size, voice minutes.
5. **Contacts**: people per app, linking them into one contact (suggested by number), export/import vCard of a
   selection.
6. **Select**: any mix of contacts, conversations, dates, kinds (photos, stickers, sticker packs, voice notes,
   documents, links, text…) and search results, saved as a named selection.
7. **Export** a selection: HTML (offline, printable), PDF (cover, contents, one chapter per conversation or one
   merged timeline, media inline/thumbnails/none, page size, fonts with full emoji and right-to-left, time zone,
   language), JSON, CSV, TXT, media zip, vCard. Progress, cancel, free-space check, never overwriting.
8. **Merge for reading**: the same people's conversations from several apps in one timeline, marked by app.
9. **Compare and restore (WhatsApp Android)**: what's missing between two backups; restore a one-to-one chat into a
   newer database (done); selected messages only; then groups (own decision).
10. **Receive**: from the phone over Wi-Fi (done) and USB (adb, Android, read-only pull of what the owner picks).
11. **Settings and trust**: language (EN, ES first), theme, updates (opt-in), wipe now, about, diagnostics report
    (structure only), the privacy rules on screen.

Every feature: the shared code in a crate, then the CLI, then the app (docs/parity.md), with tests on `oopz-gen` data
(every kind, every odd row, a million messages) and the screens checked on desktop and on the S9+.

## Modules (faster builds, clean boundaries)

```text
crates/
  oopz-model        the shared model and the kinds of part; no I/O
  oopz-core         library: open, index, search, select, stats, over the model (today's oopz-app logic, moved)
  oopz-export       HTML, JSON, CSV, TXT, vCard; oopz-export-pdf behind a feature (a PDF writer is a big dependency)
  adapters/
    whatsapp-android   the `oopz` crate (crypt15, schema, merge; moved here, decision 0014)
    whatsapp-export    WhatsApp "Export chat" text
    whatsapp-ios       iPhone backup → ChatStorage.sqlite
    telegram-export, signal-android, line-export, viber, sms-xml, meta-export, imessage
  oopz-gen          synthetic data, one generator per adapter (every kind, every odd row, any size)
  oopz-cli          the CLI: one subcommand group per feature
apps/desktop        Tauri shell (commands only) + Svelte UI
  src/lib/backend   the only boundary to Rust (interface, Tauri implementation, mock)
  src/features/     one folder per feature: library, unlock, chats, search, insights, select, export, compare,
                    restore, receive, settings; a feature imports the backend and shared UI, never another feature
```

Each adapter is its own crate implementing one trait (`detect`, `open` with an optional key, `conversations`,
`messages` as a stream), so it builds and tests alone, and an app can ship with only the adapters it needs. The CLI
and the app stay in step (docs/parity.md) for every new feature.

## Platforms

- Desktop: Windows 10+, macOS 11+, Linux (glibc 2.35+, AppImage elsewhere).
- Android: API 24 (7.0) and up, the Tauri floor; the UI is built for Chrome 67 WebViews (old phones that never update
  their WebView); tested on a Galaxy S9+ (Android 8.0, WebView 67). Files picked on Android arrive as content
  references, not paths: oopz copies the picked backup into its private storage (checked for space first), opens it
  there, and removes the copy on close, the same as desktop's decrypted copies (decision 0012).
- iOS: later, after a real device test; the backup sources there are limited (no access to other apps' files).

## Edge cases every adapter is tested against

From `oopz-gen` for each app: empty, huge (1M+ messages, tens of GB of media), every kind, unknown kinds, missing
media, renamed/moved media, duplicate ids, times missing, out of order, in another time zone, before 1970 or in the
future, right-to-left and combining text, very long text, broken references, deleted people, number changes, merged
contacts, old and new versions of the app's format side by side, a format version never seen (refused by name), a
half-copied file, the wrong key or password, a password where a key goes.

## Phases

1. **Now**: new UI (Svelte, feature modules, mock backend), Android build on old phones, WhatsApp Android complete.
2. Model + exports: `oopz-model`, selection by kind/contact/date, HTML/JSON/CSV/TXT/vCard export, PDF behind a
   feature; adapters for the official text exports (WhatsApp, LINE, Viber CSV, SMS XML, Telegram JSON).
3. Backups with the owner's key or password: WhatsApp iPhone, Signal Android, Viber Desktop, iMessage; cross-app
   timelines.
4. Writing back: WhatsApp group merges, selected messages, older own backups (0008); WhatsApp Android ↔ iPhone (each
   its own decision: risky, must be checked like 0006).
