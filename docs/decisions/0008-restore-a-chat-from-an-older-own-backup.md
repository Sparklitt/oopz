# 0008: Restoring a chat from an older backup of one's own

Status: proposed (2026-10-24); not built until there's a real case to check it against

0006 restores a chat from the *other person's* backup, mirrored. The other common loss is on one phone alone: a
chat cleared or deleted, or messages deleted, while an older backup of the same phone still has them (WhatsApp keeps
about a week of local backups, `msgstore-YYYY-MM-DD.1.db.crypt15`, and many people keep copies). oops should put them back
without mirroring anything: they were the owner's messages all along.

## Decision

`oops restore --own`, the same command as 0006 with one flag. The **source** is the older backup, the **target** a
private copy of the current one; both are the same owner's, each opened with its own key (usually the same key,
asked twice so a changed key still works). The originals are only read; the result is a new file.

1. **Everything in 0006 holds except mirroring**: keep everything the target has, add only `key_id`s its chat
   lacks; order without breaking `sort_id = _id` (added older messages take new rows, the target chat's own move
   above them through one id map); copy by column name; the same list of tables; write the file the way WhatsApp
   does with a fresh IV; the same checks before anything is written.
2. **Copied as they are**: `from_me`, statuses, `sender_jid_row_id`, `author_device_jid`, call directions. Ids that
   point at the `jid` table are mapped by the jid's `raw_string` into the target's rows (a reinstall can renumber
   them); a jid the target lacks stops the restore, naming the column.
3. **Owner check, both modes.** Messages both backups have must agree on `from_me` (`--own`) or disagree on every
   one (0006, mirrored). Otherwise the restore stops: it guards against the wrong mode or the wrong chat. With no
   message in common, `--own` also requires the chat's jid to be the same in both.
4. **The chat must exist in the target.** If it was deleted, it comes back with one message sent or received
   before the backup is made. Creating a chat row from nothing is left for later.
5. **Media**: the same phone, so the same `Media/` paths; a path is kept only if the file's hash matches, and missing
   files are counted. `--media`/`--media-out` work as in 0006, from a copy of the phone's `Media/` folder if the files
   were deleted too.
6. **Deleted for everyone stays deleted**: the target's tombstone (`message_revoked`) has the same `key_id`, so the
   older copy isn't added.

## Consequences

- Messages the owner deleted on purpose ("delete for me") come back with the rest: WhatsApp leaves no trace of
  them, so they can't be told apart. The report counts what is added; restoring one chat touches no other.
- Nothing newer than the target is lost; the restore is only as complete as the older backup.
- Restore on the phone works like 0006: WhatsApp's folder moved aside, Clear data, put the result in `Databases/`,
  restore. The risk to the account and the rollback are the same.
- No new crate: the merge code takes a mode, `Mirrored` or `Own`.
