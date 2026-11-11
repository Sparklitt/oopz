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

### Part B, Windows 11 (NSIS, per-user), 2026-11-10

The published 0.1.0 installer from GitHub, installed with `/S`; the app's own update check against the real
`latest.json` found 0.1.1, the install (as the Install button calls it) replaced it and relaunched it as 0.1.1 (the app
and the uninstall entry say 0.1.1), a setting set before (theme) was kept, the Start-menu entry stayed, and a check
afterwards found nothing newer. Before publishing: every updater file's `.sig` verified against the public key, the 21
checksums matched, `latest.json` had all 11 platform keys with each file's own signature, and the APK carried the oopz
release certificate. Then uninstalled.

### Part B, Linux (AppImage, .deb, .rpm), 2026-11-11

The published 0.1.0 packages from GitHub (checksums matched `SHA256SUMS.txt`), each updated by the app itself against
the real `latest.json`. Driven through WebDriver (`tauri-driver`): language set to Spanish and the auto-check switch on,
an invented backup opened (so there is a decrypted copy), then "Buscar actualizaciones" and "Instalar y reiniciar"
clicked. The decrypted folder was listed every second around the restart.

| Install | Where | Result |
|---|---|---|
| AppImage | WSLg, Ubuntu 24.04 | pass: the file replaced in place by 0.1.1 (its hash is the published one), relaunched as 0.1.1, the decrypted copy gone, settings kept (Spanish, the switch), "newest" afterwards |
| .deb | Ubuntu 24.04 container on WSLg, a normal user with sudo | pass: the updater asked for the password, `dpkg -i` ran, `dpkg-query` says 0.1.1, relaunched as 0.1.1, the decrypted copy gone at the restart, settings kept, menu entry kept, "newest" afterwards |
| .rpm | Fedora 42 container on WSLg, a normal user in `wheel` | pass: as the .deb, `rpm -q` says `oopz-0.1.1-1` |

Notes:
- The .deb and .rpm updates install as root: the updater tries `pkexec` first, then a zenity/kdialog password prompt
  piped to `sudo -S`, then plain `sudo`. The containers had no polkit agent, so the zenity prompt was the one tested; a
  desktop with polkit shows its own dialog first. Without a GPU in the container the zenity window drew blank, though
  it took the typed password.
- AppImage: the relaunched 0.1.1 inherits 0.1.0's AppImage environment (`LD_LIBRARY_PATH`, `XDG_DATA_DIRS` with the
  old mount after its own), so the old runtime and its mount (the old file, already deleted) stay until 0.1.1 closes.
  0.1.1's own paths come first; nothing seen goes wrong. Upstream behaviour of the updater's restart.

### Part B, Windows 11 (MSI, en-US), 2026-11-11

The published 0.1.0 MSI installed with `/passive` (UAC), driven over WebView2's DevTools port: Spanish and the switch
on, a backup opened, "Instalar y reiniciar" clicked, UAC accepted. The window closed in 3 s, msiexec registered 0.1.1
9 s later and relaunched the app (the exe's version 0.1.1); the decrypted copy in
`%LOCALAPPDATA%\com.sparklitt.oopz\decrypted` gone; settings kept (Spanish, the switch); Start-menu entry kept;
"newest" afterwards. Then uninstalled with `msiexec /x` (exe and entry gone).

Found: the MSI, which installs for all users, went into `%LOCALAPPDATA%\oopz` rather than Program Files. An earlier
NSIS install leaves `HKCU\Software\sparklitt\oopz` (with `InstallDir`) after its uninstall, and Tauri's MSI reuses a
directory found there. So an MSI after an NSIS install-and-uninstall lives in one user's folder; the key also outlives
the MSI's uninstall.

### Part B, macOS

Not run: needs a Mac.

