# The app and the CLI do the same things

Every desktop command (`apps/desktop/src-tauri/src/lib.rs`) and every `oopz` CLI command runs the same `oopz-app`
code, so both give the same results and refuse the same things. A new feature goes into `oopz-app` first, then the CLI,
then the app. `crates/oopz-cli/tests/parity.rs` fails when a command on either side is missing from this table.

The CLI never prints message text, numbers or keys: what the app shows on screen, the CLI writes to a file the person
names (`oopz export`), or prints as counts and positions.

| App command | CLI command | Shared code |
|---|---|---|
| `find_backups` | `oopz backups` | `session::find_backups` |
| `check_backup` | `oopz check`, `oopz chats` (names with `--names`), `oopz backups` | `Session::check` |
| `build_index` | `oopz stats`, `oopz search`, `oopz export` (built when needed) | `Session::build_index` |
| `stats` | `oopz stats` | `Session::stats` |
| `search`, `message_position` | `oopz search` (where, not what; filters by type, side or sender, dates) | `Session::search`, `Session::position` |
| `position_at` | `oopz jump` | `Session::position_at` |
| `thumbnail` | `oopz thumbnail` (to a file) | `Session::thumbnail` |
| `chat_len`, `chat_lines` | `oopz export --chat` (the chat's text, to a file) | `Session::lines` |
| `export_page` | `oopz export` | `Session::export` (the CLI's words: `Labels::builtin`) |
| `missing` | `oopz missing` | `Session::missing` |
| `merge_chat` | `oopz restore` | `Session::merge` |
| `receive_start`, `receive_status`, `receive_stop` | `oopz receive` | `receive::start_in` |
| `backup_tables` | `oopz tables` | `Session::tables` |
| `chat_types` | `oopz types` | `Session::types` |
| `chat_facts` | `oopz chat-facts` | `Session::chat_facts` |
| `checked_backups`, `forget_backup` | app only: the app keeps backups open between screens; each CLI run opens and closes its own | `Session` |
| `show_folder` | app only: opens the system's file manager | |
| `cancel_work` | app only: Ctrl-C stops a CLI run, and its copies go with it | the progress callbacks (`Cancelled`) |
| `wipe_now` | app only: each CLI run removes its decrypted copies when it ends | `Session` |
| `import_backup`, `save_result` | app only: Android's file picker hands the app references, not paths (decision 0012); the CLI takes paths | |
| `open_link` | app only: the CLI's `--help` names the repository | |
| `update_check`, `update_install`, `update_last_checked` | app only: the app updates itself (decision 0011); the CLI binary is replaced by hand | |

## The library (every app, decision 0016)

The library's commands run `oopz-core` (`Library`) and `oopz-export` / `oopz-export-pdf`. The CLI's library is the
folder named with `--library`; the app's lives in its cache folder and is removed when it closes, unless "Keep my
library" is on. Like the rest of the CLI, `oopz lib` prints counts, ids, dates and positions; names only with `--names`
(a name that is a number or an address masked); text only into a file (`oopz lib export`).

| App command | CLI command | Shared code |
|---|---|---|
| | `oopz lib` (the group) | |
| | `oopz lib adapters` | `oopz_app::adapters` |
| | `oopz lib import` | `Library::import_path` |
| | `oopz lib sources` | `Library::sources` |
| | `oopz lib remove` | `Library::remove_source` |
| | `oopz lib set-me` | `Library::set_me` |
| | `oopz lib convs` | `Library::conversations` |
| | `oopz lib jump` | `Library::position_at`, `Library::conversation_len` |
| | `oopz lib search` | `Library::search`, `Library::position` |
| | `oopz lib timeline` | `Library::find` (`Order::Time`), `Library::count` |
| | `oopz lib count` | `Library::count`, `Library::selected` |
| | `oopz lib stats` | `Library::stats` |
| | `oopz lib people` | `Library::people` |
| | `oopz lib contacts` | `Library::contacts` |
| | `oopz lib suggest`, `oopz lib accept` | `Library::suggestions`, `Library::accept` |
| | `oopz lib link`, `oopz lib unlink` | `Library::link`, `Library::unlink` |
| | `oopz lib rename`, `oopz lib forget` | `Library::rename_contact`, `Library::delete_contact` |
| | `oopz lib vcard-import` | `Library::import_vcards` |
| | `oopz lib vcard-export` | `oopz_export::export_contacts` |
| | `oopz lib select`, `oopz lib selections`, `oopz lib unselect` | `Library::save_selection`, `Library::selections`, `Library::delete_selection` |
| | `oopz lib export` (html, json, csv, txt, vcard, media-zip; pdf with the `pdf` feature) | `oopz_export::export`, `oopz_export_pdf::export_pdf` |
