# Telegram Desktop JSON export (`telegram-json`)

Crate `crates/adapters/telegram-export`, decision [0015](../decisions/0015-official-text-exports.md). No key, no
password: Telegram Desktop's "Export chat history" or Settings > Advanced > "Export Telegram data", format
"Machine-readable JSON".

## What's read

- `result.json`, alone, in its folder (`ChatExport_…/`, `DataExport_…/`), or in a zip (the shallowest
  `result.json`; entries with `..` or absolute paths are ignored; read in memory).
- A one-chat export (`name`, `type`, `id`, `messages`) or a full export (`personal_information` → the owner,
  `contacts.list` → people `phone:+…`, `chats.list` and `left_chats.list`).
- Times from `date_unixtime` (UTC). An export without it has only `date`, the computer's wall clock: read as UTC,
  `wall_clock` set when every time is such, and counted in the notes.

## Mapping

| Telegram | Model |
|---|---|
| chat `personal_chat`, `bot_chat` / `private_group`, `*_supergroup` / `*_channel` / `saved_messages` | `Direct` / `Group` / `Channel` / `Notes`; any other type `Unknown`, noted |
| `id`, `from_id` (`user123`), `from` | message id, sender (a person with that id and name); no `from_id`: `name:…` |
| `text` (string or list of entities) | `Text`; `link` and `text_link` entities → `Link` (with its text as title) |
| `photo` | `Media` `Photo` (`file`, size, width, height) |
| `file` + `media_type` `voice_message` / `video_message` / `video_file` / `animation` / `sticker` / `audio_file` / none | `Voice` / `VideoNote` / `Video` / `Gif` / `Sticker` / `Audio` / `Document`, with `mime_type`, `duration_seconds`, `width`/`height`, `file_name`, `file_size`; the text is the caption |
| `(File not included. …)` | `omitted`, no `file` |
| another `media_type` | `Unknown` with that code, noted |
| `location_information` (+ `live_location_period_seconds`) | `Location` (`live`) |
| `poll` | `Poll` with vote counts |
| `contact_information` | `Contact` |
| `edited_unixtime` / `edited` | `edited`, `edited_ms` |
| `reply_to_message_id`, `forwarded_from` | `reply_to` (kept when the message isn't there), `forwarded` |
| `reactions` (`recent` names who) | one `Reaction` per person in `recent`, the rest of `count` without who (at most 1000) |
| service `phone_call` (`discard_reason`, `duration_seconds`), `group_call` | `Call`: missed → `Missed`, busy → `Busy`, disconnect → `Failed`, hangup → `Answered` with a duration else `Cancelled` |
| service `create_group`, `create_channel`, `migrate_from_group`, `invite_members`, `join_group_by_link/request`, `remove_members`, `edit_group_title`, `edit_group_photo`, `delete_group_photo`, `pin_message`, `clear_history`, `set_messages_ttl`, `migrate_to_supergroup` | `System` with the event; members or title as text |
| another `action`, another message `type` | `Unknown` with Telegram's code, counted in the notes |

- **Owner**: `personal_information`; in a one-chat export of a personal chat, the sender who isn't the chat's id
  (noted as inferred); otherwise none (noted).
- **Repeated ids** in one chat get `#2`, `#3` (noted); a message without an id is `index:N`.

## Refused

Not JSON, or JSON without `chats` or `messages` (`NotThisFormat`); `chats.list` or `messages` that aren't lists
(`UnsupportedVersion`, by name).

## Tested

`oopz-gen --export telegram-json` writes a full export (owner, contacts, a personal chat with every kind, a supergroup
with service messages and an unknown action, Saved Messages, a channel); the tests read it alone, in a folder and
zipped against its manifest, and truncated JSON, wrong field types, deep nesting, a 64 KB+ text, times before 1970
and after 2099, repeated ids, a reply to a missing message, a sender in no member list.

## Still needs a real export to confirm

Whether video calls are marked (`is_video` is read if present); the exact shape of paid media, giveaways, stories,
topics in forum groups and custom-emoji reactions; exports older than `date_unixtime` (2021).
