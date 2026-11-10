# 0017: PDF exports

Status: accepted (2026-11-10)

People want a printable, shareable document of a chat: for a court, a family album, an archive. A PDF that drops
emoji or prints Arabic and Hebrew backwards is worse than none.

## Decision

1. **`crates/oopz-export-pdf`**, its own crate, built only with the CLI's and the app's `pdf` feature: a PDF writer
   with shaping is a large dependency an old phone's build may leave out.
2. **`krilla`** writes the PDF (embedded, subset fonts; colour emoji in COLR, CBDT and sbix fonts; images; outline),
   **`rustybuzz`** shapes text (joined scripts, marks, emoji sequences), **`unicode-bidi`** orders right-to-left and
   mixed lines. No other PDF dependency.
3. **Fonts are the system's**, none ships with oopz: a text font, then fonts for other scripts and emoji, found by
   file name in each system's font folders (Windows, macOS, Linux, Android's `/system/fonts`), with folders or
   files the person adds first. Each character goes to the first font that has it; joiners, variation selectors,
   skin tones and marks stay with the character before. A character no font has is counted and reported.
4. **Layout**: cover (title, dates, count, the privacy note), contents (with page numbers, from a first pass that
   only measures), one chapter per conversation or one merged timeline, day headings, the owner's messages marked
   with a bar, replies quoted, reactions, photos as thumbnails from the source's folder (JPEG, PNG, WebP, 24 MB at
   most, never outside the folder) or not at all, A4 or Letter, page numbers, a bookmark per chapter. View-once
   content is never drawn.
5. Written like every export (0016): to a temporary file named when complete, never over another, stoppable.

## Revisit when

A real export shows a script laid out wrongly (then a test with that text first).
