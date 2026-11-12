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
                assert!(
                    msgkinds::media_type::sniff(card.as_bytes(), msgkinds::media_type::Hint::None)
                        .is_some_and(|s| s.format.mime == msgkinds::media_type::formats::VCARD)
                );
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
    assert!(!session.search(summary.id, "hola", &oopz_app::Filter::default(), 20).expect("search").is_empty());
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
    assert!(session.search(summary.id, "DROP TABLE", &oopz_app::Filter::default(), 5).is_ok());
    assert!(session.search(summary.id, "\"unbalanced", &oopz_app::Filter::default(), 5).is_ok());
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

/// What the viewer shows besides text (replies, reactions, edits, forwards, polls, calls, media), the chat list's
/// names and previews, search filters, "jump to a date" and the day × hour heatmap, against the manifest's counts.
#[test]
fn the_viewer_sees_everything_the_backup_holds() {
    let spec = Spec::preset("small").unwrap();
    let (dir, key) = generate(&spec);
    let counts = manifest(dir.path());
    let tmp = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let s = session.check(&dir.path().join("Databases/msgstore.db.crypt15"), &key, tmp.path()).expect("check");
    assert!(s.chat_list.iter().any(|c| c.kind == "group" && c.name.is_some()), "a group's subject");
    assert!(s.chat_list.iter().any(|c| c.kind == "contact" && c.name.is_some()), "a contact's name from wa.db");
    assert!(s.chat_list.iter().filter(|c| c.messages > 0).all(|c| c.preview.is_some()), "every chat has a preview");
    assert!(s.chat_list.iter().filter_map(|c| c.preview.as_ref()?.text.as_ref()).all(|t| t.chars().count() <= 80));
    session.build_index(s.id, tmp.path()).expect("index");

    let (mut replies, mut found, mut reactions, mut edited, mut forwarded, mut polls, mut calls, mut media) =
        (0, 0, 0, 0, 0, 0, 0, 0);
    for chat in &s.chat_list {
        let len = session.chat_len(s.id, chat.number).unwrap();
        let mut from = 0;
        while from < len {
            for line in session.lines(s.id, chat.number, from, 500).unwrap() {
                if let Some(reply) = &line.reply {
                    replies += 1;
                    found += u64::from(reply.id.is_some());
                }
                reactions += line.reactions.len() as u64;
                edited += u64::from(line.edited);
                forwarded += u64::from(line.forwarded > 0);
                if line.kind == 46 {
                    assert!(line.poll.len() >= 2, "a poll has its options");
                    polls += 1;
                }
                calls += u64::from(line.call.is_some());
                media += u64::from(line.media.as_ref().is_some_and(|m| m.mime.is_some()));
            }
            from += 500;
        }
    }
    assert_eq!(replies, counts["replies"]);
    assert!(found > 0 && found <= replies, "replies point at the answered message");
    assert!(reactions > 0 && reactions <= counts["reactions"], "removed reactions aren't shown");
    assert_eq!(edited, counts["edits"]);
    assert!(forwarded > 0 && forwarded <= counts["forwarded"]);
    assert_eq!(polls, counts["kind.poll"]);
    assert_eq!(calls, counts["kind.call"]);
    assert!(media > 0);

    // Filters: only photos, only sent, a date range; with no words, the newest that match.
    let photos = oopz_app::Filter { kinds: vec![1], ..Default::default() };
    let hits = session.search(s.id, "", &photos, 50).unwrap();
    assert!(!hits.is_empty());
    let sent = oopz_app::Filter { from_me: Some(true), chat: Some(1), ..Default::default() };
    assert!(session.search(s.id, "", &sent, 50).unwrap().iter().all(|h| h.from_me && h.chat == 1));
    let mid = (spec.start_ms + spec.end_ms) / 2;
    let late = oopz_app::Filter { after_ms: Some(mid), ..Default::default() };
    assert!(session.search(s.id, "", &late, 50).unwrap().iter().all(|h| h.ts >= mid));
    assert!(session.search(s.id, "", &oopz_app::Filter::default(), 50).unwrap().is_empty(), "nothing asked");

    // Jump to a date: the first message at or after it.
    let at = session.position_at(s.id, 1, mid).unwrap().expect("chat 1 has messages");
    let line = &session.lines(s.id, 1, at, 1).unwrap()[0];
    assert!(line.ts >= mid);
    if at > 0 {
        assert!(session.lines(s.id, 1, at - 1, 1).unwrap()[0].ts < mid);
    }

    let stats = session.stats(s.id, None).unwrap();
    assert_eq!(stats.by_day_hour.len(), 7);
    assert_eq!(stats.by_day_hour.iter().flatten().sum::<u64>(), stats.by_hour.iter().sum::<u64>());
}

/// On Android the picked files arrive with neutral names (picked-1.crypt15, picked-2.crypt15): the contacts file is
/// still found beside the backup by what it holds; a dated message backup beside it isn't taken for contacts.
#[test]
fn names_come_from_a_contacts_file_whatever_it_is_called() {
    let spec = Spec::preset("small").unwrap();
    let (dir, key) = generate(&spec);
    let db = dir.path().join("Databases");
    fs::rename(db.join("msgstore.db.crypt15"), db.join("picked-1.crypt15")).unwrap();
    fs::copy(db.join("picked-1.crypt15"), db.join("picked-0.crypt15")).unwrap();
    fs::rename(db.join("wa.db.crypt15"), db.join("picked-2.crypt15")).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    let s = session.check(&db.join("picked-1.crypt15"), &key, tmp.path()).expect("check");
    assert!(s.chat_list.iter().any(|c| c.kind == "contact" && c.name.is_some()), "names from picked-2");
    // Without a contacts file: no names, no error.
    fs::remove_file(db.join("picked-2.crypt15")).unwrap();
    let s = session.check(&db.join("picked-1.crypt15"), &key, tmp.path()).expect("check");
    assert!(s.chat_list.iter().filter(|c| c.kind == "contact").all(|c| c.name.is_none()));
}

#[test]
fn the_whatsapp_adapter_gives_the_model_everything_the_backup_holds() {
    use oopz_model::{Adapter, Kind, Part};
    let spec = Spec::preset("small").unwrap();
    let (dir, key) = generate(&spec);
    let counts = manifest(dir.path());
    let adapter = oopz_app::adapter::WhatsAppAdapter;
    assert!(adapter.detect(dir.path()));
    assert!(matches!(adapter.open(dir.path(), None), Err(oopz_model::Error::NeedsSecret)));
    let wrong = oopz_gen::key_for(spec.seed + 1);
    assert!(matches!(adapter.open(dir.path(), Some(&wrong)), Err(oopz_model::Error::WrongSecret)));
    let mut source = adapter.open(dir.path(), Some(&key)).expect("open");
    assert_eq!(source.conversations().len() as u64, counts["chats"]);
    assert!(source.people().iter().any(|p| p.me));
    // Names come from the contacts file beside the backup; phone numbers only from phone ids.
    assert!(source.people().iter().any(|p| !p.me && p.name.is_some()));
    assert!(source.people().iter().filter_map(|p| p.phone.as_deref()).all(|p| p.starts_with("+9999")));
    let ids: Vec<String> = source.conversations().iter().map(|c| c.id.clone()).collect();
    let (mut total, mut reactions, mut replies, mut edits, mut forwarded) = (0, 0, 0, 0, 0);
    let mut kinds: HashMap<Kind, u64> = HashMap::new();
    for id in &ids {
        let mut last = i64::MIN;
        source
            .messages(id, &mut |msg| {
                total += 1;
                reactions += msg.reactions.len() as u64;
                replies += u64::from(msg.reply_to.is_some());
                edits += u64::from(msg.edited);
                forwarded += u64::from(msg.forwarded);
                *kinds.entry(msg.kind()).or_default() += 1;
                assert_eq!(&msg.conversation, id);
                assert!(msg.from_me == (msg.sender.as_deref() == Some(oopz_app::adapter::ME)));
                if let Some(t) = msg.time_ms {
                    assert!(t >= last, "oldest first");
                    last = t;
                }
                for part in &msg.parts {
                    if let Part::Media(m) = part {
                        assert!(!m.view_once || m.caption.is_none(), "a view-once message's text is never kept");
                    }
                }
                true
            })
            .expect("messages");
    }
    assert_eq!(total, counts["messages"]);
    assert!(reactions > 0 && reactions <= counts["reactions"], "removed and replaced reactions aren't kept");
    assert_eq!(edits, counts["edits"]);
    assert_eq!(forwarded, counts["forwarded"]);
    assert!(replies > 0 && replies <= counts["replies"]);
    for (kind, key) in [(Kind::Call, "kind.call"), (Kind::Contact, "kind.contact"), (Kind::Poll, "kind.poll")] {
        assert_eq!(kinds.get(&kind).copied().unwrap_or(0), counts[key], "{key}");
    }
    assert_eq!(kinds.get(&Kind::Deleted).copied().unwrap_or(0), counts["kind.deleted"]);
    // Stopping the stream is reported, not swallowed.
    assert!(matches!(source.messages(&ids[0], &mut |_| false), Err(oopz_model::Error::Cancelled)));
}

#[test]
fn a_whatsapp_backup_goes_into_the_library_whole() {
    use oopz_core::{Filter, Library, Order};
    let spec = Spec::preset("small").unwrap();
    let (dir, key) = generate(&spec);
    let counts = manifest(dir.path());
    let lib_dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(lib_dir.path()).unwrap();
    let adapters: [&dyn oopz_model::Adapter; 1] = [&oopz_app::adapter::WhatsAppAdapter];
    let wrong = lib.import_path(&adapters, dir.path(), None, Some(&oopz_gen::key_for(9)), &mut |_, _| true);
    assert_eq!(wrong.unwrap_err().code, "wrong_key");
    assert_eq!(lib.import_path(&adapters, dir.path(), None, None, &mut |_, _| true).unwrap_err().code, "needs_secret");
    let started = std::time::Instant::now();
    let source = lib.import_path(&adapters, dir.path(), None, Some(&key), &mut |_, _| true).expect("import");
    eprintln!("imported {} messages in {:?}", source.messages, started.elapsed());
    assert_eq!(source.messages, counts["messages"]);
    assert_eq!(source.conversations, counts["chats"]);
    assert!(source.owner_known && !source.wall_clock);
    let again = lib.import_path(&adapters, dir.path(), None, Some(&key), &mut |_, _| true);
    assert_eq!(again.unwrap_err().code, "exists");
    assert_eq!(lib.count(&Filter::default()).unwrap(), counts["messages"]);
    let stats = lib.stats(&Filter::default()).unwrap();
    assert_eq!(stats.messages, counts["messages"]);
    assert!(!lib.search(&Filter { words: "hola".into(), ..Default::default() }, 20).unwrap().is_empty());
    let first = lib.find(&Filter { sources: vec![source.id], ..Default::default() }, Order::Time, 0, 1).unwrap();
    assert_eq!(first[0].time_ms, stats.first_ms);
}
