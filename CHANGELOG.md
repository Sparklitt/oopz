# Changelog

All notable changes to oopz. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- First public version, under AGPL-3.0-or-later.
- `oopz backups`, `chats`, `types`, `tables`, `chat-facts`: check and inspect one's own crypt15 backups (counts and
  masked ids only).
- `oopz restore`: restore a lost one-to-one chat from the other person's backup, mirrored, checked before it is
  written; optional media pack.
- Synthetic WhatsApp backups for tests (`fixtures` feature).
- A desktop app in progress (`apps/desktop`, Tauri 2) on a new `oopz-app` crate: check backups, receive them from a
  phone over Wi-Fi with a QR code, stats and fast search, merge a chat; English and Spanish (decision 0009).
