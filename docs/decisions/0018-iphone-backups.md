# 0018: iPhone backups

Status: accepted (2026-11-10)

On an iPhone, WhatsApp keeps its chats in `ChatStorage.sqlite`, and the only way a person gets that file without
jailbreaking is the backup Finder (macOS) or iTunes / Apple Devices (Windows) makes on their own computer. The rule
for every adapter holds: only the owner's own backup, opened with the owner's own password; no jailbreak, no device
access, no iCloud login, no guessing.

## Decision

1. **`crates/ios-backup`** (`oopz-ios-backup`) reads a backup folder: `Manifest.plist` (device, iOS version, date,
   encrypted or not, the keybag), `Manifest.db` (each file's domain, path and record) and the files under
   `<first two hex digits>/<file id>`. It only reads; the folder is never written to.
2. **Encrypted backups** open with the backup password the owner set: PBKDF2-HMAC-SHA256 (`DPSL`, `DPIC`) then
   PBKDF2-HMAC-SHA1 (`SALT`, `ITER`), as the keybag says; the class keys are unwrapped with AES key wrap (RFC 3394);
   an unwrap that fails is a wrong password. `ManifestKey` decrypts `Manifest.db` into a private temporary folder
   removed when the backup is closed; each file is AES-256-CBC with its own key wrapped by its class key, decrypted
   as a stream and cut to its recorded size. The password is never stored or logged. New crates, from RustCrypto like
   0004's: `aes`, `aes-kw`, `cbc`, `pbkdf2`, `sha1`; and `plist` for the property lists. iOS 9 and older backups
   (`Manifest.mbdb`) are refused with a clear message.
3. **`crates/adapters/whatsapp-ios`**: two adapters, `whatsapp-ios` and `whatsapp-business-ios` (WhatsApp Business has
   its own app group). Detection only knows "an iPhone backup" (an encrypted manifest can't be read before the
   password); WhatsApp is picked first and the notes say when Business is in the backup too. `Adapter::secret_for`
   asks for the password only when the backup is encrypted.
4. **Reading `ChatStorage.sqlite`**: chats from `ZWACHATSESSION`, messages from `ZWAMESSAGE` in date order, senders
   in groups from `ZWAGROUPMEMBER`, media facts from `ZWAMEDIAITEM`, names from WhatsApp's address book
   (`ContactsV2.sqlite`), then push names, then the chat's partner name. Every column is optional (WhatsApp changes
   them): a missing one reads as empty. Dates are seconds since 2001-01-01 UTC. Types it doesn't know are kept as
   unknown and counted in the notes. What WhatsApp keeps in protocol buffers (reactions, replies, edits, poll votes)
   isn't read yet, and the notes say so.
5. **Media** comes out of the backup one message at a time as inline bytes; the library writes them to its own
   folder (0016) after checking free space. A file larger than 512 MB stays in the backup and is named; a file the
   backup doesn't hold is marked as not included.
6. **Test data**: `oopz-gen --ios OUT [--ios-password PW]` writes an invented backup (numbers `9999…`) in the same
   layout, encrypted with real keybag, key wrap and CBC, with small PBKDF2 counts (the reader takes them from the
   keybag). Not yet checked against a real iPhone's backup: docs/adapters/whatsapp-ios.md lists what a real one must
   confirm.
