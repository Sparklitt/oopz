use std::fs;

use rusqlite::Connection;

use super::*;
use crate::fixture::{self, KEY_HEX, SOURCE_CHAT, TARGET_CHAT, TARGET_KEY_HEX, TwoPhones};

fn keys() -> (Key, Key) {
    (Key::from_hex(KEY_HEX).unwrap(), Key::from_hex(TARGET_KEY_HEX).unwrap())
}

fn merge(fx: &TwoPhones, target: &[u8]) -> Result<Merged> {
    let (source_key, target_key) = keys();
    let source = Backup::open(&fx.source, &source_key).unwrap();
    ChatMerge { source: &source, source_chat: SOURCE_CHAT, target, target_key: &target_key, target_chat: TARGET_CHAT }
        .run()
}

/// The result, decrypted into a file that lives as long as the returned guard.
fn open(sealed: &[u8]) -> (Connection, tempfile::NamedTempFile) {
    let file = temp_copy(&crypt15::decrypt(sealed, &keys().1).unwrap()).unwrap();
    (Connection::open(file.path()).unwrap(), file)
}

fn rows<T: FromRow>(db: &Connection, sql: &str) -> Vec<T> {
    query_all(db, sql, []).unwrap()
}

#[test]
fn adds_the_missing_messages_mirrored_before_the_targets_own() {
    let fx = fixture::two_phones();
    let original = fs::read(&fx.target).unwrap();
    let Merged { backup: sealed, report, .. } = merge(&fx, &original).unwrap();

    assert_eq!((report.target_before, report.already_there, report.added, report.moved), (4, 2, 8, 4));
    assert_eq!((report.added_after_target_start, report.source_without_key, report.triggers), (0, 0, 1));
    assert_eq!((report.media_new_paths, report.media_reused, report.calls_reused), (2, 1, 1));
    let left: Vec<_> = report.left_behind.iter().map(|((t, why), n)| (t.as_str(), *why, *n)).collect();
    assert_eq!(left, [("receipt_user", LeftBehind::NotCopied, 1)]);
    assert_eq!(report.copied["message_add_on"], 2);
    assert_eq!(report.checks.len(), 9, "{report:?}");

    let (db, _file) = open(&sealed);
    // Rows 1..8 were used and 10 is still referred to: the added ones take 11..18 in Ana's order, Ben's chat moves
    // to 19..22, Cleo's rows stay where they were.
    let chat: Vec<(i64, i64, String, i64, i64)> =
        rows(&db, "SELECT _id, sort_id, key_id, from_me, status FROM message WHERE chat_row_id = 2 ORDER BY sort_id");
    let expected = [
        (11, "S1", 1, 6),
        (12, "A1", 0, 0),
        (13, "A2", 1, 13),
        (14, "A3", 1, 13),
        (15, "A4", 1, 13),
        (16, "A5", 0, 0),
        (17, "A6", 0, 0),
        (18, "A7", 0, 0),
        (19, "B1", 1, 13),
        (20, "B2", 0, 0),
        (21, "B3", 1, 13),
        (22, "B4", 0, 0),
    ];
    assert_eq!(chat.len(), expected.len());
    for ((id, sort, key, from_me, status), (eid, ekey, efrom, estatus)) in chat.iter().zip(expected) {
        assert_eq!((*id, *sort, key.as_str(), *from_me, *status), (eid, eid, ekey, efrom, estatus));
    }
    let cleo: Vec<(i64,)> = rows(&db, "SELECT _id FROM message WHERE chat_row_id = 1 ORDER BY _id");
    assert_eq!(cleo, [(1,), (3,), (4,), (8,)]);
    let seq: Vec<(String, i64)> = rows(&db, "SELECT name, seq FROM sqlite_sequence ORDER BY name");
    let seq: Vec<_> = seq.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    assert_eq!(seq, [("call_log", 2), ("message", 22), ("message_album", 13)]);
    let created: Vec<(i64,)> = rows(&db, "SELECT created_timestamp FROM chat ORDER BY _id");
    assert_eq!(created, [(1_700_000_000_000,), (1_700_000_000_000,)], "Ana's first message, S1");

    // Every reference to a moved row follows it; a stale one (row 10) stays stale.
    type Pointers = (i64, i64, i64, i64, i64, i64, i64, i64, i64);
    let pointers: Vec<Pointers> = rows(
        &db,
        "SELECT display_message_row_id, last_message_row_id, last_read_message_row_id,
            last_read_receipt_sent_message_row_id, last_important_message_row_id, last_read_message_sort_id,
            display_message_sort_id, last_message_sort_id, last_read_receipt_sent_message_sort_id
         FROM chat WHERE _id = 2",
    );
    assert_eq!(pointers, [(22, 22, 22, 22, 21, 22, 22, 22, 22)]);
    let receipts: Vec<(i64,)> = rows(&db, "SELECT message_row_id FROM receipt_user ORDER BY _id");
    assert_eq!(receipts, [(19,), (21,), (3,), (10,)]);

    // Mirrored details: the device jids Ben's chat uses, quotes and calls in his chat, media in mirrored folders.
    let devices: Vec<(i64, i64)> = rows(
        &db,
        "SELECT m.from_me, d.author_device_jid FROM message m JOIN message_details d ON d.message_row_id = m._id
         WHERE m.chat_row_id = 2 GROUP BY 1, 2",
    );
    assert_eq!(devices, [(0, 4), (1, 3)]);
    let quoted: Vec<(i64, i64, i64, i64, String)> = rows(
        &db,
        "SELECT chat_row_id, parent_message_chat_row_id, from_me, sender_jid_row_id, key_id FROM message_quoted
         WHERE message_row_id = 17",
    );
    assert_eq!(quoted, [(2, 2, 1, 0, "A3".into())]);
    let media: Vec<(i64, i64, String, i64)> =
        rows(&db, "SELECT message_row_id, chat_row_id, file_path, is_offloaded FROM message_media ORDER BY 1");
    let media: Vec<_> = media.iter().map(|(m, c, p, o)| (*m, *c, p.as_str(), *o)).collect();
    assert_eq!(
        media,
        [
            (3, 1, "Media/WhatsApp Images/Sent/IMG-20231120-WA0007.jpg", 0),
            (4, 1, "Media/WhatsApp Images/IMG-20231114-WA0001.jpg", 0),
            (14, 2, "Media/WhatsApp Images/Sent/hilo-IMG-20231114-WA0001.jpg", 0),
            (15, 2, "Media/WhatsApp Images/Sent/hilo-IMG-20231114-WA0002.jpg", 0),
            (16, 2, "Media/WhatsApp Images/Sent/IMG-20231120-WA0007.jpg", 0),
        ]
    );
    let refs: Vec<(String, i64)> = rows(&db, "SELECT path, ref_count FROM media_refs ORDER BY _id");
    assert_eq!(refs.iter().map(|r| r.1).collect::<Vec<_>>(), [2, 1, 1, 1]);
    let thumbnails: Vec<(i64,)> = rows(&db, "SELECT count(*) FROM media_hash_thumbnail");
    assert_eq!(thumbnails, [(4,)]);
    let album: Vec<(i64, i64)> =
        rows(&db, "SELECT child_message_row_id, parent_message_row_id FROM message_association ORDER BY 1");
    assert_eq!(album, [(14, 13), (15, 13)]);
    let calls: Vec<(i64, i64)> = rows(&db, "SELECT message_row_id, call_log_row_id FROM message_call_log");
    assert_eq!(calls, [(18, 2)]);
    let call_logs: Vec<(i64,)> = rows(&db, "SELECT count(*) FROM call_log");
    assert_eq!(call_logs, [(2,)]);

    // Ben's ❤️ on the reply and Ana's 😂 on B2 (now row 20); Ana's 👍 on B1 was already there.
    let reactions: Vec<(String, i64, i64, i64, i64, String)> = rows(
        &db,
        "SELECT a.key_id, a.chat_row_id, a.from_me, a.status, a.parent_message_row_id, r.reaction
         FROM message_add_on a JOIN message_add_on_reaction r ON r.message_add_on_row_id = a._id ORDER BY a._id",
    );
    let reactions: Vec<_> =
        reactions.iter().map(|(k, c, f, s, p, r)| (k.as_str(), *c, *f, *s, *p, r.as_str())).collect();
    assert_eq!(reactions, [("R2", 2, 0, 0, 19, "👍"), ("R1", 2, 1, 13, 17, "❤️"), ("R3", 2, 0, 0, 20, "😂")]);

    // A1's thread comes along mirrored, after Ben's own thread in Cleo's chat.
    let threads: Vec<(i64, i64, i64, String)> =
        rows(&db, "SELECT _id, chat_row_id, from_me, key_id FROM thread_id ORDER BY _id");
    let threads: Vec<_> = threads.iter().map(|(i, c, f, k)| (*i, *c, *f, k.as_str())).collect();
    assert_eq!(threads, [(1, 1, 0, "X1"), (2, 2, 0, "A1")]);
    let in_thread: Vec<(i64, i64)> = rows(&db, "SELECT thread_id, message_row_id FROM thread_messages ORDER BY 2");
    assert_eq!(in_thread, [(2, 12), (2, 17)]);
    let pointer: Vec<(i64,)> = rows(&db, "SELECT view_replies_thread_id FROM message WHERE key_id = 'A1'");
    assert_eq!(pointer, [(2,)]);

    // The trigger was set aside, not fired, and is back as it was.
    let fired: Vec<(i64,)> = rows(&db, "SELECT count(*) FROM trigger_fired");
    assert_eq!(fired, [(0,)]);
    let triggers: Vec<(String,)> = rows(&db, "SELECT name FROM sqlite_master WHERE type = 'trigger'");
    assert_eq!(triggers, [("message_moved".to_owned(),)]);
    let version: Vec<(i64,)> = rows(&db, "PRAGMA user_version");
    assert_eq!(version, [(7,)]);
}

#[test]
fn writes_like_whatsapp_and_leaves_the_originals_alone() {
    let fx = fixture::two_phones();
    let (source_before, target_before) = (fs::read(&fx.source).unwrap(), fs::read(&fx.target).unwrap());
    let sealed = merge(&fx, &target_before).unwrap().backup;

    assert_eq!(fs::read(&fx.source).unwrap(), source_before);
    assert_eq!(fs::read(&fx.target).unwrap(), target_before);

    // Same header except the IV: the fixture's is all 6s, at the end of the header.
    let header_len = 2 + usize::from(target_before[0]);
    let (old, new) = (&target_before[..header_len], &sealed[..header_len]);
    let iv_at = old.windows(16).position(|w| w == [6; 16]).unwrap();
    assert_eq!(old[..iv_at], new[..iv_at]);
    assert_ne!(new[iv_at..iv_at + 16], [6; 16]);
    assert_eq!(old[iv_at + 16..], new[iv_at + 16..]);

    let (body, footer) = sealed.split_at(sealed.len() - 16);
    assert_eq!(md5::Md5::digest(body).as_slice(), footer);
    assert!(crypt15::decrypt(&sealed, &keys().0).is_err(), "only the target's key opens it");
}

#[test]
fn merging_into_the_result_again_adds_nothing() {
    let fx = fixture::two_phones();
    let once = merge(&fx, &fs::read(&fx.target).unwrap()).unwrap().backup;
    let Merged { backup: twice, report, media } = merge(&fx, &once).unwrap();
    assert!(media.is_empty());
    assert_eq!((report.target_before, report.already_there, report.added), (12, 10, 0));
    let (db, _file) = open(&twice);
    let keys: Vec<(String,)> = rows(&db, "SELECT key_id FROM message WHERE chat_row_id = 2 ORDER BY sort_id");
    assert_eq!(keys.len(), 12);
    assert_eq!(keys[0].0, "S1");
    assert_eq!(keys[11].0, "B4");
}

#[test]
fn the_media_pack_holds_only_new_files_where_the_target_expects_them() {
    let fx = fixture::two_phones();
    let merged = merge(&fx, &fs::read(&fx.target).unwrap()).unwrap();
    // A3 and A4 get new names; A5's photo is the one Ben already has.
    let expected: Vec<_> = merged.media.iter().map(|m| (m.from.as_str(), m.to.as_str())).collect();
    assert_eq!(
        expected,
        [
            (
                "Media/WhatsApp Images/IMG-20231114-WA0001.jpg",
                "Media/WhatsApp Images/Sent/hilo-IMG-20231114-WA0001.jpg"
            ),
            (
                "Media/WhatsApp Images/IMG-20231114-WA0002.jpg",
                "Media/WhatsApp Images/Sent/hilo-IMG-20231114-WA0002.jpg"
            ),
        ]
    );

    let pack = fx.dir.path().join("pack");
    let report = copy_media(&merged.media, &fx.source_media, &pack).unwrap();
    assert_eq!(report, MediaReport { copied: 1, bytes: 15, missing: 1 });
    let copied = pack.join("WhatsApp Images/Sent/hilo-IMG-20231114-WA0001.jpg");
    assert_eq!(fs::read(&copied).unwrap(), b"synthetic beach");
    let original = fx.source_media.join("WhatsApp Images/IMG-20231114-WA0001.jpg");
    let mtime = |p: &std::path::Path| fs::metadata(p).unwrap().modified().unwrap();
    assert_eq!(mtime(&copied), mtime(&original));
    let files = walk(&pack);
    assert_eq!(files, 1, "only the new file");

    // Never into an existing folder.
    assert!(matches!(copy_media(&merged.media, &fx.source_media, &pack), Err(Error::Io { .. })));
    assert_eq!(fs::read(&original).unwrap(), b"synthetic beach");
}

fn walk(dir: &std::path::Path) -> usize {
    fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).map(|p| if p.is_dir() { walk(&p) } else { 1 }).sum()
}

#[test]
fn refuses_group_chats_and_unknown_chats() {
    let fx = fixture::two_phones();
    let target = fs::read(&fx.target).unwrap();
    let (source_key, target_key) = keys();
    let source = Backup::open(&fx.source, &source_key).unwrap();
    let run = |source_chat, target_chat| {
        ChatMerge { source: &source, source_chat, target: &target, target_key: &target_key, target_chat }.run()
    };
    assert!(matches!(run(fixture::GROUP_CHAT, TARGET_CHAT), Err(Error::Unsupported(_))));
    assert!(matches!(run(SOURCE_CHAT, 99), Err(Error::Unsupported(_))));
}

#[test]
fn media_paths_mirror_sent_and_received() {
    assert_eq!(
        mirror_path("Media/WhatsApp Images/IMG-1.jpg").as_deref(),
        Some("Media/WhatsApp Images/Sent/hilo-IMG-1.jpg")
    );
    assert_eq!(
        mirror_path("Media/WhatsApp Documents/Sent/a.pdf").as_deref(),
        Some("Media/WhatsApp Documents/hilo-a.pdf")
    );
    assert_eq!(
        mirror_path("Media/WhatsApp Voice Notes/202311/PTT-1.opus").as_deref(),
        Some("Media/WhatsApp Voice Notes/202311/hilo-PTT-1.opus")
    );
    assert_eq!(
        mirror_path("/storage/emulated/0/WhatsApp/Media/WhatsApp Video/Private/v.mp4").as_deref(),
        Some("/storage/emulated/0/WhatsApp/Media/WhatsApp Video/Private/hilo-v.mp4")
    );
    assert_eq!(mirror_path("Media/../../outside.pdf"), None);
    assert_eq!(mirror_path("Media/x.jpg"), None);
    assert_eq!(mirror_path("elsewhere/x.jpg"), None);
}

/// docs/messages.md, merge checks row 1: a counted value in a check takes its count's form, in each language.
#[test]
fn checks_1_a_counted_value_takes_its_form() {
    let tables = |n: &str, language: &str| Note::of(Msg::PassedTables, [n.to_owned()]).text(language);
    assert_eq!(tables("1", "en"), "tables: row counts as expected; 1 untouched table identical");
    assert_eq!(tables("2", "en"), "tables: row counts as expected; 2 untouched tables identical");
    assert_eq!(tables("1", "es"), "tablas: número de filas como se esperaba; 1 tabla sin tocar idéntica");
    assert_eq!(tables("0", "es"), "tablas: número de filas como se esperaba; 0 tablas sin tocar idénticas");
    assert_eq!(tables("1000000", "es"), "tablas: número de filas como se esperaba; 1000000 tablas sin tocar idénticas");
}

/// Row 2: a table or column name is a value, put in as it is, even holding braces or a placeholder's shape.
#[test]
fn checks_2_a_table_name_stays_text() {
    let note = Note::of(Msg::CheckRefersMissing, ["{name}", "a}{b", "2", "1"].map(String::from));
    assert_eq!(note.text("en"), "`{name}.a}{b` refers to 2 missing messages (before: 1)");
}

/// Rows 3 and 1: every check the merge passed, in the order it ran them.
#[test]
fn checks_3_the_checks_keep_their_order() {
    let fx = fixture::two_phones();
    let Merged { report, .. } = merge(&fx, &fs::read(&fx.target).unwrap()).unwrap();
    let names: Vec<_> = report.checks.iter().map(|c| c.message.name).collect();
    let expected = [
        "passed_decrypts",
        "passed_integrity",
        "passed_schema",
        "passed_count",
        "passed_sort_id",
        "passed_references",
        "passed_other_chats",
        "passed_created",
        "passed_tables",
    ];
    assert_eq!(names, expected);
    assert_eq!(report.checks[3].text("en"), "chat messages: 4 + 8 = 12");
    assert_eq!(report.checks[3].text("es"), "mensajes del chat: 4 + 8 = 12");
}

/// Row 4: a merge refused before its checks has none, never a sentence standing in for them.
#[test]
fn checks_4_a_refused_merge_has_no_checks() {
    assert!(MergeReport::default().checks.is_empty());
    // A target that isn't a backup at all: refused before anything is checked.
    assert!(merge(&fixture::two_phones(), b"").is_err());
}
