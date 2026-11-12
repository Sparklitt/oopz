//! End to end through the binary, on the synthetic fixture only.

use std::io::Write;
use std::process::{Output, Stdio};

use oopz::fixture::{self, BEN_CHAT, BEN_JID, KEY_HEX};

mod common;

/// The terminal's locale every test but the language one runs in: what they read is English.
const LOCALE: &str = "C";

fn oopz(args: &[&str], key: Option<&str>) -> Output {
    oopz_in(LOCALE, args, key)
}

fn oopz_in(locale: &str, args: &[&str], key: Option<&str>) -> Output {
    let mut child = common::oopz_in(locale)
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
fn completions_and_the_man_page() {
    for (shell, mark) in [("bash", "_oopz()"), ("zsh", "#compdef oopz"), ("fish", "complete -c oopz")] {
        let out = oopz(&["completions", shell], None);
        assert!(out.status.success() && text(&out).contains(mark), "{shell}: {}", text(&out));
    }
    let man = oopz(&["man"], None);
    assert!(man.status.success() && text(&man).contains(".TH oopz 1"), "{}", text(&man));
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
    assert!(text(&types).contains("type 9999: 1 message, 0 with text"), "{}", text(&types));
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
    // WhatsApp's contacts backup sits beside the messages: not a message backup, so not checked (nor failed).
    std::fs::write(dir.join("wa.db.crypt15"), "contacts").unwrap();

    let ok = oopz(&["backups", "--dir", dir.to_str().unwrap()], Some(KEY_HEX));
    assert!(ok.status.success(), "{}", text(&ok));
    assert!(
        text(&ok).contains("msgstore.db.crypt15  0 MB  ok: 2 chats, 19 messages, last 2023-11-14"),
        "{}",
        text(&ok)
    );
    assert!(!text(&ok).contains("notes.txt"));
    assert!(!text(&ok).contains("wa.db"));

    // A cut-off copy fails on its own line; the good one still passes, and so does nothing else.
    let bytes = std::fs::read(&fx.backup).unwrap();
    std::fs::write(dir.join("msgstore-2023-11-13.1.db.crypt15"), &bytes[..bytes.len() / 2]).unwrap();
    let mixed = oopz(&["backups", "--dir", dir.to_str().unwrap()], Some(KEY_HEX));
    assert!(!mixed.status.success());
    assert!(text(&mixed).contains("msgstore-2023-11-13.1.db.crypt15  0 MB  FAILED:"), "{}", text(&mixed));
    assert!(text(&mixed).contains("backups that didn't pass: 1 of 2"), "{}", text(&mixed));
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
fn restore_writes_a_checked_backup_in_a_new_folder() {
    let fx = fixture::two_phones();
    let out = fx.dir.path().join("out");
    std::fs::create_dir(&out).unwrap();
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
        "--out-dir",
        out.to_str().unwrap(),
        "--media",
        fx.source_media.to_str().unwrap(),
    ];
    let keys = format!("{KEY_HEX}\n{}", fixture::TARGET_KEY_HEX);
    let merged = oopz(&args, Some(&keys));
    assert!(merged.status.success(), "{}", text(&merged));
    assert!(text(&merged).contains("already there: 2; added: 8"), "{}", text(&merged));
    assert!(text(&merged).contains("check integrity_check: ok"));
    assert!(text(&merged).contains("media files copied: 1; missing: 1"), "{}", text(&merged));
    assert!(text(&merged).contains("original target unchanged: yes"));
    let result = out.join("oopz-result-1");
    assert!(result.join("Media/WhatsApp Images/Sent/hilo-IMG-20231114-WA0001.jpg").is_file());
    // WhatsApp's file-name marker for a photo: the fixture's file names start with it.
    let photo_marker = msgkinds::media_type::sources::whatsapp::NAME_MARKERS
        .iter()
        .find(|(m, k, _)| *k == msgkinds::Kind::Photo && !m.starts_with('-'))
        .map_or("", |(m, ..)| *m);
    for secret in ["Hola", "playa", photo_marker, KEY_HEX, fixture::TARGET_KEY_HEX] {
        assert!(!text(&merged).contains(secret), "printed {secret:?}");
    }

    let backup = result.join("Databases/msgstore.db.crypt15");
    let facts = oopz(
        &["chat-facts", "--backup", backup.to_str().unwrap(), "--chat", &target_chat],
        Some(fixture::TARGET_KEY_HEX),
    );
    assert!(text(&facts).contains("messages: 12"), "{}", text(&facts));
    assert!(text(&facts).contains("all messages: _id max / sort_id max: 22 / 22"), "{}", text(&facts));

    // Again: a second folder, the first left as it was.
    let before = std::fs::read(&backup).unwrap();
    let again = oopz(&args, Some(&keys));
    assert!(again.status.success(), "{}", text(&again));
    assert!(out.join("oopz-result-2/Databases/msgstore.db.crypt15").is_file());
    assert_eq!(std::fs::read(&backup).unwrap(), before);
}

#[test]
fn check_stats_search_export_missing_print_counts_only() {
    let fx = fixture::two_phones();
    let (source, target) = (fx.source.to_str().unwrap(), fx.target.to_str().unwrap());

    let check = oopz(&["check", "--backup", source], Some(KEY_HEX));
    assert!(check.status.success(), "{}", text(&check));
    assert!(text(&check).contains("database: sound"));
    assert!(text(&check).contains("messages: 11"), "{}", text(&check));
    assert!(text(&check).contains("sha256: "));

    let stats = oopz(&["stats", "--backup", source, "--chat", "1"], Some(KEY_HEX));
    assert!(stats.status.success(), "{}", text(&stats));
    assert!(text(&stats).contains("messages: 10"), "{}", text(&stats));
    assert!(text(&stats).contains("type 1: 3 messages"), "{}", text(&stats));

    // The text searched for is the person's own; the matching messages' text is never printed.
    let search = oopz(&["search", "--backup", source, "--query", "respuesta"], Some(KEY_HEX));
    assert!(search.status.success(), "{}", text(&search));
    assert!(text(&search).contains("1 result"), "{}", text(&search));
    assert!(text(&search).contains("sent"));

    let pages = fx.dir.path().join("pages");
    std::fs::create_dir(&pages).unwrap();
    let export =
        oopz(&["export", "--backup", source, "--chat", "1", "--out-dir", pages.to_str().unwrap()], Some(KEY_HEX));
    assert!(export.status.success(), "{}", text(&export));
    let page = std::fs::read_to_string(pages.join("oopz-export-1.html")).unwrap();
    assert!(page.contains("<html lang=\"en\">") && page.contains("Hola Ben"), "the file has the messages");

    let missing = oopz(
        &["missing", "--a", source, "--chat-a", "1", "--b", target, "--chat-b", "2"],
        Some(&format!("{KEY_HEX}\n{}", fixture::TARGET_KEY_HEX)),
    );
    assert!(missing.status.success(), "{}", text(&missing));
    assert!(text(&missing).contains("in both: 2"), "{}", text(&missing));

    for out in [&check, &stats, &search, &export, &missing] {
        for secret in ["Hola", "playa", "respuesta ", BEN_JID, KEY_HEX, fixture::TARGET_KEY_HEX] {
            assert!(!text(out).contains(secret), "printed {secret:?}: {}", text(out));
        }
    }
}

/// The terminal's language words what the command prints (msgkinds' docs/messages.md rows 24-29 pick it).
#[test]
fn the_terminals_language_words_the_output() {
    let fx = fixture::build();
    let backup = fx.backup.to_str().unwrap();
    let es = text(&oopz_in("es_ES.UTF-8", &["check", "--backup", backup], Some(KEY_HEX)));
    assert!(es.contains("base de datos: en buen estado") && es.contains("mensajes: "), "{es}");
    for locale in ["C", "fr_FR.UTF-8", ""] {
        let en = text(&oopz_in(locale, &["check", "--backup", backup], Some(KEY_HEX)));
        assert!(en.contains("database: sound") && en.contains("messages: "), "{locale}: {en}");
    }
}

/// `oopz check` on the fixture with these extra variables and arguments: its output, and whether it succeeded.
fn check_with(vars: &[(&str, &str)], extra: &[&str]) -> (String, bool) {
    let fx = fixture::build();
    let mut command = common::oopz_in(LOCALE);
    command.args(["check", "--backup", fx.backup.to_str().unwrap()]).args(extra).envs(vars.iter().copied());
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    writeln!(child.stdin.take().unwrap(), "{KEY_HEX}").unwrap();
    let out = child.wait_with_output().unwrap();
    (text(&out), out.status.success())
}

const SPANISH: &str = "base de datos: en buen estado";
const ENGLISH: &str = "database: sound";

/// docs/messages.md, the command's language: rows 1-8, through the binary.
#[test]
fn the_language_asked_for_and_the_language_list() {
    let (es, ok) = check_with(&[], &["--lang", "es"]);
    assert!(ok && es.contains(SPANISH), "--lang beats LC_ALL=C: {es}");
    let (es, _) = check_with(&[("OOPZ_LANG", "es")], &[]);
    assert!(es.contains(SPANISH), "OOPZ_LANG: {es}");
    let (en, _) = check_with(&[("OOPZ_LANG", "es")], &["--lang", "en"]);
    assert!(en.contains(ENGLISH), "--lang beats OOPZ_LANG: {en}");
    let (en, _) = check_with(&[("OOPZ_LANG", "")], &[]);
    assert!(en.contains(ENGLISH), "an empty OOPZ_LANG is unset: {en}");
    let (es, _) = check_with(&[("LC_ALL", "en_US.UTF-8"), ("LANGUAGE", "fr:es:en")], &[]);
    assert!(es.contains(SPANISH), "LANGUAGE's first oopz has: {es}");
    let (en, _) = check_with(&[("LANGUAGE", "es")], &[]);
    assert!(en.contains(ENGLISH), "LANGUAGE is ignored in the C locale: {en}");
    let (es, _) = check_with(&[("LC_ALL", "ES_es")], &[]);
    assert!(es.contains(SPANISH), "case is ignored: {es}");
}

#[test]
fn asking_for_a_language_oopz_lacks_is_an_error_listing_its_languages() {
    for (vars, extra) in [(&[][..], &["--lang", "fr"][..]), (&[("OOPZ_LANG", "fr")][..], &[][..])] {
        let (out, ok) = check_with(vars, extra);
        assert!(!ok && out.contains("oopz has no language fr; it has: en, es"), "{out}");
    }
    let (out, ok) = check_with(&[("LC_ALL", "es_ES.UTF-8")], &["--lang", ""]);
    assert!(!ok && out.contains("--lang está vacío; da uno de: en, es"), "in the locale's language: {out}");
}

/// Row 12: the tests' command pins the language, whatever the developer's environment says.
#[test]
fn tests_pin_the_language() {
    let command = common::oopz_in(LOCALE);
    let envs: Vec<_> = command.get_envs().collect();
    assert!(envs.contains(&("LC_ALL".as_ref(), Some(LOCALE.as_ref()))));
    for variable in common::LANGUAGE_VARIABLES {
        assert!(envs.contains(&(variable.as_ref(), None)), "{variable} isn't cleared");
    }
}

#[test]
fn search_filters_and_jump_to_a_date() {
    let fx = fixture::two_phones();
    let source = fx.source.to_str().unwrap();
    // Chat 1 of the source: 10 messages, 3 of them photos.
    let photos = oopz(&["search", "--backup", source, "--chat", "1", "--kind", "1"], Some(KEY_HEX));
    assert!(text(&photos).contains("3 results"), "{}", text(&photos));
    let sent = oopz(&["search", "--backup", source, "--chat", "1", "--from", "me"], Some(KEY_HEX));
    assert!(sent.status.success() && !text(&sent).contains("received"), "{}", text(&sent));
    let none = oopz(&["search", "--backup", source, "--after", "2999-01-01"], Some(KEY_HEX));
    assert!(text(&none).contains("0 results"), "{}", text(&none));

    let start = oopz(&["jump", "--backup", source, "--chat", "1", "--date", "1970-01-01"], Some(KEY_HEX));
    assert!(text(&start).contains("position 1 of 10"), "{}", text(&start));
    let end = oopz(&["jump", "--backup", source, "--chat", "1", "--date", "2999-01-01"], Some(KEY_HEX));
    assert!(text(&end).contains("position 10 of 10"), "{}", text(&end));
    let bad = oopz(&["jump", "--backup", source, "--chat", "1", "--date", "yesterday"], None);
    assert!(!bad.status.success());

    let stats = oopz(&["stats", "--backup", source], Some(KEY_HEX));
    assert!(text(&stats).contains("by day and hour") && text(&stats).contains("Sat: "), "{}", text(&stats));
}

/// `oopz pull` with a stand-in adb on `PATH` that only lists devices: no adb, no phone, a phone that hasn't allowed
/// this computer, and two phones without `--serial` are refused before anything is copied or written.
#[cfg(unix)]
#[test]
fn pull_refuses_without_adb_a_phone_or_a_clear_choice() {
    use std::os::unix::fs::PermissionsExt;
    let (bin, empty, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let adb = bin.path().join("adb");
    std::fs::write(
        &adb,
        "#!/bin/sh\n[ \"$1\" = devices ] || exit 1\nprintf 'List of devices attached\\n%b' \"$DEVICES\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&adb, std::fs::Permissions::from_mode(0o755)).unwrap();
    let pull = |path: &std::path::Path, devices: &str| {
        common::oopz_in(LOCALE)
            .args(["pull", "--out-dir", out.path().to_str().unwrap()])
            .env("PATH", path)
            .env_remove("ANDROID_HOME")
            .env_remove("ANDROID_SDK_ROOT")
            .env("DEVICES", devices)
            .output()
            .unwrap()
    };
    let none = pull(empty.path(), "");
    assert!(!none.status.success() && text(&none).contains("platform-tools"), "{}", text(&none));
    let cases = [
        ("", "adb sees no phone"),
        ("R58N123\tunauthorized usb:1-2 transport_id:4\\n", "hasn't allowed this computer"),
        (
            "AAA\tdevice usb:1-1\\nBBB\tdevice usb:1-2\\n",
            "several phones are connected (AAA, BBB): choose one with --serial",
        ),
    ];
    for (devices, said) in cases {
        let refused = pull(bin.path(), devices);
        assert!(!refused.status.success() && text(&refused).contains(said), "{devices}: {}", text(&refused));
    }
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0, "nothing written");
}

/// docs/messages.md, merge checks row 6: in Spanish, every check is Spanish; no English check is mixed in (none is a
/// developer's).
#[test]
fn checks_6_restore_words_its_checks_in_the_commands_language() {
    let fx = fixture::two_phones();
    let out = fx.dir.path().join("out");
    std::fs::create_dir(&out).unwrap();
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
        "--out-dir",
        out.to_str().unwrap(),
    ];
    let merged = oopz_in("es_ES.UTF-8", &args, Some(&format!("{KEY_HEX}\n{}", fixture::TARGET_KEY_HEX)));
    assert!(merged.status.success(), "{}", text(&merged));
    let checks: Vec<String> =
        text(&merged).lines().filter_map(|l| l.strip_prefix("comprobación ")).map(String::from).collect();
    assert_eq!(checks.len(), 9, "{}", text(&merged));
    assert_eq!(checks[0], "se descifra con la clave del destino: sí");
    assert_eq!(checks[3], "mensajes del chat: 4 + 8 = 12");
    assert!(checks[8].starts_with("tablas: número de filas como se esperaba; "), "{checks:?}");
    assert!(!text(&merged).lines().any(|l| l.starts_with("check ")), "{}", text(&merged));
    for english in ["yes", "unchanged", "as before", "untouched", "identical,"] {
        assert!(!text(&merged).contains(english), "{english:?} in {}", text(&merged));
    }
}
