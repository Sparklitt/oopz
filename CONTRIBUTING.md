# Contributing

Thanks for helping. A few rules come first, because oopz works on people's private messages.

## Synthetic data only

- **Never attach a real backup, key, `Media/` file, screenshot of a chat or phone number** to an issue, pull request,
  test or commit, yours or anyone else's. Describe what you saw instead: the WhatsApp version, the command, oopz's
  output (it is counts and masked ids by design) and, if it helps, the table and column names from `oopz tables`.
- Tests use the synthetic backups in `crates/oopz/src/fixture.rs` (the `fixtures` feature): invented people, numbers
  such as `999900000001`, invented messages, encrypted with a test key. Extend them to reproduce a new case.
- For a backup to click through or to time, `oopz-gen` writes one of any size, phone-shaped (`Databases/`,
  `Media/`, `key.txt`, `manifest.txt` with the counts): `cargo run --release -p oopz-gen -- OUT --preset P`, where P
  is `small` (thousands of messages), `million` (one chat of a million, about 11 s), `stress` (thousands of chats) or
  `edge` (every kind of message and the odd rows that break readers). `--help` lists the rest: seed, sizes, years,
  `--only text,voice`, `--media none`, `--keep-plain`. The same seed gives the same backup.
- Output, errors and logs never contain message text, media, numbers or keys. A change that prints one won't be merged.

## The rules of the code

- **Your own backups, with your own key, with consent.** No feature that opens a backup without its owner's key or
  gets around WhatsApp's encryption.
- **Originals are only read**; every result is a new file, never an overwrite.
- **No home-made cryptography.** Only the crates in [docs/decisions/0004](docs/decisions/0004-whatsapp-backup-decryption.md)
  and [0006](docs/decisions/0006-merge-a-chat-back-into-whatsapp.md); a new primitive needs a new decision record.
- `unsafe` is forbidden.

## Checks

Before opening a pull request:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same on Linux, macOS and Windows.

## Licence

oopz is licensed under AGPL-3.0-or-later. By contributing you agree that your contribution is licensed under the same
terms.
