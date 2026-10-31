//! A synthetic WhatsApp backup: two invented people (Ana, the owner, and Ben), invented numbers, placeholder media
//! bytes. Shaped like the current msgstore schema, reduced to what the importer reads. Never real data.

use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::crypt15::{self, Key};

pub mod group;

/// The fixture's key: 64 hex digits, all `1`.
pub const KEY_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";
pub const BEN_JID: &str = "10000000001@s.whatsapp.net";
pub const BEN_CHAT: i64 = 1;
pub const GROUP_CHAT: i64 = 2;

/// (chat, from_me, key_id, message_type, text, starred)
type FixtureMessage = (i64, i64, &'static str, i64, Option<&'static str>, i64);

pub struct Fixture {
    /// Holds everything below; removed on drop.
    pub dir: TempDir,
    pub backup: PathBuf,
    pub media: PathBuf,
}

/// Ben's chat has 18 messages: 4 text (one starred, one reply), 2 photos with the same bytes, a voice note, a video
/// whose file is missing, a document whose path tries to leave `Media/`, an album header (99), and after the group's
/// message, an unknown type (11), a video call of 125 s (90), a view-once photo (42), a sticker pack (105), a
/// video note (81), a place (5), a live location (16) with its last position, a contact card (4) and a poll (46) with
/// two options. Also: both photos are in the album, K1 was edited, K2 forwarded, K7 has a link preview; Ben
/// reacted ❤️ to K1, and Ana reacted 😂 to K3 then took it back; in the poll Ana chose the first option, and Ben the
/// first and then the second.
pub fn build() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("msgstore.db");
    let db = Connection::open(&db_path).expect("fixture db");
    db.execute_batch(
        "CREATE TABLE jid (_id INTEGER PRIMARY KEY, user TEXT, server TEXT, raw_string TEXT);
         CREATE TABLE chat (_id INTEGER PRIMARY KEY, jid_row_id INTEGER, subject TEXT);
         CREATE TABLE message (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
             sender_jid_row_id INTEGER, timestamp INTEGER, message_type INTEGER, text_data TEXT, starred INTEGER);
         CREATE TABLE message_media (message_row_id INTEGER PRIMARY KEY, file_path TEXT, mime_type TEXT,
             file_size INTEGER, width INTEGER, height INTEGER, media_duration INTEGER, media_name TEXT);
         CREATE TABLE message_quoted (message_row_id INTEGER PRIMARY KEY, key_id TEXT, from_me INTEGER);
         CREATE TABLE call_log (_id INTEGER PRIMARY KEY, jid_row_id INTEGER, from_me INTEGER, video_call INTEGER,
             duration INTEGER);
         CREATE TABLE message_call_log (_id INTEGER PRIMARY KEY, message_row_id INTEGER, call_log_row_id INTEGER);
         CREATE TABLE message_association (_id INTEGER PRIMARY KEY, child_message_row_id INTEGER,
             parent_message_row_id INTEGER, association_type INTEGER);
         CREATE TABLE message_edit_info (message_row_id INTEGER PRIMARY KEY, original_key_id TEXT,
             edited_timestamp INTEGER, sender_timestamp INTEGER);
         CREATE TABLE message_forwarded (message_row_id INTEGER PRIMARY KEY, forward_score INTEGER,
             forward_origin INTEGER);
         CREATE TABLE message_text (message_row_id INTEGER PRIMARY KEY, description TEXT, page_title TEXT, url TEXT);
         CREATE TABLE message_add_on (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
             parent_message_row_id INTEGER, timestamp INTEGER, message_add_on_type INTEGER);
         CREATE TABLE message_add_on_reaction (message_add_on_row_id INTEGER PRIMARY KEY, reaction TEXT,
             sender_timestamp INTEGER);
         CREATE TABLE message_location (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER, latitude REAL,
             longitude REAL, place_name TEXT, place_address TEXT, url TEXT, live_location_share_duration INTEGER,
             live_location_final_latitude REAL, live_location_final_longitude REAL,
             live_location_final_timestamp INTEGER);
         CREATE TABLE message_vcard (_id INTEGER PRIMARY KEY, message_row_id INTEGER, vcard TEXT);
         CREATE TABLE message_poll (message_row_id INTEGER PRIMARY KEY, poll_type INTEGER,
             selectable_options_count INTEGER);
         CREATE TABLE message_poll_option (_id INTEGER PRIMARY KEY, message_row_id INTEGER, option_name TEXT,
             vote_total INTEGER);
         CREATE TABLE message_add_on_poll_vote (message_add_on_row_id INTEGER PRIMARY KEY, sender_timestamp INTEGER);
         CREATE TABLE message_add_on_poll_vote_selected_option (message_add_on_row_id INTEGER,
             message_poll_option_id INTEGER);
         INSERT INTO jid VALUES (1, '10000000001', 's.whatsapp.net', '10000000001@s.whatsapp.net');
         INSERT INTO jid VALUES (2, '120000000000000001', 'g.us', '120000000000000001@g.us');
         INSERT INTO chat VALUES (1, 1, NULL), (2, 2, 'Synthetic group');",
    )
    .expect("fixture schema");

    let t0 = 1_700_000_000_000_i64;
    let messages: &[FixtureMessage] = &[
        (1, 1, "K1", 0, Some("Hola Ben"), 0),
        (1, 0, "K2", 0, Some("Hola Ana 🌙"), 1),
        (1, 0, "K3", 1, Some("playa"), 0),
        (1, 1, "K4", 2, None, 0),
        (1, 1, "K5", 1, None, 0),
        (1, 0, "K6", 3, None, 0),
        (1, 0, "K7", 0, Some("respuesta"), 0),
        (1, 0, "K8", 99, None, 0),
        (1, 1, "K9", 9, None, 0),
        (2, 0, "G1", 0, Some("en el grupo"), 0),
        (1, 0, "K10", 11, None, 0),
        (1, 1, "K11", 90, None, 0),
        (1, 0, "K12", 42, None, 0),
        (1, 0, "K13", 105, None, 0),
        (1, 1, "K14", 81, None, 0),
        (1, 0, "K15", 5, None, 0),
        (1, 1, "K16", 16, None, 0),
        (1, 0, "K17", 4, Some("Carla Prueba"), 0),
        (1, 1, "K18", 46, Some("¿Cena?"), 0),
    ];
    for (i, (chat, from_me, key, kind, text, starred)) in messages.iter().enumerate() {
        db.execute(
            "INSERT INTO message (_id, chat_row_id, from_me, key_id, timestamp, message_type, text_data, starred)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![i as i64 + 1, chat, from_me, key, t0 + i as i64 * 60_000, kind, text, starred],
        )
        .expect("fixture message");
    }
    let media: &[(i64, &str, &str)] = &[
        (3, "Media/WhatsApp Images/IMG-00000001-WA0001.jpg", "image/jpeg"),
        (4, "Media/WhatsApp Voice Notes/202311/PTT-00000001-WA0001.opus", "audio/ogg; codecs=opus"),
        (5, "Media/WhatsApp Images/Sent/IMG-00000001-WA0002.jpg", "image/jpeg"),
        (6, "Media/WhatsApp Video/VID-00000001-WA0001.mp4", "video/mp4"),
        (9, "Media/../../outside.pdf", "application/pdf"),
    ];
    for (row, path, mime) in media {
        db.execute(
            "INSERT INTO message_media (message_row_id, file_path, mime_type, file_size, width, height, media_duration)
             VALUES (?1, ?2, ?3, 15, 640, 480, 3)",
            params![row, path, mime],
        )
        .expect("fixture media row");
    }
    db.execute("INSERT INTO message_quoted VALUES (7, 'K1', 1)", []).expect("fixture reply");
    db.execute_batch(
        "INSERT INTO call_log VALUES (1, 1, 1, 1, 125);
         INSERT INTO message_call_log VALUES (1, 12, 1);
         INSERT INTO message_association VALUES (1, 3, 8, 1), (2, 5, 8, 1);
         INSERT INTO message_edit_info VALUES (1, 'K1', 1700000500000, 1700000500000);
         INSERT INTO message_forwarded VALUES (2, 1, 0);
         INSERT INTO message_text VALUES (7, 'A synthetic page', 'Example', 'https://example.org/');
         INSERT INTO message_add_on VALUES (1, 1, 0, 'R1', 1, 1700000600000, 56), (2, 1, 1, 'R2', 3, 1700000600000, 56),
             (3, 1, 1, 'R3', 3, 1700000700000, 56);
         INSERT INTO message_add_on_reaction VALUES (1, '❤️', 0), (2, '😂', 0), (3, '', 0);
         INSERT INTO message_location VALUES
             (16, 1, 40.4168, -3.7038, 'Plaza Inventada', 'Calle Falsa 1', NULL, NULL, NULL, NULL, NULL),
             (17, 1, 30.0, 31.2, NULL, NULL, NULL, 900, 30.05, 31.25, 1700001100000);
         INSERT INTO message_vcard VALUES (1, 18, 'BEGIN:VCARD' || char(10) || 'VERSION:3.0' || char(10) ||
             'N:Prueba;Carla;;;' || char(10) || 'FN:Carla Prueba' || char(10) ||
             'item1.TEL;waid=10000000003:+1 000-000-0003' || char(10) || 'END:VCARD');
         INSERT INTO message_poll VALUES (19, 0, 1);
         INSERT INTO message_poll_option VALUES (7, 19, 'Pizza', 1), (8, 19, 'Sushi', 1);
         INSERT INTO message_add_on VALUES (4, 1, 1, 'V1', 19, 1700002000000, 66),
             (5, 1, 0, 'V2', 19, 1700002000000, 66), (6, 1, 0, 'V3', 19, 1700002100000, 66);
         INSERT INTO message_add_on_poll_vote VALUES (4, 0), (5, 0), (6, 0);
         INSERT INTO message_add_on_poll_vote_selected_option VALUES (4, 7), (5, 7), (6, 8);",
    )
    .expect("fixture call");
    drop(db);

    let media_dir = dir.path().join("Media");
    for (path, bytes) in [
        ("WhatsApp Images/IMG-00000001-WA0001.jpg", &b"synthetic photo"[..]),
        ("WhatsApp Images/Sent/IMG-00000001-WA0002.jpg", b"synthetic photo"),
        ("WhatsApp Voice Notes/202311/PTT-00000001-WA0001.opus", b"synthetic voice"),
    ] {
        let path = media_dir.join(path);
        fs::create_dir_all(path.parent().expect("has parent")).expect("fixture media dir");
        fs::write(path, bytes).expect("fixture media file");
    }
    // The file the traversal path points at exists, so the test proves it's refused, not just absent.
    fs::write(dir.path().join("outside.pdf"), b"must not be imported").expect("fixture outside file");

    let sqlite = fs::read(&db_path).expect("fixture db bytes");
    fs::remove_file(&db_path).expect("only the encrypted copy remains");
    let key = Key::from_hex(KEY_HEX).expect("fixture key");
    let backup = dir.path().join("msgstore.db.crypt15");
    fs::write(&backup, crypt15::encrypt(&sqlite, &key, [7; 16], true)).expect("fixture backup");

    Fixture { dir, backup, media: media_dir }
}

/// The target fixture's key: 64 hex digits, all `2`.
pub const TARGET_KEY_HEX: &str = "2222222222222222222222222222222222222222222222222222222222222222";
/// Ana's chat with Ben in her backup (the source), and Ben's chat with Ana in his (the target).
pub const SOURCE_CHAT: i64 = 1;
pub const TARGET_CHAT: i64 = 2;

pub struct TwoPhones {
    pub dir: TempDir,
    /// Ana's backup (key [`KEY_HEX`]).
    pub source: PathBuf,
    /// Ben's backup (key [`TARGET_KEY_HEX`]).
    pub target: PathBuf,
    /// Ana's `Media/` folder: A3's photo and A5's (which Ben has); A4's is missing.
    pub source_media: PathBuf,
}

/// Two invented phones' backups of the same chat, shaped like decision 0006 measured: `sort_id = _id`, one-to-one
/// senders `0`, the target filed under a `lid`, with columns in another order and some missing or extra.
///
/// Ana's chat 1 (11 rows, a group message among them): a system row, a text, an album (header and two photos, one
/// named like a different photo of Ben's), a photo whose bytes Ben has in another chat, a reply, a call, and B1, B2,
/// which Ben has too. Ben reacted ❤️ to the reply; Ana reacted 👍 to B1 (Ben has it) and 😂 to B2 (he doesn't).
/// Ben's chat 2 has B1, B2, a reply to A1 and one more, between his rows of a chat with Cleo; his database used
/// message rows up to 10 (`sqlite_sequence`), a receipt still points at row 10, he still has the call's log row,
/// and an update trigger that must not fire. Ana's A1 starts a "view replies" thread with A6 in it; Ben has a thread
/// in Cleo's chat. Ben's chat with Ana was created just before B1; his album table counts its own ids but has never
/// had a row.
pub fn two_phones() -> TwoPhones {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = Connection::open_in_memory().expect("source db");
    source
        .execute_batch(
            "CREATE TABLE jid (_id INTEGER PRIMARY KEY, user TEXT, server TEXT, raw_string TEXT);
             CREATE TABLE chat (_id INTEGER PRIMARY KEY, jid_row_id INTEGER, subject TEXT,
                 display_message_row_id INTEGER, last_message_row_id INTEGER, last_read_message_row_id INTEGER,
                 last_read_receipt_sent_message_row_id INTEGER, last_important_message_row_id INTEGER,
                 last_read_message_sort_id INTEGER, display_message_sort_id INTEGER, last_message_sort_id INTEGER,
                 last_read_receipt_sent_message_sort_id INTEGER, last_message_reaction_row_id INTEGER);
             CREATE TABLE message (_id INTEGER PRIMARY KEY AUTOINCREMENT, chat_row_id INTEGER, from_me INTEGER,
                 key_id TEXT, sender_jid_row_id INTEGER, status INTEGER, timestamp INTEGER, message_type INTEGER,
                 text_data TEXT, starred INTEGER, lookup_tables INTEGER, sort_id INTEGER, translated_text TEXT,
                 view_replies_thread_id INTEGER);
             CREATE TABLE message_media (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER, file_path TEXT,
                 mime_type TEXT, file_size INTEGER, file_hash TEXT, media_name TEXT, media_caption TEXT);
             CREATE TABLE message_quoted (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER,
                 parent_message_chat_row_id INTEGER, from_me INTEGER, sender_jid_row_id INTEGER, key_id TEXT,
                 timestamp INTEGER, message_type INTEGER, text_data TEXT);
             CREATE TABLE message_details (message_row_id INTEGER PRIMARY KEY, author_device_jid INTEGER);
             CREATE TABLE message_secret (message_row_id INTEGER PRIMARY KEY, message_secret BLOB);
             CREATE TABLE message_association (_id INTEGER PRIMARY KEY, child_message_row_id INTEGER,
                 parent_message_row_id INTEGER, association_type INTEGER);
             CREATE TABLE message_album (message_row_id INTEGER PRIMARY KEY, image_count INTEGER, video_count INTEGER);
             CREATE TABLE message_system (message_row_id INTEGER PRIMARY KEY, action_type INTEGER);
             CREATE TABLE message_call_log (message_row_id INTEGER PRIMARY KEY, call_log_row_id INTEGER);
             CREATE TABLE call_log (_id INTEGER PRIMARY KEY AUTOINCREMENT, jid_row_id INTEGER, from_me INTEGER,
                 call_id TEXT, timestamp INTEGER, video_call INTEGER, duration INTEGER, group_jid_row_id INTEGER,
                 call_creator_device_jid_row_id INTEGER);
             CREATE TABLE message_add_on (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
                 sender_jid_row_id INTEGER, parent_message_row_id INTEGER, timestamp INTEGER, status INTEGER,
                 message_add_on_type INTEGER);
             CREATE TABLE message_add_on_reaction (message_add_on_row_id INTEGER PRIMARY KEY, reaction TEXT,
                 sender_timestamp INTEGER);
             CREATE TABLE media_hash_thumbnail (media_hash TEXT PRIMARY KEY, thumbnail BLOB);
             CREATE TABLE media_refs (_id INTEGER PRIMARY KEY, path TEXT, ref_count INTEGER);
             CREATE TABLE receipt_user (_id INTEGER PRIMARY KEY, message_row_id INTEGER,
                 receipt_user_jid_row_id INTEGER, read_timestamp INTEGER);
             CREATE TABLE thread_id (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
                 sender_jid_row_id INTEGER, thread_type INTEGER);
             CREATE TABLE thread_messages (_id INTEGER PRIMARY KEY, thread_id INTEGER, message_row_id INTEGER);

             INSERT INTO jid VALUES (1, '10000000001', 's.whatsapp.net', '10000000001@s.whatsapp.net'),
                 (2, '120000000000000001', 'g.us', '120000000000000001@g.us'),
                 (3, '10000000002', 's.whatsapp.net', '10000000002.0:1@s.whatsapp.net'),
                 (4, '10000000001', 's.whatsapp.net', '10000000001.0:0@s.whatsapp.net');
             INSERT INTO chat (_id, jid_row_id, subject, last_message_row_id, last_message_sort_id)
                 VALUES (1, 1, NULL, 11, 11), (2, 2, 'Synthetic group', 3, 3);
             INSERT INTO message (_id, chat_row_id, from_me, key_id, sender_jid_row_id, status, timestamp,
                 message_type, text_data, starred, lookup_tables, sort_id, translated_text) VALUES
                 (1, 1, 0, 'S1', 0, 6, 1700000000000, 7, NULL, 0, 0, 1, NULL),
                 (2, 1, 1, 'A1', 0, 13, 1700000060000, 0, 'Hola Ben', 1, 0, 2, NULL),
                 (3, 2, 0, 'G1', 4, 0, 1700000120000, 0, 'en el grupo', 0, 0, 3, NULL),
                 (4, 1, 0, 'A2', 0, 0, 1700000180000, 99, NULL, 0, 0, 4, NULL),
                 (5, 1, 0, 'A3', 0, 0, 1700000181000, 1, NULL, 0, 0, 5, NULL),
                 (6, 1, 0, 'A4', 0, 0, 1700000182000, 1, NULL, 0, 0, 6, NULL),
                 (7, 1, 1, 'A5', 0, 13, 1700000240000, 1, 'playa', 0, 0, 7, 'beach'),
                 (8, 1, 1, 'A6', 0, 13, 1700000300000, 0, 'respuesta', 0, 0, 8, NULL),
                 (9, 1, 1, 'A7', 0, 13, 1700000360000, 90, NULL, 0, 0, 9, NULL),
                 (10, 1, 0, 'B1', 0, 0, 1700000420000, 0, 'hola Ana', 0, 0, 10, NULL),
                 (11, 1, 1, 'B2', 0, 13, 1700000480000, 0, 'qué tal', 0, 0, 11, NULL);
             INSERT INTO message_media VALUES
                 (5, 1, 'Media/WhatsApp Images/IMG-20231114-WA0001.jpg', 'image/jpeg', 15, 'aGFzaDE=', NULL, NULL),
                 (6, 1, 'Media/WhatsApp Images/IMG-20231114-WA0002.jpg', 'image/jpeg', 15, 'aGFzaDI=', NULL, NULL),
                 (7, 1, 'Media/WhatsApp Images/Sent/IMG-20231114-WA0003.jpg', 'image/jpeg', 15, 'c2hhcmVk', NULL,
                     'playa');
             INSERT INTO message_quoted VALUES (8, 1, 1, 0, 0, 'A3', 1700000181000, 1, NULL);
             INSERT INTO message_details SELECT _id, CASE WHEN from_me = 1 THEN 3 ELSE 4 END FROM message;
             INSERT INTO message_secret SELECT _id, randomblob(32) FROM message WHERE chat_row_id = 1;
             INSERT INTO message_association VALUES (1, 5, 4, 1), (2, 6, 4, 1);
             INSERT INTO message_album VALUES (4, 2, 0);
             INSERT INTO message_system VALUES (1, 18);
             INSERT INTO call_log VALUES (1, 1, 1, 'C1', 1700000360000, 1, 125, 0, 3);
             INSERT INTO message_call_log VALUES (9, 1);
             INSERT INTO message_add_on VALUES (1, 1, 0, 'R1', 0, 8, 1700000400000, 0, 56),
                 (2, 1, 1, 'R2', 0, 10, 1700000500000, 13, 56), (3, 1, 1, 'R3', 0, 11, 1700000500000, 13, 56);
             INSERT INTO message_add_on_reaction VALUES (1, '❤️', 0), (2, '👍', 0), (3, '😂', 0);
             INSERT INTO media_hash_thumbnail VALUES ('aGFzaDE=', x'01'), ('aGFzaDI=', x'02'), ('c2hhcmVk', x'03');
             INSERT INTO receipt_user VALUES (1, 2, 1, 1700000070000);
             INSERT INTO thread_id VALUES (1, 2, 0, 'G1', 4, 1), (2, 1, 1, 'A1', 0, 1);
             INSERT INTO thread_messages VALUES (1, 1, 3), (2, 2, 2), (3, 2, 8);
             UPDATE message SET view_replies_thread_id = 2 WHERE key_id = 'A1';",
        )
        .expect("source fixture");

    let target = Connection::open_in_memory().expect("target db");
    target
        .execute_batch(
            "PRAGMA user_version = 7;
             CREATE TABLE jid (_id INTEGER PRIMARY KEY, user TEXT, server TEXT, raw_string TEXT);
             CREATE TABLE chat (_id INTEGER PRIMARY KEY, jid_row_id INTEGER, subject TEXT, created_timestamp INTEGER,
                 display_message_row_id INTEGER, last_message_row_id INTEGER, last_read_message_row_id INTEGER,
                 last_read_receipt_sent_message_row_id INTEGER, last_important_message_row_id INTEGER,
                 last_read_message_sort_id INTEGER, display_message_sort_id INTEGER, last_message_sort_id INTEGER,
                 last_read_receipt_sent_message_sort_id INTEGER, last_message_reaction_row_id INTEGER);
             CREATE TABLE message (_id INTEGER PRIMARY KEY AUTOINCREMENT, chat_row_id INTEGER, key_id TEXT,
                 from_me INTEGER, sender_jid_row_id INTEGER, status INTEGER, timestamp INTEGER, message_type INTEGER,
                 text_data TEXT, starred INTEGER, lookup_tables INTEGER, view_mode INTEGER DEFAULT 0, sort_id INTEGER,
                 view_replies_thread_id INTEGER);
             CREATE UNIQUE INDEX message_key_index ON message (chat_row_id, from_me, key_id);
             CREATE INDEX message_sort_index ON message (chat_row_id, sort_id);
             CREATE TABLE message_media (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER, file_path TEXT,
                 mime_type TEXT, file_size INTEGER, file_hash TEXT, media_name TEXT, is_offloaded INTEGER DEFAULT 0);
             CREATE TABLE message_quoted (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER,
                 parent_message_chat_row_id INTEGER, from_me INTEGER, sender_jid_row_id INTEGER, key_id TEXT,
                 timestamp INTEGER, message_type INTEGER, text_data TEXT);
             CREATE TABLE message_details (message_row_id INTEGER PRIMARY KEY, author_device_jid INTEGER);
             CREATE TABLE message_secret (message_row_id INTEGER PRIMARY KEY, message_secret BLOB);
             CREATE TABLE message_association (_id INTEGER PRIMARY KEY, child_message_row_id INTEGER,
                 parent_message_row_id INTEGER, association_type INTEGER);
             CREATE TABLE message_album (message_row_id INTEGER PRIMARY KEY AUTOINCREMENT, image_count INTEGER, video_count INTEGER);
             CREATE TABLE message_system (message_row_id INTEGER PRIMARY KEY, action_type INTEGER);
             CREATE TABLE message_call_log (message_row_id INTEGER PRIMARY KEY, call_log_row_id INTEGER);
             CREATE TABLE call_log (_id INTEGER PRIMARY KEY AUTOINCREMENT, jid_row_id INTEGER, from_me INTEGER,
                 call_id TEXT, timestamp INTEGER, video_call INTEGER, duration INTEGER, group_jid_row_id INTEGER,
                 call_creator_device_jid_row_id INTEGER);
             CREATE TABLE message_add_on (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
                 sender_jid_row_id INTEGER, parent_message_row_id INTEGER, timestamp INTEGER, status INTEGER,
                 message_add_on_type INTEGER);
             CREATE TABLE message_add_on_reaction (message_add_on_row_id INTEGER PRIMARY KEY, reaction TEXT,
                 sender_timestamp INTEGER);
             CREATE TABLE media_hash_thumbnail (media_hash TEXT PRIMARY KEY, thumbnail BLOB);
             CREATE TABLE media_refs (_id INTEGER PRIMARY KEY, path TEXT, ref_count INTEGER);
             CREATE TABLE receipt_user (_id INTEGER PRIMARY KEY, message_row_id INTEGER,
                 receipt_user_jid_row_id INTEGER, read_timestamp INTEGER);
             CREATE TABLE thread_id (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
                 sender_jid_row_id INTEGER, thread_type INTEGER);
             CREATE TABLE thread_messages (_id INTEGER PRIMARY KEY, thread_id INTEGER, message_row_id INTEGER);
             CREATE TABLE props (_id INTEGER PRIMARY KEY, key TEXT, value TEXT);
             CREATE TABLE trigger_fired (message_row_id INTEGER);
             CREATE TRIGGER message_moved AFTER UPDATE ON message
                 BEGIN INSERT INTO trigger_fired VALUES (new._id); END;

             INSERT INTO jid VALUES (1, '10000000009', 's.whatsapp.net', '10000000009@s.whatsapp.net'),
                 (2, '30000000003', 'lid', '30000000003@lid'),
                 (3, '10000000001', 's.whatsapp.net', '10000000001.0:0@s.whatsapp.net'),
                 (4, '30000000003', 'lid', '30000000003.0:1@lid');
             INSERT INTO chat VALUES (1, 1, NULL, 1700000000000, 8, 8, 8, 8, 8, 8, 8, 8, 8, NULL),
                 (2, 2, NULL, 1700000400000, 7, 7, 7, 7, 6, 7, 7, 7, 7, 1);
             INSERT INTO message (_id, chat_row_id, key_id, from_me, sender_jid_row_id, status, timestamp,
                 message_type, text_data, starred, lookup_tables, sort_id) VALUES
                 (1, 1, 'X1', 0, 0, 0, 1700000010000, 0, 'hola Ben', 0, 0, 1),
                 (2, 2, 'B1', 1, 0, 13, 1700000420000, 0, 'hola Ana', 0, 0, 2),
                 (3, 1, 'X2', 1, 0, 13, 1700000430000, 1, NULL, 0, 0, 3),
                 (4, 1, 'X3', 0, 0, 0, 1700000440000, 1, NULL, 0, 0, 4),
                 (5, 2, 'B2', 0, 0, 0, 1700000480000, 0, 'qué tal', 0, 0, 5),
                 (6, 2, 'B3', 1, 0, 13, 1700000540000, 0, 'sí', 1, 0, 6),
                 (7, 2, 'B4', 0, 0, 0, 1700000600000, 0, 'vale', 0, 0, 7),
                 (8, 1, 'X4', 1, 0, 13, 1700000610000, 0, 'adiós', 0, 0, 8);
             UPDATE sqlite_sequence SET seq = 10 WHERE name = 'message';
             INSERT INTO message_media (message_row_id, chat_row_id, file_path, mime_type, file_size, file_hash) VALUES
                 (3, 1, 'Media/WhatsApp Images/Sent/IMG-20231120-WA0007.jpg', 'image/jpeg', 15, 'c2hhcmVk'),
                 (4, 1, 'Media/WhatsApp Images/IMG-20231114-WA0001.jpg', 'image/jpeg', 15, 'aGFzaDk=');
             INSERT INTO message_quoted VALUES (6, 2, 2, 0, 0, 'A1', 1700000060000, 0, 'Hola Ben');
             INSERT INTO message_details SELECT _id, CASE WHEN chat_row_id = 1 THEN 1 WHEN from_me = 1 THEN 3 ELSE 4 END
                 FROM message;
             INSERT INTO message_secret SELECT _id, randomblob(32) FROM message;
             INSERT INTO call_log VALUES (1, 1, 0, 'CX', 1700000020000, 0, 30, 0, 1),
                 (2, 2, 0, 'C1', 1700000360000, 1, 125, 0, 4);
             INSERT INTO message_add_on VALUES (1, 2, 0, 'R2', 0, 2, 1700000500000, 0, 56);
             INSERT INTO message_add_on_reaction VALUES (1, '👍', 0);
             INSERT INTO media_hash_thumbnail VALUES ('c2hhcmVk', x'03'), ('aGFzaDk=', x'09');
             INSERT INTO media_refs VALUES (1, 'Media/WhatsApp Images/Sent/IMG-20231120-WA0007.jpg', 1),
                 (2, 'Media/WhatsApp Images/IMG-20231114-WA0001.jpg', 1);
             INSERT INTO receipt_user VALUES (1, 2, 4, 1700000430000), (2, 6, 4, 1700000550000),
                 (3, 3, 1, 1700000435000), (4, 10, 1, 1700000700000);
             INSERT INTO props VALUES (1, 'synthetic', 'yes');
             INSERT INTO thread_id VALUES (1, 1, 0, 'X1', 0, 1);",
        )
        .expect("target fixture");

    let write = |db: Connection, name: &str, key_hex: &str, iv: u8| {
        let path = dir.path().join(name);
        db.execute("VACUUM INTO ?1", [path.with_extension("plain").to_str().expect("utf-8 temp path")])
            .expect("fixture db file");
        let sqlite = fs::read(path.with_extension("plain")).expect("fixture db bytes");
        fs::remove_file(path.with_extension("plain")).expect("only the encrypted copy remains");
        let key = Key::from_hex(key_hex).expect("fixture key");
        fs::write(&path, crypt15::encrypt(&sqlite, &key, [iv; 16], true)).expect("fixture backup");
        path
    };
    let source = write(source, "ana.crypt15", KEY_HEX, 5);
    let target = write(target, "ben.crypt15", TARGET_KEY_HEX, 6);
    let source_media = dir.path().join("ana-media");
    for (path, bytes) in [
        ("WhatsApp Images/IMG-20231114-WA0001.jpg", &b"synthetic beach"[..]),
        ("WhatsApp Images/Sent/IMG-20231114-WA0003.jpg", b"synthetic shared"),
    ] {
        let path = source_media.join(path);
        fs::create_dir_all(path.parent().expect("has parent")).expect("fixture media dir");
        fs::write(path, bytes).expect("fixture media file");
    }
    TwoPhones { dir, source, target, source_media }
}
