# WhatsApp on iPhone (`whatsapp-ios`, `whatsapp-business-ios`)

Decision [0018](../decisions/0018-iphone-backups.md). Reads WhatsApp's chats from the backup Finder or iTunes made of
the person's own iPhone on their own computer. An encrypted backup asks for the backup password chosen when
"Encrypt local backup" was turned on; an unencrypted one asks for nothing.

## How a person gets the backup

- **Mac**: Finder → the iPhone → "Back up all of the data on your iPhone to this Mac" → Back Up Now. The backup is
  in `~/Library/Application Support/MobileSync/Backup/<id>/`.
- **Windows**: Apple Devices (or iTunes) → Back Up Now. The backup is in
  `%APPDATA%\Apple Computer\MobileSync\Backup\<id>\` or `%USERPROFILE%\Apple\MobileSync\Backup\<id>\`.
- Pick that `<id>` folder (or its `Manifest.plist`) in oopz.

## What's read

| WhatsApp | Model |
|---|---|
| `ZWACHATSESSION` (`ZCONTACTJID`, `ZPARTNERNAME`, `ZSESSIONTYPE`, `ZREMOVED`) | conversations: `@g.us` group, `@newsletter` channel, `@broadcast` broadcast (`status@broadcast` named Status), else one-to-one; removed ones skipped |
| `ZWAGROUPMEMBER` (`ZMEMBERJID`, `ZCONTACTNAME`) | group members and senders, members who left included |
| `ContactsV2.sqlite` `ZWAADDRESSBOOKCONTACT`, then `ZWAPROFILEPUSHNAME`, then the chat's partner name | names |
| `ZWAMESSAGE.ZMESSAGEDATE` | seconds since 2001-01-01 UTC (negative is before 2001) |
| `ZMESSAGETYPE` | its kind from msgkinds' registry (`whatsapp_ios`, decision [0020](../decisions/0020-the-shared-registry.md)): text, link, photo, video, voice (`.opus`) or audio, document (`ZTITLE`), GIF, sticker, video note, contact card (`ZVCARDNAME`, `ZVCARDSTRING`), location (`ZLATITUDE`, `ZLONGITUDE`), view-once media (only that it was sent), poll (question only), call, album, business messages, deleted; bytes from `Message/Media/…` |
| `ZGROUPEVENTTYPE` of a type 6 or 10 | what happened (`whatsapp_ios_system`, `whatsapp_ios_notice`): group changes, encryption and security-code notices; a missed-call notice is a call |
| a type no row names | unknown (code kept), counted in the notes |
| `ZSTARRED` | starred |

## Not read yet (said in the notes)

Reactions, replies (quoted messages), edits and poll votes: WhatsApp keeps them in protocol buffers
(`ZWAMESSAGEINFO`, `ZMETADATA`) whose layout needs a real backup to confirm. Group events take their event from msgkinds' rows (reported by other readers, not yet seen in a backup of ours);
a call's duration and outcome live in `CallHistory.sqlite`, not read yet. Poll options are in a protocol buffer.

## Edge cases covered by tests

Plain and encrypted backups; no, empty and wrong passwords; a keybag cut short; a damaged or missing `Manifest.db`;
an iOS 9 backup (`Manifest.mbdb`); a file cut short or missing from the backup; media not in the backup; a type from
a future version; an empty text; a member who left; a message before 2001; WhatsApp Business beside WhatsApp; a
backup without WhatsApp; `%` and `_` in paths; file names on the library side cleaned of `/`, `\` and leading dots.

## Needs a real backup to confirm

The type codes beyond 0–8 and the group event codes (msgkinds marks them "reported"), whether `ZMEDIALOCALPATH` is always relative, and the
protocol-buffer fields for replies and reactions. Test data: `oopz-gen --ios OUT [--ios-password PW]`.
