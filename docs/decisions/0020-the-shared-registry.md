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
4. **Adapters look codes up through one `msgkinds::Reading` per source**, made with the version the source says
   wrote it: WhatsApp Android's from the crypt15 header (before any key), WhatsApp iPhone's from the manifest's
   `Applications`, SMS Backup & Restore's from the file's comment; Telegram's, LINE's, Viber's and the WhatsApp chat
   export's don't say, so the newest meaning of each code is taken and the report says "version not recorded". The
   version shows in the source's format (`crypt15, WhatsApp 2.24.10.80`). Every adapter reads its codes from
   msgkinds: WhatsApp (`message_type`, `action_type`, `ZMESSAGETYPE`, `ZGROUPEVENTTYPE`, the export's words),
   Telegram (`media_type`, actions), SMS, LINE, Viber's CSV. At the end, the reading's report joins the source's
   notes: unknown types with their source and version and a count, codes known only for other versions, and lines
   whose wording no table has (another language), with where to add them (msgkinds' `adding-a-source.md`). Codes and
   counts only, never a message's words.
5. **A code no row names is never an error and never dropped**: an unknown part with the app's own code, shown as a
   labelled card ("WhatsApp: 9999, not shown yet", in the app's language), counted and exported.
6. **The readers' tests** msgkinds asks for live here: every row of each source an adapter reads maps without
   falling to unknown (`oopz-app` adapter, `whatsapp-ios`); every kind and every event exports in HTML, JSON, CSV,
   TXT and PDF with no duration for a kind without one (`oopz-export`, `oopz-export-pdf`, from
   `oopz_gen::every_kind`); an unknown code is labelled, counted and exported; an unknown WhatsApp version (new
   columns, an optional table gone, an unknown type, a newer `user_version`) opens with all its messages
   (`oopz-gen/tests/schema_changes.rs`); each export adapter reads a changed format (a column, key or line it
   doesn't know, one moved or missing) and keeps what it can or says what's missing (`mod format_changes` in
   `line-export`, `sms-xml`, `telegram-export` and `viber-export`'s `tests/export.rs`). The desktop app's `Kind`,
   `EventCode` and the other codes oopz stores come from `apps/desktop/src/lib/generated/oopz.ts`, written from
   oopz's enums (`oopz-export`'s `bindings`, its test fails when stale), and msgkinds' data from
   `generated/msgkinds.ts`; its tables are `Record<Kind, …>` in every language. No msgkinds value is spelled in
   oopz's Rust, TypeScript or Svelte (`oopz-model/tests/values.rs`, `msgkinds::scan`).
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

msgkinds is published (the path becomes a git dependency), or a format starts saying which version wrote it.
