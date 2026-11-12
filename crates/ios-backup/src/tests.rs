use super::write::{Encryption, File, write};
use super::*;

const DOMAIN: &str = "AppDomainGroup-group.example.app";

fn files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("Data/store.sqlite", b"not really sqlite, 31 bytes!!!!".to_vec()),
        ("Media/a/b/photo.jpg", vec![7u8; 16]),
        ("Media/a/empty.bin", Vec::new()),
        ("Media/big.bin", (0..100_000u32).map(|i| (i % 251) as u8).collect()),
        ("Media/100%_odd name.txt", b"percent".to_vec()),
    ]
}

fn make(dir: &Path, password: Option<&str>) {
    let owned = files();
    let list: Vec<File<'_>> = owned.iter().map(|(p, b)| File { domain: DOMAIN, relative_path: p, bytes: b }).collect();
    let enc = password.map(|password| Encryption { password, dpic: 1000, iter: 10 });
    write(dir, "Invented iPhone", &list, enc.as_ref()).unwrap();
}

fn check_contents(b: &Backup) {
    for (path, bytes) in files() {
        let e = b.file(DOMAIN, path).unwrap().unwrap_or_else(|| panic!("{path}"));
        assert!(b.has(&e));
        assert_eq!(b.read(&e, u64::MAX).unwrap(), bytes, "{path}");
    }
    assert_eq!(b.files(DOMAIN, "Media/").unwrap().len(), 4);
    assert_eq!(b.files(DOMAIN, "Media/100%").unwrap().len(), 1, "% and _ are matched literally");
    assert_eq!(b.files(DOMAIN, "Media/1").unwrap().len(), 1);
    assert!(b.file(DOMAIN, "Media/a").unwrap().is_none(), "folders aren't files");
    assert!(b.file("OtherDomain", "Data/store.sqlite").unwrap().is_none());
    assert_eq!(b.domains("AppDomainGroup-").unwrap(), [DOMAIN]);
    let big = b.file(DOMAIN, "Media/big.bin").unwrap().unwrap();
    assert!(b.read(&big, 10).is_err(), "a size limit");
    let out = tempfile::tempdir().unwrap();
    let p = b.extract(&big, out.path()).unwrap();
    assert_eq!(fs::read(p).unwrap().len(), 100_000);
}

use std::fs;

#[test]
fn a_plain_backup_reads_without_a_password() {
    let dir = tempfile::tempdir().unwrap();
    make(dir.path(), None);
    let i = info(dir.path()).unwrap();
    assert!(!i.encrypted);
    assert_eq!(i.device_name.as_deref(), Some("Invented iPhone"));
    assert_eq!(i.date_ms, Some(1_704_067_200_000));
    let b = Backup::open(&dir.path().join("Manifest.plist"), Some("ignored")).unwrap();
    check_contents(&b);
}

#[test]
fn an_encrypted_backup_needs_its_own_password() {
    let dir = tempfile::tempdir().unwrap();
    make(dir.path(), Some("invented pässword"));
    assert!(info(dir.path()).unwrap().encrypted);
    assert!(matches!(Backup::open(dir.path(), None), Err(Error::NeedsPassword)));
    assert!(matches!(Backup::open(dir.path(), Some("")), Err(Error::NeedsPassword)));
    assert!(matches!(Backup::open(dir.path(), Some("invented password")), Err(Error::WrongPassword)));
    let b = Backup::open(dir.path(), Some("invented pässword")).unwrap();
    check_contents(&b);
    // The manifest was decrypted into a private folder that goes with the backup.
    let tmp = b._tmp.as_ref().unwrap().path().to_path_buf();
    assert!(tmp.join("Manifest.db").is_file());
    drop(b);
    assert!(!tmp.exists());
}

#[test]
fn not_a_backup_too_old_damaged_or_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(Backup::open(dir.path(), None), Err(Error::NotABackup)));
    fs::write(dir.path().join("Manifest.plist"), b"garbage").unwrap();
    fs::write(dir.path().join("Manifest.mbdb"), b"mbdb").unwrap();
    assert!(matches!(Backup::open(dir.path(), None), Err(Error::TooOld)));
    fs::write(dir.path().join("Manifest.db"), b"x").unwrap();
    assert!(matches!(Backup::open(dir.path(), None), Err(Error::Damaged(_))));

    let dir = tempfile::tempdir().unwrap();
    make(dir.path(), Some("pw"));
    let b = Backup::open(dir.path(), Some("pw")).unwrap();
    let e = b.file(DOMAIN, "Media/a/b/photo.jpg").unwrap().unwrap();
    // A file cut short, then gone (a backup stopped halfway).
    let stored = dir.path().join(&e.file_id[..2]).join(&e.file_id);
    fs::write(&stored, [1u8; 15]).unwrap();
    assert!(matches!(b.read(&e, u64::MAX), Err(Error::Damaged(_))));
    fs::remove_file(&stored).unwrap();
    assert!(!b.has(&e));
    assert!(matches!(b.read(&e, u64::MAX), Err(Error::Io(..))));
    // A keybag cut short.
    let mut m = plist::Value::from_file(dir.path().join("Manifest.plist")).unwrap();
    let d = m.as_dictionary_mut().unwrap();
    let bag = d.get("BackupKeyBag").unwrap().as_data().unwrap().to_vec();
    d.insert("BackupKeyBag".into(), plist::Value::Data(bag[..bag.len() - 3].to_vec()));
    m.to_file_binary(dir.path().join("Manifest.plist")).unwrap();
    assert!(matches!(Backup::open(dir.path(), Some("pw")), Err(Error::Damaged(_))));
}

/// docs/messages.md, errors row 9: a damaged backup's diagnostic is a developer's (English, from the catalog) inside
/// a user's message in the person's language; what was being done is a user's.
#[test]
fn errors_9_a_diagnostic_is_a_developers_inside_a_users_words() {
    let dir = tempfile::tempdir().unwrap();
    make(dir.path(), Some("pw"));
    let b = Backup::open(dir.path(), Some("pw")).unwrap();
    let e = b.file(DOMAIN, "Media/a/b/photo.jpg").unwrap().unwrap();
    let stored = dir.path().join(&e.file_id[..2]).join(&e.file_id);
    fs::write(&stored, [1u8; 15]).unwrap();
    let Err(Error::Damaged(why)) = b.read(&e, u64::MAX) else { panic!("a cut-off file is damaged") };
    assert!(!why.message.for_users && why.message.catalog == "oopz", "{why:?}");
    let damaged = Error::Damaged(why);
    assert_eq!(damaged.note().text("es"), "dañado: la copia de iPhone: an encrypted file's length isn't whole blocks");

    fs::remove_file(&stored).unwrap();
    let Err(gone @ Error::Io(..)) = b.read(&e, u64::MAX) else { panic!("a missing file is an io error") };
    let spanish = gone.note().text("es");
    let doing = format!("la copia de iPhone, leyendo {} en la copia: ", e.relative_path);
    assert!(spanish.starts_with(&doing), "{spanish}");
}
