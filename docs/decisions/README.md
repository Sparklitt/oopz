# Decision records

Why oopz is the way it is. Numbers are kept from the hilo project, where oopz began, so comments in the code
(`decision 0006`) still point at the right record here.

- [0004](0004-whatsapp-backup-decryption.md): the crates for decrypting crypt15 backups; nothing else without a record.
- [0006](0006-merge-a-chat-back-into-whatsapp.md): merging a chat back into the other person's backup (`oopz restore`).
- [0007](0007-oopz-a-whatsapp-backup-tool.md): oopz itself, its rules, and publishing it.
- [0008](0008-restore-a-chat-from-an-older-own-backup.md): restoring from an older backup of one's own (proposed).
- [0009](0009-a-desktop-app-with-tauri.md): the desktop app with Tauri 2, its Wi-Fi receive page and search
  ([plan](../desktop-plan.md)).
- [0010](0010-unsigned-installers-on-github-releases.md): the first releases, unsigned installers on GitHub Releases.
- [0011](0011-an-opt-in-update-check.md): the desktop app's update check, off unless switched on; signed updates.
- [0012](0012-the-android-app.md): the Android app: old phones first, content references copied in, release signing.
- [0013](0013-one-model-for-every-app.md): one model for every app (`oopz-model`) and the adapter trait.
- [0014](0014-whatsapp-as-an-adapter.md): WhatsApp Android's crate moves to `crates/adapters/whatsapp-android` and implements the adapter trait.
- [0015](0015-official-text-exports.md): adapters for the apps' official text exports (WhatsApp, Telegram, SMS, LINE, Viber).
- [0016](0016-the-library-selections-and-exports.md): the library (`oopz-core`), selections, the cross-app timeline, contacts and exports.

Code comments also cite hilo's decision 0034 (WhatsApp groups and sticker packs). What it settled for oopz: a chat's
names are read from `wa.db` only for the jid asked for (point 6, `contacts.rs`), and the synthetic fixtures include a
group, a two-phones group and a sticker-pack media folder (points 28 to 30, `fixture/group.rs`).
