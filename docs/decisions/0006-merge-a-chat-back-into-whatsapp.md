# 0006: Merging a chat back into someone's WhatsApp backup

Status: accepted (2026-10-23)

One person lost a chat; the other person in it still has it in their backup. They want it back inside WhatsApp. This
is the riskiest thing oopz does: a wrong row can make WhatsApp's restore fail or misbehave later. It is general: any
one-to-one chat between the owners of two backups, in either direction, each backup opened with its owner's own key
and consent. (`oopz restore`; it began as `hilo merge whatsapp` in the hilo project.)

## What we measured (`oopz chat-facts`, on two real phones' backups, 2026-10-23)

Only the shapes are recorded here, never the data:

- `sort_id = _id` for **every** message, and WhatsApp orders a chat by it, not by time (late arrivals break time
  order). The lost chat's rows in the target sit among the rows of its other chats.
- In a one-to-one chat `sender_jid_row_id` is `0` (all but 0.1%); `message_details.author_device_jid` is set on every
  message and points at a device jid of the backup it's in.
- Statuses: sent messages are almost all `13` (read); received `0` or `13`; system `6`.
- The chat can be filed under a phone jid in one backup and under a private `lid` in the other; message `key_id`s are
  the same on both phones, which is how messages are matched.
- Five chat columns point at the chat's messages (`display_`, `last_`, `last_read_`, `last_read_receipt_sent_`,
  `last_important_message_row_id`), plus `*_sort_id` copies.
- Two phones' schemas can have the same tables with some columns in another order or missing; one may lack
  `message_ftsv2`.

## Decision

The **target** is a private copy of the receiving backup, the **source** the other. The originals are only read; the
result is a new file.

1. **Keep everything the target has.** Its messages are never changed in content; the source's are added only for
   `key_id`s the target chat lacks.
2. **Order without breaking `sort_id = _id`.** Older added messages must sort before the target chat's first one, and
   there may be no room below it. So only the target chat's own rows move: added messages take new rows above the
   target's highest `_id`, in source order, then the target chat's messages move above them, keeping their order;
   `sort_id = _id` throughout. Every reference to a moved row is rewritten through one id map: the chat row's five
   pointers and four sort ids, and every `message_row_id`-style column of dependent tables (checked: after the move,
   no column may still hold an old id of the chat). Other chats are untouched, row for row.
   Rejected: giving added rows `sort_id`s unequal to `_id` (breaks an invariant seen on every row of two phones);
   renumbering the whole database (moves every chat to fix one).
3. **Mirror each copied row.** `from_me` flipped (also in quotes, reactions, calls); `chat_row_id` and quoted chats →
   the target chat; `sender_jid_row_id` → `0`; `author_device_jid` → the device jid the target chat already uses for
   that direction; call log rows → the target chat's jid; statuses → `13` for the target owner's sent messages, `0`
   for received, `6` kept for system rows. `key_id`s, timestamps, text, media keys and hashes are copied as they are.
4. **Copy by column name**, never position: only columns both schemas have; the target's keep their defaults
   otherwise.
5. **Copy a known list of tables** for the added messages: `message`, `message_media`, `message_quoted`,
   `message_quoted_media`, `message_details`, `message_secret`, `message_thumbnail`, `message_album`,
   `message_association`, `message_edit_info`, `message_forwarded`, `message_text`, `message_link`,
   `message_location`, `message_revoked`, `message_system`, `message_view_once_media`, `audio_data`,
   `message_sticker_pack`, `message_sticker_pack_stickers`, `message_streaming_sidecar`, `mms_thumbnail_metadata`,
   `message_call_log` + `call_log`, `message_add_on` + `message_add_on_reaction`, `media_hash_thumbnail` and
   `media_refs` for the source's files. Not copied: delivery receipts (`receipt_*`), `reporting_info`,
   `message_send_count`, and anything else; the merge report counts every source row it leaves behind, per table.
6. **Media paths can't collide.** WhatsApp names files with a per-phone counter, so the same name can be a different
   photo. The source's files go to the same `Media/` folders, mirrored (what one received the other sent:
   `…/Sent/`), with a `hilo-` name prefix (kept from the project oopz came from, so results stay comparable). A file
   whose `file_hash` the target already has keeps the target's path and isn't sent again. The media pack for the
   target's `Media/` folder is built from these paths after the merge.
7. **Write the file the way WhatsApp does.** The target's header bytes are kept exactly, except the 16-byte IV,
   replaced in place with a fresh random one (reusing an IV with the same key would break AES-GCM); zlib, AES-256-GCM
   as in 0004; the MD5 footer computed for real, since WhatsApp may check it. New crates, both RustCrypto: `md-5`
   (the footer only, not for security) and `getrandom` (the IV).
8. **Check before anything is written:** decrypt the result with the target's key; `PRAGMA integrity_check`; the
   target chat's count = its own + added; `sort_id = _id` on every row; no reference to a missing message; other
   chats' rows identical to the original's; `oopz chat-facts` on the result shaped like both inputs.

## Consequences

- Rollback on the phone: the target's owner keeps their own backup, WhatsApp's data is cleared (its `WhatsApp/`
  folder moved aside first: clearing deletes `Android/media/com.whatsapp`, seen on Android 16) and they restore it;
  they lose nothing but time.
- WhatsApp's terms may not allow changing its data; a risk to the account that its owner must accept knowingly.
- Receipts aren't copied: old messages show WhatsApp's status ticks only.
- Messages sent between the target's backup and the restore aren't in the result: don't use WhatsApp in between.
- Built and tested on synthetic backups only.

## Built (2026-10-23)

`oopz restore`, in `crates/oopz/src/merge.rs`, tested on a synthetic two-phone fixture. Where the code settles what
the text above left open:

- **References are found by name, not listed by hand.** Schemas have about 200 tables pointing at messages, so every
  column named `message_row_id` or `…_message_row_id` (and `…message_sort_id`) is rewritten through the id map. The
  check is what step 2 asked for: per column, references to missing messages are counted before and after and must
  be equal.
- **New rows start above every id the target has used or still refers to** (`sqlite_sequence` and every reference
  column), so a stale reference can't land on a new row.
- **Triggers are set aside** during the merge and recreated from their exact SQL; the check compares the whole schema
  and `user_version`.
- **Every id-like column of a copied row has a rule** (`*_row_id`, `*jid*`); one without a rule that holds a value
  stops the merge, naming the column. Reactions are copied (by `key_id`, also on messages both have); other add-ons
  are left behind and counted. A call the target already has (same `call_id`) is linked, not copied.
  `call_creator_device_jid_row_id` is mirrored like `author_device_jid`.
- **Tables added to step 5 after the first dry run**: "view replies" threads (`thread_id`, mirrored like a message,
  and `thread_messages`), and per-message details with no foreign ids: `message_vcard`, the `message_quoted_*`
  previews (text, call log, location, vcard), `message_inline_video_metadata`, `mms_metadata`,
  `message_system_block_contact`, `message_system_initial_privacy_provider`. Still left behind: receipts,
  `message_send_count`, `reporting_info`, non-reaction add-ons, `message_vcard_jid`, bot and newsletter info.
- **`chat.created_timestamp`** moves back to the first message's time: both phones had it at or before the first
  message, and added older messages broke that.
- **`sqlite_sequence`** may gain a row: a counted table (`AUTOINCREMENT`) that never had a row gets its first
  counter when rows are copied into it. The check allows only that, and counters that grow.
- **Not decided above, left as copied:** `starred` (the source owner's stars).
- Added messages newer than the target chat's first one are counted in the report; they still sort before it.
