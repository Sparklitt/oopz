# Update test: 0.1.0 → 0.1.1 (decision 0011)

Two parts. **A** runs on each OS against a local server, so a tampered or broken update never goes near the public
release. **B** is the real thing: 0.1.1 published on GitHub, installed over 0.1.0 by the app itself.

## A. Local: refusals and failures (each OS, before publishing 0.1.1)

1. Build a **test-only** 0.1.0 whose updater asks a local server, never published, by adding a config overlay to the
   usual build: `pnpm tauri build --config updater-test.json`, with
   `{"plugins": {"updater": {"endpoints": ["http://127.0.0.1:8765/latest.json"], "dangerousInsecureTransportProtocol": true}}}`.
   Signing as in CI (`TAURI_SIGNING_PRIVATE_KEY` from `~/.tauri/`; never print it).
2. Build 0.1.1's updater files the same way (version bumped), signed. Serve a folder with them and a `latest.json`
   written like the release job's (`python3 -m http.server 8765`).
3. In the test 0.1.0, open a backup (so there is a decrypted copy to wipe), then on Welcome, "Check for updates":

| Case | How | Expected |
|---|---|---|
| Tampered file | flip one byte of the update file, keep its `.sig` | download refused (signature), nothing installed, 0.1.0 still runs, the error is shown |
| Signature from another key | sign with a throwaway `tauri signer generate` key | refused |
| Missing or empty `.sig` in `latest.json` | `"signature": ""` | refused |
| Same or older version | `"version": "0.1.0"` / `"0.0.9"` | "You have the newest oopz" |
| No entry for this system | remove its platform key | "newest" or a clear error, never a crash |
| Malformed `latest.json` | truncate it | a clear error |
| Server down / offline | stop the server; also Wi-Fi off | a clear error, quickly; the app keeps working |
| 404 | rename `latest.json` | a clear error |
| Interrupted download | stop the server mid-download (serve a large file, throttled) | error; 0.1.0 untouched; a second try works |
| Auto-check switch on, offline at start | switch on, restart offline | no hang at start, no popup storm |
| The good update | correct file and `.sig` | progress moves, installs, restarts as 0.1.1; the decrypted copy is gone (`%LOCALAPPDATA%\com.sparklitt.oopz`, `~/.cache`, `~/Library/Caches`); settings (language, the switch) kept |

## B. Real: 0.1.1 on GitHub Releases

1. Install the published **0.1.0** on Windows (NSIS and MSI), macOS (DMG) and Linux (AppImage, .deb, .rpm where
   the updater supports them; `latest.json` lists what is).
2. Release 0.1.1 the usual way (tag, workflow, the draft checked, then published). Before publishing: every updater
   file's `.sig` verifies against the public key (as for 0.1.0), and `latest.json` has every platform key.
3. On each install: "Check for updates" finds 0.1.1, "Install and restart" ends on 0.1.1 (About/Welcome shows it),
   settings kept, nothing decrypted left behind. On Windows: the per-user install stays per-user; the Start-menu
   entry and uninstaller still work. On macOS: Gatekeeper's prompt after the update (unsigned, 0010) is noted.
4. Check again: "You have the newest oopz".

Record the results (OS, installer kind, pass/fail, the error text) in the release to-do before 0.1.2.
