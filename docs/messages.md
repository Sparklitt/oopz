# Messages: every sentence oopz shows, as data

Every sentence oopz writes is a message in a catalog, compiled with msgkinds' generator (msgkinds'
`docs/messages.md` has the format, the checks and the edge cases). A note is a message with its values, worded in
the person's language at the last moment.

| Catalog | File | What it holds |
|---|---|---|
| `oopz` | `crates/oopz-model/data/messages.toml` | an import's notes (what an adapter kept as unknown or read around), the export's words and labels |
| `oopz_cli` | `crates/oopz-cli/data/messages.toml` | what the `oopz` command prints: sentences, field labels, table headers, prompts |
| `oopz_app` | `crates/oopz-app/data/messages.toml` | the desktop app's logic and its shell (`apps/desktop/src-tauri`): their errors and what they say |

A sentence with a count is a counted message: each language gives its CLDR plural forms (English `one`, `other`;
Spanish `one`, `many`, `other`, its `many` the same as `other` since oopz shows digits), so "1 file" and "2 files"
are both right. A sentence with two counts words the second as a label (`oopz read: 3`) or splits.

Names are stable: notes are stored by catalog and name, so a name is never renamed or reused. Every user message
has English and Spanish; Spanish follows the desktop app's `apps/desktop/src/lib/i18n/es.ts` where it has the same
sentence, so the app and the command say the same.

## The command's language

The command's language is decided once, before any output (`say::choose`), in the owner's order (2026-11-12):
`--lang`, then `OOPZ_LANG`, then GNU's `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`, then the system's setting
(`sys-locale`: Windows, and macOS terminals without `LANG`), then English. msgkinds decides from those
(`Catalog::language`, its `docs/messages.md` rows 24–40); oopz reads them. Clap's `--help` text stays English (owner
decision, 2026-11-12: localizing it means walking `Cli::command()` at run time).

| # | Case | What happens | Test |
|---|---|---|---|
| 1 | `LANGUAGE=fr:es:en` with an English locale | Spanish: the first oopz has | `the_language_asked_for_and_the_language_list` |
| 2 | `LANGUAGE=es` in the `C` locale | English: GNU ignores the list there | same |
| 3 | `LANGUAGE` empty or `:` | skipped | msgkinds row 33 |
| 4 | `es-ES`, `pt-BR` (Windows, macOS) | read as `es_ES`, `pt_BR` | msgkinds row 34 |
| 5 | `LC_ALL=ES_es` | Spanish: case is ignored | `the_language_asked_for_and_the_language_list` |
| 6 | `zh-Hans-CN`, `sr_RS@latin` | script and region first, then the language | msgkinds row 36 |
| 7 | `--lang fr`, `OOPZ_LANG=fr` | an error listing `en, es` (exit 2), never English by surprise | `asking_for_a_language_oopz_lacks_is_an_error_listing_its_languages` |
| 8 | `--lang ""` | an error, in the language the rest gives; an empty `OOPZ_LANG` is unset | same, and `the_language_asked_for_and_the_language_list` |
| 9 | Windows without `LANG` | the user's interface language (`sys-locale`) | msgkinds row 39 (the system's tag); `sys-locale` reads it |
| 10 | A macOS terminal with `LANG` unset | the same | same |
| 11 | Read once per run | no line in another language | `say::tests::the_language_is_decided_once_per_run` |
| 12 | Tests | every test's command pins `LC_ALL` and clears `OOPZ_LANG`, `LANGUAGE`, `LC_MESSAGES`, `LANG` (`tests/common`) | `tests_pin_the_language` |

## Numbers and dates

Numbers and dates are plain in every language: ISO 8601 days (`2026-11-12`), digits without grouping, `.` for a
decimal (`1.5 MB/s`), so scripts read the output whatever the language; the desktop app formats with the platform
(`Intl`). Owner decision, 2026-11-12.

| # | Case | What happens | Test |
|---|---|---|---|
| 1 | A decimal in Spanish | `1.5`, not `1,5` (`say::decimal`) | `numbers_1_a_decimal_is_the_same_in_every_language` |
| 2 | A missing date (or position, or name) | `-` in every language (`say::MISSING`) | `numbers_2_dates_are_iso_and_a_missing_one_is_a_dash` |

## Agreement

A Spanish participle or article agrees with its noun, so a wording agrees with a noun it writes itself ("selección
{id} guardada"); next to a value whose noun isn't fixed (a path, a name) it takes a form without gender ("se guardó:
{path}"). Rows (msgkinds' `docs/messages.md` has the rule), a test each in `crates/oopz-cli/src/say.rs`:

| # | Case | What happens |
|---|---|---|
| 1 | A value whose gender we don't know (`saved`, `saved_bytes`, `saved_contacts`, `saved_export`, `received_file`) | "se guardó", "se recibió": no gendered word |
| 2 | The review of `oopz_cli`'s Spanish (2026-11-12) | `copied_folder` "carpeta {name} copiada" (was "copiado", wrong for a folder), `linked_into` "personas unidas" (the people of a suggestion); the rest agree with their own nouns |

## Errors

Every error is a stable `code` and a note: `oopz_model::Error::note()`, `oopz_core::Error.note`, `oopz_app::AppError.note`,
worded in the person's language at the last moment; `Display` is the English. What was being done is a `doing_…`
note and the system's reason a value. The desktop's wire stays `{code, detail}`, the detail English, until notes go
on it (roadmap item 6). Rows (owner, 2026-11-12), a test each:

| # | Case | What happens | Test |
|---|---|---|---|
| 1 | An OS error's text (`io::Error`) | a value, as the system wrote it (on Windows in its own language), never translated by oopz | `oopz_model` `errors_1_a_systems_reason_is_a_value_as_given` |
| 2 | A path; a key, password, number or message text | a path is a value, never a message; the others never appear in an error | to come |
| 3 | Cancelled, needs a secret, wrong secret | no values | `oopz_model` `errors_3_a_refusal_with_no_values` |
| 4 | The CLI's `anyhow` chain | an oopz error inside is worded by its note in the command's language, never English mixed in | to come |
| 5 | The desktop's wire | `{code, detail}`, English detail, codes unchanged (stored names) | `oopz_core` `errors_5_the_wire_is_code_and_english_detail`; the shell's `errors_5_the_apps_wire_is_code_and_english_detail` |
| 6 | Conversions (SQLite, JSON, crypt15) | keep their codes; which crypt15 header check failed is typed (`not_crypt15_no_iv` is `crypt14`), never read from English | `oopz_core` `errors_6_conversions_keep_their_codes`; `oopz_app` `errors_6_the_backup_errors_keep_their_codes`; `oopz` `errors_6_each_header_refusal_says_which` |
| 7 | An error wrapped by another crate's | keeps its note (a `Value::Note`, or the same note): one language throughout | `oopz_model` `errors_7_a_note_inside_is_worded_in_the_same_language`; the shell's `errors_7_an_app_error_keeps_the_librarys_note` |
| 8 | Developer-only texts (a programming mistake's) | `for = "developer"`, English only | to come |
| 9 | ios-backup's and whatsapp-android's diagnostics | both crates word their errors from the `oopz` catalog (they depend on `oopz-model`, as the adapters do); what a person can only report (a keybag field, a table name, a merge check) is a developer message inside a user one, the rest user messages | `oopz_ios_backup` `errors_9_a_diagnostic_is_a_developers_inside_a_users_words` |
