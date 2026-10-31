# 0011: An opt-in update check, signed updates

Status: accepted (2026-10-31)

The desktop app is installed from GitHub Releases ([0010](0010-unsigned-installers-on-github-releases.md)). Without an
updater, people keep whatever version they first installed, fixes included. An updater goes online, and oopz promises
that nothing leaves the computer, so the check has to be the person's choice.

## Decision

1. **Off by default.** The Welcome screen has an "Updates" section: a switch "Check for updates when oopz starts"
   (off, remembered on this computer) and a "Check for updates" button. Nothing goes online unless one of them is
   used. The check is one HTTPS request to GitHub for
   `https://github.com/Sparklitt/oopz/releases/latest/download/latest.json`: GitHub sees the computer's address and
   oopz's version, nothing else. Backups, keys and results never leave the computer; Welcome rule 4 and the README
   say so.
2. **Installed only on a click**, "Install and restart". `tauri-plugin-updater` downloads the update and refuses it
   unless its minisign signature matches the public key in `tauri.conf.json`; this is separate from OS code signing,
   which 0010 leaves for later.
3. **The webview gets no updater permission.** Two commands in `src-tauri/src/lib.rs` (`update_check`,
   `update_install`) do the work in Rust. Before the Windows installer runs (it quits the app without its exit
   event) they wipe what this run decrypted, as a normal exit does; on macOS and Linux the restart runs the exit
   event.
4. **The key.** Made with `pnpm tauri signer generate`, password-protected. The private key and its password are the
   CI secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` and are kept in the maintainer's
   password manager. **If the key is lost, installed copies can never update again**: a new key means people
   download a new installer by hand.
5. **The release workflow** passes the secrets to `tauri build`, publishes each updater file and its `.sig` with the
   installers, and writes `latest.json` (version, CHANGELOG notes, and one entry per system and installer kind).
   The release stays a draft until a person publishes it, so no installed copy sees an update before then.

## Revisit when

The project moves off GitHub Releases, the key has to be rotated (ship a release that trusts both keys first), or
people ask for automatic installs.
