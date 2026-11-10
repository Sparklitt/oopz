# SMS Backup & Restore XML (`sms-xml`)

Crate `crates/adapters/sms-xml`, decision [0015](../decisions/0015-official-text-exports.md). No key, no password: the
`.xml` files the Android app "SMS Backup & Restore" writes (`sms-YYYYMMDDHHMMSS.xml`, `calls-….xml`).

## What's read

- `<smses>` with `<sms …/>` and `<mms>` (`<parts><part …/></parts><addrs><addr …/></addrs>`), and `<calls>` with
  `<call …/>`; one file at a time (the library joins them: conversation ids match across files).
- The app's version from its comment (`File Created By SMS Backup & Restore v10.20.002`) as `format`.
- Attribute values decoded by the adapter: `&amp; &lt; &gt; &quot; &apos;`, decimal and hex entities, and emoji
  written as two UTF-16 surrogate entities (`&#55357;&#56842;` → 😊); a lone half becomes U+FFFD.
- Times: `date` in UTC milliseconds (`wall_clock` false); a record without one keeps no time (noted).

## Mapping

| XML | Model |
|---|---|
| `address` / `number` | the conversation: [`normalize_phone`] of it (so `+999 9000 0002` = `+99990000002`), else as written (short codes, named senders), else `unknown` |
| `contact_name` (not `(Unknown)`) | the person's and conversation's name |
| `<sms type>` 1 | received from the address |
| `<sms type>` 2 sent, 3 draft, 4 outbox, 5 failed, 6 queued | from the owner (`me`) |
| another type | received, counted in the notes |
| `body` (+ `subject` when not `null`) | `Text` (+ `Link`s) |
| `<mms address="a~b">` | a group conversation `group:a,b` (sorted numbers); one address: the same conversation as its SMS |
| `<mms msg_box>` 1 / 2–6 | received (sender = `<addr type="137">`) / from the owner; a sent MMS's 137 address is the owner's number |
| `<part ct="application/smil">` | skipped (layout only) |
| `<part ct="text/plain" text>` | `Text` |
| `<part ct="image/…">`, `image/gif`, `video/…`, `audio/amr`/`3gpp`, `audio/…`, other | `Media` `Photo`, `Gif`, `Video`, `Voice`, `Audio`, `Document`, with mime, name (`cl`), size of `data` |
| `<part ct="text/x-vcard">` | `Contact` (name and phones from the card, the card kept) |
| an MMS with no readable part | `Unknown` (`mms`), noted |
| `<call type>` 1 / 2 / 3 / 4 / 5 / 6 / other | `Call`: `Answered` / `Answered` (`Cancelled` with no duration) from the owner / `Missed` / `Missed` (voicemail) / `Declined` (rejected) / `Declined` (blocked) / `Unknown`; `duration` in seconds |

- **Ids**: `sms:N`, `mms:N`, `call:N` (the record's place in the file).
- **Owner**: always `me`; phone from a sent MMS when there is one.

## Refused

A file whose root element isn't `<smses>` or `<calls>`, or that isn't XML (`NotThisFormat`). A file cut short keeps
the records before the cut, and the notes say so (and when the root's `count` disagrees with what was read).

## Not yet

MMS files are held inside the XML in base64: their size is read, but the bytes aren't kept, because `oopz_model::Media`
has no place for them yet (a model change to decide). Call logs don't say whether a call was video.

## Tested

`oopz-gen --export sms-xml` writes an `sms` file (every type, the same number written two ways, a short code, a named
sender, emoji as surrogate pairs, MMS with a picture and caption, a contact card, a voice note, a group) and a `calls`
file (every type and an unknown one); the tests read both against the manifest, and files cut at many points, noise,
a 64 KB+ body, times before 1970 and after 2099, bad base64, unknown elements.

## Still needs a real export to confirm

RCS messages (newer app versions write them as `<mms>` with extra attributes), and dual-SIM fields (`sub_id`, unused).
