# oopz user guide

[Español](es.md)

oopz opens your own chat history on your own computer or phone: WhatsApp backups, and the exports other apps make.
You can read it, search it, export it, and bring a lost WhatsApp chat back. Nothing leaves your device.

Every picture in this guide shows invented people and numbers (Ana Prueba, Ben Ejemplo, `+9999 555 0187`). They are
made from the app itself, so they change when the app does.

- [What oopz opens, and what it never will](#what-oopz-opens-and-what-it-never-will)
- [Get the backup off the phone](#get-the-backup-off-the-phone)
- [Your 64-digit key](#your-64-digit-key)
- [Open a backup](#open-a-backup)
- [Read, search and insights](#read-search-and-insights)
- [The library: every app in one place](#the-library-every-app-in-one-place)
- [Export](#export)
- [Restore a lost chat](#restore-a-lost-chat)
- [Settings, privacy and updates](#settings-privacy-and-updates)
- [The command line](#the-command-line)
- [Troubleshooting](#troubleshooting)

## What oopz opens, and what it never will

| You have | What oopz needs |
|---|---|
| WhatsApp on Android | the end-to-end encrypted backup, `msgstore.db.crypt15`, and its 64-digit key |
| WhatsApp on iPhone | the backup Finder, Apple Devices or iTunes made on your computer, and its password if it is encrypted |
| WhatsApp, any phone | a chat's **Export chat** (`.txt` or `.zip`) |
| Telegram | Telegram Desktop's export in JSON (`result.json`) |
| SMS and calls (Android) | the `.xml` files of the SMS Backup & Restore app |
| LINE | **Export chat history** (`.txt`) |
| Viber | **Export message history** (a zip of CSV files) |

oopz never opens:

- **Google Drive or iCloud backups.** Only WhatsApp can read them; oopz doesn't try. Use the backup on the phone
  instead.
- **Older WhatsApp backups, `crypt12` and `crypt14`.** Their key lives in a file only a rooted phone can read. oopz
  lists them, and says so.
- **A backup whose key is lost.** Without the key nobody can open a crypt15 backup, and oopz will never try to guess
  it.
- **Someone else's backup without them.** oopz is for your own data. To use another person's backup (to restore a
  chat), they give it to you and type their own key.

## Get the backup off the phone

On Android, WhatsApp keeps its backups in
`Android/media/com.whatsapp/WhatsApp/Databases/` (older phones: `WhatsApp/Databases/`). The newest is
`msgstore.db.crypt15`; the others, `msgstore-YYYY-MM-DD.1.db.crypt15`, are the last few days. Copy the whole
`Databases` folder if you can. To see names instead of numbers, copy `wa.db.crypt15` too, if it is there.

First make a fresh backup: WhatsApp › Settings › Chats › Chat backup › **Back up**.

Then pick whichever way suits you.

### With a cable

Plug the phone into the computer and choose **File transfer** on the phone. On Windows and Linux, the phone appears
as a drive; open the folder above and copy `Databases` to the computer. On a Mac, use Android File Transfer or
OpenMTP. If the folder looks empty, unplug and plug again, and unlock the phone.

### SD card or USB stick

Copy the `Databases` folder to an SD card or a USB stick (with an OTG adapter) in the phone's Files app, then plug it
into the computer.

### Quick Share, or a cloud folder

Share the files from the phone's Files app with Quick Share (Nearby Share), or put them in a cloud folder you use
(Google Drive as a folder, Dropbox, OneDrive) and download them on the computer. The backup is encrypted, so this is
safe enough; photos and videos are not encrypted, so keep those on a cable.

### Over Wi-Fi with a QR code

In oopz on the computer: **Receive from phone** (or **Add backup › Over Wi-Fi with a QR code**), choose where the
files go, and **Show the QR code**. Scan it with the phone's camera: a page opens in the phone's browser; pick the
backup files there. They arrive in a new folder, and oopz offers to open them. The page lives only on your Wi-Fi, for
a few minutes, and only while this screen is open.

![Receive from phone, before the QR code](images/en/receive.png)

![The QR code, waiting for the phone](images/en/receive-qr.png)

The page works with no internet at all: the computer and the phone only need to share a network, even a router
with no internet. With no Wi-Fi around, switch on the phone's hotspot and join the computer to it (no mobile data is
used). If the computer has several network cards, pick the Wi-Fi one under **Address**; the ones marked "virtual?"
(WSL, Docker, VPN) rarely work. Once the phone loads the page, oopz says **The phone opened the page**.

If the phone can't open the page:

- Both must be on the **same Wi-Fi**. Switch off mobile data on the phone for a moment, so it doesn't try that way.
- **Guest networks** and many public or office networks keep devices apart ("client isolation"). Use your home
  Wi-Fi, or the phone's hotspot with the computer joined to it.
- A **firewall** may block it. On Windows, allow oopz on **private** networks when asked (or in Windows Security ›
  Firewall); make sure your home network is set as private. On macOS, allow incoming connections for oopz.
- A **VPN** on either device can hide the other one. Pause it.
- Type the address shown under the code into the phone's browser by hand, exactly, `http://` included.
- Still nothing? Use a cable, or adb below.

After 45 seconds without the phone, the same tips show on the screen:

![The Wi-Fi tips, when the phone hasn't opened the page](images/en/receive-tips.png)

### Copy with adb (USB or wireless debugging)

adb is Android's own tool for developers; it copies files without any app on the phone. It works when the phone
doesn't show up as a drive, or the Wi-Fi page won't connect.

1. On the phone: Settings › About phone, tap **Build number** seven times. Then Settings › Developer options, switch
   on **USB debugging** (or **Wireless debugging**, Android 11 and newer, same Wi-Fi as the computer).
2. In oopz: **Add backup › With USB or wireless debugging (adb)**.
3. The first time, if this computer has no adb, oopz offers to get it: **Show Google's licence** shows the version,
   the size and Google's licence; **Accept and download** fetches Android's platform-tools from Google (once, about
   8–17 MB), checks the file against Google's own checksum, and keeps it in oopz's folder. Nothing is downloaded
   before you accept. **Settings › General › Android's adb (platform-tools) › Remove** deletes it again.
4. Connect the phone. With a cable: plug it in and tap **Allow** on the phone. Wirelessly: open **Wireless
   debugging** in oopz, tap **Pair device with pairing code** on the phone, type the address, port and code it shows,
   then the address and port under "IP address & port", and **Connect**.
5. Pick the phone, tick **Media** if you want photos and videos too, and **Choose where to copy**. oopz copies
   WhatsApp's folder into a new folder, checks every file's size, keeps nothing half-copied, and offers **Use the
   copied files**.

![No adb yet: Google's licence, before anything is downloaded](images/en/adb-get.png)

![Copying with adb: the phones adb sees](images/en/adb-copy.png)

On the command line: `oopz pull --out-dir FOLDER` (add `--media`, `--business`, or `--pair HOST:PORT --code CODE`
and `--connect HOST:PORT` for wireless debugging). It uses the adb oopz downloaded if there's no other.

Without oopz, the same copy by hand:

```sh
adb pull /sdcard/Android/media/com.whatsapp/WhatsApp/Databases/ Databases/
```

Switch USB debugging off again when you are done.

### On the phone itself: the Android app

oopz also runs on Android (7.0 and newer). There, **Choose a backup file** opens the phone's file picker: go to
`Android/media/com.whatsapp/WhatsApp/Databases/` and pick `msgstore.db.crypt15`. oopz copies it into its private
storage, opens the copy, and deletes the copy when it closes. It asks for no storage permission. Results (an export,
a restored backup) are saved where you choose.

<p>
  <img alt="The Android app: Add a backup" src="images/en/phone-add.png" width="200">
  <img alt="The Android app: the library" src="images/en/phone-library.png" width="200">
  <img alt="The Android app: chats from every app" src="images/en/phone-library-chats.png" width="200">
  <img alt="The Android app: Tools" src="images/en/phone-tools.png" width="200">
</p>

### iPhone

oopz reads WhatsApp from the backup your computer makes of your iPhone (no iCloud, no jailbreak).

- **Mac**: Finder › your iPhone › **Back up all of the data on your iPhone to this Mac** › **Back Up Now**.
- **Windows**: Apple Devices (or iTunes) › **Back Up Now**.

If you tick **Encrypt local backup**, you choose a password; oopz asks for it. Then add the backup to the library
(**Library › Sources › Add a source**) and pick its folder:

- Mac: `~/Library/Application Support/MobileSync/Backup/<id>/`
- Windows: `%APPDATA%\Apple Computer\MobileSync\Backup\<id>\` or `%USERPROFILE%\Apple\MobileSync\Backup\<id>\`

### Other apps' exports

- **WhatsApp** (any phone): open a chat › More › **Export chat**, with or without media. One chat at a time.
- **Telegram**: in Telegram Desktop (Windows, macOS, Linux), Settings › Advanced › **Export Telegram data**, or a
  chat's menu › **Export chat history**; format **Machine-readable JSON**. The phone apps can't export.
- **SMS and calls**: the SMS Backup & Restore app on Android makes `sms-….xml` and `calls-….xml`.
- **LINE**: a chat › Settings › **Export chat history**.
- **Viber**: Settings › Calls and Messages › **Export message history**.

Send the file to the computer any way above, or open it in the Android app.

## Your 64-digit key

A crypt15 backup opens only with its key: 64 digits and letters (0-9, a-f).

WhatsApp shows it once, when you turn on encrypted backups: WhatsApp › Settings › Chats › Chat backup ›
**End-to-end encrypted backup** › Turn on › **Use 64-digit encryption key instead**. Write it down or save it in a
password manager then. WhatsApp won't show it again.

- If you chose a **password** instead of the key, oopz can't open the backup: the password only works through
  WhatsApp's servers. To get a key, turn encrypted backup off and on again with the 64-digit key, then make a new
  backup. That only helps from now on.
- If the key is **lost**, nothing can open the backups made with it, oopz included. Turn on a new key for future
  backups.
- oopz uses the key once to open the backup, then wipes it from memory. It is never saved or sent.

## Open a backup

The first time, oopz asks you to confirm that these are your own backups.

![The welcome screen](images/en/welcome.png)

**Add backup**, then **Choose a folder** (the copied `Databases` folder) or **Choose a backup file**. You can also
drop the file on the window.

![Add a backup](images/en/add-backup.png)

oopz lists every backup in the folder. Older formats are shown but can't be opened. Pick one.

![The backups found in a folder](images/en/found.png)

Type or paste the key; spaces don't matter. Tick **Also add it to my library** to read it with your other apps too.

![The key](images/en/unlock.png)

oopz checks the backup while it opens it: does it decrypt, is its database sound. The overview shows what it holds.

![A backup's overview](images/en/overview.png)

**Close** forgets it; the decrypted copy is deleted. **Diagnostics** shows a backup's tables and checks, with counts
only, for a bug report.

![Diagnostics](images/en/diagnostics.png)

## Read, search and insights

**Chats** lists every chat; open one to read it, with replies, reactions, polls, calls and edits. **Go to a date**
jumps through long chats. To see photos, choose the phone's copied `Media` folder (**Show photos**); nothing is
copied.

![Reading a chat](images/en/chats.png)

**Search** (or Ctrl K, ⌘K on a Mac) finds words in every chat. Accents and case don't matter: `cafe` finds `café`.
Narrow it by chat, type, sender or dates.

![Search](images/en/search.png)

**Insights** shows messages by month, weekday and hour, and who writes most.

![Insights](images/en/insights.png)

oopz also follows your system's dark mode, or you choose one in Settings.

![Reading a chat in the dark theme](images/en/chats-dark.png)

## The library: every app in one place

The library holds what you add from every app: WhatsApp (Android and iPhone), Telegram, SMS, LINE, Viber. Search
them together, read a timeline across apps, and link the same person across apps.

**Library › Sources › Add a source**, then pick a file or a folder; oopz finds the app. A WhatsApp **Export chat**
doesn't say which person is you: oopz asks once (**Which one is you?**), so your messages show as sent.

![The library's sources](images/en/library.png)

![Adding a source](images/en/library-add.png)

- **Chats**: every conversation from every app, with filters by app.
- **Search**: across everything.
- **Timeline**: the messages of a filter from every app, in time order.
- **Insights**: the same charts as for a backup, over everything.
- **Contacts**: the same person in different apps, linked into one contact. oopz suggests links when numbers or
  emails match, and never links on its own. You can import and export vCards.
- **Select**: what to export. A filter, plus messages you tick or untick by hand. Save it under a name.

![Chats from every app](images/en/library-chats.png)

![The timeline](images/en/timeline.png)

![Contacts](images/en/contacts.png)

![Select](images/en/select.png)

The library is deleted when oopz closes, unless you switch on **Keep my library on this device** in Settings.

## Export

From **Select** (or a backup's **Export…**), choose a format:

| Format | What you get |
|---|---|
| Web page (HTML) | one file, opens in any browser, prints well |
| PDF | pages to print or keep, with emoji and right-to-left text; optional cover, contents and small photos |
| CSV | one row per message, for spreadsheets |
| JSON | everything, for other programs |
| Text | plain text |
| vCard | the people in it, as contacts |
| Media files | the photos, audio and files, in a zip |

![Exporting a selection](images/en/export-library.png)

![Exporting a backup](images/en/export-backup.png)

An export is a new file; oopz never writes over another one. **It isn't encrypted**: keep it somewhere safe.

## Restore a lost chat

Two people share a one-to-one chat. One lost it (or part of it); the other still has it. oopz takes the messages from
the backup that has them and adds them to a **new copy** of the other person's backup, mirrored to their side.

1. Open both backups (each with its own owner's key).
2. **Restore a chat**: choose the backup that still has the chat, and the one that lost it; pick the chat on each
   side. **Compare: what's missing?** counts what would be added.
3. Type the key of the backup that lost it, choose a folder for the result, and optionally the other phone's `Media`
   folder to bring photos too.
4. **Merge into a new copy**. Before anything is written, oopz decrypts the result again and checks it (database
   integrity, counts, order, every other chat unchanged). If a check fails, nothing is written.
5. **Put it on the phone** walks you through it: move WhatsApp's folder aside, clear WhatsApp's data, put the result
   in `Databases/`, and restore from it in WhatsApp.

![Restore a chat](images/en/restore.png)

oopz refuses, and says why, when:

- the chat is a **group** (only one-to-one chats for now);
- both sides are the **same backup**, or the chat doesn't exist in the target;
- the messages both backups have don't look **mirrored** (it's the wrong chat, or the wrong person's backup);
- the target's WhatsApp has **tables or columns oopz can't fill** from the other backup (a newer WhatsApp): it names
  them;
- the key of the target doesn't open it, or the result folder already exists.

The originals are never changed. Changing a WhatsApp backup may go against WhatsApp's terms: the account owner
decides and takes that risk. Keep the original backup and the moved-aside folder until you are sure.

## Settings, privacy and updates

![Settings](images/en/settings.png)

- **Language** and **theme**; **Hide names** shows chats by kind and the end of their number, for sharing your screen.
- **Privacy**: **Keep my library on this device**, and **Wipe now**, which deletes every decrypted copy and the
  library at once.
- oopz has no account, no cloud and no telemetry. Backups, keys and messages stay on your device. The only network
  use is the Wi-Fi page for your phone (while it is open) and the update check.
- **Updates** are off until you switch on **Check for updates when oopz starts** or click **Check for updates**. The
  check asks GitHub for the newest version; GitHub sees your IP address, nothing else. Updates install only when you
  click, and only if their signature matches. On Android, install the new APK from the
  [releases page](https://github.com/Sparklitt/oopz/releases).

![Settings: privacy](images/en/settings-privacy.png)

![Settings: updates](images/en/settings-updates.png)

## The command line

The `oopz` command does what the app does, in a terminal; it prints counts, dates and masked ids, never message
text. The installers put it on your `PATH`; with the AppImage or on macOS, use **Settings › Install the oopz command**.

![Settings › General, with the oopz command row (AppImage)](images/en/settings.png)

```sh
oopz backups --dir Databases/          # do they all open, and are they sound?
oopz chats --backup Databases/msgstore.db.crypt15
oopz lib import result.json --library my-library/
```

`oopz --help` lists every command; the [README](../../README.md#usage) has the table.

## Troubleshooting

| What you see | What to do |
|---|---|
| "This key doesn't open this backup" | Check each character. If it's right, the file may be cut short: copy it again and compare its size with the phone's. |
| "older crypt14 backup" | Its key file can only be read on a rooted phone. Turn on end-to-end encrypted backup with the 64-digit key, then make a new backup. |
| "isn't a WhatsApp crypt15 backup, or it's incomplete" | The copy stopped early, or it's another file. Copy it again. |
| "its database is damaged" | Try an older backup, `msgstore-YYYY-MM-DD.1.db.crypt15`. |
| Names show as numbers | Copy `wa.db.crypt15` from the same folder too, if your phone has it. |
| No backup in the folder | WhatsApp may keep only a Google Drive backup. Make a local one: Settings › Chats › Chat backup › Back up. |
| The phone can't open the Wi-Fi page | See [the Wi-Fi tips](#over-wi-fi-with-a-qr-code), or use a cable or adb. |
| "There isn't enough free space" | Free some space, or choose another folder. Nothing was written. |
| Windows says "Windows protected your PC" | The installer isn't signed yet: **More info › Run anyway**. |
| macOS says it can't check oopz | System Settings › Privacy & Security › **Open Anyway**. |

Something else? [Open an issue](https://github.com/Sparklitt/oopz/issues), with what you did and what oopz said. Never
attach a backup, a key, messages or phone numbers.

<sub>The emoji in the pictures are [Twemoji](https://github.com/twitter/twemoji) (CC-BY 4.0).</sub>
