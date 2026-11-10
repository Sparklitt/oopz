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

## Results

### Part A, Linux (AppImage, WSLg, Ubuntu 24.04), 2026-11-10

Test builds 0.1.0 and 0.1.1 from `db2451d` with the local-server overlay, signed with the updater key; driven through
WebDriver (`tauri-driver`), the commands called as the UI calls them and the screens read back.

| Case | Result |
|---|---|
| Tampered file | pass: "The signature verification failed"; AppImage unchanged; app kept running |
| Signature from another key | pass: "The signature was created with a different key than the one provided" |
| Empty `.sig` | pass: "Invalid encoding in minisign data" |
| Same version / older (0.1.0, 0.0.9) | pass: no update; Settings shows "You have the newest oopz." |
| No entry for this system | pass: a clear error naming the missing `linux-x86_64` keys; no crash |
| Malformed `latest.json` | pass: "error decoding response body …", shown in Settings |
| Server down | pass: error in 0.13 s; the app keeps working |
| 404 | pass: "Could not fetch a valid release JSON from the remote", shown in Settings |
| Interrupted download (3 MB of 80, then hung up) | pass: "error decoding response body"; AppImage unchanged; a second try in the same run installed |
| Auto-check on, offline at start | pass: window up at once, no popup; the error waits in Settings |
| The good update | pass: progress reported, AppImage replaced by 0.1.1 (hash checked), relaunched as 0.1.1, decrypted copy gone from `~/.cache/com.sparklitt.oopz`, settings kept (language, theme, the switch) |

Found: the release notes are shown as raw Markdown (`**0.1.1**`); they're rendered since (see the changelog).

### Part A, Windows 11 (NSIS, per-user), 2026-11-10

Test builds 0.1.0 and 0.1.1 from `f529bc2` with the local-server overlay, cross-built on Linux (`pnpm tauri build
--runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis`; clang, lld and NSIS unpacked from Ubuntu's packages,
no install) and signed with the updater key. 0.1.0 installed with `/S` into `%LOCALAPPDATA%\oopz`. Most cases driven
through WebDriver (tauri-driver built for Windows, msedgedriver matching WebView2 157); WebDriver gives WebView2 a
throwaway profile, so the cases that need settings to persist (auto-check at start, the good update) ran on the app
started normally, driven over WebView2's DevTools port (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`).

| Case | Result |
|---|---|
| Tampered file | pass: "The signature verification failed"; the installed exe unchanged (hash) |
| Signature from another key | pass: "The signature was created with a different key than the one provided" |
| Empty `.sig` | pass: "Invalid encoding in minisign data" |
| Same version / older (0.1.0, 0.0.9) | pass: no update found |
| No entry for this system | pass: a clear error naming `windows-x86_64-nsis` and `windows-x86_64`; no crash |
| Malformed `latest.json` | pass: "error decoding response body …", shown in Settings; the app kept running |
| Server down | pass: a clear error in about 2 s (Windows retries a refused local connection; Linux 0.13 s) |
| 404 | pass: "Could not fetch a valid release JSON from the remote" |
| Interrupted download (3 MB of 5.4, then hung up) | pass: "error decoding response body"; the exe unchanged; the next install (a later run) worked |
| Auto-check on, offline at start | pass: page up in 1.3 s, no popup; the error waits in Settings → Updates |
| The good update (twice) | pass: installed passively and relaunched from the same per-user path as 0.1.1 (the app and the uninstall entry say 0.1.1); the decrypted copy gone from `%LOCALAPPDATA%\com.sparklitt.oopz\decrypted`; settings kept (consent, the switch, light theme, language); Start-menu entry kept; "newest" afterwards; the uninstaller then removed it |

Seen once and not repeated: after the first good update the saved language was English (the system's is Spanish). A
second downgrade-and-update kept Spanish with nothing saved, so it most likely came from the test's own scripted
clicks; watch for it in part B.

### Part A, macOS

Not run: needs a Mac.
