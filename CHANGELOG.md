# Changelog

All notable changes to oopz. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
