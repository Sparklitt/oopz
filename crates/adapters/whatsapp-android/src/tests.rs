use crate::crypt15::{self, Error as CryptError};
use crate::fixture::{self, BEN_CHAT, GROUP_CHAT, KEY_HEX};
use crate::*;
use msgkinds::media_type::formats;

fn key() -> Key {
    Key::from_hex(KEY_HEX).unwrap()
}

#[test]
fn crypt15_round_trips_with_and_without_checksum() {
    let sqlite = b"SQLite format 3\0synthetic".to_vec();
    for with_checksum in [true, false] {
        let file = crypt15::encrypt(&sqlite, &key(), [3; 16], with_checksum);
        assert_eq!(crypt15::decrypt(&file, &key()).unwrap(), sqlite);
    }
}

#[test]
fn crypt15_rejects_wrong_keys_and_damage_without_leaking_the_key() {
    let file = crypt15::encrypt(b"SQLite format 3\0synthetic", &key(), [3; 16], true);
    let other = "2".repeat(64);
    let err = crypt15::decrypt(&file, &Key::from_hex(&other).unwrap()).unwrap_err();
    assert!(matches!(err, CryptError::WrongKeyOrDamaged));
    assert!(!err.to_string().contains(&other));

    let mut flipped = file.clone();
    *flipped.last_mut().unwrap() ^= 1; // in the unchecked checksum: still opens
    assert!(crypt15::decrypt(&flipped, &key()).is_ok());
    let mid = file.len() / 2;
    flipped[mid] ^= 1;
    assert!(matches!(crypt15::decrypt(&flipped, &key()), Err(CryptError::WrongKeyOrDamaged)));

    assert!(matches!(
        crypt15::decrypt(&file[..10], &key()),
        Err(CryptError::WrongKeyOrDamaged | CryptError::NotCrypt15(_))
    ));
    assert!(matches!(crypt15::decrypt(&[], &key()), Err(CryptError::NotCrypt15(_))));
    assert!(format!("{:?}", key()) == "Key(..)");
}

/// docs/messages.md, errors row 6: which header check failed is typed, so the app's codes don't read English. An
/// empty file; a header length past the end; a zero-length header (a protobuf with no fields: no crypt15 IV, as a
/// crypt14 backup's); a header that isn't protobuf (field 0 doesn't exist).
#[test]
fn errors_6_each_header_refusal_says_which() {
    use oopz_model::messages::Message as M;
    let refused = |file: &[u8]| match crypt15::decrypt(file, &key()) {
        Err(CryptError::NotCrypt15(why)) => why,
        other => panic!("{other:?}"),
    };
    assert_eq!(refused(&[]), M::NotCrypt15Empty);
    assert_eq!(refused(&[9, 8]), M::NotCrypt15Truncated);
    assert_eq!(refused(&[0, 7, 7]), M::NotCrypt15NoIv);
    assert_eq!(refused(&[1, 0]), M::NotCrypt15Unreadable);
}

#[test]
fn the_whatsapp_version_is_read_from_the_header_before_any_key() {
    let with = crypt15::encrypt_as(b"SQLite format 3\0synthetic", &key(), [3; 16], true, Some("2.24.10.80"));
    assert_eq!(crypt15::app_version(&with).as_deref(), Some("2.24.10.80"));
    assert_eq!(crypt15::decrypt(&with, &key()).unwrap(), b"SQLite format 3\0synthetic");
    let without = crypt15::encrypt(b"SQLite format 3\0synthetic", &key(), [3; 16], true);
    assert_eq!(crypt15::app_version(&without), None);
    assert_eq!(crypt15::app_version(&[]), None);
    assert_eq!(crypt15::app_version(&with[..5]), None);
}

#[test]
fn keys_are_64_hex_digits_spaces_allowed() {
    let spaced = KEY_HEX.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join(" ");
    assert!(Key::from_hex(&spaced).is_ok());
    assert!(matches!(Key::from_hex("123"), Err(CryptError::BadKeyFormat)));
    assert!(matches!(Key::from_hex(&"g".repeat(64)), Err(CryptError::BadKeyFormat)));
}

#[test]
fn chats_are_listed_without_full_numbers() {
    let fx = fixture::build();
    let backup = Backup::open(&fx.backup, &key()).unwrap();
    let chats = backup.chats().unwrap();
    assert_eq!(chats.len(), 2);
    let ben = chats.iter().find(|c| c.row_id == BEN_CHAT).unwrap();
    assert_eq!(ben.messages, 18);
    assert_eq!(ben.masked(), "contact ••••001");
    assert!(!format!("{ben:?}").contains(fixture::BEN_JID));
    assert_eq!(chats.iter().find(|c| c.row_id == GROUP_CHAT).unwrap().masked(), "group ••••001");
    let lid = Chat { row_id: 9, jid: "123456789012345@lid".into(), messages: 0, last_ms: None, subject: None };
    assert_eq!(lid.masked(), "contact (lid) ••••345");
}

#[test]
fn type_profiles_count_structure_only() {
    let fx = fixture::build();
    let backup = Backup::open(&fx.backup, &key()).unwrap();
    let profiles = backup.type_profile(BEN_CHAT).unwrap();
    let get = |code| profiles.iter().find(|p| p.code == code).unwrap();

    assert_eq!(profiles.iter().map(|p| p.messages).sum::<u64>(), 18);
    let photos = get(1);
    assert_eq!((photos.messages, photos.with_text), (2, 1));
    assert_eq!(photos.linked, vec![("message_media".to_owned(), 2)]);
    assert_eq!(photos.mimes, vec![(formats::JPEG.to_owned(), 2)]);
    let text_links: Vec<_> = get(0).linked.iter().map(|(t, n)| (t.as_str(), *n)).collect();
    let expected = [("message_edit_info", 1), ("message_forwarded", 1), ("message_quoted", 1), ("message_text", 1)];
    assert_eq!(text_links, expected);
    assert_eq!(get(90).linked, vec![("message_call_log".to_owned(), 1)]);
}

#[test]
fn health_checks_the_database_and_counts_what_it_holds() {
    let fx = fixture::build();
    let health = Backup::open(&fx.backup, &key()).unwrap().health().unwrap();
    assert_eq!((health.check.as_str(), health.chats, health.messages), ("ok", 2, 19));
    assert!(health.last_ms.is_some());
}

#[test]
fn tables_list_structure_and_counts() {
    let fx = fixture::build();
    let tables = Backup::open(&fx.backup, &key()).unwrap().tables().unwrap();
    let (_, columns, rows) = tables.iter().find(|(name, ..)| name == "message_call_log").unwrap();
    assert_eq!((columns.join(","), *rows), ("_id,message_row_id,call_log_row_id".to_owned(), 1));
}

#[test]
fn chat_facts_are_counts_and_survive_missing_columns() {
    let fx = fixture::build();
    let facts = Backup::open(&fx.backup, &key()).unwrap().chat_facts(BEN_CHAT).unwrap();
    let get = |label: &str| facts.iter().find(|(l, _)| *l == label).unwrap().1.clone();
    assert_eq!(get("messages"), "18");
    assert_eq!(get("from_me 0 / 1"), "10 / 8");
    assert_eq!(get("sort_id range"), "n/a"); // the fixture's schema has no sort_id
    assert_eq!(get("_id order breaks time order"), "0");
    let all: String = facts.iter().map(|(_, v)| v.as_str()).collect();
    for text in ["Hola", "playa", "respuesta", fixture::BEN_JID] {
        assert!(!all.contains(text), "printed {text:?}");
    }
}

#[test]
fn the_decrypted_copy_stays_in_the_given_folder_and_goes_with_the_backup() {
    let fx = fixture::build();
    let scratch = tempfile::tempdir().unwrap();
    let files = || std::fs::read_dir(scratch.path()).unwrap().count();
    let backup = Backup::open_in(&fx.backup, &key(), scratch.path()).unwrap();
    assert_eq!(files(), 1);
    assert_eq!(backup.chats().unwrap().len(), 2);
    drop(backup);
    assert_eq!(files(), 0);

    // A wrong key leaves nothing behind either.
    assert!(Backup::open_in(&fx.backup, &Key::from_hex(&"2".repeat(64)).unwrap(), scratch.path()).is_err());
    assert_eq!(files(), 0);
}
