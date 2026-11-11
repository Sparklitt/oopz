# 0019: Android's platform-tools downloaded on request

Status: accepted (2026-11-11)

Copying a backup off a phone with adb (USB or wireless debugging) needs Google's adb. oopz doesn't ship it: Google's
platform-tools come with Google's own licence, which the person accepts when they download them. A tool written from
scratch would cover USB and already-paired phones only (wireless pairing is a SPAKE2 key exchange plus TLS with
Android's key formats), and on Windows a phone's adb interface is reachable only through the driver Google's adb
uses. Most people never need adb at all: a copied folder, the Wi-Fi page (local network only, no internet; a phone's
hotspot works) and the Android app cover them.

## Decision

1. **The adb option is always offered on desktop.** oopz uses adb on `PATH`, else in `$ANDROID_HOME` or
   `$ANDROID_SDK_ROOT`, else the copy it downloaded (`oopz_app::adb::downloaded_dir`: oopz's local data folder,
   `platform-tools/`). A system adb comes first, so two adb servers of different versions don't fight over port 5037.
2. **Nothing is downloaded without a click and the licence.** With no adb, the adb screen explains and offers "Show
   Google's licence": one HTTPS request to Google's package index (`dl.google.com/android/repository/
   repository2-3.xml`, as `sdkmanager` reads it). The screen shows the version, size and the licence text the index
   names for platform-tools; "Accept and download" fetches that archive from `dl.google.com` only.
3. **Checked before it's kept.** The archive's size and SHA-1 must be the index's; it's unpacked into a temporary
   folder beside the destination (names that leave the folder refused, executable bits kept) and swapped in only when
   complete. A failed download keeps nothing.
4. **Removed on request.** Settings shows the downloaded copy with "Remove" (it stops that copy's adb server first,
   which on Windows holds the files open). The NSIS uninstaller (per user) removes it too; the MSI, installed for all
   users, can't reach one person's folder, so there it's Settings' button or the uninstaller's "delete app data". The CLI (`oopz pull`) uses the
   downloaded copy when it's there but never downloads anything: the command line connects to nothing.
5. **The privacy text says so.** oopz goes online only for the update check and, if asked, this download; Google
   sees the computer's address, nothing else.

## Revisit when

Google's licence allows shipping adb, a pure-Rust adb with pairing becomes dependable, or Windows phones' adb
interface becomes reachable without Google's driver stack.
