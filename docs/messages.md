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

## A merge's checks

A merge's checks (whatsapp-android `merge.rs` `verify`) are `passed_…` notes of the `oopz` catalog, user messages:
the person reads them as the proof the result is safe. A failed check is an error (`merge_check`) whose note is a
developer's `check_…` inside a user's. A column or pragma name (`sort_id`, `integrity_check`) stays as WhatsApp and
SQLite write it, in every language. Rows (owner, 2026-11-12), a test each:

| # | Case | What happens | Test |
|---|---|---|---|
| 1 | A counted value in a check | its form: "1 untouched table", "2 untouched tables"; Spanish one, many, other | `oopz` `checks_1_a_counted_value_takes_its_form` |
| 2 | A table or column name holding braces or `{name}` | a value, put in as it is (msgkinds row 19) | `oopz` `checks_2_a_table_name_stays_text` |
| 3 | The checks' order | the order they ran, kept from the merge to the CLI and the wire | `oopz` `checks_3_the_checks_keep_their_order` |
| 4 | A merge refused before its checks | an error and no checks, never a sentence standing in for them | `oopz` `checks_4_a_refused_merge_has_no_checks` |
| 5 | The desktop's wire (`MergeOutcome.checks`) | each check's English text until notes travel as notes (roadmap item 6) | `oopz_app` `merge_writes_a_new_folder_and_leaves_the_originals` |
| 6 | The CLI in Spanish | every check in Spanish, no English line (no check is a developer's) | the binary's `checks_6_restore_words_its_checks_in_the_commands_language` |

## The receive page

The phone's receive page (`crates/oopz-app/src/receive_page.html`) keeps no words of its own: the computer serves it
with every `receive_page_…` and `receive_refused_…` message of the `oopz_app` catalog worded in the phone's language,
as a JSON object in a `<script type="application/json">`, and the page shows each element's words by message name
(`data-t`). The phone's language is read by the computer from its `Accept-Language` (msgkinds' `Catalog::find`, in
the order the phone lists its languages; weights aren't read), so the rule is msgkinds' and tested in Rust. A
refused file is answered with its HTTP status (unchanged) and a stable code, the message's name
(`receive_refused_exists`); a received one with its bytes and SHA-256, as before. Rows (owner, 2026-11-12), a test
each (`oopz_app` `receive::page::tests`, the HTTP test `a_phone_sends_files_with_the_token_and_they_arrive_whole`,
and `pnpm check:receive-page` in `apps/desktop` for the page's own `answer`):

| # | Case | What happens | Test |
|---|---|---|---|
| 7 | A code the computer answers | has a message the page is sent, in every language; every `receive_refused_…` message is a code it can answer; statuses unchanged | `page_7_every_code_has_its_words_in_every_language`, `page_7_the_statuses_are_unchanged` |
| 8 | A phone language oopz lacks (`fr`, `pt-BR`), none, `*` | the catalog's default (English), never a blank | `page_8_a_language_oopz_lacks_is_the_default`; the HTTP test |
| 9 | `es-419`, `es-MX`, `ES-es`; several languages listed | Spanish; the first listed that oopz has | `page_9_regional_spanish_is_spanish`; the HTTP test |
| 10 | A code the page doesn't know (a newer computer) | the generic "not received" words and the code | `check:receive-page` |
| 11 | A wording holding `</script>`, `<!--`, quotes, backslashes, U+2028 | escaped in the embedded JSON (`<`, `>`, `&`, U+2028, U+2029 as `\u` escapes): it can't end the script | `page_11_the_words_cant_break_the_page` |
| 12 | A body that isn't a code (a proxy's error page), or none | "not received" and the HTTP status, never the body | `check:receive-page` |
| 13 | What an answer holds | a code, or bytes and SHA-256: never a file's name, its content or the token; the receiver logs nothing | `page_13_an_answer_is_a_code_only`; the HTTP test |
| 14 | The token | only in the page's URL (and its QR code), never in the served page | the HTTP test |

Rows 1–6 are a merge's checks (above).

## Errors

Every error is a stable `code` and a note: `oopz_model::Error::note()`, `oopz_core::Error.note`, `oopz_app::AppError.note`,
worded in the person's language at the last moment; `Display` is the English. What was being done is a `doing_…`
note and the system's reason a value. The desktop's wire stays `{code, detail}`, the detail English, until notes go
on it (roadmap item 6). Rows (owner, 2026-11-12), a test each:

| # | Case | What happens | Test |
|---|---|---|---|
| 1 | An OS error's text (`io::Error`), a library's (SQLite's `integrity_check`) | a value, as the system wrote it (on Windows in its own language), never translated by oopz | `oopz_model` `errors_1_a_systems_reason_is_a_value_as_given`; SQLite's `integrity_check` text too: `say` `errors_1_sqlites_own_words_are_a_value` |
| 2 | A path; a key, password, number or message text | a path is a value, never a message; the others never appear in an error | `oopz_app` `errors_2_a_key_never_appears_in_an_error` (an invented 64-hex key, spaced or not, on garbage, an empty file and the wrong backup) |
| 3 | Cancelled, needs a secret, wrong secret | no values | `oopz_model` `errors_3_a_refusal_with_no_values` |
| 4 | The CLI's `anyhow` chain | each cause once, `: ` between them; one that is an `oopz_model::Error`, `oopz_core::Error` or `AppError` is worded by its note in the command's language (`say::error`), others as they wrote themselves; no error has a `source()`, so the system's reason isn't said twice | the binary's `errors_4_a_failing_import_is_worded_in_the_commands_language`; `say` `errors_4_each_cause_of_the_chain_once`; `oopz_app` `errors_4_no_error_repeats_its_cause` |
| 5 | The desktop's wire | `{code, detail}`, English detail, codes unchanged (stored names) | `oopz_core` `errors_5_the_wire_is_code_and_english_detail`; the shell's `errors_5_the_apps_wire_is_code_and_english_detail` |
| 6 | Conversions (SQLite, JSON, crypt15) | keep their codes; which crypt15 header check failed is typed (`not_crypt15_no_iv` is `crypt14`), never read from English | `oopz_core` `errors_6_conversions_keep_their_codes`; `oopz_app` `errors_6_the_backup_errors_keep_their_codes`; `oopz` `errors_6_each_header_refusal_says_which` |
| 7 | An error wrapped by another crate's | keeps its note (a `Value::Note`, or the same note): one language throughout | `oopz_model` `errors_7_a_note_inside_is_worded_in_the_same_language`; the shell's `errors_7_an_app_error_keeps_the_librarys_note` |
| 8 | Developer-only texts (a programming mistake's) | `for = "developer"`, English only, when a person can see them (the shell's `window_failed`, the CLI's `handled_above`, the backup crates' diagnostics, a chat's facts: `fact_…`, shown English by the CLI's `chat-facts` and the desktop's diagnostics); a pure invariant's `expect` (a slice's fixed length, a match arm's card kind) stays as it is | the generator's checks (msgkinds' rows 12, 13) |
| 9 | ios-backup's and whatsapp-android's diagnostics | both crates word their errors from the `oopz` catalog (they depend on `oopz-model`, as the adapters do); what a person can only report (a keybag field, a table name, a merge check) is a developer message inside a user one, the rest user messages | `oopz_ios_backup` `errors_9_a_diagnostic_is_a_developers_inside_a_users_words` |

A stop asked for (a progress callback saying no) is code `cancelled` everywhere: oopz-core's, oopz-app's and the
backup crates' errors (tests: `oopz_app` `errors_6_the_backup_errors_keep_their_codes`, the index's and the shell's
stop tests). A copy's or a receive page's `stopped` is their status (why they stopped), not an error.
