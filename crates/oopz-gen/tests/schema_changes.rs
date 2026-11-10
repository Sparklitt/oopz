//! What happens when WhatsApp changes its database: a generated (or fixture) backup is decrypted, changed the way a
//! WhatsApp update might change it, encrypted again, and run through the app's whole path. Each change either works
//! (extra or missing optional parts) or is refused with a clear error naming the table or column, never a panic, a
//! wrong result or a written file.

use std::fs;
use std::path::{Path, PathBuf};

use oopz::crypt15::{self, Key};
use oopz::fixture;
use oopz_app::Session;
use oopz_gen::spec::{MediaMode, Spec};

/// Runs `sql` on the database inside the crypt15 file at `path`, in place.
fn change(path: &Path, key_hex: &str, sql: &str) {
    let key = Key::from_hex(key_hex).unwrap();
    let plain = crypt15::decrypt(&fs::read(path).unwrap(), &key).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("db");
    fs::write(&db_path, plain).unwrap();
    let db = rusqlite::Connection::open(&db_path).unwrap();
    db.execute_batch(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
    drop(db);
    fs::write(path, crypt15::encrypt(&fs::read(&db_path).unwrap(), &key, [9; 16], true)).unwrap();
}

/// A small generated backup to change.
fn generated() -> (tempfile::TempDir, PathBuf, String) {
    let mut spec = Spec::preset("small").unwrap();
    spec.main_chat = 300;
    spec.per_chat = (5, 40);
    spec.media = MediaMode::None;
    spec.contacts_db = false;
    let dir = tempfile::tempdir().unwrap();
    let key = oopz_gen::key_for(spec.seed);
    oopz_gen::generate(&spec, dir.path(), &key, &mut |_, _| {}).unwrap();
    let backup = dir.path().join("Databases/msgstore.db.crypt15");
    (dir, backup, key)
}

/// Everything the app does with one backup: check, index, stats, search, the viewer, and the diagnostics.
fn use_it(backup: &Path, key: &str) -> Result<(), oopz_app::AppError> {
    let tmp = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let s = session.check(backup, key, tmp.path())?;
    session.build_index(s.id, tmp.path())?;
    session.stats(s.id, None)?;
    session.search(s.id, "hola", &oopz_app::Filter::default(), 50)?;
    for chat in &s.chat_list {
        let n = session.chat_len(s.id, chat.number)?;
        session.lines(s.id, chat.number, n.saturating_sub(50), 50)?;
        session.types(s.id, chat.number)?;
        session.chat_facts(s.id, chat.number)?;
        session.missing(s.id, chat.number, s.id, chat.number)?;
    }
    session.tables(s.id)?;
    Ok(())
}

#[test]
fn optional_parts_missing_still_open() {
    let (_dir, backup, key) = generated();
    change(
        &backup,
        &key,
        "DROP TABLE message_media; DROP TABLE message_quoted; DROP TABLE message_add_on;
         DROP TABLE message_add_on_reaction; DROP TABLE message_location; DROP TABLE message_vcard;
         DROP TABLE message_poll; DROP TABLE message_poll_option; DROP TABLE call_log; DROP TABLE message_call_log;
         DROP TABLE message_forwarded; DROP TABLE message_edit_info; DROP TABLE message_text;
         DROP TABLE message_association; DROP TABLE message_system;
         ALTER TABLE message DROP COLUMN starred; ALTER TABLE message DROP COLUMN sender_jid_row_id;
         ALTER TABLE chat DROP COLUMN subject;",
    );
    use_it(&backup, &key).expect("a backup without the optional tables and columns opens");
}

#[test]
fn new_tables_and_columns_are_ignored() {
    let (_dir, backup, key) = generated();
    change(
        &backup,
        &key,
        "CREATE TABLE message_future (message_row_id INTEGER PRIMARY KEY, data BLOB NOT NULL);
         INSERT INTO message_future SELECT _id, x'00' FROM message LIMIT 10;
         ALTER TABLE message ADD COLUMN future_flag INTEGER NOT NULL DEFAULT 7;
         ALTER TABLE message ADD COLUMN future_text TEXT;
         ALTER TABLE chat ADD COLUMN future INTEGER; ALTER TABLE jid ADD COLUMN future INTEGER;
         PRAGMA user_version = 9999;",
    );
    use_it(&backup, &key).expect("unknown tables, columns and a newer user_version don't matter");
}

#[test]
fn odd_values_in_known_columns_open() {
    let (_dir, backup, key) = generated();
    // Values of the wrong type, NULLs where WhatsApp always writes one, negative and huge times.
    change(
        &backup,
        &key,
        "UPDATE message SET timestamp = 'not a time' WHERE _id = 5;
         UPDATE message SET message_type = NULL WHERE _id = 6;
         UPDATE message SET text_data = x'fffe00' WHERE _id = 8;
         UPDATE message SET timestamp = -5 WHERE _id = 9;
         UPDATE message SET timestamp = 9223372036854775807 WHERE _id = 10;
         UPDATE message SET key_id = NULL WHERE _id = 11;
         UPDATE message SET chat_row_id = 999999 WHERE _id = 12;
         UPDATE message SET sender_jid_row_id = 424242 WHERE _id = 13;
         UPDATE jid SET raw_string = NULL WHERE _id = 3;",
    );
    use_it(&backup, &key).expect("odd values open");
}

/// A required table or column gone: refused, naming it, with the app's `unsupported` code.
#[test]
fn a_missing_required_part_is_refused_by_name() {
    for (sql, named) in [
        ("ALTER TABLE message DROP COLUMN key_id;", "message.key_id"),
        ("ALTER TABLE message DROP COLUMN timestamp;", "message.timestamp"),
        ("ALTER TABLE jid DROP COLUMN raw_string;", "jid.raw_string"),
        ("DROP TABLE chat;", "chat"),
        ("ALTER TABLE message RENAME TO messages;", "messages"),
    ] {
        let (_dir, backup, key) = generated();
        // Dropping an indexed column needs its index gone first.
        let sql = format!("DROP INDEX IF EXISTS message_key_index; DROP INDEX IF EXISTS jid_key_index; {sql}");
        change(&backup, &key, &sql);
        let e = use_it(&backup, &key).expect_err(sql.as_str());
        assert_eq!(e.code, "unsupported", "{sql}: {e}");
        assert!(e.detail.contains(named), "{sql}: {e}");
    }
}

#[test]
fn an_empty_database_is_refused() {
    let (_dir, backup, key) = generated();
    let tables: Vec<String> = {
        let k = Key::from_hex(&key).unwrap();
        let plain = crypt15::decrypt(&fs::read(&backup).unwrap(), &k).unwrap();
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("db"), plain).unwrap();
        let db = rusqlite::Connection::open(d.path().join("db")).unwrap();
        let mut s =
            db.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'").unwrap();
        s.query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap()
    };
    let drops: String = tables.iter().map(|t| format!("DROP TABLE \"{t}\";")).collect();
    change(&backup, &key, &drops);
    assert_eq!(use_it(&backup, &key).unwrap_err().code, "unsupported");
}

/// Merging into a target whose WhatsApp is newer than the source's.
fn merge(change_target: &str) -> (Result<oopz_app::MergeOutcome, oopz_app::AppError>, tempfile::TempDir) {
    let fx = fixture::two_phones();
    if !change_target.is_empty() {
        change(&fx.target, fixture::TARGET_KEY_HEX, change_target);
    }
    let out = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let source = session.check(&fx.source, fixture::KEY_HEX, out.path()).unwrap().id;
    let target = session.check(&fx.target, fixture::TARGET_KEY_HEX, out.path()).unwrap().id;
    let done = session.merge(&oopz_app::MergeRequest {
        source,
        source_chat: fixture::SOURCE_CHAT,
        target,
        target_chat: fixture::TARGET_CHAT,
        target_key: fixture::TARGET_KEY_HEX,
        out_parent: out.path(),
        media: None,
    });
    (done, out)
}

#[test]
fn merge_into_a_target_with_new_optional_parts_works() {
    let (done, _out) = merge(
        "ALTER TABLE message ADD COLUMN future_flag INTEGER DEFAULT 0;
         CREATE TABLE message_future (message_row_id INTEGER PRIMARY KEY, data TEXT);
         ALTER TABLE message_media ADD COLUMN future TEXT;",
    );
    let done = done.expect("extra optional columns and tables in the target");
    assert!(done.added > 0 && done.original_untouched);
}

#[test]
fn merge_into_a_target_needing_a_value_the_source_lacks_is_refused_cleanly() {
    // A new NOT NULL column without a default: rows copied from an older source can't fill it.
    let (done, out) = merge(
        "ALTER TABLE message_quoted ADD COLUMN future_required INTEGER NOT NULL DEFAULT 0;
        CREATE TABLE q2 AS SELECT * FROM message_quoted; DROP TABLE message_quoted;
        CREATE TABLE message_quoted (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER,
            parent_message_chat_row_id INTEGER, from_me INTEGER, sender_jid_row_id INTEGER, key_id TEXT,
            timestamp INTEGER, message_type INTEGER, text_data TEXT, future_required INTEGER NOT NULL);
        INSERT INTO message_quoted SELECT * FROM q2; DROP TABLE q2;",
    );
    let e = done.expect_err("refused");
    assert_eq!(e.code, "merge_check", "{e}");
    assert!(e.detail.contains("message_quoted.future_required"), "{e}");
    // Nothing written.
    let made: Vec<_> = fs::read_dir(out.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("oopz-result"))
        .collect();
    assert!(made.is_empty(), "a folder was written");
}
