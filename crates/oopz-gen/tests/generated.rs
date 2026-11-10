//! Generated backups open with oopz's own code: decryption, schema detection, every per-message query, the search
//! index, the stats, the export and the contacts' names.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use oopz::Backup;
use oopz::crypt15::Key;
use oopz_app::Session;
use oopz_gen::spec::Spec;

fn manifest(dir: &Path) -> HashMap<String, u64> {
    fs::read_to_string(dir.join("manifest.txt"))
        .expect("manifest")
        .lines()
        .filter_map(|l| l.split_once(' '))
        .filter_map(|(k, v)| Some((k.to_owned(), v.parse().ok()?)))
        .collect()
}

fn generate(spec: &Spec) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("temp dir");
    let key = oopz_gen::key_for(spec.seed);
    oopz_gen::generate(spec, dir.path(), &key, &mut |_, _| {}).expect("generate");
    (dir, key)
}

/// Every chat's messages, reactions, places, cards and polls read without an error, and the counts add up.
fn read_everything(dir: &Path, key: &str) -> u64 {
    let backup_path = dir.join("Databases/msgstore.db.crypt15");
    let key = Key::from_hex(key).expect("key");
    let tmp = tempfile::tempdir().expect("temp dir");
    let backup = Backup::open_in(&backup_path, &key, tmp.path()).expect("opens with the key");
    let health = backup.health().expect("health");
    assert_eq!(health.check, "ok");
    let schema = backup.schema();
    let db = backup.db();
    let mut read = 0;
    for chat in backup.chats().expect("chats") {
        let mut stmt = db.prepare(&schema.messages_query()).expect("messages query");
        let mut rows = stmt.query([chat.row_id]).expect("messages");
        while let Some(r) = rows.next().expect("a row") {
            read += 1;
            let row: i64 = r.get(23).expect("row id");
            let kind: i64 = r.get(3).expect("type");
            if kind == 5 || kind == 16 {
                let q = schema.location_query().expect("has locations");
                db.query_row(&q, [row], |r| r.get::<_, f64>(0)).expect("a place");
            }
            if kind == 4 {
                let q = schema.vcards_query().expect("has vcards");
                let card: String = db.query_row(q, [row], |r| r.get(0)).expect("a card");
                assert!(card.starts_with("BEGIN:VCARD"));
            }
            if kind == 46 {
                let q = schema.poll_query().expect("has polls");
                db.query_row(&q, [row], |r| r.get::<_, Option<i64>>(0)).expect("a poll");
                let options = schema.poll_options_query().expect("options");
                assert!(db.prepare(options).unwrap().query_map([row], |r| r.get::<_, i64>(0)).unwrap().count() >= 2);
                let votes = schema.poll_votes_query().expect("votes");
                db.prepare(&votes).unwrap().query_map([row], |r| r.get::<_, i64>(1)).unwrap().for_each(drop);
            }
        }
        let q = schema.reactions_query().expect("has reactions");
        db.prepare(&q).unwrap().query_map([chat.row_id], |r| r.get::<_, String>(0)).unwrap().for_each(drop);
        backup.type_profile(chat.row_id).expect("type profile");
        backup.chat_facts(chat.row_id).expect("chat facts");
        backup.key_ids(chat.row_id).expect("key ids");
    }
    assert_eq!(read, health.messages);
    read
}

#[test]
fn small_backup_opens_indexes_searches_and_exports() {
    let spec = Spec::preset("small").unwrap();
    let (dir, key) = generate(&spec);
    let counts = manifest(dir.path());
    assert_eq!(read_everything(dir.path(), &key), counts["messages"]);

    let tmp = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let summary = session.check(&dir.path().join("Databases/msgstore.db.crypt15"), &key, tmp.path()).expect("check");
    assert!(summary.sound);
    assert_eq!(summary.messages, counts["messages"]);
    assert_eq!(summary.groups, u64::from(spec.groups));
    assert_eq!(summary.one_to_one, u64::from(spec.contacts));
    // Every message falls between the start and end dates.
    assert!(summary.first_ms.unwrap() >= spec.start_ms && summary.last_ms.unwrap() <= spec.end_ms);
    session.build_index(summary.id, tmp.path()).expect("index");
    let stats = session.stats(summary.id, None).expect("stats");
    assert_eq!(stats.messages, counts["messages"]);
    for code in [0, 1, 2, 3, 7, 9, 20, 90] {
        assert!(stats.by_kind.iter().any(|(k, n, _)| *k == code && *n > 0), "type {code} in the stats");
    }
    assert!(!session.search(summary.id, "hola", None, 20).expect("search").is_empty());
    let main = session.chat_len(summary.id, 1).expect("chat 1");
    assert_eq!(main, counts["main_chat_messages"]);
    assert_eq!(session.lines(summary.id, 1, 0, 50).expect("lines").len(), 50);

    // Placeholder media where the rows point, and the contacts' names beside the backup.
    assert!(counts["media_files"] > 0);
    assert!(dir.path().join("Media/WhatsApp Images").is_dir());
    let contacts = oopz::contacts::Contacts::beside(
        &dir.path().join("Databases/msgstore.db.crypt15"),
        &Key::from_hex(&key).unwrap(),
        tmp.path(),
    )
    .expect("contacts open");
    assert!(contacts.name("999900000001@s.whatsapp.net").unwrap().is_some());
}

#[test]
fn edge_backup_reads_without_errors() {
    let spec = Spec::preset("edge").unwrap();
    let (dir, key) = generate(&spec);
    let counts = manifest(dir.path());
    assert_eq!(read_everything(dir.path(), &key), counts["messages"]);
    for kind in oopz_gen::spec::Kind::ALL {
        assert!(counts.get(&format!("kind.{}", kind.name())).is_some_and(|n| *n > 0), "{} written", kind.name());
    }
    let tmp = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let summary = session.check(&dir.path().join("Databases/msgstore.db.crypt15"), &key, tmp.path()).expect("check");
    session.build_index(summary.id, tmp.path()).expect("index");
    // Text that looks like SQL or markup is only text.
    assert!(session.search(summary.id, "DROP TABLE", None, 5).is_ok());
    assert!(session.search(summary.id, "\"unbalanced", None, 5).is_ok());
    let stats = session.stats(summary.id, Some(1)).expect("stats");
    assert!(stats.by_kind.iter().any(|(k, _, _)| *k == 12_345), "the unknown type is counted, not dropped");
}

#[test]
fn same_seed_same_backup_and_a_full_folder_is_refused() {
    let mut spec = Spec::preset("small").unwrap();
    spec.main_chat = 500;
    spec.media = oopz_gen::spec::MediaMode::None;
    let (a, key) = generate(&spec);
    let (b, _) = generate(&spec);
    let file = |d: &Path| fs::read(d.join("Databases/msgstore.db.crypt15")).unwrap();
    assert_eq!(file(a.path()), file(b.path()));
    assert!(oopz_gen::generate(&spec, a.path(), &key, &mut |_, _| {}).is_err());
    spec.seed = 2;
    let (c, _) = generate(&spec);
    assert_ne!(file(a.path()), file(c.path()));
}

#[test]
fn a_bad_spec_is_refused_before_anything_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut spec = Spec::preset("small").unwrap();
    spec.end_ms = spec.start_ms;
    assert!(oopz_gen::generate(&spec, &dir.path().join("x"), &oopz_gen::key_for(1), &mut |_, _| {}).is_err());
    assert!(!dir.path().join("x").exists());
    let spec = Spec::preset("small").unwrap();
    assert!(oopz_gen::generate(&spec, &dir.path().join("y"), "not a key", &mut |_, _| {}).is_err());
    assert!(!dir.path().join("y").exists());
}
