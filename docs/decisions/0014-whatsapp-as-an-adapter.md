# 0014: WhatsApp Android becomes an adapter

Status: accepted (2026-11-10)

WhatsApp Android's code (crypt15, the schema, `Backup`, the chat merge, the fixtures) lived in `crates/oopz`, the
crate everything began with. With one model for every app (0013), it's one adapter among several.

## Decision

1. The crate moves to `crates/adapters/whatsapp-android`. Its package and library name stay `oopz`: the CLI, the
   app, the fuzz targets and decision 0004's "nothing else without a record" keep pointing at the same crate, and
   history (`git log --follow`) stays readable. Only paths change.
2. The adapter itself, `WhatsAppAdapter` (`whatsapp-android`, secret: the 64-digit key), lives in `oopz-app`, next
   to the session and index that already read every extra WhatsApp keeps (replies, reactions, edits, forwards, polls,
   calls, media facts, group subjects, names from a contacts file beside the backup) and that the app's tests cover.
   `detect` looks at the file's name and crypt15 header; `open` decrypts into a private temporary folder, builds the
   index, and streams each chat's messages as the model's; the decrypted copy is removed when the source is dropped.
   Writing that reading a second time in the `oopz` crate would be two places to fix one WhatsApp change.
3. The library (0016) takes adapters as a list it's given, so it depends on the model only; the CLI and the app hand
   it the WhatsApp adapter with the text-export ones.
4. Merging back into WhatsApp (0006) stays WhatsApp-only and unchanged.

## Revisit when

A second app can be written back to (then merge gets its own trait).
