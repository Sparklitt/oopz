# Messages: every sentence oopz shows, as data

Every sentence oopz writes is a message in a catalog, compiled with msgkinds' generator (msgkinds'
`docs/messages.md` has the format, the checks and the edge cases). A note is a message with its values, worded in
the person's language at the last moment.

| Catalog | File | What it holds |
|---|---|---|
| `oopz` | `crates/oopz-model/data/messages.toml` | an import's notes (what an adapter kept as unknown or read around), the export's words and labels |
| `oopz_cli` | `crates/oopz-cli/data/messages.toml` | what the `oopz` command prints: sentences, field labels, table headers, prompts |

A sentence with a count is a counted message: each language gives its CLDR plural forms (English `one`, `other`;
Spanish `one`, `many`, `other`, its `many` the same as `other` since oopz shows digits), so "1 file" and "2 files"
are both right. A sentence with two counts words the second as a label (`oopz read: 3`) or splits.

Names are stable: notes are stored by catalog and name, so a name is never renamed or reused. Every user message
has English and Spanish; Spanish follows the desktop app's `apps/desktop/src/lib/i18n/es.ts` where it has the same
sentence, so the app and the command say the same.

The command's language is the terminal's: msgkinds' `Catalog::locale_language` over `LC_ALL`, `LC_MESSAGES`,
`LANG` (the first set decides; `C` and a language oopz lacks are English). Its tests run in `LC_ALL=C`;
`the_terminals_language_words_the_output` checks the Spanish. Clap's `--help` text is still English (the derive
attributes are fixed at compile time).
