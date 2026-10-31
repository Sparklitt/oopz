# 0009: A desktop app, with Tauri 2

Status: accepted (2026-10-31); first version built, local only

0007 said "CLI first. A UI later, if ever." The people who lose a chat are mostly not terminal users: they need to be
walked through getting a backup off a phone, typing a key, checking it and putting a result back. The full plan,
with the screens, the ways to move files and every edge case, is [docs/desktop-plan.md](../desktop-plan.md).

## Decision

1. **Tauri 2** for the desktop app (`apps/desktop`), over Compose Multiplatform, Slint and egui: the core stays plain
   Rust in the same process, installers stay a few MB (system webview), the UI is ordinary HTML (accessible,
   well-typeset English and Spanish, easy to contribute to), and Tauri 2 also builds Android from the same core.
   The web UI is plain TypeScript with Vite, no framework; pnpm only.
2. **`crates/oopz-app`** holds the app's logic without a UI toolkit (find, check, merge into a new folder, the Wi-Fi
   receive page, the search index and stats), in the workspace and tested there. The Tauri shell
   (`apps/desktop/src-tauri`) wraps it one command per call and is its **own Cargo workspace**, so the library's
   checks and CI don't need webkit2gtk.
3. **Keys** go from a masked field to one command as a `Zeroizing<String>`, become an `oopz::Key`, and are dropped
   when the command returns. Never stored; asked again when needed.
4. **The Wi-Fi receive page** is the one exception to "no network": a server bound to the computer's LAN address
   only, only while the person has the screen open, behind a 128-bit one-time token in the path, answering only
   private/link-local/loopback peers, writing into a new folder without overwriting, stopping when the phone is
   done, when asked, after 10 idle minutes or after 50 refused requests. Plain HTTP: the backup is already
   encrypted; media is not, and the screen says to use a home network or the cable. No new cryptography.
5. **Search** uses SQLite FTS5 from rusqlite's bundled SQLite (`unicode61`, accents removed), not tantivy: no new
   dependency, the index is one private SQLite file deleted with the backup's decrypted copy, and it measures about
   1 ms per query on a million synthetic messages. The app shows the person's own message text on their screen
   (search, viewer); it never goes to a log, an error or a file outside the app's private cache.
6. **Password-protected backups** (key in WhatsApp's server-side vault) are explained, not opened: oopz never talks to
   WhatsApp's servers as if it were the app. It walks the person through switching to a 64-digit key inside WhatsApp
   and making a fresh backup.
7. **Style shared, code not**: the palette of `assets/make_assets.py` as design tokens; no code from the private hilo
   app.

New crates, none cryptographic: `serde` (results for the UI), `tiny_http` (the receive page), `qrcode` (its QR code,
SVG only), `fs4` (free disk space before writing a decrypted copy or a merge; added 2026-10-31), in `oopz-app`; `tauri`, `tauri-build`, `tauri-plugin-dialog` (file and folder pickers) in the shell.

## Consequences

- The README's "oopz makes no network connections" stays true of the CLI and the library; the app's Wi-Fi page is
  opt-in, local-network only, and documented here.
- Two Cargo workspaces to check (the plan's section 11 lists the commands); CI gets a job for the shell when it's
  ready.
- Tauri and WebView versions become part of what a release pins and updates.
