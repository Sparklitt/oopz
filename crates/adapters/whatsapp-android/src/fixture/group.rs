//! Synthetic WhatsApp group backups (decision 0034, points 28–30): a group in Ana's backup with its members, events,
//! a reaction and a reply by members, a sender-less row and a sticker pack message; a contacts file (`wa.db`) naming
//! two of the members; sticker pack zips; and Ben's backup of the same group, for merging both phones. Invented
//! numbers only, all starting [`NUMBERS`] (`99990000…` for numbers, `88880000…` for `lid`s), so a test can grep for
//! them. Never real data.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

use super::{KEY_HEX, TARGET_KEY_HEX};
use crate::crypt15::{self, Key};

/// Every invented number and `lid` here starts with one of these: none may show anywhere but the store's aliases.
pub const NUMBERS: [&str; 2] = ["99990000", "88880000"];
pub const GROUP_JID: &str = "120363000000000001@g.us";
/// The group's chat row in Ana's backup and in Ben's.
pub const GROUP_CHAT: i64 = 1;
/// A one-to-one chat (with Ben in Ana's backup, with Ana in Ben's).
pub const DIRECT_CHAT: i64 = 2;
/// A group only Ben is in (in his backup).
pub const BENS_OWN_GROUP: i64 = 3;
pub const ANA: &str = "999900000001@s.whatsapp.net";
pub const BEN: &str = "999900000011@s.whatsapp.net";
pub const BEN_LID: &str = "888800000022@lid";
pub const CLEO: &str = "999900000033@s.whatsapp.net";
pub const CLEO_LID: &str = "888800000033@lid";
pub const DAN: &str = "999900000044@s.whatsapp.net";
pub const DAN_LID: &str = "888800000044@lid";
pub const EVE: &str = "999900000055@s.whatsapp.net";
pub const FAY: &str = "999900000066@s.whatsapp.net";
/// Someone in Ana's contacts who is in none of her groups: never read.
pub const ZED: &str = "999900000099@s.whatsapp.net";
pub const PACK_ID: &str = "pack.synthetic.one";
/// Bytes of a sticker sent in the group and also in [`PACK_ID`]'s zip: stored once.
pub const SHARED_STICKER: &[u8] = b"RIFF synthetic shared sticker WEBP";
pub const T0: i64 = 1_700_100_000_000;

/// The action codes the fixture writes, as WhatsApp's `message_system.action_type` (the importer's mapping), and one
/// it doesn't know.
pub const UNKNOWN_ACTION: i64 = 999;

pub struct GroupFixture {
    /// Holds everything below; removed on drop.
    pub dir: TempDir,
    /// Ana's `msgstore.db.crypt15` (key [`KEY_HEX`]), with `wa.db.crypt15` beside it.
    pub backup: PathBuf,
    pub media: PathBuf,
    /// Ben's backup of the same group (key [`TARGET_KEY_HEX`]), no contacts file, no media.
    pub bens: PathBuf,
}

const SCHEMA: &str = "
    CREATE TABLE jid (_id INTEGER PRIMARY KEY, user TEXT, server TEXT, raw_string TEXT);
    CREATE TABLE chat (_id INTEGER PRIMARY KEY, jid_row_id INTEGER, subject TEXT);
    CREATE TABLE message (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
        sender_jid_row_id INTEGER, timestamp INTEGER, message_type INTEGER, text_data TEXT, starred INTEGER);
    CREATE TABLE message_media (message_row_id INTEGER PRIMARY KEY, file_path TEXT, mime_type TEXT,
        file_size INTEGER, width INTEGER, height INTEGER, media_duration INTEGER, media_name TEXT);
    CREATE TABLE message_quoted (message_row_id INTEGER PRIMARY KEY, key_id TEXT, from_me INTEGER);
    CREATE TABLE message_add_on (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
        sender_jid_row_id INTEGER, parent_message_row_id INTEGER, timestamp INTEGER, message_add_on_type INTEGER);
    CREATE TABLE message_add_on_reaction (message_add_on_row_id INTEGER PRIMARY KEY, reaction TEXT,
        sender_timestamp INTEGER);
    CREATE TABLE group_participant_user (_id INTEGER PRIMARY KEY, group_jid_row_id INTEGER, user_jid_row_id INTEGER,
        rank INTEGER, pending INTEGER);
    CREATE TABLE group_past_participant_user (_id INTEGER PRIMARY KEY, group_jid_row_id INTEGER,
        user_jid_row_id INTEGER, is_leave INTEGER, timestamp INTEGER);
    CREATE TABLE jid_map (lid_row_id INTEGER PRIMARY KEY, jid_row_id INTEGER);
    CREATE TABLE message_system (message_row_id INTEGER PRIMARY KEY, action_type INTEGER);
    CREATE TABLE message_system_chat_participant (_id INTEGER PRIMARY KEY, message_row_id INTEGER,
        user_jid_row_id INTEGER);
    CREATE TABLE message_system_value_change (message_row_id INTEGER PRIMARY KEY, old_data TEXT);
    CREATE TABLE message_sticker_pack (message_row_id INTEGER PRIMARY KEY, sticker_pack_id TEXT, name TEXT,
        publisher TEXT);";

/// One message row: (row id, chat, from_me, key_id, sender jid row, minutes after [`T0`], type, text).
type Row = (i64, i64, i64, Option<&'static str>, i64, i64, i64, Option<&'static str>);

fn insert_messages(db: &Connection, rows: &[Row]) {
    for (id, chat, from_me, key, sender, minutes, kind, text) in rows {
        db.execute(
            "INSERT INTO message (_id, chat_row_id, from_me, key_id, sender_jid_row_id, timestamp, message_type,
                 text_data, starred) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            rusqlite::params![id, chat, from_me, key, sender, T0 + minutes * 60_000, kind, text],
        )
        .expect("fixture message");
    }
}

/// Ana's group, Ben's backup of it, the contacts file and the pack zips.
///
/// Ana's group (chat 1, "Synthetic family"): Ana is its creator (rank 2), Ben an admin (rank 1, listed under his
/// `lid`, which `jid_map` links to his number; he writes under both), Cleo and Dan members (Dan has no name anywhere),
/// Eve left and Fay was removed (past participants). Events: created (no key_id), added the five (key `-1`), left,
/// removed (no key_id), renamed, description, admin added, icon changed, joined by link and one unknown code. Cleo
/// reacted 👍 to Ben's first message, Ana replied to Cleo's; a row with sender `0`; a sticker pack message (105)
/// for [`PACK_ID`]; a sticker Ben sent whose bytes are also in that pack. Chat 2 is Ana's chat with Ben.
pub fn build() -> GroupFixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = Connection::open_in_memory().expect("fixture db");
    db.execute_batch(SCHEMA).expect("fixture schema");
    db.execute_batch(&format!(
        "INSERT INTO jid VALUES (1, '999900000011', 's.whatsapp.net', '{BEN}'), (2, '888800000022', 'lid', '{BEN_LID}'),
             (3, '999900000033', 's.whatsapp.net', '{CLEO}'), (4, '999900000044', 's.whatsapp.net', '{DAN}'),
             (5, '999900000055', 's.whatsapp.net', '{EVE}'), (6, '999900000066', 's.whatsapp.net', '{FAY}'),
             (7, '120363000000000001', 'g.us', '{GROUP_JID}'), (8, '', '', ''),
             (9, '999900000011', 's.whatsapp.net', '999900000011.0:1@s.whatsapp.net');
         INSERT INTO jid_map VALUES (2, 1);
         INSERT INTO chat VALUES (1, 7, 'Synthetic family'), (2, 1, NULL);
         INSERT INTO group_participant_user VALUES (1, 7, 8, 2, 0), (2, 7, 2, 1, 0), (3, 7, 3, 0, 0), (4, 7, 4, 0, 0);
         INSERT INTO group_past_participant_user VALUES (1, 7, 5, 1, {left}), (2, 7, 6, 0, {removed});",
        left = T0 + 8 * 60_000,
        removed = T0 + 9 * 60_000,
    ))
    .expect("fixture people");
    insert_messages(
        &db,
        &[
            (1, 1, 1, None, 0, 0, 7, None),
            (2, 1, 1, Some("-1"), 0, 1, 7, None),
            (3, 1, 0, Some("GA1"), 2, 2, 0, Some("hola grupo")),
            (4, 1, 0, Some("GA2"), 9, 3, 0, Some("otra vez")),
            (5, 1, 0, Some("GA3"), 3, 4, 0, Some("hola Ben")),
            (6, 1, 1, Some("GA4"), 0, 5, 0, Some("respuesta")),
            (7, 1, 0, Some("GA5"), 4, 6, 0, Some("soy Dan")),
            (8, 1, 0, Some("GS1"), 5, 8, 7, None),
            (9, 1, 1, None, 0, 9, 7, None),
            (10, 1, 0, Some("GS2"), 1, 10, 7, None),
            (11, 1, 0, Some("GS3"), 3, 11, 7, None),
            (12, 1, 1, Some("GS4"), 0, 12, 7, None),
            (13, 1, 0, Some("GS5"), 1, 13, 7, None),
            (14, 1, 0, Some("GS6"), 3, 14, 7, None),
            (15, 1, 0, Some("GS7"), 1, 15, 7, None),
            (16, 1, 0, Some("GA6"), 0, 16, 0, Some("sin remitente")),
            (17, 1, 0, Some("GA7"), 3, 17, 105, None),
            (18, 1, 0, Some("GA8"), 1, 18, 20, None),
            (19, 2, 0, Some("D1"), 0, 20, 0, Some("hola Ana")),
            (20, 2, 1, Some("D2"), 0, 21, 0, Some("hola Ben")),
        ],
    );
    db.execute_batch(&format!(
        "INSERT INTO message_system VALUES (1, 11), (2, 12), (8, 5), (9, 14), (10, 1), (11, 27), (12, 15), (13, 6),
             (14, 20), (15, {UNKNOWN_ACTION});
         INSERT INTO message_system_chat_participant (message_row_id, user_jid_row_id) VALUES
             (2, 2), (2, 3), (2, 4), (2, 5), (2, 6), (8, 5), (9, 6), (12, 3), (14, 3);
         INSERT INTO message_system_value_change VALUES (1, 'Synthetic family'), (10, 'Synthetic family 2'),
             (11, 'A synthetic description');
         INSERT INTO message_quoted VALUES (6, 'GA3', 0);
         INSERT INTO message_add_on VALUES (1, 1, 0, 'R1', 3, 3, {at}, 56);
         INSERT INTO message_add_on_reaction VALUES (1, '👍', 0);
         INSERT INTO message_sticker_pack VALUES (17, '{PACK_ID}', 'Synthetic pack', 'Nobody');
         INSERT INTO message_media (message_row_id, file_path, mime_type, file_size) VALUES
             (18, 'Media/WhatsApp Stickers/STK-00000001-WA0001.webp', 'image/webp', 34);",
        at = T0 + 7 * 60_000,
    ))
    .expect("fixture events");

    let key = Key::from_hex(KEY_HEX).expect("fixture key");
    let databases = dir.path().join("Databases");
    fs::create_dir_all(&databases).expect("fixture dir");
    let backup = databases.join("msgstore.db.crypt15");
    seal(db, &backup, &key, 3);
    seal(contacts(), &databases.join("wa.db.crypt15"), &key, 4);

    let media = dir.path().join("Media");
    let sticker = media.join("WhatsApp Stickers/STK-00000001-WA0001.webp");
    fs::create_dir_all(sticker.parent().expect("has parent")).expect("fixture media dir");
    fs::write(sticker, SHARED_STICKER).expect("fixture sticker");
    packs(&media.join("WhatsApp Sticker Packs"));

    let bens = dir.path().join("ben").join("msgstore.db.crypt15");
    fs::create_dir_all(bens.parent().expect("has parent")).expect("fixture dir");
    seal(bens_backup(), &bens, &Key::from_hex(TARGET_KEY_HEX).expect("fixture key"), 5);
    GroupFixture { dir, backup, media, bens }
}

/// Ana's contacts (`wa.db`): Ben by the name in her address book, Cleo by the name she set herself, and Zed, who is
/// in none of her groups.
fn contacts() -> Connection {
    let db = Connection::open_in_memory().expect("contacts db");
    db.execute_batch(&format!(
        "CREATE TABLE wa_contacts (_id INTEGER PRIMARY KEY, jid TEXT, display_name TEXT, wa_name TEXT,
             status TEXT, number TEXT);
         INSERT INTO wa_contacts VALUES (1, '{BEN}', 'Ben Prueba', 'Benny', 'busy', '+999900000011'),
             (2, '{CLEO}', NULL, 'Cleo', NULL, '+999900000033'),
             (3, '{ZED}', 'Zed Nadie', NULL, NULL, '+999900000099');"
    ))
    .expect("contacts");
    db
}

/// Ben's backup: the same group from his phone. He's the owner here (Ana the creator); Cleo under a `lid` with no
/// `jid_map` row (another person, until someone says "same person as"), Dan under a `lid` that `jid_map` links to his
/// number. He joined after the start: he has GA3 to GA5 and the left event (same key), the removed event without a
/// key at the same time and code as Ana's, his own GB1, and Ana adding him (only on his phone). Chat 2 is his chat
/// with Ana; chat 3 a group only he is in.
fn bens_backup() -> Connection {
    let db = Connection::open_in_memory().expect("fixture db");
    db.execute_batch(SCHEMA).expect("fixture schema");
    db.execute_batch(&format!(
        "INSERT INTO jid VALUES (1, '999900000001', 's.whatsapp.net', '{ANA}'), (2, '888800000033', 'lid', '{CLEO_LID}'),
             (3, '999900000044', 's.whatsapp.net', '{DAN}'), (4, '888800000044', 'lid', '{DAN_LID}'),
             (5, '999900000055', 's.whatsapp.net', '{EVE}'), (6, '999900000066', 's.whatsapp.net', '{FAY}'),
             (7, '120363000000000001', 'g.us', '{GROUP_JID}'), (8, '', '', ''),
             (9, '120363000000000009', 'g.us', '120363000000000009@g.us');
         INSERT INTO jid_map VALUES (4, 3);
         INSERT INTO chat VALUES (1, 7, 'Synthetic family 2'), (2, 1, NULL), (3, 9, 'Ben only');
         INSERT INTO group_participant_user VALUES (1, 7, 1, 2, 0), (2, 7, 8, 1, 0), (3, 7, 2, 0, 0), (4, 7, 4, 0, 0);
         INSERT INTO group_past_participant_user VALUES (1, 7, 5, 1, {left}), (2, 7, 6, 0, {removed});",
        left = T0 + 8 * 60_000,
        removed = T0 + 9 * 60_000,
    ))
    .expect("fixture people");
    insert_messages(
        &db,
        &[
            (1, 1, 0, Some("GB0"), 1, 1, 7, None),
            (2, 1, 0, Some("GA3"), 2, 4, 0, Some("hola Ben")),
            (3, 1, 0, Some("GA4"), 1, 5, 0, Some("respuesta")),
            (4, 1, 0, Some("GA5"), 4, 6, 0, Some("soy Dan")),
            (5, 1, 0, Some("GS1"), 5, 8, 7, None),
            (6, 1, 0, None, 1, 9, 7, None),
            (7, 1, 1, Some("GB1"), 0, 19, 0, Some("ya estoy")),
            (8, 2, 1, Some("D1"), 0, 20, 0, Some("hola Ana")),
            (9, 2, 0, Some("D2"), 0, 21, 0, Some("hola Ben")),
            (10, 3, 0, Some("X1"), 0, 22, 0, Some("solo Ben")),
        ],
    );
    db.execute_batch(
        "INSERT INTO message_system VALUES (1, 12), (5, 5), (6, 14);
         INSERT INTO message_system_chat_participant (message_row_id, user_jid_row_id) VALUES (1, 8), (5, 5), (6, 6);",
    )
    .expect("fixture events");
    db
}

/// The pack zips (point 30): [`PACK_ID`]'s three stickers and icon (one sticker byte-for-byte the one sent in the
/// group), a third-party pack with `contents.json`, a `.enc` file that isn't a zip, a zip with a `../` entry and one
/// with a sticker over the size cap.
fn packs(dir: &Path) {
    fs::create_dir_all(dir).expect("packs dir");
    zip(
        &dir.join(format!("{PACK_ID}.zip")),
        &[
            ("1.webp", SHARED_STICKER),
            ("2.webp", b"RIFF synthetic sticker two WEBP"),
            ("3.webp", b"RIFF synthetic sticker three WEBP"),
            ("tray.png", b"\x89PNG synthetic tray"),
        ],
    );
    let contents = br#"{"name": "Json pack", "publisher": "Someone", "tray_image_file": "tray.png",
        "stickers": [{"image_file": "a.webp", "emojis": ["\ud83d\ude00"]}, {"image_file": "b.webp", "emojis": []}]}"#;
    zip(
        &dir.join("third.party.zip"),
        &[
            ("contents.json", contents),
            ("a.webp", b"RIFF synthetic third a WEBP"),
            ("b.webp", b"RIFF synthetic third b WEBP"),
            ("tray.png", b"\x89PNG synthetic tray two"),
        ],
    );
    fs::write(dir.join("locked.enc"), b"not a zip at all").expect("fixture file");
    zip(&dir.join("escape.zip"), &[("../outside.webp", b"RIFF escape WEBP")]);
    let big = vec![b'x'; (1 << 20) + 1];
    zip(&dir.join("big.zip"), &[("big.webp", &big)]);
}

fn zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).expect("fixture zip"));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        zip.start_file(*name, options).expect("zip entry");
        zip.write_all(bytes).expect("zip bytes");
    }
    zip.finish().expect("fixture zip");
}

/// Writes `db` encrypted with `key` to `path`, as WhatsApp's crypt15.
fn seal(db: Connection, path: &Path, key: &Key, iv: u8) {
    let plain = path.with_extension("plain");
    db.execute("VACUUM INTO ?1", [plain.to_str().expect("utf-8 temp path")]).expect("fixture db file");
    let sqlite = fs::read(&plain).expect("fixture db bytes");
    fs::remove_file(&plain).expect("only the encrypted copy remains");
    fs::write(path, crypt15::encrypt(&sqlite, key, [iv; 16], true)).expect("fixture backup");
}
