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
| `ZMESSAGETYPE` 0 | text |
| 1, 2, 3, 8, 11, 15 | photo, video, voice (`.opus`) or audio, document (`ZTITLE`), GIF, sticker; bytes from `Message/Media/…` |
| 4 | contact card (`ZVCARDNAME`, `ZVCARDSTRING`) |
| 5 | location (`ZLATITUDE`, `ZLONGITUDE`) |
| 6 | system line |
| 7 | text with a link |
| 14 | deleted |
| anything else | unknown (code kept), counted in the notes |
| `ZSTARRED` | starred |

## Not read yet (said in the notes)

Reactions, replies (quoted messages), edits and poll votes: WhatsApp keeps them in protocol buffers
(`ZWAMESSAGEINFO`, `ZMETADATA`) whose layout needs a real backup to confirm. Group events are system lines without
wording (`ZGROUPEVENTTYPE` values unconfirmed). Calls (types seen as 59 / 66 in some versions) are kept as unknown.

## Edge cases covered by tests

Plain and encrypted backups; no, empty and wrong passwords; a keybag cut short; a damaged or missing `Manifest.db`;
an iOS 9 backup (`Manifest.mbdb`); a file cut short or missing from the backup; media not in the backup; a type from
a future version; an empty text; a member who left; a message before 2001; WhatsApp Business beside WhatsApp; a
backup without WhatsApp; `%` and `_` in paths; file names on the library side cleaned of `/`, `\` and leading dots.

## Needs a real backup to confirm

The type codes above beyond 0–8, the group event codes, whether `ZMEDIALOCALPATH` is always relative, and the
protocol-buffer fields for replies and reactions. Test data: `oopz-gen --ios OUT [--ios-password PW]`.
