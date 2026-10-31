## What and why

<!-- What this changes and why. Link the issue it fixes, if any (Fixes #123). -->

## Checklist

- [ ] No real backup, key, media, message text or phone number anywhere: code, tests, fixtures, commits, screenshots.
- [ ] Output, errors and logs still print only counts and masked ids.
- [ ] Originals are still only read; results are new files, never overwrites.
- [ ] No new cryptographic crate or primitive (or a decision record in `docs/decisions/` for it).
- [ ] Tests on the synthetic fixtures cover the change.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] `CHANGELOG.md` updated under "Unreleased", if users will notice.
