# Changelog

All notable changes to oopz. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.1] - 2026-11-10

### Added

- **More apps, one library.** Besides WhatsApp on Android, oopz now reads an iPhone's own Finder/iTunes backup
  (WhatsApp and WhatsApp Business; an encrypted one opens with the owner's backup password, decision 0018) and the
  official exports of WhatsApp (Export chat), Telegram Desktop (JSON, checked against Telegram's published schema),
  SMS Backup & Restore (messages, MMS pictures and calls), LINE and Viber (decisions 0013–0015). Each is added to a
  library on this device (decision 0016): chats from every app side by side, search across all of them, a timeline,
  insights, contacts joined across apps by hand (with suggestions, vCard in and out), saved selections, and exports
  as a web page, PDF (right-to-left text and emoji, decision 0017), JSON, CSV, text, vCard or a zip of the media. The
  library is wiped when oopz closes unless "Keep my library" is on. It stays fast at a million messages
  ([docs/performance.md](docs/performance.md)).
- `oopz lib …` in the CLI does everything the library screens do ([docs/parity.md](docs/parity.md)).
- On a phone, swipe sideways to move between the bottom tabs.
- The app's languages are one file each (`apps/desktop/src/lib/i18n/`); adding one needs no backend change. Import
  notes from every app are shown in Spanish too.
- A launch screen, the oopz icon everywhere, and Back inside the app on Android.
- `oopz-gen` (not released): writes synthetic WhatsApp backups of any size for testing, with every kind of message.

- The CLI does what the app does, with the same code ([docs/parity.md](docs/parity.md)): new `check`, `stats`,
  `search` (where, never the text), `export` (a web page, to a file), `missing` and `receive` (over Wi-Fi, a QR code
  in the terminal). A test fails if a command is added to one and not the other.
- The app has the CLI's diagnostics as commands (tables, types, chat facts) for problem reports.
- Release binaries of the CLI for Linux (x86_64, arm64), Windows and macOS.
- The chat viewer shows replies (and jumps to the answered message), reactions, edited and forwarded marks, polls
  with their votes, calls with their outcome and length, and photo thumbnails from the phone's Media folder.
- Chats show their names (a group's subject; a contact's name from `wa.db.crypt15` beside the backup) and their last
  message; names can be hidden.
- Search filters by type, sent or received, sender and dates, and works without words; "jump to a date" in a chat; a
  day × hour grid in the stats. CLI: `search --kind/--from/--after/--before`, `jump`, `thumbnail`, `chats --names`.
- Opening a backup names each step, estimates the time left and can be cancelled; the Wi-Fi page shows the speed and
  when it will stop by itself; "Wipe now" removes every decrypted copy at once.
- The update check shows the release notes and when it last checked.
- Android: a signed APK in each release; a picked backup is copied into the app's private storage and results are
  saved where the person picks (decision 0012).

### Changed

- `oopz restore` writes a new `oopz-result-N` folder (`--out-dir`), media in its `Media/` (`--media`), as the app
  does; `--out` and `--media-out` are gone.

### Fixed

- A folder scan no longer lists WhatsApp's contacts backup (`wa.db.crypt15`) and its other non-message backups as
  message backups (the app could take `wa.db.crypt15` for the current backup; `oopz backups` failed on it).
- "What's missing" no longer fails on a message row with no time.
- One value of an unexpected type (a time that is text, a type that is missing) no longer stops a whole backup from
  opening.
- A restore into a phone whose newer WhatsApp requires data the source lacks is refused naming the column, instead of
  failing as if the backup were damaged.

## [0.1.0] - 2026-10-31

### Added

- First public version, under AGPL-3.0-or-later.
- The desktop app (Tauri 2, Windows, macOS and Linux, English and Spanish), a guided setup assistant: get a backup
  (folder, file, or from the phone over Wi-Fi with a QR code), type its key, check what's inside, explore it (stats,
  search as you type, a chat viewer, save as a web page), see what's missing between two backups, merge a lost
  one-to-one chat into a new copy, and the steps to put it on the phone. Originals are never changed; nothing leaves
  the computer.
- An update check in the desktop app, off unless switched on; updates are signed and installed only on a click.
- `oopz backups`, `chats`, `types`, `tables`, `chat-facts`: check and inspect one's own crypt15 backups (counts and
  masked ids only).
- `oopz restore`: restore a lost one-to-one chat from the other person's backup, mirrored, checked before it is
  written; optional media pack.
- Synthetic WhatsApp backups for tests (`fixtures` feature).

The installers are unsigned: see the README's "Install" for the Windows and macOS warnings and how to check a
download.
