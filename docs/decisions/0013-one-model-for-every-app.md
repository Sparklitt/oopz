# 0013: One model for every app, and the adapter trait

Status: accepted (2026-11-10)

oopz began as a WhatsApp tool (0007). The plan ([plan-all-apps.md](../plan-all-apps.md)) opens every messaging app's
history the owner can take out. Each app stores different things in different shapes; the features after reading
(the library, search, selection, exports, timelines, contacts) shouldn't care which app a message came from.

## Decision

1. **`crates/oopz-model`**, no I/O: `App`, `Person`, `Conversation` (+ kind), `Message` (envelope: ids, sender,
   from-me, times, edited, deleted, forwarded, starred, reply-to, reactions) with one or more `Part`s (text, media with
   its facts, location, contact, poll, call, link, system event, unknown). The catalogue in
   [data-types.md](../data-types.md) is what the types grow towards; what isn't modelled yet stays in the app's own
   wording inside a `System` or `Unknown` part, never dropped.
2. **Times are UTC milliseconds or `None`**: an adapter never invents a time. Exports written in the phone's local
   time with no zone (WhatsApp, LINE, Viber text) are read as if UTC and marked so in the source's notes, so a
   message shows at the hour the person saw it.
3. **Ids** are the app's own; where an export has none, the adapter makes one stable for that file (`line:120`). A
   message is unique by (source, conversation, id).
4. **One kind per message** (`Message::kind`) for filters and counts: the first part that isn't text (a link counts
   only when nothing else does); deleted for everyone wins.
5. **The adapter trait**: `id`, `app`, `secret` (none, key, password), `detect` (a cheap look: name and first bytes),
   `open(path, secret) -> Source`. A `Source` gives its info (app, adapter, format version, owner when known, media
   folder, notes), people, conversations, and messages per conversation as a stream that can be stopped. Text exports
   are small and read whole (`Loaded`); backups stream.
6. **Every adapter follows the rule** in the plan: official exports, or backups opened with the owner's own key or
   password; nothing else. One crate per adapter under `crates/adapters/`, built and tested alone, each with its
   `oopz-gen` generator, edge tests and `docs/adapters/<app>.md`.

## Consequences

- WhatsApp Android's crypt15 code moves to an adapter crate (0014); the index and viewer of `oopz-app` stay as they
  are for WhatsApp, and the new library (0016) takes every source, WhatsApp included, through the model.
- A field an app has and the model doesn't is a model change, made here first, with the catalogue updated.

## Revisit when

An app's data can't be expressed without losing meaning (then the model grows, never a per-app side channel).
