# 0012: The Android app

Status: accepted (2026-11-10)

oopz's desktop app is Tauri 2 over `oopz-app` (0009). Most people's backups are on an Android phone, often an old one,
so the same app runs there: the same Rust, the same UI, built for Android.

## Decision

1. **The same shell and UI**, built with `tauri android`. The Android project (`apps/desktop/src-tauri/gen/android`)
   is committed; only generated schemas are ignored. Desktop-only plugins (window state, updater) are left out of the
   Android build.
2. **Old phones first.** Android 7.0 (API 24, Tauri's floor) and up. The UI is built for the Chrome 67 WebView
   (`build.target`) with a few hand-written polyfills, because old phones often never update their WebView. The
   reference device is a Galaxy S9+ on Android 8.0 with WebView 67; every release is tried there.
3. **Files arrive as content references**, not paths (Android's file picker). oopz copies the picked backup into its
   private storage after checking the free space, opens the copy, and removes it on close, as desktop does with its
   decrypted copies. Results (a merged backup, an export) are written where the person picks, through the same
   picker. No storage permission is asked for.
4. **Signing.** Debug builds use the SDK's debug key. Release APKs are signed with an oopz release keystore made for
   this, password-protected, kept in the maintainer's password manager and the repo secrets
   (`ANDROID_KEYSTORE`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`). **If it is lost,
   installed copies can't be updated**: people would uninstall and install again. The keystore is never committed or
   printed.
5. **Distribution**: the signed APK on GitHub Releases with the other files (checksums, provenance). F-Droid later (it
   builds from source); Google Play only if chosen later (a one-off fee).
6. **No network** on Android beyond what desktop has: the Wi-Fi receive page doesn't apply (the backup is already on
   the phone); the update check is the store's or a new APK by hand.

## Revisit when

Tauri raises its Android floor, a phone without a usable WebView matters, or Play distribution is chosen.
