# LINE "Export chat history" (`line-export`)

Crate `crates/adapters/line-export`, decision [0015](../decisions/0015-official-text-exports.md). No key, no password:
the text file LINE writes from a chat's menu (Settings > Export chat history).

## What's read

- A `.txt`, UTF-8 (BOM or not) or UTF-16 with a BOM, LF or CRLF.
- Header `[LINE] Chat history with X` (one-to-one), `[LINE] Chat history in X` (group), Japanese
  `[LINE] Xとのトーク履歴` / `[LINE] Xのトーク履歴`; `Saved on:` / `保存日時：`. A file without the header is read when a
  date line is followed by a message line.
- Date lines: `2024/01/01(Mon)`, `2024/1/1(月)`, `2024.01.01 Monday`, `2024-01-01`, `Mon, 01/15/2024` (its
  day/month order decided from the whole file; when nothing decides it, month first, noted).
- Message lines `time<TAB>name<TAB>text`; times `21:41`, `9:41`, `PM10:00`, `10:00 PM`, `午後10:00`.
- A text in double quotes runs over several lines (`""` is a quote); a quote never closed is read as plain text
  (noted). Notices: `time<TAB>text`.
- Any other line continues the message before it; before any message it's kept as `Unknown` (code `line`).

## Mapping

| LINE | Model |
|---|---|
| `time name text` | `Text` (+ `Link`s) |
| `[Photo]` `[写真]` / `[Sticker]` `[スタンプ]` / `[Video]` `[動画]` / `[Voice message]` `[ボイスメッセージ]` | `Media` `Photo` / `Sticker` / `Video` / `Voice`, `omitted` |
| `[File]` `[ファイル]` (+ name) | `Media` `Document`, `omitted` |
| `[Album]` `[アルバム]` (+ name) | `Media` `Photo`, `omitted` (the name as caption) |
| `[Contact]` `[連絡先]` | `Contact` |
| `[Location]` `[位置情報]` (+ name) | `Location` without coordinates |
| `☎ Call time 0:15` / `☎ 通話時間 0:15` | `Call` `Answered`, 15 s |
| `☎ Missed call` `☎ 不在着信` / `☎ Canceled call` `☎ キャンセル` / other `☎` | `Call` `Missed` / `Cancelled` / `Unknown` |
| notice `X unsent a message.` / `Xが送信を取り消しました` | a `deleted` message from X |
| other notices | `System` (joined, invited, removed, left, group name, group photo, announcement; else `Other`) |

- **Ids**: `line:N`, the message's first line. **Conversation**: one, named from the header (else the file name); a
  group for `Chat history in`, more than two senders, or a member notice.
- **Times**: the phone's wall clock read as UTC (`wall_clock`); a message before any date line, or at a time that
  doesn't exist, keeps no time (noted).
- **Owner**: in a one-to-one chat, the one sender the header doesn't name (noted); otherwise none (noted).

## Refused

A file with no `[LINE]` header and no date line followed by a message line (`NotThisFormat`).

## Tested

`oopz-gen --export line-txt` writes a one-to-one chat in three English layouts and a Japanese group (BOM, CRLF), with
every placeholder, call, unsent message, notice and quoted multi-line text; the tests read each against its manifest,
and truncated files, noise with quotes and tabs, thousands of unclosed quotes, a 64 KB+ line, dates before 1970,
after 2099 and times that don't exist.

## Still needs a real export to confirm

Other languages' headers, placeholders and call lines (Thai, Traditional Chinese, Korean, Indonesian); the PC app's
layout; whether video calls have their own wording; album and file lines that carry names.
