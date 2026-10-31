# 0010: The first releases: unsigned installers on GitHub Releases

Status: accepted (2026-10-31)

The desktop app builds installers for Windows, macOS and Linux (`.github/workflows/desktop-build.yml`). Publishing
them needs a choice about code signing: unsigned, Windows shows a SmartScreen warning and macOS (no Developer ID, no
notarization) refuses to open the app until the person allows it in System Settings.

## Decision

1. **Unsigned for now.** No Apple Developer ID ($99 a year) or Windows certificate (Azure Trusted Signing or an OV
   certificate) yet: oopz has no users to weigh the cost against, and signing keys would be new secrets in CI. The
   README's "Install" says what each warning looks like and what to do.
2. **GitHub Releases**, made by the workflow from a pushed tag `vX.Y.Z` that must equal the app's version: the
   installers for the three systems, one `SHA256SUMS.txt`, build provenance (GitHub artifact attestations, so
   `gh attestation verify <file> --repo Sparklitt/oopz` shows it was built here from that tag), and the CHANGELOG's
   section as notes. The release is a **draft**: a person reads it and publishes it.
3. The command-line `oopz` isn't released as a binary yet (`cargo install --locked --path crates/oopz-cli`).

## Revisit when

People report the warnings stop them, or someone else packages oopz; then sign (macOS first: its warning is the
hardest to get past) and record the keys' home and rotation here.
