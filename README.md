<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/banner-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/banner-light.svg">
    <img alt="oopz: your own chat history, from every app, in one place. Search, select, export, restore a lost chat. Offline." src="assets/banner-light.svg" width="100%">
  </picture>
</p>

<p align="center"><b>Your own chat history from WhatsApp, Telegram, SMS, LINE and Viber, together on your own device: search it, select it, export it, and bring a lost WhatsApp chat back. Offline.</b></p>

<p align="center">
  <a href="https://github.com/sparklitt/oopz/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/sparklitt/oopz/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="Licence: AGPL-3.0-or-later" src="https://img.shields.io/badge/licence-AGPL--3.0--or--later-blue"></a>
  <img alt="Rust 1.89+" src="https://img.shields.io/badge/rust-1.89%2B-orange?logo=rust">
  <img alt="Linux, macOS, Windows" src="https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey">
  <img alt="Network: an opt-in update check only" src="https://img.shields.io/badge/network-opt--in%20update%20check%20only-success">
</p>

oopz opens your own chat history: a desktop and Android app and a command-line tool. It reads only what is yours to
read: an app's official export, or a backup opened with your own key or password.

| Source | What you give oopz |
|---|---|
| WhatsApp on Android | the end-to-end encrypted backup (`msgstore.db.crypt15`) and its 64-digit key |
| WhatsApp on iPhone | the Finder / iTunes backup of your iPhone, and its backup password when it is encrypted |
| WhatsApp, any phone | a chat's "Export chat" (`.txt` or `.zip`) |
| Telegram | Telegram Desktop's JSON export (`result.json`; made on a computer, opens on the phone too) |
| SMS and calls (Android) | SMS Backup & Restore's XML |
| LINE | "Export chat history" (`.txt`) |
| Viber | "Export message history" (CSV) |

Everything you add goes into one **library** on your device: search every app at once, read any conversation, a
timeline across apps, insights, contacts linked across apps (with vCards in and out), saved selections, and exports
to HTML, JSON, CSV, text, vCard, a media zip or PDF. For WhatsApp's Android backups, oopz also checks whether they
open and are sound, and can restore a lost one-to-one chat from the other person's backup into a new, checked
backup. Each adapter's page in [docs/adapters](docs/adapters/README.md) says what it reads and what a real export
still needs to confirm.

## Why

Phones break, chats get deleted by mistake, a restore picks the wrong backup. The messages often still exist: in an
older backup of yours, or on the phone of the person you were talking to. oopz helps you find out what your backups
hold and put a lost chat back, without sending anything anywhere and without touching the originals.

## Features

- 🔍 **Check** every backup in a folder with one key: does it open, is its database sound, how many chats and messages.
- 🗂️ **Inspect** a backup: chats with masked ids, message types per chat, tables and columns. Counts only.
- ↩️ **Restore** a lost one-to-one chat from the other person's backup, mirrored to your side, checked before it is
  written; optionally with the media files it needs.
- 📚 **One library for every app**: add exports and backups from WhatsApp (Android and iPhone), Telegram, SMS, LINE and
  Viber; search them together, read a merged timeline, see insights, link the same person across apps.
- 📤 **Export what you select**: HTML, JSON, CSV, text, vCard, a media zip, or a PDF with emoji and right-to-left text.
- 🔒 **Private by design**: offline, keys from a hidden prompt and wiped after use, output is counts and masked ids.

## Demo

<p align="center">
  <img alt="A terminal session: oopz checks a folder of backups, lists the chats in one with masked ids, and restores a lost chat into a new, checked backup." src="assets/demo.svg" width="100%">
</p>

<sub>Replayed from a run on the synthetic test backups (invented people, test keys), never on real data.</sub>

> [!IMPORTANT]
> **Safety first**
>
> - **Your own backups, with your own key, with consent.** oopz opens a backup only with the 64-digit key its owner
>   chose when they turned on end-to-end encrypted backups. It never tries to get around WhatsApp's encryption of
>   anyone's account. To use someone else's backup (for `oopz restore`), they give you their backup and type their
>   own key, knowing what you will do with it.
> - **Originals are never modified.** Backups and `Media/` folders are only read; every result is a new file or
>   folder, and oopz refuses to overwrite an existing one. Work on a copy anyway.
> - **Offline.** oopz makes no network connections. The one exception is the desktop app's update check, off unless
>   you switch it on or click "Check for updates": it asks GitHub whether a newer oopz exists, and sends nothing else.
> - **Counts and masked ids only.** Its output never contains message text, media, phone numbers or keys.
> - **Keys** are read from a hidden prompt (or one line of stdin per key, `oopz … < keyfile`), never from an argument
>   or an environment variable, and wiped from memory after use.

## Install

The desktop app: download the installer for your system from the
[latest release](https://github.com/Sparklitt/oopz/releases/latest).

- **Windows**: `oopz_<version>_x64-setup.exe` (or the `.msi`). It isn't signed yet, so SmartScreen says "Windows
  protected your PC": choose **More info › Run anyway**.
- **macOS** (Apple silicon and Intel): `oopz_<version>_universal.dmg`. It isn't signed or notarized yet: drag oopz to
  Applications and open it once; when macOS says it can't check it, go to **System Settings › Privacy & Security** and
  choose **Open Anyway**.
- **Linux**: `.deb` (Debian, Ubuntu), `.rpm` (Fedora, openSUSE) or `.AppImage` (any; make it executable).

The command-line tool `oopz`: `oopz-cli-<version>-<system>.tar.gz` (`.zip` on Windows) from the same release, for
Linux (x86_64, arm64), Windows (x86_64) and macOS (universal). Unpack it and put `oopz` on your `PATH`; on macOS, if
it's blocked, run `xattr -d com.apple.quarantine oopz` once.

To check a download is the one built here from this source, compare its hash with the release's `SHA256SUMS.txt`
(`sha256sum <file>`, or `Get-FileHash <file>` in PowerShell) and verify its provenance with the GitHub CLI:
`gh attestation verify <file> --repo Sparklitt/oopz`.

## Quick start (command line)

Use a release binary (above), or build it: you need Rust 1.89 or newer ([rustup.rs](https://rustup.rs)) and a C
compiler (SQLite is built from source).

```sh
git clone https://github.com/sparklitt/oopz && cd oopz
cargo install --locked --path crates/oopz-cli   # installs the `oopz` binary
# or build without installing:
cargo build --release                           # target/release/oopz
```

Copy your backup off the phone first (WhatsApp keeps it in `Android/media/com.whatsapp/WhatsApp/Databases/`) and work
on the copy:

```sh
oopz backups --dir Databases/                          # do they all open, and are they sound?
oopz chats --backup Databases/msgstore.db.crypt15      # which chats, how many messages
```

Every command that opens a backup asks for its 64-digit key.

## Usage

Each command does what the desktop app's matching screen does, with the same code ([docs/parity.md](docs/parity.md)).
The terminal shows counts, dates and masked ids only; message text goes only into a file you name (`oopz export`).

| Command | What it does |
|---|---|
| `oopz backups --dir <folder or file>` | Checks every backup in a folder with one key: does it open, is its database sound, how many chats and messages, the newest message's date. Older formats are listed, not opened. Fails if any backup doesn't pass (another key, damaged, cut off). |
| `oopz check --backup <file>` | What's inside one backup: soundness, counts, dates, the file's SHA-256 fingerprint. |
| `oopz chats --backup <file>` | Lists the chats in a backup: number, masked id, messages, last message's date. |
| `oopz stats --backup <file> [--chat N]` | Messages by month, hour and type, voice minutes, who writes most (masked). |
| `oopz search --backup <file> --query <words> [--chat N]` | Where your words appear (chat, date, sent or received, position), never the text. Accents and case don't matter. |
| `oopz export --backup <file> [--chat N] --out-dir <folder>` | Saves the stats (and a chat's messages) as a web page in a new file. It isn't encrypted: keep it private. |
| `oopz missing --a <file> --chat-a N --b <file> --chat-b M` | What's missing between two backups' copies of the same chat, matched by message id. |
| `oopz restore --source <file> --source-chat N --target <file> --target-chat M --out-dir <folder> [--media <folder>]` | Restores a lost one-to-one chat into a new `oopz-result-N` folder (see below). |
| `oopz receive --out-dir <folder>` | Receives a backup from the phone over your Wi-Fi: a QR code in the terminal, files saved in a new folder. |
| `oopz types --backup <file> --chat N` | For each WhatsApp message type in chat N: counts and which tables its rows link to. No content. |
| `oopz tables --backup <file>` | Every table with its columns and row count. Structure only, no values. |
| `oopz chat-facts --backup <file> --chat N` | How WhatsApp keeps one chat: ordering ids, senders, statuses, pointers. Counts and ranges only; useful to compare two phones. |

`oopz --help` and `oopz <command> --help` describe every flag.

### The library

`oopz lib … --library <folder>` works on a library folder you name (it holds your messages unencrypted: keep it
private, delete it when done). Like the rest of the CLI it prints counts, ids and dates; names only with `--names`;
message text only into a file you ask for.

| Command | What it does |
|---|---|
| `oopz lib adapters` | The apps and formats oopz reads, and which ask for a key or password. |
| `oopz lib import <file or folder> [--adapter ID]` | Adds an export or a backup (the app is detected; the key or password is asked for when needed). |
| `oopz lib sources`, `oopz lib remove --source N`, `oopz lib set-me --person N` | What's in the library; take a source out; say which person is you in a text export. |
| `oopz lib convs`, `oopz lib search <words>`, `oopz lib timeline`, `oopz lib jump`, `oopz lib count`, `oopz lib stats` | Conversations, search, the timeline across apps, numbers. Filters: `--app`, `--source`, `--conv`, `--contact`, `--person`, `--kind`, `--from me\|them`, `--after`, `--before`, `--words`. |
| `oopz lib people`, `oopz lib contacts`, `oopz lib suggest`, `oopz lib accept N`, `oopz lib link`, `oopz lib unlink`, `oopz lib rename`, `oopz lib forget`, `oopz lib vcard-import`, `oopz lib vcard-export` | The same person across apps, and vCards in and out. |
| `oopz lib select`, `oopz lib selections`, `oopz lib unselect` | Saved selections: a filter, plus messages added or left out by id. |
| `oopz lib export --format html\|json\|csv\|txt\|vcard\|media-zip\|pdf --out-dir <folder> (--all \| --selection N \| filters)` | Writes a new file, never over another. |

### Restoring a lost chat

Two people share a one-to-one chat; one of them lost it (or part of it), the other still has it. `oopz restore` adds
the messages of the chat in the **source** backup (the one that has them) that the same chat in the **target** backup
(the person who lost it) lacks, mirrored to the target owner's side, and writes a new, checked backup for the target's
owner. It asks for the source's key, then the target's.

- Nothing the target has is changed or lost; added messages are placed before the target's own, in order.
- Before writing, the result is decrypted again and checked (database integrity, counts, ordering, references, every
  other chat identical). If any check fails, nothing is written.
- The result is a new `oopz-result-N` folder laid out like WhatsApp's (`Databases/msgstore.db.crypt15`); nothing is
  ever overwritten. With `--media` (a copy of the source phone's `Media/` folder), the files it needs are copied into
  its `Media/`.
- Afterwards, run `oopz chat-facts` on the result.

> [!WARNING]
> **Read before restoring on a phone.** This changes a WhatsApp backup, which WhatsApp's terms may not allow; the
> account owner accepts that risk. It has been used for real restores only a few times, on a few phones and WhatsApp
> versions. Keep the original backup and move the phone's `WhatsApp/` folder aside before clearing WhatsApp's data, so
> you can always go back. The design and its checks are in
> [docs/decisions/0006](docs/decisions/0006-merge-a-chat-back-into-whatsapp.md).

## Built with

- [Rust](https://www.rust-lang.org) (edition 2024, `unsafe` forbidden) and [clap](https://crates.io/crates/clap) for
  the command line.
- [rusqlite](https://crates.io/crates/rusqlite), with SQLite built from source, to read and check the backup's
  database.
- Vetted [RustCrypto](https://github.com/RustCrypto) crates for the backup's encryption:
  [aes-gcm](https://crates.io/crates/aes-gcm), [hkdf](https://crates.io/crates/hkdf) and
  [sha2](https://crates.io/crates/sha2); [md-5](https://crates.io/crates/md-5) only for the backup's footer (not for
  security). No home-made cryptography; the choice is recorded in
  [docs/decisions/0004](docs/decisions/0004-whatsapp-backup-decryption.md).
- [prost](https://crates.io/crates/prost) for the backup's header, [flate2](https://crates.io/crates/flate2) for its
  compressed database, [zeroize](https://crates.io/crates/zeroize) and [rpassword](https://crates.io/crates/rpassword)
  for keys, [jiff](https://crates.io/crates/jiff) for dates.

## FAQ

<details>
<summary><b>Is this legal?</b></summary>

oopz opens only backups whose owner gives you their key; it never breaks or bypasses anyone's encryption. Reading
your own data is yours to do. `oopz restore` writes a changed backup, which WhatsApp's terms may not allow: the
account owner decides and accepts that risk. This is not legal advice.
</details>

<details>
<summary><b>Does it upload anything?</b></summary>

No. oopz has no telemetry and never uploads backups, keys or results. The command line connects to nothing; only
`oopz receive` opens a page on your local network, for your phone, while it runs. The desktop app does the same for
"Receive over Wi-Fi", and goes online only to check for updates, and only if you switch that on or click "Check for
updates" ([decision 0011](docs/decisions/0011-an-opt-in-update-check.md)).
</details>

<details>
<summary><b>Which backups does it support?</b></summary>

WhatsApp's end-to-end encrypted Android backups, `msgstore.db.crypt15`, opened with the 64-digit key. Older formats
(`crypt12`, `crypt14`) and iPhone backups are not supported.
</details>

<details>
<summary><b>What if a WhatsApp update changes its database?</b></summary>

oopz reads tables and columns by name, not position. New tables and columns are ignored; missing optional ones (calls,
reactions, polls…) just leave those details out; a value of an unexpected type in one row doesn't stop the rest. If
something oopz needs is gone, it says which table or column, and opens nothing. A restore into a phone whose newer
WhatsApp needs data the other phone's backup doesn't have is refused by name, before anything is written; what can't be
carried over is listed. `oopz tables` shows the structure (never values) for a bug report.
</details>

<details>
<summary><b>I lost my key. Can oopz help?</b></summary>

No. Without its key a crypt15 backup cannot be opened, and oopz will never try.
</details>

## Privacy

- Your backups, keys and messages never leave your computer: no account, no cloud, no telemetry, no analytics.
- Keys are used once to open a backup, then wiped from memory; never saved, logged or sent.
- Originals are only read. Decrypted copies live in a private temporary folder and are removed when oopz closes (after
  a crash, on its next start).
- The only network use is the local Wi-Fi page for your phone (while you use it) and the update check, which is off
  unless you turn it on: it asks GitHub for the latest version's number, and GitHub sees your IP address.
- Output, errors and logs hold counts and masked ids, never message text, phone numbers or keys.

## Roadmap

- [x] `backups`, `check`, `chats`, `stats`, `search`, `missing`, `types`, `tables`, `chat-facts`: check and inspect.
- [x] `restore`: a lost one-to-one chat, checked before it is written. `export`: a web page of your own backup.
- [x] Desktop app (Tauri 2, English and Spanish), doing what the CLI does ([parity](docs/parity.md)).
- [x] Release binaries for Linux, macOS and Windows.
- [x] A new desktop design; an Android app ([plan](docs/desktop-plan.md) §12).
- [x] One library for every app: WhatsApp (Android backups, iPhone backups, exports), Telegram, SMS, LINE, Viber
  ([plan](docs/plan-all-apps.md)); exports including PDF.
- [ ] Confirm each adapter against real exports ([docs/adapters](docs/adapters/README.md)); Signal and iMessage.
- [ ] Signed installers (macOS notarization, Windows).

## Contributing

Issues and pull requests are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first: **never attach a real
backup, key, media or phone number**; tests use the synthetic backups of the `fixtures` feature. Design records are
in [docs/decisions](docs/decisions/README.md), vulnerabilities go through [SECURITY.md](SECURITY.md), and everyone
follows the [Code of Conduct](CODE_OF_CONDUCT.md).

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Licence

[GNU Affero General Public License v3.0 or later](LICENSE) (`AGPL-3.0-or-later`).

---

<sub>oopz is not affiliated with, endorsed by or sponsored by WhatsApp or Meta. WhatsApp is a trademark of its
owner.</sub>
