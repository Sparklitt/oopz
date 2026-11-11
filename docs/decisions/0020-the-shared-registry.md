# 0020: The shared registry (msgkinds)

Status: accepted (2026-11-11)

oopz and hilo each kept their own tables of what an app's type codes mean, and they disagreed (WhatsApp's 81, its
system actions, albums, sticker packs). msgkinds' decision
[0001](https://github.com/Sparklitt/msgkinds/blob/main/docs/decisions/0001-one-registry-shared-by-hilo-and-oopz.md)
(`../msgkinds/docs/decisions/0001-one-registry-shared-by-hilo-and-oopz.md` in a checkout) puts them in one crate,
used by both: every kind, event and app, and every code of every source per app version, with what's kept and how
it's known.

## Decision

1. **oopz depends on msgkinds** (`[workspace.dependencies]`), by path from a checkout next to oopz's
   (`../msgkinds/crates/msgkinds`) until it's published, then by git pinned to a revision. The desktop app's own
   workspace reaches it through `oopz-model`.
2. **Licences**: msgkinds is MIT OR Apache-2.0, so oopz (AGPL-3.0-or-later) can use it and hilo can too; oopz stays
   AGPL. A change to the registry is made in msgkinds first (rows, tests, docs), then picked up here.
3. **oopz-model maps both ways, exhaustively.** `Kind` has msgkinds' kinds under the same stored names (kept as
   oopz's own type so it serializes; `From` both ways, no `_` arm, so a new msgkinds kind doesn't build here until
   it's handled). `Event` has every msgkinds event under oopz's stored names: `topic_created` is msgkinds'
   `topic_changed`, and `admin_changed` (an export that doesn't say which way) maps to `admin_added`. `App` converts
   too. Kinds without a part of their own (album, sticker pack, event, payment, game, business, story, a
   reaction kept as its own message) are a `Part::Card`.
4. **Adapters look codes up** with `msgkinds::lookup` where msgkinds has the source: WhatsApp Android
   (`message_type`, and `message_system.action_type` for a system line) and WhatsApp iPhone (`ZMESSAGETYPE`, and
   `ZGROUPEVENTTYPE` of a group event or a notice). The others keep their own mapping for now, each with a `TODO(msgkinds)`
   naming the msgkinds source to switch to (Telegram, SMS and LINE are listed on msgkinds' main; Viber's CSV and
   the WhatsApp chat export's words are next). The app's version isn't read from a backup yet: the
   registry takes its newest row for each code.
5. **A code no row names is never an error and never dropped**: an unknown part with the app's own code, shown as a
   labelled card ("WhatsApp: 9999, not shown yet", in the app's language), counted and exported.
6. **The readers' tests** msgkinds asks for live here: every row of each source an adapter reads maps without
   falling to unknown (`oopz-app` adapter, `whatsapp-ios`); every kind and every event exports in HTML, JSON, CSV,
   TXT and PDF with no duration for a kind without one (`oopz-export`, `oopz-export-pdf`, from
   `oopz_gen::every_kind`); an unknown code is labelled, counted and exported; an unknown WhatsApp version (new
   columns, an optional table gone, an unknown type, a newer `user_version`) opens with all its messages
   (`oopz-gen/tests/schema_changes.rs`). The desktop app's `Kind` and `EventCode` are compared with `oopz-model` by a
   Rust test, and its tables are `Record<Kind, …>` in every language.
7. **A library written by a newer oopz still opens.** Stored names this build doesn't know read as unknown, never
   an error: a message's kind (`kind_of`), a conversation's kind, an event (`other`), a call's outcome, and a part
   (`Part::Unknown` with its stored type: `hologram`, `card:quiz`, `media:hologram`, and any text it had). Fields
   added later are ignored. An `App` this build doesn't know can't be shown, so adding one bumps the library's
   `SCHEMA` and an older oopz refuses the library ("made by a newer oopz"); a test holds the two together. Tests:
   `oopz-model` (`newer_data`), `oopz-core` (`rows_a_newer_oopz_wrote_still_open`).

## Consequences

- Video notes are their own kind (they were counted as videos): a library made before keeps "video" for them until
  the source is added again.
- Stickers, photos and GIFs never show a duration, whatever the source wrote.

## Revisit when

msgkinds is published (the path becomes a git dependency), or msgkinds gains Telegram, SMS, LINE, Viber or the
WhatsApp chat export's words as sources the adapters can switch to.
