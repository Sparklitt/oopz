# Viber "Export message history" (`viber-csv`)

Crate `crates/adapters/viber-export`, decision [0015](../decisions/0015-official-text-exports.md). No key, no
password: the zip of CSV files Viber writes (Settings > Calls and Messages > Export message history), or one of its
CSV files.

## What's read

- A zip (read in memory; entries with `..` or absolute paths ignored, files that aren't Viber CSV skipped and
  counted), or one `.csv`. One CSV file is one conversation, named after the file (`Viber ` dropped).
- UTF-8 (BOM or not) or UTF-16 with a BOM; `,`, `;` or tab between fields (whichever the first line has most of);
  quoted fields with commas, quotes (`""`) and line breaks.
- A header naming the columns in any order and case (`Date`, `Time`, `Sender`/`Name`, `Phone`/`Number`,
  `Message`/`Text`), or none: date, time, sender, phone, message (four fields: no phone). Without a header, fields
  past the message are its own unquoted commas and are joined back.
- Dates `31/12/2023`, `12/31/23`, `31.12.2023`, `2023-12-31` (a date and time in one field too); the day/month order
  decided per file (0015 point 3; when nothing decides it, day first, noted). Times 24-hour or 12-hour (`PM`,
  `p. m.`).

## Mapping

| CSV | Model |
|---|---|
| a row | a message, id `line:N` (the row's line in the file) |
| sender `Me` (also `Yo`, `Ich`, `Moi`, `Eu`, `Я`) | from the owner (`me`) |
| other sender + phone | a person with id = the phone in one form (else the name), named |
| message | `Text` (+ `Link`s); media placeholders ("Photo message", "Sticker message") stay text, as Viber wrote them |
| a row with fewer than three fields | `Unknown` (code `row`), noted |
| a date or time that can't be read or doesn't exist | no time, noted |
| more than two senders in a file | `Group`, else `Direct` |

- **Times**: the phone's wall clock read as UTC (`wall_clock`).
- **Owner**: `me` when any row is from "Me"; otherwise none (noted).

## Refused

A file whose first row is neither a header with a date and a message column nor a row starting with a date; a zip
with no such file (`NotThisFormat`).

## Tested

`oopz-gen --export viber-csv` writes a one-to-one chat (header, day first, 24-hour) and a group (no header, month
first, two-digit years, 12-hour), with quoted commas, quotes, line breaks, placeholders, a link and right-to-left and
combining text; the tests read them zipped and alone against the manifest, and truncated files and zips, noise with
quotes and commas, short rows, an unclosed quote, a 64 KB+ message, dates before 1970, after 2099 and that don't exist.

## Still needs a real export to confirm

The exact header Viber writes in each language and version, whether it writes one; the placeholder wording for each
media kind (then they could become media parts); how group chats name their senders' phones.
