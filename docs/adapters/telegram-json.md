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
| chat `personal_chat`, `bot_chat` / `private_group`, `*_supergroup` / `*_channel`, `replies` / `saved_messages` | `Direct` / `Group` / `Channel` / `Notes`; any other type `Unknown`, noted |
| `id`, `from_id` (`user123`), `from` | message id, sender (a person with that id and name); no `from_id`: `name:…` |
| `text` (string or list of entities; `text_entities` when `text` is empty or missing) | `Text`; `link` and `text_link` entities → `Link` (with its text as title); other entity types (bold, mention, spoiler, …) keep only their text |
| `photo` | `Media` `Photo` (`file`, size, width, height) |
| `file` + `media_type` `voice_message` / `video_message` / `video_file` / `animation` / `sticker` / `audio_file` / none | `Voice` / `VideoNote` / `Video` / `Gif` / `Sticker` / `Audio` / `Document`, with `mime_type`, `duration_seconds`, `width`/`height`, `file_name`, `file_size`; the text is the caption |
| `(File not included. …)` | `omitted`, no `file` |
| another `media_type` | `Unknown` with that code, noted |
| `location_information` (+ `live_location_period_seconds`) | `Location` (`live`) |
| `audio_file` without `file_name` | its name is `performer – title` |
| `poll` | `Poll` with vote counts; an answer with `chosen` lists the owner as its voter |
| `contact_information` (+ `contact_vcard`) | `Contact`, then the card file as a `Document` (`text/vcard`) |
| `invoice_information` | `Unknown` `invoice` with "title · description · amount": `amount` is in the currency's smallest unit (145 USD → 1.45 USD; JPY, Stars `XTR` and others whole; KWD and others three places); noted |
| `game_title`, `game_description`, `game_link` | `Text` (the description) and `Link` (the game, titled) |
| `giveaway_information` | `Unknown` `giveaway` with quantity, months and end date; noted |
| `edited_unixtime` / `edited` | `edited`, `edited_ms` |
| `reply_to_message_id`, `forwarded_from` | `reply_to` (kept when the message isn't there), `forwarded` |
| `reactions` (`recent` names who) | one `Reaction` per person in `recent`, the rest of `count` without who (at most 1000) |
| service `phone_call` (`discard_reason`, `duration_seconds`), `group_call` | `Call`: missed → `Missed`, busy → `Busy`, disconnect → `Failed`, hangup → `Answered` with a duration else `Cancelled`; a group call with a duration → `Answered` |
| service `create_group`, `create_channel`, `migrate_from_group`, `topic_created` / `invite_members`, `invite_to_group_call` / `join_group_by_link/request`, `joined_telegram` / `remove_members` / `edit_group_title`, `topic_edit` / `edit_group_photo`, `delete_group_photo`, `suggest_profile_photo` / `pin_message` / `clear_history` / `set_messages_ttl` / `migrate_to_supergroup`, `edit_chat_theme`, `set_chat_wallpaper`, `set_same_chat_wallpaper`, `allow_sending_messages`, `attach_menu_bot_allowed`, `web_app_bot_allowed` | `System`: `GroupCreated` / `MemberAdded` / `MemberJoined` / `MemberRemoved` / `Renamed` / `PictureChanged` / `Pinned` / `HistoryCleared` / `Disappearing` / `SettingsChanged`; members, `title`, `new_title` or `emoticon` as text. A pin's (or score's) `message_id` → `reply_to` |
| service `score_in_game`, `send_payment`, `take_screenshot`, `send_passport_values`, `proximity_reached`, `requested_phone_number`, `group_call_scheduled`, `send_webview_data`, `send_premium_gift`, `requested_peer`, `gift_code_prize`, `giveaway_launch` | `System` `Other`, text "score in game (score 120)", "send payment (1.45 USD)", "group call scheduled (2024-01-23 08:53 UTC)", … |
| another `action`, another message `type` | `Unknown` with Telegram's code, counted in the notes |

- **Not kept** (no place in the model): `via_bot`, `author` (a channel post's signature), `saved_from`, `thumbnail`,
  `inviter`, `information_text`, `reason_*`, `boost_peer_id`, `gift_code`, `unclaimed`, `via_giveaway`, a poll's
  `closed`; and the account parts of a full export that aren't chats (`profile_pictures`, `stories`, `frequent_contacts`,
  `sessions`, `web_sessions`, `other_data`).
- **Owner**: `personal_information`; in a one-chat export of a personal chat, the sender who isn't the chat's id
  (noted as inferred); otherwise none (noted).
- **Repeated ids** in one chat get `#2`, `#3` (noted); a message without an id is `index:N`.

Checked against the schema at <https://core.telegram.org/import-export> (2026-11-10): every chat type, every
`media_type`, every service `action` and every message object it lists is mapped above or listed as not kept.
`reactions` aren't in that page but are in current exports, and are read.

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
