# Changelog

All notable changes to oopz. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `oopz-gen` (not released): writes synthetic WhatsApp backups of any size for testing, with every kind of message.

- The CLI does what the app does, with the same code ([docs/parity.md](docs/parity.md)): new `check`, `stats`,
  `search` (where, never the text), `export` (a web page, to a file), `missing` and `receive` (over Wi-Fi, a QR code
  in the terminal). A test fails if a command is added to one and not the other.
- The app has the CLI's diagnostics as commands (tables, types, chat facts) for problem reports.
- Release binaries of the CLI for Linux (x86_64, arm64), Windows and macOS.

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
