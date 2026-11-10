# 0016: The library, selections and exports

Status: accepted (2026-11-10)

With adapters for several apps (0013-0015), people need one place that holds what they opened, from every app, to
read, search, select and export from. Until now each WhatsApp backup was opened for one session and its index thrown
away on close.

## Decision

1. **`crates/oopz-core`, the library**: one SQLite file (`library.db`) holding the model (sources, people, contacts,
   conversations, members, messages with their parts as JSON) and a full-text index (FTS5, `unicode61` with accents
   removed: `cafe` finds `café`, case ignored). It depends on the model only; adapters are handed to it.
2. **Where it lives, and for how long.** The app keeps the library in its private cache folder and **wipes it on
   close** like a decrypted copy (0009), unless the person switches on "Keep my library on this device" (Settings,
   off by default), when it lives in the app's private data folder. "Wipe now" removes it either way. The CLI takes
   `--library DIR` and keeps what's there (it's the person's own folder). Nothing in it leaves the device.
3. **Importing** a file: the adapters' `detect` picks the adapter (or the person names one); the source's original
   file is hashed (SHA-256) and an already-imported file is refused by name (`exists`), not doubled. Import is one
   transaction: a failure or a stop leaves the library as it was. Embedded media (MMS parts) is written next to the
   library, under `media/<source>/`, never over a file.
4. **Removing a source** deletes its rows and its media folder, and nothing else; contacts linked only through it
   lose that link.
5. **Who's who**: each app's people stay per app; the owner links them into **contacts**. Suggestions come from the
   same normalised phone number or email across sources, never applied silently. A vCard import makes contacts and
   suggests links the same way; a vCard export writes the selected contacts (names, numbers, emails, and which apps
   they're in as notes).
6. **One filter** for everything (search, the conversation list, stats, timelines, selections, exports): apps,
   sources, conversations, contacts, people, kinds, from me or not, a date range, and words. A **selection** is a
   filter plus messages added or taken out by hand, saved under a name.
7. **The cross-app timeline** is a filter's messages from every conversation it covers, in time order, each marked
   with its app and conversation: the same people across WhatsApp, Telegram, SMS… read as one.
8. **Exports (`crates/oopz-export`)** of a selection: HTML (one file, works offline, prints well, `dir="auto"` for
   right-to-left), JSON (the model), CSV (one row per message), TXT, vCard, and a zip of the selection's media files
   from the sources' folders. Each is written to a new file next to nothing it would overwrite, after a free-space
   check, with progress and a way to stop (a stopped export leaves no partial file). Times in the time zone the
   person picks (the computer's by default). View-once content is never exported.
9. **PDF** is its own crate (`oopz-export-pdf`), behind the `pdf` feature of the CLI and the app, because a PDF
   writer with text shaping is a large dependency (decision 0017).

## Revisit when

A library grows past what SQLite does comfortably on an old phone (measure at a million messages on the S9+).
