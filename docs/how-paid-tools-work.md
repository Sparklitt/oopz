# How the paid tools get at chat data, and what oopz does instead

Paid tools (Dr.Fone WhatsApp Transfer, iMazing, iMyFone iTransor, Backuptrans, MobileTrans, WazzapMigrator…) advertise
WhatsApp, LINE, Viber, Kik and WeChat backup, restore and transfer. They use a handful of techniques. oopz uses the
ones that only touch what the owner already controls, and documents the rest so the line stays clear (rule: oopz
opens what an app gives its owner, an export or a backup opened with the owner's own key or password; it never cracks,
roots, downgrades or impersonates).

## On an iPhone: the device's own local backup (oopz: yes)

An iPhone's **local backup** (made by Finder on a Mac, iTunes or Apple Devices on Windows, or any client speaking the
same protocol) contains most apps' sandboxes: their databases, preferences and media, unless the app marks files as
not backed up. This is how the paid tools reach WeChat, Kik, LINE, Viber and WhatsApp on iPhone without a jailbreak.

- **Connecting**: USB (or Wi-Fi sync) to the device's `lockdownd`; the person taps **Trust** and types the passcode,
  which creates a pairing record. Services used: `com.apple.mobilebackup2` (backup and restore), `com.apple.afc`
  (media folder), `house_arrest` (only apps that share their documents, so rarely useful). Open implementations:
  libimobiledevice (C, LGPL) and pure-Rust clients of the same protocols; on Windows the Apple Mobile Device driver
  comes with Apple's apps.
- **Layout**: `Manifest.db` (SQLite) maps each file's domain and path (for example `AppDomain-com.tencent.xin`,
  `AppDomainGroup-group.net.whatsapp.WhatsApp.shared`) to a file named by the SHA-1 of `domain-path`, stored under its
  first two hex digits. `Manifest.plist` and `Info.plist` describe the device and the backup.
- **Encrypted backups** (the person sets a backup password; Health, Keychain and Wi-Fi are only in these): the
  password, through PBKDF2 (SHA-256 then SHA-1, salts and rounds from `Manifest.plist`'s keybag), unwraps the class
  keys (AES key wrap); each file's own key is in `Manifest.db`, unwrapped with its class key; files are AES-256-CBC.
  The password is the owner's: this is the same "owner's own key" as a crypt15 backup.
- **What's inside per app** (to verify per app version): WhatsApp `ChatStorage.sqlite` + `Message/Media/`; WeChat
  per-account folders with message databases (newer versions encrypt them with a key held in the app: if so, oopz
  refuses by name, as with crypt14); Kik `kik.sqlite`; LINE `Line.sqlite`; Viber `Contacts.data` and its database;
  Messages `sms.db` + `Attachments/`; Signal keeps its database out of backups by design.
- **Restore**: the tools change files inside the backup, update `Manifest.db` (sizes, digests, keys for encrypted
  backups) and restore it to the device with `mobilebackup2`. This replaces the device's data with the backup's: the
  riskiest operation there is, and the paid tools' most common failure. oopz writes back only after its own decision
  (like 0006), always to a new backup, checked, never in place.

## On Android: three techniques, one of which oopz uses

1. **The app's own backup file with the owner's key** (WhatsApp crypt15 + 64-digit key): oopz, done.
2. **"Downgrade" extraction** (how tools reach WeChat, Kik, LINE, Viber and old WhatsApp on Android without root): they
   uninstall or replace the current app with an **old APK** that allowed `adb backup`, run `adb backup` to pull the
   app's private data, then reinstall the current version. Problems: it installs old, unpatched apps from wherever the
   tool got them; it can lose data if anything goes wrong mid-way; it breaks the app's own protection; `adb backup` is
   deprecated and, on Android 12+, excludes apps targeting new SDKs. **oopz: no.**
3. **Deriving keys**: WeChat's Android database (`EnMicroMsg.db`) is SQLCipher, keyed from device identifiers and the
   account's id; tools compute the key after extracting the app's files. That is getting around the app's protection
   rather than the owner unlocking their own backup. **oopz: no.** Root-only tools (reading `/data/data`) are likewise
   out.

## Cloud backups (oopz: no)

Some tools sign in to iCloud or Google with the person's account to download a cloud backup, or imitate the app to
fetch a key (WhatsApp's password-protected backups). That means handling the person's account credentials and
imitating servers. oopz never does; it tells the person how to make a local backup instead.

## Cross-platform transfer (WhatsApp Android ↔ iPhone)

The paid tools convert WhatsApp's Android database (`msgstore.db`) to the iPhone's (`ChatStorage.sqlite`) or back,
put it into a local iPhone backup (or onto the Android phone), and restore. oopz can do the same with its own checks:
read both sides (adapters), map through the model, write a new database, check it like 0006, put it in a new backup.
Its own decision record first: it rewrites a whole device's chat history.

## What this means for oopz's plan

- **iPhone local backups are the way to WeChat, Kik, LINE and Viber**: legitimate, no jailbreak, the owner's password.
  An `ios-backup` crate (read `Manifest.db`, decrypt with the owner's password, extract one app's files) comes first
  in phase 3; each app is then an adapter over the extracted files.
- **Making the backup** can be oopz's own (the same USB protocol, a Rust client), or Finder/Apple Devices with oopz
  reading the folder they write. Start with reading the folder; making it is a later decision (it needs the device
  pairing flow and a protocol implementation).
- On Android, oopz stays with official exports and owner-keyed backups.
