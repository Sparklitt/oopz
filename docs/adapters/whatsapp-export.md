# WhatsApp "Export chat" (`whatsapp-export`)

Crate `crates/adapters/whatsapp-export`, decision [0015](../decisions/0015-official-text-exports.md). No key, no
password: the chat as the app writes it from a chat's menu (More > Export chat).

## What's read

- A `.txt` alone (`WhatsApp Chat with X.txt`, `Chat de WhatsApp con X.txt`, iPhone's `_chat.txt`), a `.zip` with the
  text and its media (`WhatsApp Chat - X.zip`), or a folder holding them. A zip is read in memory; entries with `..` or
  absolute paths are ignored and counted in the notes.
- UTF-8 with or without a BOM, UTF-16 with a BOM; LF or CRLF.
- First lines: Android `31/12/2023, 21:41 - Name: text`, US `12/31/23, 9:41 PM - …`, iPhone
  `[31/12/2023, 21:41:05] Name: text` with U+200E marks; `/ . -` between numbers; 2- or 4-digit years; year first
  (`2023-12-31`); 24-hour or 12-hour with `AM/PM`, `a. m./p. m.`, `a.m.`, after a space, U+00A0 or U+202F.
- Any other line continues the message before it (multi-line messages); lines before the first message are kept as
  `Unknown` parts (code `line`) and counted.

## Mapping

| Export | Model |
|---|---|
| `Name: text` | `Text` (+ a `Link` per `http(s)://` link) |
| line with no `Name: ` | `System` (event from the wording, English and Spanish; else `Other`) |
| iPhone: `Name: ` + U+200E + plain text | `System` (iPhone writes notices under the chat's name) |
| `<Media omitted>`, `<Multimedia omitido>` | `Media` kind `Document`, `omitted` (the export doesn't say which kind) |
| iPhone `image/video/audio/sticker/GIF/document omitted` (and Spanish) | `Media` of that kind, `omitted` |
| `IMG-….jpg (file attached)`, `(archivo adjunto)`; next lines | `Media` (kind from `IMG/VID/PTT/AUD/STK/DOC-` or the extension), `file`, caption |
| `… • 2 pages (file attached)` | `Document` with its name |
| iPhone `<attached: 00000012-PHOTO-….jpg>` | `Media` (kind from `PHOTO/VIDEO/AUDIO/STICKER/GIF`), `file` |
| `X.vcf (file attached)` | `Contact` (name and phones from the card when it's in the zip or folder) |
| `location: https://maps.google.com/?q=lat,lng` (`ubicación:`, iPhone `Location:`) | `Location` |
| `live location shared` | `Location` without coordinates, `live` |
| `POLL:` / `ENCUESTA:` + `OPTION: x (n votes)` | `Poll` with vote counts |
| `This message was deleted`, `Se eliminó este mensaje` | `deleted` |
| `You deleted this message`, `Eliminaste este mensaje` | `deleted`; its sender is the owner |
| `… <This message was edited>`, `<Se editó este mensaje.>` | `edited` (no time: `edited_ms` stays `None`) |
| `View once photo omitted` (and video/voice, Spanish) | `Media`, `view_once`, `omitted` |
| `Missed voice call`, `Missed video call`, `Llamada de voz perdida` | `Call`, `Missed` |
| `Voice call`, `Video call` | `Call`, outcome `Unknown` |

- **Ids**: `line:N`, the message's first line in the file. **Conversation**: one per export, named from the file name
  (zip name for `_chat.txt`); a group when more than two people write or a group notice appears.
- **People**: by the name written (a number for someone not in the phone's contacts, its phone filled in).
- **Times**: the phone's wall clock read as UTC (`wall_clock`); a date that doesn't exist keeps no time (noted). The
  day/month order is decided from the whole file (a first number over 12: day first; a second: month first); when
  nothing decides it, day first, noted.
- **Owner**: the sender of "You deleted this message", or a sender written "You"/"Tú"; otherwise none, and the notes
  say so (the library lets the owner pick).

## Refused

A file whose first five non-empty lines hold no message line (`NotThisFormat`); a zip with no chat text in it.

## Tested

`oopz-gen --export whatsapp-txt` writes one group chat in four layouts (Android, US 12-hour, Spanish with CRLF, iPhone
with BOM and marks) with every line above; the tests read each (alone, zipped, in a folder) against its manifest, and
truncated files, noise, a 64 KB+ line, dates before 1970, after 2099 and that don't exist, unsafe zip entries.

## Still needs a real export to confirm

The exact wording of view-once, call and live-location lines in each language; documents' ` • N pages` in other
languages; exports in languages other than English and Spanish (dates and placeholders are read, notices fall back to
`Other`); old exports from Windows Phone or BlackBerry.
