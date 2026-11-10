# 0015: Adapters for the apps' official text exports

Status: accepted (2026-11-10)

The most common history people can take out of an app with no key at all is the app's own export: WhatsApp's "Export
chat", Telegram Desktop's "Export chat history" (JSON), SMS Backup & Restore's XML, LINE's "Export chat history"
(text), Viber's "Export message history" (CSV). None of these needs root, a key, a password or a login.

## Decision

1. One crate per format under `crates/adapters/`: `whatsapp-export`, `telegram-export`, `sms-xml`, `line-export`,
   `viber-export`, each implementing the adapter trait (0013) and returning a `Loaded` source.
2. **Readers are lenient about layout and strict about meaning**: every date format, separator, 12/24-hour clock,
   language and byte-order mark the apps are known to write is read; a line that fits none is kept as text of the
   message before it (as the app wrote it) or as an `Unknown` part, and counted in the source's notes. A file that is
   another format altogether is refused (`NotThisFormat`); a format version never seen is refused by name.
3. **Day/month order** in WhatsApp and Viber exports follows the phone's locale and isn't written in the file: it's
   decided from the whole file (a first number over 12 means day first; a second over 12 means month first); when
   nothing decides it, day first, and the notes say so.
4. **Who "me" is** isn't written in WhatsApp's or LINE's text exports. The adapter marks "me" only where the export
   says so ("You", the owner's name from the file's header, Viber's "Me"); otherwise the library lets the owner pick
   (`set-me`).
5. **New dependencies** (workspace): `serde_json` (Telegram), `quick-xml` (SMS), `csv` (Viber), `base64` (MMS parts),
   with `zip` (already used, 0009) for zipped exports. They read data, never execute it; a zipped export is read in
   memory, never unpacked to disk, and entries with `..` or absolute paths are ignored.
6. **Test data** is made by `oopz-gen` for each format (invented people, numbers starting `9999`), with every odd
   line the format allows; the docs per app (`docs/adapters/<app>.md`) list what was checked and what still needs a
   real export to confirm.

## Revisit when

An app changes its export format (a new reader branch, tested with a generated sample), or offers a richer official
export (prefer it).
