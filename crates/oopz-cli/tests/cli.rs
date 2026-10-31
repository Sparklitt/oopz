//! End to end through the binary, on the synthetic fixture only.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use oopz::fixture::{self, BEN_CHAT, BEN_JID, KEY_HEX};

fn oopz(args: &[&str], key: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_oopz"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(key) = key {
        writeln!(child.stdin.take().unwrap(), "{key}").unwrap();
    }
    child.wait_with_output().unwrap()
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

#[test]
fn chats_types_tables_print_structure_only() {
    let fx = fixture::build();
    let backup = fx.backup.to_str().unwrap();
    let chat = BEN_CHAT.to_string();

    let chats = oopz(&["chats", "--backup", backup], Some(KEY_HEX));
    assert!(chats.status.success(), "{}", text(&chats));
    assert!(text(&chats).contains("contact ••••001"));

    let types = oopz(&["types", "--backup", backup, "--chat", &chat], Some(KEY_HEX));
    assert!(text(&types).contains("type 11: 1 messages, 0 with text"), "{}", text(&types));
    assert!(text(&types).contains("linked message_media: 2"));

    let tables = oopz(&["tables", "--backup", backup], Some(KEY_HEX));
    assert!(text(&tables).contains("message_call_log (1): _id, message_row_id, call_log_row_id"), "{}", text(&tables));

    for out in [&chats, &types, &tables] {
        let all = text(out);
        for secret in ["Hola", "playa", BEN_JID, KEY_HEX] {
            assert!(!all.contains(secret), "printed {secret:?}");
        }
    }
}

#[test]
fn backups_checks_every_backup_in_a_folder() {
    let fx = fixture::build();
    let dir = fx.dir.path().join("Databases");
    std::fs::create_dir(&dir).unwrap();
    std::fs::copy(&fx.backup, dir.join("msgstore.db.crypt15")).unwrap();
    std::fs::write(dir.join("notes.txt"), "not a backup").unwrap();

    let ok = oopz(&["backups", "--dir", dir.to_str().unwrap()], Some(KEY_HEX));
    assert!(ok.status.success(), "{}", text(&ok));
    assert!(
        text(&ok).contains("msgstore.db.crypt15  0 MB  ok: 2 chats, 19 messages, last 2023-11-14"),
        "{}",
        text(&ok)
    );
    assert!(!text(&ok).contains("notes.txt"));

    // A cut-off copy fails on its own line; the good one still passes, and so does nothing else.
    let bytes = std::fs::read(&fx.backup).unwrap();
    std::fs::write(dir.join("msgstore-2023-11-13.1.db.crypt15"), &bytes[..bytes.len() / 2]).unwrap();
    let mixed = oopz(&["backups", "--dir", dir.to_str().unwrap()], Some(KEY_HEX));
    assert!(!mixed.status.success());
    assert!(text(&mixed).contains("msgstore-2023-11-13.1.db.crypt15  0 MB  FAILED:"), "{}", text(&mixed));
    assert!(text(&mixed).contains("1 of 2 backups didn't pass"), "{}", text(&mixed));
    for secret in ["Hola", "playa", BEN_JID, KEY_HEX] {
        assert!(!text(&mixed).contains(secret), "printed {secret:?}");
    }
}

#[test]
fn a_wrong_key_fails_without_echoing_it() {
    let fx = fixture::build();
    let wrong = "2".repeat(64);
    let out = oopz(&["chats", "--backup", fx.backup.to_str().unwrap()], Some(&wrong));
    assert!(!out.status.success());
    assert!(text(&out).contains("doesn't open this backup"));
    assert!(!text(&out).contains(&wrong));
}

#[test]
fn restore_writes_a_checked_backup_once() {
    let fx = fixture::two_phones();
    let out = fx.dir.path().join("merged.crypt15");
    let pack = fx.dir.path().join("pack");
    let (source_chat, target_chat) = (fixture::SOURCE_CHAT.to_string(), fixture::TARGET_CHAT.to_string());
    let args = [
        "restore",
        "--source",
        fx.source.to_str().unwrap(),
        "--source-chat",
        &source_chat,
        "--target",
        fx.target.to_str().unwrap(),
        "--target-chat",
        &target_chat,
        "--out",
        out.to_str().unwrap(),
        "--media",
        fx.source_media.to_str().unwrap(),
        "--media-out",
        pack.to_str().unwrap(),
    ];
    let keys = format!("{KEY_HEX}\n{}", fixture::TARGET_KEY_HEX);
    let merged = oopz(&args, Some(&keys));
    assert!(merged.status.success(), "{}", text(&merged));
    assert!(text(&merged).contains("2 already there, 8 added"), "{}", text(&merged));
    assert!(text(&merged).contains("check integrity_check: ok"));
    assert!(text(&merged).contains("media files: 1 copied (15 bytes), 1 missing"), "{}", text(&merged));
    assert!(pack.join("WhatsApp Images/Sent/hilo-IMG-20231114-WA0001.jpg").is_file());
    for secret in ["Hola", "playa", "IMG-", KEY_HEX, fixture::TARGET_KEY_HEX] {
        assert!(!text(&merged).contains(secret), "printed {secret:?}");
    }

    let facts =
        oopz(&["chat-facts", "--backup", out.to_str().unwrap(), "--chat", &target_chat], Some(fixture::TARGET_KEY_HEX));
    assert!(text(&facts).contains("messages: 12"), "{}", text(&facts));
    assert!(text(&facts).contains("all messages: _id max / sort_id max: 22 / 22"), "{}", text(&facts));

    let before = std::fs::read(&out).unwrap();
    let again = oopz(&args, Some(&keys));
    assert!(!again.status.success());
    assert!(text(&again).contains("already exists"));
    assert_eq!(std::fs::read(&out).unwrap(), before);
}
