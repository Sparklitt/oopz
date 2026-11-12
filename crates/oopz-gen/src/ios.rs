//! A synthetic iPhone backup holding WhatsApp's `ChatStorage.sqlite` (and `ContactsV2.sqlite`, media files), in the
//! layout Finder writes; encrypted with an invented password when one is given (decision 0018). Invented data only:
//! numbers start with 9999. The schema is the subset of WhatsApp's Core Data tables the adapter reads, with the
//! columns' real names; every odd row the adapter must survive is here (a missing media file, a type it doesn't know,
//! an empty text, a member who left, a message before 2001).

use std::sync::LazyLock;

use msgkinds::media_type::{fixtures, formats};
use std::path::Path;

use anyhow::{Context, Result};
use oopz_ios_backup::write::{Encryption, File, write};
use rusqlite::{Connection, params};

/// WhatsApp's app group, as the backup names its domain.
pub const DOMAIN: &str = "AppDomainGroup-group.net.whatsapp.WhatsApp.shared";
pub const BUSINESS_DOMAIN: &str = "AppDomainGroup-group.net.whatsapp.WhatsAppSMB.shared";
/// Seconds between 1970 and Apple's epoch (2001-01-01 UTC).
pub const APPLE_EPOCH: i64 = 978_307_200;

/// What the generated backup holds, for tests to compare with.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub conversations: usize,
    pub messages: usize,
    pub from_me: usize,
    pub media_files: usize,
    pub missing_media: usize,
    pub unknown_types: usize,
}

/// `(type, from_me, text, media)`: media is `(local path, title, bytes)`; `None` bytes mean the file isn't in the
/// backup.
type Row = (i64, bool, Option<&'static str>, Option<(&'static str, Option<&'static str>, Option<&'static [u8]>)>);

/// The backup's files: each starts as its format's files do (msgkinds' fixtures), then says it's invented.
fn synthetic(head: Vec<u8>, tag: &str) -> Vec<u8> {
    [head, tag.as_bytes().to_vec()].concat()
}
static PICTURE: LazyLock<Vec<u8>> = LazyLock::new(|| synthetic(fixtures::of(formats::PNG, 0), " invented picture"));
static VOICE: LazyLock<Vec<u8>> = LazyLock::new(|| synthetic(fixtures::opus(), " invented"));
static REPORT: LazyLock<Vec<u8>> = LazyLock::new(|| synthetic(fixtures::of(formats::PDF, 0), " invented"));
static STICKER: LazyLock<Vec<u8>> = LazyLock::new(|| synthetic(fixtures::of(formats::WEBP, 0), " invented"));

fn chat_storage(path: &Path, m: &mut Manifest) -> Result<()> {
    let db = Connection::open(path)?;
    db.execute_batch(
        "CREATE TABLE ZWACHATSESSION (Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, ZCONTACTJID VARCHAR,
             ZPARTNERNAME VARCHAR, ZSESSIONTYPE INTEGER, ZLASTMESSAGEDATE TIMESTAMP, ZARCHIVED INTEGER,
             ZREMOVED INTEGER);
         CREATE TABLE ZWAGROUPMEMBER (Z_PK INTEGER PRIMARY KEY, ZCHATSESSION INTEGER, ZMEMBERJID VARCHAR,
             ZCONTACTNAME VARCHAR, ZISADMIN INTEGER, ZISACTIVE INTEGER);
         CREATE TABLE ZWAMESSAGE (Z_PK INTEGER PRIMARY KEY, ZCHATSESSION INTEGER, ZGROUPMEMBER INTEGER,
             ZMEDIAITEM INTEGER, ZISFROMME INTEGER, ZMESSAGETYPE INTEGER, ZGROUPEVENTTYPE INTEGER, ZSTARRED INTEGER,
             ZFLAGS INTEGER, ZMESSAGEDATE TIMESTAMP, ZSENTDATE TIMESTAMP, ZFROMJID VARCHAR, ZTOJID VARCHAR,
             ZTEXT VARCHAR, ZSTANZAID VARCHAR, ZPUSHNAME VARCHAR);
         CREATE TABLE ZWAMEDIAITEM (Z_PK INTEGER PRIMARY KEY, ZMESSAGE INTEGER, ZMEDIALOCALPATH VARCHAR,
             ZTITLE VARCHAR, ZFILESIZE INTEGER, ZMOVIEDURATION INTEGER, ZLATITUDE FLOAT, ZLONGITUDE FLOAT,
             ZVCARDNAME VARCHAR, ZVCARDSTRING VARCHAR, ZMEDIAURL VARCHAR);
         CREATE TABLE ZWAPROFILEPUSHNAME (Z_PK INTEGER PRIMARY KEY, ZJID VARCHAR, ZPUSHNAME VARCHAR);",
    )?;
    let ana = "99990000001@s.whatsapp.net";
    let ben = "99990000002@s.whatsapp.net";
    let gone = "99990000009@s.whatsapp.net";
    let group = "99990000001-1700000000@g.us";
    db.execute_batch(&format!(
        "INSERT INTO ZWACHATSESSION VALUES (1, 4, '{ana}', 'Ana Inventada', 0, 700000000, 0, 0);
         INSERT INTO ZWACHATSESSION VALUES (2, 4, '{group}', 'Familia 🌙', 1, 700000500, 0, 0);
         INSERT INTO ZWACHATSESSION VALUES (3, 4, 'status@broadcast', NULL, 3, 700000600, 0, 0);
         INSERT INTO ZWACHATSESSION VALUES (4, 4, '99990000003@s.whatsapp.net', NULL, 0, NULL, 0, 0);
         INSERT INTO ZWAGROUPMEMBER VALUES (1, 2, '{ana}', 'Ana Inventada', 1, 1);
         INSERT INTO ZWAGROUPMEMBER VALUES (2, 2, '{ben}', NULL, 0, 1);
         INSERT INTO ZWAGROUPMEMBER VALUES (3, 2, '{gone}', NULL, 0, 0);
         INSERT INTO ZWAPROFILEPUSHNAME VALUES (1, '{ben}', 'Ben ~ push');"
    ))?;
    m.conversations = 4;
    let direct: Vec<Row> = vec![
        (0, false, Some("Hola, ¿qué tal? Invented text"), None),
        (0, true, Some("Bien 👍🏽 https://example.invalid/x"), None),
        (
            1,
            false,
            Some("a caption"),
            Some(("Media/99990000001@s.whatsapp.net/a/b/photo.jpg", None, Some(PICTURE.as_slice()))),
        ),
        (2, true, None, Some(("Media/99990000001@s.whatsapp.net/c/d/video.mp4", None, None))),
        (3, false, None, Some(("Media/99990000001@s.whatsapp.net/e/f/voice.opus", None, Some(VOICE.as_slice())))),
        (
            8,
            true,
            None,
            Some((
                "Media/99990000001@s.whatsapp.net/g/h/doc.pdf",
                Some("Invented report.pdf"),
                Some(REPORT.as_slice()),
            )),
        ),
        (4, false, None, None),
        (5, true, None, None),
        (7, false, Some("https://example.invalid/page"), None),
        (11, false, None, Some(("Media/99990000001@s.whatsapp.net/i/j/anim.mp4", None, Some(b"gif-ish")))),
        (14, false, None, None),
        (15, true, None, Some(("Media/99990000001@s.whatsapp.net/k/l/sticker.webp", None, Some(STICKER.as_slice())))),
        (0, false, Some(""), None),
        (999, false, Some("from a future version"), None),
    ];
    let mut pk = 0i64;
    let mut media_pk = 0i64;
    let mut insert =
        |chat: i64, member: Option<i64>, t: f64, row: &Row, from: Option<&str>, m: &mut Manifest| -> Result<()> {
            pk += 1;
            let (kind, me, text, media) = row;
            let media_id = if media.is_some() || matches!(kind, 4 | 5) {
                media_pk += 1;
                Some(media_pk)
            } else {
                None
            };
            db.execute(
                "INSERT INTO ZWAMESSAGE VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?9, ?10, ?11, ?12, ?13, NULL)",
                params![
                    pk,
                    chat,
                    member,
                    media_id,
                    me,
                    kind,
                    if *kind == 6 { Some(2) } else { None },
                    pk == 2,
                    t,
                    if *me { None } else { from },
                    if *me { from } else { None },
                    text,
                    format!("STANZA{pk}")
                ],
            )?;
            if let Some(id) = media_id {
                let (path, title, bytes) = media.unwrap_or(("", None, None));
                let (lat, lng) = if *kind == 5 { (Some(40.4168), Some(-3.7038)) } else { (None, None) };
                let (vname, vcard) = if *kind == 4 {
                    (
                        Some("Carla Inventada"),
                        Some("BEGIN:VCARD\nVERSION:3.0\nFN:Carla Inventada\nTEL:+99990000004\nEND:VCARD"),
                    )
                } else {
                    (None, None)
                };
                db.execute(
                    "INSERT INTO ZWAMEDIAITEM VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
                    params![
                        id,
                        pk,
                        (!path.is_empty()).then_some(path),
                        title,
                        bytes.map(|b| b.len() as i64),
                        if *kind == 3 { Some(7) } else { None },
                        lat,
                        lng,
                        vname,
                        vcard
                    ],
                )?;
                if !path.is_empty() {
                    if bytes.is_some() {
                        m.media_files += 1;
                    } else {
                        m.missing_media += 1;
                    }
                }
            }
            m.messages += 1;
            m.from_me += *me as usize;
            if *kind == 999 {
                m.unknown_types += 1;
            }
            Ok(())
        };
    for (i, row) in direct.iter().enumerate() {
        insert(1, None, 700_000_000.0 + i as f64 * 60.0, row, Some(ana), m)?;
    }
    let group_rows: Vec<(Option<i64>, Row)> = vec![
        (None, (6, false, None, None)),
        (Some(1), (0, false, Some("Grupo: hola a todos"), None)),
        (Some(2), (0, false, Some("Ben aquí"), None)),
        (Some(3), (0, false, Some("someone who left"), None)),
        (None, (0, true, Some("Mine in the group"), None)),
    ];
    for (i, (member, row)) in group_rows.iter().enumerate() {
        insert(2, *member, 700_000_100.0 + i as f64 * 60.0, row, Some(group), m)?;
    }
    // A status update, and a message from before 2001 (a negative Apple date).
    insert(3, None, 700_000_600.0, &(0, false, Some("a status"), None), Some(ana), m)?;
    insert(4, None, -86_400.0, &(0, false, Some("from 2000"), None), Some("99990000003@s.whatsapp.net"), m)?;
    Ok(())
}

fn contacts(path: &Path) -> Result<()> {
    let db = Connection::open(path)?;
    db.execute_batch(
        "CREATE TABLE ZWAADDRESSBOOKCONTACT (Z_PK INTEGER PRIMARY KEY, ZFULLNAME VARCHAR, ZGIVENNAME VARCHAR,
             ZWHATSAPPID VARCHAR, ZPHONENUMBER VARCHAR);
         INSERT INTO ZWAADDRESSBOOKCONTACT VALUES (1, 'Ana Agenda', 'Ana', '99990000001@s.whatsapp.net', '+99990000001');
         INSERT INTO ZWAADDRESSBOOKCONTACT VALUES (2, 'Dani Agenda', 'Dani', '99990000003@s.whatsapp.net', NULL);",
    )?;
    Ok(())
}

/// Writes the backup into `out` (an empty or missing folder). `business` puts it in WhatsApp Business's domain too.
pub fn generate(out: &Path, password: Option<&str>, business: bool) -> Result<Manifest> {
    let scratch = tempfile_dir(out)?;
    let mut m = Manifest::default();
    chat_storage(&scratch.join("ChatStorage.sqlite"), &mut m)?;
    contacts(&scratch.join("ContactsV2.sqlite"))?;
    let chat = std::fs::read(scratch.join("ChatStorage.sqlite"))?;
    let book = std::fs::read(scratch.join("ContactsV2.sqlite"))?;
    std::fs::remove_dir_all(&scratch)?;
    let media: Vec<(String, &[u8])> = [
        ("Message/Media/99990000001@s.whatsapp.net/a/b/photo.jpg", PICTURE.as_slice()),
        ("Message/Media/99990000001@s.whatsapp.net/e/f/voice.opus", VOICE.as_slice()),
        ("Message/Media/99990000001@s.whatsapp.net/g/h/doc.pdf", REPORT.as_slice()),
        ("Message/Media/99990000001@s.whatsapp.net/i/j/anim.mp4", b"gif-ish".as_slice()),
        ("Message/Media/99990000001@s.whatsapp.net/k/l/sticker.webp", STICKER.as_slice()),
    ]
    .into_iter()
    .map(|(p, b)| (p.to_owned(), b))
    .collect();
    let mut files = vec![
        File { domain: DOMAIN, relative_path: "ChatStorage.sqlite", bytes: &chat },
        File { domain: DOMAIN, relative_path: "ContactsV2.sqlite", bytes: &book },
        File { domain: "HomeDomain", relative_path: "Library/Preferences/invented.plist", bytes: b"other app data" },
    ];
    for (p, b) in &media {
        files.push(File { domain: DOMAIN, relative_path: p, bytes: b });
    }
    if business {
        files.push(File { domain: BUSINESS_DOMAIN, relative_path: "ChatStorage.sqlite", bytes: &chat });
    }
    let enc = password.map(|password| Encryption { password, dpic: 1000, iter: 10 });
    write(out, "Invented iPhone", &files, enc.as_ref()).context("writing the backup")?;
    Ok(m)
}

fn tempfile_dir(out: &Path) -> Result<std::path::PathBuf> {
    let dir = out.with_extension("oopz-gen-scratch");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
