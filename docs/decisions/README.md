# Decision records

Why oops is the way it is. Numbers are kept from the hilo project, where oops began, so comments in the code
(`decision 0006`) still point at the right record here.

- [0004](0004-whatsapp-backup-decryption.md): the crates for decrypting crypt15 backups; nothing else without a record.
- [0006](0006-merge-a-chat-back-into-whatsapp.md): merging a chat back into the other person's backup (`oops restore`).
- [0007](0007-oops-a-whatsapp-backup-tool.md): oops itself, its rules, and publishing it.
- [0008](0008-restore-a-chat-from-an-older-own-backup.md): restoring from an older backup of one's own (proposed).

Code comments also cite hilo's decision 0034 (WhatsApp groups and sticker packs). What it settled for oops: a chat's
names are read from `wa.db` only for the jid asked for (point 6, `contacts.rs`), and the synthetic fixtures include a
group, a two-phones group and a sticker-pack media folder (points 28 to 30, `fixture/group.rs`).
