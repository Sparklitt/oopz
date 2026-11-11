# What's missing, and the edge cases to handle

The working list of gaps (2026-11-11). Tick items off by moving them into a decision or a test; nothing here is
promised by date. The data types themselves live in [data-types.md](data-types.md).

## Getting the data in

- **App to app over the local network.** The oopz app on the phone scans the computer's QR code and sends the
  backup (and Media) itself; the browser page stays for phones without oopz. Both use the same one-time token and
  stay on the local network.
- **The browser page's look**: it still has the old style (navy and coral); match the app's design, wrap long paths,
  show the files picked and the progress per file, work in old browsers (Android 7's WebView 67), light and dark.
- **Media in the copy**: the adb copy and the Wi-Fi page should make Media easy (a size estimate before, then a
  resumable copy); without Media, stickers, photos and voice notes show as placeholders.
- **Contacts, from every source**: on Android every account (Google, Samsung, SIM, Exchange, phone-only) ends in the
  phone's one address book; read it over adb, with photos. Without adb: vCard 2.1/3.0/4.0 (Samsung Contacts, Google
  Contacts, iPhone, Outlook exports), Google CSV, Outlook CSV. Merge duplicates across sources by normalized phone
  number and e-mail, never silently. WhatsApp's own profile pictures live in its private storage: not readable
  without root, said so.
- **Names**: Android backups carry no contact names (no `wa.db` in Databases), so chats show the number; show the
  full number when there's no name and "Hide names" is off.
- **More sources**: hilo (an adapter on the same trait); Signal (its own encrypted backup with its passphrase);
  Messenger and Instagram (Meta's "Download your information" JSON/HTML); Google Messages (via SMS Backup &
  Restore today); Discord and Slack exports; e-mail (mbox) later.
- **iPhone without Finder/iTunes**: libimobiledevice-style backup over USB.

## Showing every type

- **Shared with hilo**: hilo (~/projects/hilo, Rust, its own importers: WhatsApp, Telegram 0028, Signal 0029) and oopz
  should share one crate for the data-type registry and the adapters, its own repository used by both (a git
  dependency), so a type or an app version is mapped once. First check both projects' licences, then decide where
  it lives and migrate one adapter as the pilot.

- **One registry of every data type**, per app and per app version: kind, parts, what's kept, how it's shown, how
  it's exported. Tests: every adapter's every row maps to an entry; every entry has a renderer and an export in each
  format; an unknown type shows a labelled card ("Telegram: dice roll, not shown yet"), never an error and never a
  gap; an unknown app version still opens (unknown columns ignored, missing optional ones leave details out).
- **Known gaps**: stickers show "0:00" (no duration for stickers) and a placeholder instead of the image; animated
  stickers (WebP, Lottie/TGS), view-once media, polls with votes, reactions per person, edits with history, pinned
  and starred, replies to media, forwarded-many-times, status updates, communities and channels, call links, group
  calls, disappearing-message changes, Telegram topics (`topic_created` shows as "Group created"), adapter notes in
  English in the Spanish UI.
- **Reuse hilo's** WhatsApp type and group mapping where it fits (hilo's decision 0034 is already cited).

## The library

- **Speed**: opening Chats and Insights from the library is slower than from one backup on real data; measure on a
  real-size library first (query plans, indexes, paging), then fix; a budget per screen in docs/performance.md.
- **One chat or separate, both**: for a linked contact, a "Show as one chat" switch in Chats (all apps' messages
  with that person merged by time, each marked with its app), next to the separate chats as today. A view only;
  writing one merged chat into a backup belongs to "Moving to another phone".
- **Linking contacts**: say plainly that linking only marks the same person across apps (nothing in any backup
  changes), show what will be linked and the result before, and allow unlinking.

## Moving to another phone

- **Fully flexible selection**: any contacts, groups, chats, date ranges, message types and kinds of media (photos,
  videos, voice notes, documents, stickers), with a size estimate; write a new backup for the new phone and the
  matching Media folder, never touching the originals. Groups are refused today (decision 0006): a new decision
  replaces that. WhatsApp's terms may not allow restoring a changed backup: the account owner decides and accepts it.
- **Across apps**: WhatsApp to WhatsApp first; others only where the target app has an import (Telegram imports
  WhatsApp exports).

## Edge cases

**Backups and files**
- Multi-GB databases, tens of GB of Media, 100k+ media files; free space checked before every copy, and again near
  the end; a disk filling mid-copy leaves nothing half-written.
- crypt12/crypt14 (named, not opened); half-copied or truncated files; a backup being written by WhatsApp while
  copied; two backups of the same day; WhatsApp and WhatsApp Business side by side; a dual-app/"Secure Folder"
  WhatsApp (other user profile on the phone).
- A newer WhatsApp schema; a much older one; a database from a beta; a backup from another phone number.
- The wrong key, a key with spaces or dashes, a password typed where the 64-digit key goes, a lost key (never
  guessed), a key file instead of the digits.
- Corrupt media files, zero-byte files, files whose type doesn't match their name, HEIC/HEIF, Opus voice notes,
  very long videos, giant images (memory), emoji and right-to-left names in file names.

**Paths and storage**
- Accents, spaces, emoji, very long names; Windows' 260-character limit; reserved names (`CON`, `NUL`); case-only
  differences; network drives; OneDrive/iCloud/Google Drive placeholders not downloaded; read-only folders; USB
  stick pulled mid-merge; FAT32's 4 GB file limit; exFAT on macOS; an external drive asleep.

**Phones and transfer**
- Wi-Fi: guest networks and client isolation, VPNs, several network cards, IPv6-only networks, captive portals, the
  phone sleeping or switching networks mid-transfer, the firewall prompt refused, a second phone opening the page.
- adb: unauthorized, offline, two phones, a phone on USB and wireless at once, an old adb server of another version,
  Samsung's USB driver missing on Windows, no udev rule on Linux, a phone locked mid-copy, Android 11+ scoped
  storage, a work profile.
- The Android app: old WebViews, very low storage, the app killed in the background mid-import, the file picker
  returning a cloud file, a content reference that expires.

**The app**
- Two oopz windows; closing during a merge, an import or a receive; a crash (decrypted copies wiped at the next
  start); sleep and resume; the clock changed; time zones and daylight-saving changes in message times.
- Display: 125–200% scaling, the 640×560 minimum window, dark mode and high contrast, keyboard only, screen readers
  (NVDA, VoiceOver, Orca, TalkBack), Spanish strings longer than English, right-to-left chats.
- Updates: offline, GitHub down, a bad signature, an interrupted download, corporate proxies; installs over older
  versions; per-user and per-machine on one PC (the stale NSIS key found in the update test); uninstall while
  running; Windows 10 without WebView2 and offline; Linux without webkit2gtk-4.1.

**Privacy and ownership**
- Everything a backup holds belongs to its owner, and oopz reads it all, but chats also hold other people's messages:
  exports and shares are the owner's responsibility. oopz never opens a backup without its key, and goes online only
  when asked (updates, Google's platform-tools).
