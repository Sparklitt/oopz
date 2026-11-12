//! `oopz lib …` end to end through the binary, on invented exports (oopz-gen) and the synthetic backup only: every
//! command runs, the counts match what was generated, and nothing printed holds message text, numbers or keys.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use oopz::fixture::{self, BEN_JID, KEY_HEX};

fn oopz(lib: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_oopz"))
        .arg("lib")
        .args(args)
        .arg("--library")
        .arg(lib)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(line) = stdin {
        writeln!(child.stdin.take().unwrap(), "{line}").unwrap();
    }
    child.wait_with_output().unwrap()
}

fn ok(lib: &Path, args: &[&str]) -> String {
    let out = oopz(lib, args, None);
    let all = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "oopz lib {args:?}: {all}");
    for secret in ["+9999", "99990000", "Hola", "playa", BEN_JID, KEY_HEX] {
        assert!(!all.contains(secret), "oopz lib {args:?} printed {secret:?}: {all}");
    }
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn fails(lib: &Path, args: &[&str]) -> String {
    let out = oopz(lib, args, None);
    assert!(!out.status.success(), "oopz lib {args:?} should fail");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Writes oopz-gen's `format` export (the first layout) into `dir` and returns the file or folder to import and how
/// many messages it holds.
fn export(dir: &Path, format: &str) -> (PathBuf, usize) {
    let (_, e) = oopz_gen::exports::generate(format, 7).unwrap().remove(0);
    let root = dir.join(format);
    for (name, bytes) in &e.files {
        let p = root.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
    }
    let path = if e.files.len() == 1 { root.join(&e.files[0].0) } else { root };
    (path, e.manifest.messages)
}

fn number(line: &str, after: &str) -> u64 {
    line.split(after).nth(1).unwrap().split_whitespace().next().unwrap().trim_end_matches(',').parse().unwrap()
}

fn first(out: &str) -> u64 {
    out.split_whitespace().next().unwrap().parse().unwrap()
}

#[test]
fn every_lib_command_on_invented_exports_and_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    let lib = lib.as_path();
    let out = dir.path().join("out");
    std::fs::create_dir(&out).unwrap();

    assert!(ok(lib, &["adapters"]).contains("whatsapp-android"));
    let (telegram, telegram_n) = export(dir.path(), "telegram-json");
    let (sms, sms_n) = export(dir.path(), "sms-xml");
    let added = ok(lib, &["import", telegram.to_str().unwrap()]);
    assert_eq!(number(&added, "people, ") as usize, telegram_n, "{added}");
    // SMS Backup & Restore writes messages and calls to two files: each is its own source.
    let mut sms_added = 0;
    for name in ["sms-20261110.xml", "calls-20261110.xml"] {
        sms_added += number(&ok(lib, &["import", sms.join(name).to_str().unwrap()]), "people, ") as usize;
    }
    assert_eq!(sms_added, sms_n);
    let again = sms.join("sms-20261110.xml");
    assert!(fails(lib, &["import", again.to_str().unwrap()]).contains("already in the library"));

    // A backup asks for its key (piped here, one line) and is refused with the wrong one.
    let fx = fixture::build();
    let backup = fx.backup.to_str().unwrap();
    let wrong = oopz(lib, &["import", backup], Some(&"2".repeat(64)));
    assert!(!wrong.status.success());
    let right = oopz(lib, &["import", backup], Some(KEY_HEX));
    assert!(right.status.success(), "{}", String::from_utf8_lossy(&right.stderr));

    let sources = ok(lib, &["sources"]);
    assert_eq!(sources.lines().filter(|l| l.trim_start().starts_with(char::is_numeric)).count(), 4, "{sources}");
    assert!(sources.contains("telegram") && sources.contains("sms") && sources.contains("whatsapp"));
    // The version the backup's header names, and the type the registry doesn't know, with where to add it.
    assert!(sources.contains("format: crypt15, WhatsApp 2.99.1.1"), "{sources}");
    assert!(sources.contains("unknown type 9999 in whatsapp_android 2.99.1.1"), "{sources}");
    assert!(sources.contains(msgkinds::HOW_TO_ADD), "{sources}");

    let convs = ok(lib, &["convs", "--app", "telegram"]);
    let first_conv = convs.lines().nth(1).unwrap().split_whitespace().next().unwrap().to_owned();
    assert!(ok(lib, &["convs", "--names"]).lines().count() > 3);
    assert!(ok(lib, &["jump", "--conv", &first_conv, "--date", "2023-06-01"]).contains("position"));

    let total = first(&ok(lib, &["count", "--all"]));
    assert!(total as usize >= telegram_n + sms_n);
    let stats = ok(lib, &["stats", "--names"]);
    assert!(stats.contains(&format!("messages: {total} ")) && stats.contains("by app:"), "{stats}");
    let hits = ok(lib, &["search", "Hola"]);
    assert!(hits.contains("results, newest first") && hits.contains("whatsapp"), "{hits}");
    let timeline = ok(lib, &["timeline", "--limit", "5"]);
    assert_eq!(timeline.lines().count(), 7, "{timeline}");

    ok(lib, &["people", "--names"]);
    ok(lib, &["contacts", "--names"]);
    let suggest = ok(lib, &["suggest"]);
    assert!(suggest.contains("suggestions"));
    let people = ok(lib, &["people"]);
    let ids: Vec<&str> = people
        .lines()
        .skip(1)
        .filter(|l| !l.contains("(you)"))
        .map(|l| l.split_whitespace().next().unwrap())
        .take(2)
        .collect();
    let linked = ok(lib, &["link", "--person", &ids.join(","), "--name", "Invented Person"]);
    let contact = linked.split("contact ").nth(1).unwrap().trim().to_owned();
    ok(lib, &["rename", "--contact", &contact, "--name", "Renamed"]);
    let vcf = ok(lib, &["vcard-export", "--contact", &contact, "--out-dir", out.to_str().unwrap()]);
    let vcf = vcf.split("saved: ").nth(1).unwrap().split(" (").next().unwrap().to_owned();
    assert!(std::fs::read_to_string(&vcf).unwrap().contains("FN:Renamed"));
    ok(lib, &["unlink", "--person", ids[0]]);
    ok(lib, &["forget", "--contact", &contact]);
    assert!(ok(lib, &["vcard-import", &vcf]).contains("1 contacts"));

    let saved = ok(lib, &["select", "--name", "sms only", "--app", "sms"]);
    assert!(saved.contains(&format!("{sms_n} messages")), "{saved}");
    let id = saved.split("selection ").nth(1).unwrap().split_whitespace().next().unwrap().to_owned();
    assert!(ok(lib, &["selections"]).contains("sms only"));
    assert_eq!(first(&ok(lib, &["count", "--selection", &id])) as usize, sms_n);
    for format in oopz_export::Format::ALL.map(|f| f.code()) {
        let w = ok(lib, &["export", "--format", format, "--selection", &id, "--out-dir", out.to_str().unwrap()]);
        assert!(w.contains("saved: "), "{w}");
    }
    let w = ok(
        lib,
        &[
            "export",
            "--format",
            oopz_export::Format::Txt.code(),
            "--app",
            "telegram",
            "--order",
            "time",
            "--tz",
            "UTC",
            "--out-dir",
        ]
        .into_iter()
        .chain([out.to_str().unwrap()])
        .collect::<Vec<_>>(),
    );
    assert!(w.contains(&format!("{telegram_n} messages")), "{w}");
    assert!(
        fails(lib, &["export", "--format", "not-a-format", "--out-dir", out.to_str().unwrap()]).contains("--format")
    );
    ok(lib, &["unselect", "--selection", &id]);

    let telegram_id = sources.lines().find(|l| l.contains("telegram")).unwrap().split_whitespace().next().unwrap();
    ok(lib, &["remove", "--source", telegram_id]);
    let left = ok(lib, &["sources"]);
    assert!(!left.contains("telegram") && left.contains("sms"), "{left}");
    assert!(fails(lib, &["remove", "--source", telegram_id]).contains("no source"));
}

#[test]
fn set_me_on_a_text_export_and_flags_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    let lib = lib.as_path();
    let (line, _) = export(dir.path(), "line-txt");
    let added = ok(lib, &["import", line.to_str().unwrap()]);
    if added.contains("set-me") {
        let people = ok(lib, &["people"]);
        let person = people.lines().nth(1).unwrap().split_whitespace().next().unwrap();
        assert!(ok(lib, &["set-me", "--person", person]).contains("is you"));
        assert!(ok(lib, &["people"]).contains("(you)"));
    }
    assert!(fails(lib, &["convs", "--app", "myspace"]).contains("unknown app"));
    assert!(fails(lib, &["count", "--kind", "hologram"]).contains("unknown kind"));
    assert!(fails(lib, &["count", "--from", "everyone"]).contains("--from"));
    assert!(fails(lib, &["accept", "99"]).contains("no suggestion 99"));
    let out = dir.path().join("out");
    std::fs::create_dir(&out).unwrap();
    assert!(
        fails(lib, &["export", "--format", oopz_export::Format::Csv.code(), "--out-dir", out.to_str().unwrap()])
            .contains("--all")
    );
    assert!(
        ok(lib, &["export", "--format", oopz_export::Format::Csv.code(), "--all", "--out-dir", out.to_str().unwrap()])
            .contains("saved: ")
    );
    let no_lib = Command::new(env!("CARGO_BIN_EXE_oopz")).args(["lib", "sources"]).output().unwrap();
    assert!(String::from_utf8_lossy(&no_lib.stderr).contains("--library"));
}

#[test]
fn an_encrypted_iphone_backup_with_its_password_and_its_photos() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    let lib = lib.as_path();
    let backup = dir.path().join("iphone");
    let m = oopz_gen::ios::generate(&backup, Some("invented pw"), false).unwrap();
    let wrong = oopz(lib, &["import", backup.to_str().unwrap()], Some("not it"));
    assert!(!wrong.status.success());
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("doesn't open"));
    let right = oopz(lib, &["import", backup.to_str().unwrap()], Some("invented pw"));
    let out = String::from_utf8_lossy(&right.stdout).into_owned();
    assert!(right.status.success(), "{}", String::from_utf8_lossy(&right.stderr));
    assert!(String::from_utf8_lossy(&right.stderr).contains("as whatsapp-ios"));
    assert_eq!(number(&out, "people, ") as usize, m.messages, "{out}");
    assert!(out.contains("media files aren't in the backup"));
    // The photos came out of the backup into the library's own folder.
    let media: Vec<_> = std::fs::read_dir(lib.join("media").join("1")).unwrap().collect();
    assert_eq!(media.len(), m.media_files);
    let zip = dir.path().join("media");
    std::fs::create_dir(&zip).unwrap();
    let w = ok(lib, &["export", "--format", "media-zip", "--all", "--out-dir", zip.to_str().unwrap()]);
    assert!(w.contains("saved: "), "{w}");
}
