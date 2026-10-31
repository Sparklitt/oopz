# oopz desktop: the plan

Status: first version built (2026-10-31), local only. Decision: [0009](decisions/0009-a-desktop-app-with-tauri.md).

oopz today is a command line. Most people who lose a chat will never open a terminal. The desktop app walks them
through the same work, step by step, in plain words, in English and Spanish: get the backup off the phone, type the
key, see what is inside, search it, merge a lost chat, put the result on the other phone. The rules don't change:

- **Your own backups, your own key, with consent.** The app opens a backup only with the 64-digit key its owner
  chose. It never tries to get around WhatsApp's encryption of anyone's account, and never talks to WhatsApp's
  servers.
- **Originals are never modified.** Every result is a new folder; originals are hashed (SHA-256) when checked and
  again after a merge, and the screen says so.
- **Offline.** No account, no cloud, no telemetry. The one network feature, the Wi-Fi receive page, listens on the
  local network only, only while the person has it open, and stops by itself.
- **No message text, numbers or keys in logs, errors or tests.** The app does show the person's own messages on
  their own screen (search, chat viewer); that text never goes to a log or a file outside the app's private cache,
  except a web page the person asks to save (section 8.3).
- **No home-made cryptography.** The app adds no cryptographic primitive; decryption stays in `crates/oopz`
  ([0004](decisions/0004-whatsapp-backup-decryption.md)).

## 1. Architecture

```
crates/oopz          WhatsApp's format: crypt15, schema, Backup, ChatMerge, fixtures      (exists)
crates/oopz-app      the app's logic without a UI toolkit: find/check/merge, the Wi-Fi     (new, in the workspace)
                     receive page, the search index and stats; plain Rust, tested
apps/desktop/        Tauri 2 shell (src-tauri/: commands wrap oopz-app one to one)        (new, own Cargo workspace)
                     + a light web UI (src/: TypeScript, no framework, Vite)
apps/android/        later: Tauri 2 mobile, the same oopz-app                              (planned)
```

- `oopz-app` is where every decision lives, so the desktop shell stays thin and an Android build wraps the same
  calls. Its tests run in the workspace's normal checks.
- `apps/desktop/src-tauri` is its **own Cargo workspace** (`exclude`d from the root): Tauri needs a webview toolkit
  (webkit2gtk on Linux) to compile, which the library's CI and contributors who only touch the library shouldn't
  need. It is checked on its own (section 11).
- The UI never touches files or keys itself beyond handing a path or a key to one command. The key input is a
  password field, cleared the moment it is sent; Rust receives it as a `Zeroizing<String>`, turns it into an
  `oopz::Key` (wiped on drop) and drops both before the command returns. Nothing is stored: oopz asks again when it
  needs a key again (the merge).
- Decrypted copies (and the search index) live in a fresh folder in the app's cache directory, removed when the
  backup is closed, when the app exits, and at the next start if the app crashed.

## 2. Why Tauri 2

| | **Tauri 2** (chosen) | Compose Multiplatform | Slint | egui |
|---|---|---|---|---|
| Core in Rust, called directly | yes, same process | through JNI/FFI (uniffi), a JVM in the app | yes | yes |
| Installer size | ~3-10 MB (system webview) | 60-100 MB+ (bundled JVM runtime) | ~5-10 MB | ~5-10 MB |
| Windows, macOS, Linux | yes (WebView2, WKWebView, webkit2gtk) | yes | yes | yes |
| Android with the same core | yes (Tauri 2 mobile) | yes (best of the four) | yes, younger | possible, not its focus |
| Accessibility (screen readers) | the platform's web accessibility | good | partial | weak (immediate mode) |
| Text layout, Spanish, RTL, emoji, fonts | the browser engine: excellent | good | good | basic |
| Long virtualized lists, charts | plain HTML/CSS, well-known techniques | good | possible, more work | possible, everything by hand |
| Who can contribute | anyone who knows HTML/TS | Kotlin developers | Slint's markup | Rust only |
| Licence fit with AGPL | MIT/Apache | Apache | GPL/commercial/royalty-free (fine, but one more licence to watch) | MIT/Apache |

Tauri wins on the three things that matter most for this app: the core stays plain Rust in the same process (no
JVM, no FFI layer to keep in sync), installers stay small enough to download on a slow connection, and the UI is
ordinary HTML, which gives accessible, well-typeset Spanish and English text, and lets non-Rust contributors help.
Compose Multiplatform would be the pick if the app were mostly Android; it isn't, and a JVM runtime per desktop
install is heavy. Slint and egui are lighter still, but text, accessibility and long lists are more work for less.

**No heavy web framework.** The UI is about 1,250 lines of plain TypeScript (a third of it the English and Spanish words) with a 15-line DOM helper; the built page
is ~37 kB of JavaScript. A framework (Svelte, Solid) would be justified if screens multiply or forms get complex;
switching is cheap because each screen is one function of the state.

**No code from the private hilo app** is shared or copied. The visual style is shared on purpose: oopz's palette
(warm cream `#FFF7EE`, indigo ink `#23244F`, coral `#F2604F`, amber `#F7A928`, and their dark-mode pairs) from
`assets/make_assets.py`, as CSS custom properties in `apps/desktop/src/styles.css`.

## 3. The screens

Every screen says what it does and why in one or two plain sentences; every word is in `src/i18n.ts` in English and
Spanish (the app follows the system language; a button switches it).

1. **Welcome.** The four rules (own backups and key; never around WhatsApp's encryption; originals never changed;
   nothing leaves the computer) and a consent tick box: "These are my own backups, or their owner agreed and will
   type their own key." Nothing continues without it.
2. **Get backup.** Three ways (section 4): a folder or file already on the computer (USB, SD card, stick, Quick
   Share, cloud folder), or Wi-Fi with a QR code. oopz lists what it finds in the folder (or in its `Databases/`
   subfolder): the current backup first, then dated ones newest first, each with its size and whether it can be
   opened (crypt15), is an older unsupported format (crypt14/crypt12), or is named like a backup but doesn't read
   like one (an incomplete copy).
3. **Key.** A masked field with a character counter ("37/64"), the promise that the key is used once and never saved,
   and a "Did you choose a password instead?" panel (section 5).
4. **Check.** Whether it opened and its database is sound; messages, chats (one-to-one/groups), first and last
   date, size; the chat list with masked ids (kind + last 3 characters); the original's SHA-256. "Check another
   backup" adds a second one for a merge.
5. **Explore.** Stats, search and a chat viewer (section 8).
6. **Merge.** Pick the backup that still has the chat and the chat; the backup that lost it and the chat; type that
   backup's key again; pick where the result goes; optionally the Media folder. The result: added / already there,
   media copied / missing, the checks it passed, "original backups unchanged (fingerprint checked again)".
7. **Put on phone 2.** A checklist the person ticks, step by step (section 6).
8. **Done.** Where the result is; keep everything until you're sure; start again (closes and wipes the decrypted
   copies).

Opening a backup and building its search index show a progress bar (thousandths sent over a Tauri channel, only
when they change). "What's missing" (built): on the Merge screen, "Compare" counts the messages
in both chats, only in the source (what the merge adds) and only in the target, with their dates, matched by
`key_id`. Planned on top: the "restore from an older own backup" mode once [0008](decisions/0008-restore-a-chat-from-an-older-own-backup.md) is built.

## 4. Getting a backup off phone 1

WhatsApp keeps local backups on the phone in **`Android/media/com.whatsapp/WhatsApp/Databases/`**:
`msgstore.db.crypt15` (the newest) and `msgstore-YYYY-MM-DD.1.db.crypt15` (older days, about seven kept). Photos,
videos, voice notes and documents are in **`Android/media/com.whatsapp/WhatsApp/Media/`**. For **WhatsApp Business**
the folder is `Android/media/com.whatsapp.w4b/WhatsApp Business/` (`Databases/`, `Media/`). Old phones (Android 10
and before, old WhatsApp versions) may still have `WhatsApp/` at the root of the storage. The app tells the person to
copy the whole `Databases` folder (it's small) and, only if they need media, `Media` (it can be tens of GB).

**Make a fresh backup first** if the phone still works: WhatsApp › Settings › Chats › Chat backup › Back up. That
writes a new `msgstore.db.crypt15` (locally, and to Google Drive if that is on).

### 4.1 USB cable (MTP)

Plug the phone in, unlock it, choose "File transfer" in the USB notification, and open the phone in the file
manager (Windows Explorer; Android File Transfer or OpenMTP on macOS; GNOME Files/Dolphin on Linux). Copy
`Databases` (and `Media` if needed) to the computer, then pick that folder in oopz.

- **Dual apps.** Xiaomi "Dual apps", Samsung "Dual Messenger" and similar run a second WhatsApp under another Android
  user (often user 999). Its folder lives in that user's storage (`/storage/emulated/999/...`) and usually **doesn't
  show over MTP** or in the phone's normal file picker. oopz explains this, suggests the phone's own file manager
  (some show the dual app's folder and can copy it to the main storage or an SD card) and offers nothing that
  needs root.
- **Windows MTP timeouts on big folders.** Explorer often stalls or fails on a `Media` folder with tens of thousands
  of files. The app suggests: copy `Databases` first (seconds); copy `Media` one subfolder at a time; or zip `Media`
  on the phone with its file manager and copy the zip; or use a USB-C stick or the Wi-Fi page for the zip. A copy
  that stopped half way shows up in oopz as "not a crypt15" or as a wrong key/damaged file: compare the size with the
  phone's.

### 4.2 Wi-Fi with a QR code (built)

For people without a cable. The app opens a short-lived page on the computer; the phone opens it by scanning a QR
code, and sends the files from its browser. Nothing goes through the internet.

How it works (`crates/oopz-app/src/receive.rs`):

1. The person picks where received files go; oopz creates a **new** folder `oopz-received-N` in it (never reuses
   one).
2. It finds the computer's LAN address (the route the system would use to a private address; nothing is sent),
   binds a server **to that address only** on a random free port, and makes a **128-bit random token** (from the OS,
   `getrandom`). The address is `http://192.168.x.y:port/<token>/`; the QR code encodes exactly that.
3. The phone scans it and opens the page (English or Spanish by the phone's language; no outside resources): "Choose
   files" opens the phone's own picker (Databases folder, or a zip of Media), "Send" uploads them one at a time with
   a progress bar, "I'm done" ends it.
4. The server answers **only** requests whose path starts with the token (compared in constant time) **and** that come
   from a private, link-local or loopback address. Anything else gets a 404 and is counted; after 50 refused requests
   the page stops.
5. Each file is streamed to `.name.part` with its SHA-256 computed on the way, then renamed. Names are checked: no
   folders, no `..`, not hidden, no control or reserved characters, at most 200 bytes, never over a file already
   received. A transfer that is cut off (fewer bytes than announced) is deleted, never kept half.
6. **Checks.** Phone browsers can't hash a file on a plain-HTTP page (Web Crypto needs a secure context), so: the
   page checks that the computer received exactly the file's size; the computer records each file's SHA-256 (shown
   on the computer); and opening a crypt15 backup verifies its AES-GCM authentication tag, which fails on any
   changed byte.
7. It **stops** when the phone taps "I'm done", when the person clicks "Stop receiving", after **10 minutes** without
   a request, or when the app closes. The token dies with it; a new page has a new token, port and folder.

Why plain HTTP and not HTTPS: a phone's browser would show a full-screen certificate warning for a self-signed
certificate, which teaches people to click through warnings, and a browser can't pin a certificate from a QR code.
What this means: a crypt15 backup is already encrypted with its owner's key, so someone listening on the same Wi-Fi
learns nothing from it; **media files are not encrypted**, and the token is visible to a listener on the same
network. The app says so on the screen: use your own home Wi-Fi, and for Media prefer the cable. Later options: the
Android app sending to the desktop over a channel authenticated by the QR code (a key in the QR, a vetted library,
with its own decision record); or HTTPS with a per-session certificate for people who accept the warning.

Practical snags the app explains: Windows Defender Firewall asks the first time ("Private networks": allow); macOS
asks to accept incoming connections; guest and public Wi-Fi networks often isolate devices from each other (use the
cable, or connect the computer to the phone's own hotspot, which works because the phone is then on the same
network); a VPN on the computer or phone can hide the LAN.

### 4.3 Quick Share / Nearby Share

Android's Quick Share sends files to a Windows PC with Google's Quick Share app (and to Chromebooks; to some Linux
desktops with community apps). In the phone's file manager, select the files in `Databases` (and a zip of `Media`) ›
Share › Quick Share › the computer. They land in Downloads; pick that folder in oopz.

### 4.4 SD card or USB-C stick

Copy `Databases` (and `Media`) with the phone's file manager to the SD card or a USB-C stick, then plug it into the
computer and pick the folder in oopz. Good for big Media folders and for Windows MTP trouble. FAT32-formatted sticks
cap files at 4 GB: zips of Media may need exFAT.

### 4.5 A cloud folder the person already uses

Uploading to their own Google Drive (as files, not WhatsApp's backup), OneDrive, Dropbox and downloading on the
computer works. The app warns plainly: **the files leave your devices** and sit in that company's cloud until you
delete them there; the backup is encrypted, but Media files are not. It's the person's choice.

### 4.6 Google Drive backups: not readable, not attempted

WhatsApp's own Google Drive backup is stored in an app-private area of the person's Drive that only WhatsApp can
read, and with end-to-end encryption it's encrypted with the same key. Getting it would mean talking to Google's and
WhatsApp's servers as if oopz were WhatsApp. oopz doesn't do that and never will. The app says: use the local backup
on the phone; if the phone is gone, restore Drive's backup onto a phone with WhatsApp (WhatsApp's own way), then
make a local backup there and copy it.

## 5. Keys

- **The 64-digit key** (end-to-end encrypted backup "with a 64-digit key"): 64 characters 0-9 and a-f, which WhatsApp
  showed once. With it, a crypt15 backup opens **offline**: this is all oopz supports. Spaces and line breaks are
  ignored; anything else is "the key must be 64 characters: digits 0-9 and letters a-f". A key that doesn't open the
  file: "This key doesn't open this backup… or the file is incomplete" (AES-GCM can't tell the two apart, so the
  app suggests both checks).
- **Lost key:** nothing anyone can do; oopz will never try to guess or recover it.
- **crypt14, crypt12 and older:** encrypted with a key file kept in WhatsApp's private app data
  (`/data/data/com.whatsapp/files/key`), readable only on a rooted phone. Not supported, and the file list says so
  with the reason. oopz doesn't ask anyone to root their phone.

### 5.1 Backups protected with a password (built: the guided explanation)

WhatsApp also offers end-to-end encrypted backups protected with a **password** instead of the 64-digit key. Then the
real 64-digit key is created by WhatsApp and stored in WhatsApp's **server-side key vault** (an HSM-based vault),
released only to the WhatsApp app after it proves the password. The backup file on the phone looks the same as one
made with a 64-digit key, as far as we know: oopz **cannot tell from the file** which protection was chosen, so it
asks ("Did you choose a password instead of a 64-digit key?"), and a password-protected backup simply fails like a
wrong key.

oopz **will not** fetch the key from the vault by talking to WhatsApp's servers as if it were the app: that would be
getting around WhatsApp's systems and could get the person's account flagged or banned. Instead the app walks them
through switching their backup to a 64-digit key inside WhatsApp:

1. On the phone: WhatsApp › Settings › Chats › Chat backup › End-to-end encrypted backup.
2. Choose to use a 64-digit encryption key instead; WhatsApp asks for the current password to confirm.
3. Write the 64 digits down on paper; keep them safe. Nobody can recover them.
4. Back in Chat backup, tap **Back up** to make a fresh backup with the new key (local, and Drive if on).
5. Copy the new `msgstore.db.crypt15` to the computer and continue in oopz, offline.

Edge cases, all on the screen:

- **Forgot the password:** then nobody can open those backups, not oopz, not WhatsApp support.
- **The old backups stay unreadable:** dated backups made under the password (`msgstore-YYYY-MM-DD.1.db.crypt15`) are
  not re-encrypted; only backups made after the switch use the 64-digit key. If the chat was lost before the switch
  and only an old backup has it, oopz can't help with that backup.
- **Make the new backup before uninstalling, resetting or changing phones:** the switch needs the working WhatsApp
  with the account on it.
- **The phone is gone:** restore WhatsApp's Drive backup on a new phone with the password (WhatsApp's own way), switch
  to a 64-digit key there, back up, copy.

## 6. Putting the result on phone 2

The merge writes `oopz-result-N/Databases/msgstore.db.crypt15` (and `Media/` if asked), encrypted with the target's
own key, laid out like WhatsApp's folder. The guided screen (a checklist the person ticks) says:

1. **Mind the nightly backup.** Every night around **02:00** WhatsApp makes a local backup by itself; there is no
   switch to turn it off. It replaces `msgstore.db.crypt15` and deletes dated backups beyond about seven
   (`msgstore-YYYY-MM-DD.1.db.crypt15`). So: copy `Databases` off phone 1 *now*; set "Back up to Google" to Never on
   both phones while working (so Drive isn't overwritten either); finish the same day, or keep phone 1 off until
   done if it holds the only copy.
2. **Keep a copy of everything**: phone 2's whole `WhatsApp` folder, before anything else. That's the rollback.
3. **Same phone number**: a backup restores only into the account (number) that made it.
4. **Don't open WhatsApp yet.** On a fresh phone: install WhatsApp from the Play Store but don't open it. On a phone
   that already has WhatsApp: make sure its own chats are in the target backup (they are, if the target is that
   phone's latest backup), move its `WhatsApp` folder aside, then clear WhatsApp's storage (Android Settings › Apps ›
   WhatsApp › Storage › Clear data) or uninstall and reinstall.
5. **Copy the result** into `Android/media/com.whatsapp/WhatsApp/Databases/msgstore.db.crypt15` (create the folders
   if WhatsApp hasn't yet) and `Media/` into `…/WhatsApp/Media/`. For Business: `com.whatsapp.w4b/WhatsApp
   Business/`. The file must be named exactly `msgstore.db.crypt15`.
6. **Open WhatsApp and restore.** Verify the number. If a Google Drive backup exists for the number, WhatsApp offers
   it first, and it isn't the merged one. What works most reliably: before opening WhatsApp, **remove the Google
   account from phone 2 for now** (Settings › Accounts), so WhatsApp finds only the local backup; tap **Restore**,
   type the 64-digit key; add the account back afterwards. (Screens differ between WhatsApp versions; the app words
   this as "if you see…".)
7. **Check**: open the chat, scroll to the old messages, open a photo. Then make a new backup on phone 2.

## 7. Edge cases, and what oopz does

| Case | What oopz does |
|---|---|
| Wrong key | "This key doesn't open this backup… or the file is incomplete"; nothing else is tried; the key is never shown back. Tested (`check_names_a_wrong_key_a_bad_key_and_damage`). |
| Truncated or corrupt backup | AES-GCM fails: the same message plus "copy it again and compare its size with the phone's". A file that opens but whose database fails SQLite's quick check is shown as opened-but-damaged with SQLite's (structural) message; the app suggests an older dated backup. Tested with a half file. |
| Very large media folders, low disk | Media is copied file by file, streamed, never loaded whole; only the files the merge needs are copied. A write error stops with "disk full, cable unplugged, no permission?". The Wi-Fi page answers 507 when the disk is full and deletes the partial file. Free space is checked first: a merge needs its exact size (the result and the media it copies), a check about four times the backup in the app's cache (decrypted copy and index, an estimate); short of it, nothing is written (`no_space`). The Wi-Fi screen shows the space free where files go. |
| Windows MTP timeouts | Section 4.1: copy Databases first, Media by subfolder or as a zip, or use a stick or the Wi-Fi page. Incomplete copies are flagged in the file list. |
| Schema/version differences | oopz detects the schema and reads optional columns only if present (decision 0006); the index copies only columns that exist (missing ones become empty); the merge copies by column name and refuses, writing nothing, if a check fails. Unknown types show as "#code" in stats. `oopz tables` helps report a new version (structure only). |
| Duplicate messages across two backups | Matched by WhatsApp's message id (`key_id`), never by time or text: "already there" are skipped, only missing ones are added. |
| Clock and time-zone differences | WhatsApp stores UTC milliseconds; matching is by id, so a wrong phone clock doesn't create duplicates. Added messages are placed before the target chat's own (0006), and the report counts those newer than the target's first. Dates on screen and stats buckets use the computer's time zone (stats: as of when the index was built). |
| Number changes and LID ids | A chat can be filed under a phone jid in one backup and a private `lid` in the other; the person picks both chats; the merge's owner check (every shared message must have mirrored `from_me`) stops a wrong pairing. A changed number is a different chat id: same procedure, the person picks the old chat. Ids are always masked (kind + last 3 characters). |
| Deleted for everyone | The "this message was deleted" row (type 15, `message_revoked`) is copied as it is; the text was never there to recover. |
| View once | Copied as the row WhatsApp has (`message_view_once_media`); the media is usually gone, counted as missing. |
| Disappearing messages | Only what's still in the backup exists; tables the merge doesn't know are counted as "left behind" in the report, never silently dropped. |
| Polls | Shown in stats; `message_poll` rows aren't in the merge's table list yet, so they're reported "left behind" (the poll message itself is added). |
| Reactions | Copied with their parent (`message_add_on`), including the other person's (0006). |
| Edits | `message_edit_info` copied: the edited text as WhatsApp last stored it. |
| Groups and communities | Check, stats, search and viewer work for groups (senders masked). The merge refuses groups ("only one-to-one chats can be merged"); group merging is future work with its own decision. Communities are groups plus an announcement group: same. |
| WhatsApp vs WhatsApp Business | Same backup format, its own folder (`com.whatsapp.w4b`) and its own key. Merge only between the same kind of account's chats; restore into the Business folder. |
| Dual apps | Section 4.1: the second copy often isn't reachable over MTP; no root tricks. |
| iPhone backups | Not supported now: iPhone WhatsApp backs up to iCloud in a different format (no local crypt15 file to copy), and moving between iPhone and Android is WhatsApp's own transfer tool. Supporting local iPhone (Finder/iTunes) backups would be a new importer with its own decision. |
| WhatsApp opened too early on phone 2 | It will have restored Drive or started empty, and may write its own `msgstore.db.crypt15`. Fix: move WhatsApp's folder aside, clear its data, copy the result again (it's still in `oopz-result-N`), restore again. |
| Restore refused by WhatsApp | Try, in order: same number; the target owner's key (not the source's); exact name and folder; WhatsApp on phone 2 updated (not older than the one that made the backup); free space; remove the Google account so Drive doesn't get in the way. If it still refuses, restore the **original** target backup to get the phone back, and report with `oopz chat-facts` output (counts only). |
| Keeping originals and a rollback | Originals are only read and hashed before and after; results never overwrite (new `oopz-result-N` / `oopz-received-N` folders). The rollback is the original backup plus the phone's WhatsApp folder copied in step 2 of section 6. |
| Password-protected backup | Section 5.1. |
| Google Drive backup | Section 4.6: not readable offline, not attempted. |

## 8. Stats, search and the chat viewer (built: index, search, stats, viewer)

Native screens in the app, not only an HTML export:

- **Stats** per chat or for the whole backup: messages, sent vs received, first and last message, messages per month
  (activity over time) and per hour of the day, messages and media size per type (text, photo, voice, video,
  document, sticker, call, …), minutes of voice notes and audio, and in groups who writes most (ids masked).
- **Search** over every message's text: results as you type, newest first, the matched words highlighted, scoped to
  one chat or the whole backup; a click opens the chat viewer at that message.
- **Chat viewer**: a virtualized list that only puts the rows on screen into the page (a 100,000-message chat scrolls
  like a short one), loading 200 lines at a time.

### 8.1 The index

On first opening Explore, `oopz-app` (`src/index.rs`) builds a private SQLite file next to the decrypted copy
(removed with it): one row per message (time, side, type, sender row, text, media size and duration) and an **FTS5**
full-text index with the `unicode61` tokenizer and `remove_diacritics 2`, so case and accents don't matter (`cancion`
finds `canción`, `CAFE` finds `café`). The last word typed is a prefix (`cumple` finds `cumpleaños`) when it has two
letters or more; words typed are always quoted, so `AND`, `*` or `"` are just text. Each message also gets its
position in its chat, so the viewer reads a screenful by an index lookup instead of `OFFSET`; and stats come from
aggregate tables (chat × month × hour × type × side) made at build time.

**FTS5 or tantivy?** FTS5, because:

- It's already in the binary: rusqlite's bundled SQLite includes FTS5, so there is no new dependency and no binary
  growth (tantivy adds a few MB and dozens of crates).
- The index is a SQLite file next to data that is already SQLite: one query joins search results to messages and
  chats, transactional, one file to delete.
- It is fast enough with a wide margin (numbers below: about 1 ms per search on a million messages).
- tantivy would win at ranking (BM25 with fields), fuzzy matching (typos) and very large corpora; none of those is
  needed for one person's chats. If typo-tolerant search is wanted later, FTS5's `trigram` tokenizer (substring
  search, about 3× larger index) or a spellfix table is the next step before switching engines.
- Spanish stemming (`canciones` → `canción`) isn't in FTS5's built-in tokenizers; prefix matching covers most of it.

### 8.2 Measured (synthetic data, `cargo run --release -j 2 -p oopz-app --example search_bench -- N`)

Invented words (Spanish and English words plus 5,000 invented syllable words, Zipf-like frequencies), 200 chats,
86% of messages with text. AMD Ryzen 7 7840HS laptop, WSL2, one run each, median of 21 queries (slowest in
brackets), 50 results asked:

| | 100,000 messages | 1,000,000 messages |
|---|---|---|
| Index build | 1.2 s, 21 MB | 20.1 s, 175 MB |
| Common word (`hola`) | 0.52 ms (0.87) | 1.27 ms (1.65) |
| Without accent (`cancion` → canción) | 0.51 ms (0.93) | 0.79 ms (1.28) |
| Prefix (`cumple`) | 0.31 ms (1.26) | 0.88 ms (1.18) |
| Two words (`good morning`) | 0.23 ms (0.44) | 1.15 ms (2.69) |
| No match | 0.04 ms | 0.05 ms |
| One letter (`m`, exact word only) | 0.03 ms | 0.03 ms |
| Common word in the biggest chat | 1.16 ms (1.69) | 1.31 ms (2.70) |
| Common word in a small chat | 0.54 ms (1.04) | 35.6 ms (43.8) |
| Viewer: 60 lines at the end of a chat | 0.06 ms | 0.07 ms |
| Jump from a hit to its place in the chat | 0.01 ms | 0.01 ms |
| Stats, whole backup | 23 ms (46) | 63 ms (87) |
| Stats, biggest chat | 3.4 ms (4.9) | 7.6 ms (13.1) |

Every search is under the 50 ms target at a million messages. Two measured fixes went in before these numbers: a
one-letter prefix (`m*`) took 63 ms on 1M, so a single letter matches only the whole word; the viewer's `OFFSET`
took 59 ms at the end of a 37,000-message chat, so each message stores its position. Known next steps:

- **Build time** at 1M (20 s) is mostly the stats aggregation's local-time conversion per message (7 s before it):
  compute month and hour in Rust in the copy loop, or build the aggregates in the background after search is ready.
- **Common word in a small chat** (36 ms on 1M) filters every match of the word by chat; a per-chat FTS column or
  scanning the small chat directly would bring it to a few ms.
- Measure on the slowest machine we support and with a real-shaped (but synthetic) distribution of chat sizes.

### 8.3 HTML viewer as an export only

The README's planned `oopz view` (an offline HTML file of one's own backup) becomes an **export** from the app (built,
`crates/oopz-app/src/export.rs`): on Explore, "Save as a web page" writes a new `oopz-export-N.html` in a folder the
person picks, with the stats of what is shown (the whole backup, or a chat) and, for a chat, all its messages by day,
for keeping or printing. One file, light and dark, no scripts or outside resources (a Content-Security-Policy forbids
them), every value escaped, ids masked; the words come from the UI's `i18n.ts`. The screen and the page say it holds
the person's messages and isn't encrypted. The app itself never needs a browser tab.

## 9. What runs today

- `crates/oopz-app`: find backups (folder, `Databases/` subfolder or file; current first; crypt14/12 and incomplete
  copies named), check (counts, dates, masked chat list, SHA-256 of the original, soundness), merge into a new
  `oopz-result-N` folder with media (the original's hash checked before and after), the Wi-Fi receive page, the
  search index, search, stats, the chat viewer's pages. 14 unit tests, synthetic data only.
- `apps/desktop`: the Tauri 2 shell (17 commands, 2 tests through the same state the commands use) and the web UI:
  Welcome → Get backup (folder, file, Wi-Fi QR with live progress) → Key (masked, cleared once sent, password panel)
  → Check → Explore (stats charts, search as you type, virtualized viewer) → Merge (calls the library's merge) → Put
  on phone 2 (guided checklist) → Done. English and Spanish. Laid out like a setup assistant: a sidebar of steps, content
  on the page (no cards), bordered tiles only for choices, one action bar per screen (Back left, the main action
  right); WCAG AA contrast in light and dark. Like a desktop app: no web page menu on right-click and no
  selecting labels (both kept where there is text to copy), the window's size and place remembered, and "Show folder"
  next to each result (only folders this run made).
- `.github/workflows/desktop-build.yml`: unsigned installers for the three systems, by hand as run artifacts with
  checksums, or from a tag `vX.Y.Z` as a draft GitHub Release with provenance (decision 0010).
- **Not yet**: group
  merges; restoring from an older own backup (0008); signed installers (0010); Android.
  The phone-2 screen is guidance only: oopz doesn't touch the phone.
- **Verified by hand**: every screen clicked through on Linux (WSLg, driven over WebDriver with `tauri-driver`) with the
  synthetic fixtures, Welcome to Done; and on Windows 11 (the installer from "Desktop installers", driven over WebView2's
  debugging port). Not yet on macOS. On closing, the app wipes what it decrypted (Tauri doesn't drop its state on
  exit, so `AppState::close` runs on the exit event).

## 10. Android later

Tauri 2 builds Android apps from the same shell; `oopz-app` compiles for Android as it is (`Backup::open_in` takes the
app's private cache folder because Android has no shared temp folder). Differences to plan: files come from the
Storage Access Framework (the person picks the `Databases` folder; `Android/media` is readable through SAF without
special permissions on Android 11+), the Wi-Fi page becomes "send to a computer" (the phone as the sender), and the
put-on-phone-2 step can copy the result into place on the phone itself (still never over an existing file without a
backup first).

## 11. Building and running

### What to install

- **All:** Rust 1.90+ (`rustup`), Node.js 20+ and pnpm (`corepack enable` then `corepack prepare pnpm@11.1.0
  --activate`, or pnpm's installer). **pnpm only**: never npm, npx or yarn (`pnpm dlx` instead of `npx`).
- **Linux (Debian/Ubuntu, also WSL with WSLg):**

  ```sh
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev \
      libayatana-appindicator3-dev librsvg2-dev
  ```

  (Fedora: `sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel
  librsvg2-devel libxdo-devel` and `sudo dnf group install "c-development"`; Arch: `sudo pacman -S webkit2gtk-4.1
  base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool`.) On this development
  machine webkit2gtk 4.1, libsoup 3 and GTK 3 were already present, so everything built without installing anything.
- **Windows:** Microsoft C++ Build Tools ("Desktop development with C++") and WebView2 (preinstalled on Windows 10
  and 11). Bundling an `.msi` needs nothing else; NSIS `.exe` installers are downloaded by Tauri's CLI.
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).

### Run it

```sh
cd apps/desktop
pnpm install
pnpm tauri dev          # the window, with the UI reloading on change
pnpm tauri build        # installers in src-tauri/target/release/bundle/ (deb, rpm, AppImage / msi, exe / dmg, app)
```

On a limited machine add `-- -j 2` to the cargo side (`pnpm tauri build -- -j 2`) or set `CARGO_BUILD_JOBS=2`.

### Checks

```sh
# the library and the app's logic (root workspace)
cargo fmt --check && cargo clippy -j 2 --all-targets -- -D warnings && cargo test -j 2
# the Tauri shell (its own workspace; needs the system packages above)
cd apps/desktop/src-tauri && cargo fmt --check && cargo clippy -j 2 --all-targets -- -D warnings && cargo test -j 2
# the web UI
cd apps/desktop && pnpm check
```

CI: the `desktop` job (`ubuntu-latest`) installs the Linux packages above, builds the UI (`pnpm build`, which
type-checks it) and runs the shell's checks; later the release job builds installers on the three systems with `tauri-apps/tauri-action`. Installer
signing (Windows Authenticode, Apple notarization) needs certificates and is left for the release decision.

## 12. Milestones

1. **Done (this version):** the core flow end to end, Wi-Fi receive, search, stats, viewer, guided screens.
2. Click through on Windows, macOS and Linux with the synthetic fixtures (Linux and Windows: done); progress bars (done); free-space checks (done); the
   index build in the background (done: it runs without holding the session, so other screens stay usable); a CI job for the shell and the UI (done).
3. "What's missing" between two backups (done); restore from an older own backup (0008); the HTML export (done).
4. Release builds (unsigned, by hand: done); publishing decided (0010: unsigned, draft GitHub Releases from a tag).
5. Android (Tauri 2 mobile), the same `oopz-app`.
