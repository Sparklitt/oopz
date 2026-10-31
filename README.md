# oops

Your own WhatsApp backups, from a terminal: check them, look inside them (structure only), and restore a lost chat.
Works with WhatsApp's end-to-end encrypted Android backups (`*.crypt15`).

> **Your own backups, with your own key, with consent.** oops opens a backup only with the 64-digit key its owner
> chose when they turned on end-to-end encrypted backups. It never tries to get around WhatsApp's encryption of
> anyone's account. To use someone else's backup (for `oops restore`), they give you their backup and type their own
> key, knowing what you will do with it.

oops is not affiliated with, endorsed by or sponsored by WhatsApp or Meta. WhatsApp is a trademark of its owner.

## Privacy

- **Offline.** oops makes no network connections.
- **Counts and masked ids only.** Its output never contains message text, media, phone numbers or keys.
- **Originals are never modified.** Backups and `Media/` folders are only read; every result is a new file or folder,
  and oops refuses to overwrite an existing one.
- **Keys** are read from a hidden prompt (or one line of stdin per key, `oops … < keyfile`), never from an argument
  or an environment variable, and wiped from memory after use.

## Install

You need Rust 1.89 or newer ([rustup.rs](https://rustup.rs)) and a C compiler (SQLite is built from source).

```sh
cargo install --locked --path crates/oops-cli   # installs the `oops` binary
# or build without installing:
cargo build --release                           # target/release/oops
```

## Usage

Copy your backup off the phone first (WhatsApp keeps it in `Android/media/com.whatsapp/WhatsApp/Databases/`) and work
on the copy. Every command that opens a backup asks for its 64-digit key.

| Command | What it does |
|---|---|
| `oops backups --dir <folder>` | Checks every `*.crypt15` in a folder with one key: does it open, is its database sound, how many chats and messages, the newest message's date. Fails if any backup doesn't pass (another key, damaged, cut off). |
| `oops chats --backup <file>` | Lists the chats in a backup: number, masked id, messages, last message's date. |
| `oops types --backup <file> --chat N` | For each WhatsApp message type in chat N: counts and which tables its rows link to. No content. |
| `oops tables --backup <file>` | Every table with its columns and row count. Structure only, no values. |
| `oops chat-facts --backup <file> --chat N` | How WhatsApp keeps one chat: ordering ids, senders, statuses, pointers. Counts and ranges only; useful to compare two phones. |
| `oops restore --source <file> --source-chat N --target <file> --target-chat M --out <new file> [--media <folder> --media-out <new folder>]` | Restores a lost one-to-one chat (see below). |

`oops --help` and `oops <command> --help` describe every flag.

### Restoring a lost chat

Two people share a one-to-one chat; one of them lost it (or part of it), the other still has it. `oops restore` adds
the messages of the chat in the **source** backup (the one that has them) that the same chat in the **target** backup
(the person who lost it) lacks, mirrored to the target owner's side, and writes a new, checked backup for the target's
owner. It asks for the source's key, then the target's.

- Nothing the target has is changed or lost; added messages are placed before the target's own, in order.
- Before writing, the result is decrypted again and checked (database integrity, counts, ordering, references, every
  other chat identical). If any check fails, nothing is written.
- With `--media` (a copy of the source phone's `Media/` folder) and `--media-out`, the files the result needs that the
  target lacks are copied out, laid out like the target's `Media/` folder.
- Afterwards, run `oops chat-facts` on the result.

**Risks, read before restoring on a phone.** This changes a WhatsApp backup, which WhatsApp's terms may not allow; the
account owner accepts that risk. It has been used for real restores only a few times, on a few phones and WhatsApp
versions. Keep the original backup and move the phone's `WhatsApp/` folder aside before clearing WhatsApp's data, so
you can always go back. The design and its checks are in
[docs/decisions/0006](docs/decisions/0006-merge-a-chat-back-into-whatsapp.md).

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests run on synthetic backups built by the `fixtures` feature of the `oops` crate (invented people, numbers and
messages, encrypted with a test key). See [CONTRIBUTING.md](CONTRIBUTING.md); design records are in
[docs/decisions](docs/decisions/README.md).

## Licence

[GNU Affero General Public License v3.0 or later](LICENSE) (`AGPL-3.0-or-later`).
