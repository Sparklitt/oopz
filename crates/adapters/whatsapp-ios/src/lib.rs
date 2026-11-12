//! WhatsApp from an iPhone's own backup (decision 0018; docs/adapters/whatsapp-ios.md). The backup is the one Finder
//! or iTunes made on the owner's computer; an encrypted one opens with the backup password the owner chose. WhatsApp's
//! `ChatStorage.sqlite` (Core Data tables `ZWACHATSESSION`, `ZWAMESSAGE`, `ZWAMEDIAITEM`, `ZWAGROUPMEMBER`) is
//! decrypted into a private temporary folder and read there; media files come out of the backup one message at a
//! time, as inline bytes the library keeps in its own folder.
//!
//! WhatsApp changes these tables between versions, so every column is optional: a missing one reads as empty. What
//! the tables keep in protocol buffers (reactions, replies, edits, polls) isn't read, and is said so in the notes.

use std::collections::HashMap;
use std::path::Path;

use msgkinds::{Code, Flag, Source as Registry};
use oopz_ios_backup::{Backup, Entry};
use oopz_model::{
    self as m, Adapter, App, Conversation, ConversationKind, Event, Media, MediaKind, Message, Part, Person, Secret,
    Source, SourceInfo,
};
use rusqlite::{Connection, OpenFlags};

pub const DOMAIN: &str = "AppDomainGroup-group.net.whatsapp.WhatsApp.shared";
pub const BUSINESS_DOMAIN: &str = "AppDomainGroup-group.net.whatsapp.WhatsAppSMB.shared";
/// Seconds from 1970 to Apple's epoch, 2001-01-01 UTC.
const APPLE_EPOCH: f64 = 978_307_200.0;
/// A media file larger than this stays in the backup (named, not copied).
pub const MAX_MEDIA: u64 = 512 << 20;
const ME: &str = "me";

/// WhatsApp, or WhatsApp Business, from an iPhone backup.
pub struct WhatsAppIosAdapter {
    pub business: bool,
}

/// The app an adapter reads: WhatsApp, or WhatsApp Business (its name is msgkinds').
fn app_of(business: bool) -> msgkinds::App {
    if business { msgkinds::App::WhatsAppBusiness } else { msgkinds::App::WhatsApp }
}

pub const WHATSAPP: WhatsAppIosAdapter = WhatsAppIosAdapter { business: false };
pub const BUSINESS: WhatsAppIosAdapter = WhatsAppIosAdapter { business: true };

fn err(e: oopz_ios_backup::Error) -> m::Error {
    use oopz_ios_backup::Error as E;
    match e {
        E::NotABackup => m::Error::NotThisFormat("iPhone backup"),
        E::TooOld => m::Error::UnsupportedVersion("an iPhone backup from iOS 9 or older".into()),
        E::NeedsPassword => m::Error::NeedsSecret,
        E::WrongPassword => m::Error::WrongSecret,
        E::Damaged(d) => m::Error::Damaged(d),
        E::Io(what, e) => m::Error::Io(what, e),
    }
}

fn sql(e: rusqlite::Error) -> m::Error {
    m::Error::Damaged(format!("ChatStorage.sqlite: {e}"))
}

impl Adapter for WhatsAppIosAdapter {
    fn id(&self) -> &'static str {
        if self.business { "whatsapp-business-ios" } else { "whatsapp-ios" }
    }

    fn app(&self) -> App {
        App::WhatsApp
    }

    fn secret(&self) -> Secret {
        Secret::Password
    }

    fn secret_for(&self, path: &Path) -> Secret {
        match oopz_ios_backup::info(path) {
            Ok(i) if !i.encrypted => Secret::None,
            _ => Secret::Password,
        }
    }

    /// An iPhone backup folder (or a file in it). Whether WhatsApp is inside shows only once it's open: an
    /// encrypted manifest can't be read before.
    fn detect(&self, path: &Path) -> bool {
        oopz_ios_backup::backup_dir(path).is_some()
    }

    fn open(&self, path: &Path, secret: Option<&str>) -> m::Result<Box<dyn Source>> {
        let backup = Backup::open(path, secret).map_err(err)?;
        let (domain, other) = if self.business { (BUSINESS_DOMAIN, DOMAIN) } else { (DOMAIN, BUSINESS_DOMAIN) };
        let Some(entry) = backup.file(domain, "ChatStorage.sqlite").map_err(err)? else {
            let what = app_of(self.business).name();
            return Err(m::Error::Damaged(format!("this iPhone backup holds no {what} chats")));
        };
        let tmp = tempfile::Builder::new()
            .prefix("oopz-wa-ios-")
            .tempdir()
            .map_err(|e| m::Error::Io("a private temporary folder".into(), e))?;
        let db_path = backup.extract(&entry, tmp.path()).map_err(err)?;
        // The write-ahead log, when the backup kept one: its pages are the newest messages.
        for side in ["ChatStorage.sqlite-wal", "ChatStorage.sqlite-shm"] {
            if let Some(e) = backup.file(domain, side).map_err(err)? {
                backup.extract(&e, tmp.path()).map_err(err)?;
            }
        }
        let db =
            Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
                .map_err(sql)?;
        let mut notes = Vec::new();
        if !self.business && backup.file(other, "ChatStorage.sqlite").map_err(err)?.is_some() {
            notes.push(
                "WhatsApp Business is in this backup too: add it again choosing WhatsApp Business (iPhone)".into(),
            );
        }
        let book = match backup.file(domain, "ContactsV2.sqlite").map_err(err)? {
            Some(e) => {
                let p = backup.extract(&e, tmp.path()).map_err(err)?;
                address_book(&p)
            }
            None => HashMap::new(),
        };
        // The WhatsApp version backed up, from the manifest's `Applications` (decision 0020).
        let bundle = if self.business { "net.whatsapp.WhatsAppSMB" } else { "net.whatsapp.WhatsApp" };
        let version = backup.info.apps.get(bundle).cloned();
        let mut source = IosSource {
            info: SourceInfo {
                app: App::WhatsApp,
                adapter: self.id().into(),
                format: Some(format!(
                    "{}{} on iPhone{}",
                    app_of(self.business).name(),
                    version.as_deref().map(|v| format!(" {v}")).unwrap_or_default(),
                    backup.info.product_version.as_deref().map(|v| format!(" (iOS {v})")).unwrap_or_default()
                )),
                owner: Some(Person { id: ME.into(), me: true, ..Default::default() }),
                media_root: None,
                wall_clock: false,
                notes,
            },
            people: Vec::new(),
            conversations: Vec::new(),
            columns: HashMap::new(),
            domain,
            reading: msgkinds::Reading::new(version.as_deref()),
            unknown: HashMap::new(),
            missing_media: 0,
            large_media: 0,
            backup,
            db,
            _tmp: tmp,
        };
        source.load(&book)?;
        Ok(Box::new(source))
    }
}

/// WhatsApp's own address book: name by jid.
fn address_book(path: &Path) -> HashMap<String, String> {
    let Ok(db) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return HashMap::new() };
    let Ok(mut stmt) = db.prepare(
        "SELECT ZWHATSAPPID, coalesce(nullif(ZFULLNAME, ''), ZGIVENNAME) FROM ZWAADDRESSBOOKCONTACT
         WHERE ZWHATSAPPID IS NOT NULL",
    ) else {
        return HashMap::new();
    };
    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))
        .map(|rows| rows.flatten().filter_map(|(j, n)| Some((j, n.filter(|n| !n.trim().is_empty())?))).collect())
        .unwrap_or_default()
}

/// A chat session: its key, jid, partner's name, session type, removed flag.
type ChatRow = (i64, Option<String>, Option<String>, Option<i64>, Option<i64>);

struct IosSource {
    info: SourceInfo,
    people: Vec<Person>,
    conversations: Vec<Conversation>,
    /// Columns each table has (this WhatsApp version's).
    columns: HashMap<&'static str, Vec<String>>,
    domain: &'static str,
    /// Every lookup, at the WhatsApp version the backup's manifest names; its report joins the notes.
    reading: msgkinds::Reading,
    /// Types a row names but this reader can't place (none today): kept as unknown parts.
    unknown: HashMap<i64, u64>,
    missing_media: u64,
    large_media: u64,
    backup: Backup,
    db: Connection,
    _tmp: tempfile::TempDir,
}

/// The phone number a WhatsApp id holds, when it's a person's (`<digits>@s.whatsapp.net`).
fn phone_of(jid: &str) -> Option<String> {
    let (user, server) = jid.split_once('@')?;
    (server == "s.whatsapp.net" && !user.is_empty() && user.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("+{user}"))
}

fn apple_ms(secs: f64) -> Option<i64> {
    secs.is_finite().then(|| ((secs + APPLE_EPOCH) * 1000.0).round() as i64)
}

impl IosSource {
    fn has(&self, table: &str, column: &str) -> bool {
        self.columns.get(table).is_some_and(|c| c.iter().any(|x| x == column))
    }

    /// `column` of `alias` when the table has it, else NULL.
    fn col(&self, table: &'static str, alias: &str, column: &str) -> String {
        if self.has(table, column) { format!("{alias}.{column}") } else { "NULL".into() }
    }

    fn load(&mut self, book: &HashMap<String, String>) -> m::Result<()> {
        for table in ["ZWACHATSESSION", "ZWAMESSAGE", "ZWAMEDIAITEM", "ZWAGROUPMEMBER", "ZWAPROFILEPUSHNAME"] {
            let cols: Vec<String> = self
                .db
                .prepare(&format!("PRAGMA table_info({table})"))
                .and_then(|mut s| s.query_map([], |r| r.get::<_, String>(1))?.collect())
                .map_err(sql)?;
            self.columns.insert(table, cols);
        }
        for needed in ["ZWACHATSESSION", "ZWAMESSAGE"] {
            if self.columns[needed].is_empty() {
                return Err(m::Error::UnsupportedVersion(format!("WhatsApp without a {needed} table")));
            }
        }
        let mut names: HashMap<String, String> = book.clone();
        if self.has("ZWAPROFILEPUSHNAME", "ZJID") {
            let mut stmt = self.db.prepare("SELECT ZJID, ZPUSHNAME FROM ZWAPROFILEPUSHNAME").map_err(sql)?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)));
            for (jid, name) in rows.map_err(sql)?.flatten() {
                if let (Some(jid), Some(name)) = (jid, name.filter(|n| !n.trim().is_empty())) {
                    names.entry(jid).or_insert(name);
                }
            }
        }
        let mut people: HashMap<String, Person> = HashMap::new();
        let person = |jid: &str, name: Option<String>, people: &mut HashMap<String, Person>| {
            let p = people.entry(jid.to_owned()).or_insert_with(|| Person {
                id: jid.to_owned(),
                phone: phone_of(jid),
                name: names.get(jid).cloned(),
                ..Default::default()
            });
            if p.name.is_none() {
                p.name = name.filter(|n| !n.trim().is_empty());
            }
        };
        let q = format!(
            "SELECT s.Z_PK, {}, {}, {}, {} FROM ZWACHATSESSION s ORDER BY s.Z_PK",
            self.col("ZWACHATSESSION", "s", "ZCONTACTJID"),
            self.col("ZWACHATSESSION", "s", "ZPARTNERNAME"),
            self.col("ZWACHATSESSION", "s", "ZSESSIONTYPE"),
            self.col("ZWACHATSESSION", "s", "ZREMOVED"),
        );
        let chats: Vec<ChatRow> = self
            .db
            .prepare(&q)
            .and_then(|mut s| {
                s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect()
            })
            .map_err(sql)?;
        let mut members: HashMap<i64, Vec<String>> = HashMap::new();
        if self.has("ZWAGROUPMEMBER", "ZMEMBERJID") {
            let q = format!(
                "SELECT ZCHATSESSION, ZMEMBERJID, {} FROM ZWAGROUPMEMBER g",
                self.col("ZWAGROUPMEMBER", "g", "ZCONTACTNAME")
            );
            let mut stmt = self.db.prepare(&q).map_err(sql)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
                })
                .map_err(sql)?;
            for (chat, jid, name) in rows.flatten() {
                if let (Some(chat), Some(jid)) = (chat, jid) {
                    person(&jid, name, &mut people);
                    members.entry(chat).or_default().push(jid);
                }
            }
        }
        for (pk, jid, partner, kind, removed) in chats {
            if removed == Some(1) {
                continue;
            }
            let jid = jid.unwrap_or_default();
            let kind = if jid.ends_with("@g.us") || kind == Some(1) {
                ConversationKind::Group
            } else if jid.ends_with("@newsletter") {
                ConversationKind::Channel
            } else if jid.ends_with("@broadcast") {
                ConversationKind::Broadcast
            } else {
                ConversationKind::Direct
            };
            let mut conv_members = vec![ME.to_owned()];
            if kind == ConversationKind::Direct && !jid.is_empty() {
                person(&jid, partner.clone(), &mut people);
                conv_members.push(jid.clone());
            }
            conv_members.extend(members.remove(&pk).unwrap_or_default());
            let name = match kind {
                ConversationKind::Direct => names.get(&jid).cloned().or(partner),
                ConversationKind::Broadcast if jid == "status@broadcast" => Some("Status".into()),
                _ => partner,
            };
            self.conversations.push(Conversation {
                id: pk.to_string(),
                kind,
                name: name.filter(|n| !n.trim().is_empty()),
                members: conv_members,
                created_ms: None,
            });
        }
        let mut list: Vec<Person> = people.into_values().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        self.people = list;
        Ok(())
    }

    /// The media part of a message, its bytes taken out of the backup when they're there.
    fn media(
        &mut self,
        kind: MediaKind,
        path: Option<String>,
        title: Option<String>,
        size: Option<i64>,
        secs: Option<i64>,
        caption: Option<String>,
    ) -> Part {
        let name = title.clone().or_else(|| path.as_deref().and_then(|p| p.rsplit('/').next()).map(str::to_owned));
        let mut media = Media {
            kind,
            name,
            size: size.and_then(|s| u64::try_from(s).ok()).filter(|&s| s > 0),
            secs: secs.and_then(|s| u64::try_from(s).ok()).filter(|&s| s > 0),
            caption: caption.filter(|c| !c.is_empty()),
            ..Default::default()
        };
        let entry = path.as_deref().and_then(|p| self.entry_for(p));
        match entry {
            Some(e) if e.size.is_some_and(|s| s > MAX_MEDIA) => {
                self.large_media += 1;
                media.omitted = true;
            }
            Some(e) if self.backup.has(&e) => match self.backup.read(&e, MAX_MEDIA) {
                Ok(bytes) => {
                    media.size = media.size.or(Some(bytes.len() as u64));
                    media.data = Some(bytes);
                }
                Err(_) => {
                    self.missing_media += 1;
                    media.omitted = true;
                }
            },
            _ => {
                if path.is_some() {
                    self.missing_media += 1;
                }
                media.omitted = true;
            }
        }
        Part::Media(media)
    }

    /// The backup's file for `ZMEDIALOCALPATH`: relative (`Media/…`) or absolute (`…/Message/Media/…`).
    fn entry_for(&self, local: &str) -> Option<Entry> {
        let rel = match local.find("Message/Media/") {
            Some(i) => local[i..].to_owned(),
            None => format!("Message/{}", local.trim_start_matches('/')),
        };
        self.backup.file(self.domain, &rel).ok().flatten()
    }
}

impl Source for IosSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }

    fn people(&self) -> &[Person] {
        &self.people
    }

    fn conversations(&self) -> &[Conversation] {
        &self.conversations
    }

    fn messages(&mut self, conversation: &str, each: &mut dyn FnMut(Message) -> bool) -> m::Result<()> {
        let chat: i64 = conversation.parse().map_err(|_| m::Error::Damaged(format!("no chat {conversation}")))?;
        let has_media = !self.columns["ZWAMEDIAITEM"].is_empty() && self.has("ZWAMESSAGE", "ZMEDIAITEM");
        let mc = |c: &str| if has_media && self.has("ZWAMEDIAITEM", c) { format!("i.{c}") } else { "NULL".into() };
        let member_jid = if self.has("ZWAMESSAGE", "ZGROUPMEMBER") && self.has("ZWAGROUPMEMBER", "ZMEMBERJID") {
            "(SELECT g.ZMEMBERJID FROM ZWAGROUPMEMBER g WHERE g.Z_PK = msg.ZGROUPMEMBER)".to_owned()
        } else {
            "NULL".into()
        };
        let q = format!(
            "SELECT msg.Z_PK, {from_me}, {kind}, {date}, {from}, {text}, {starred}, {member},
                 {path}, {title}, {size}, {secs}, {lat}, {lng}, {vname}, {vcard}, {event}, {flags}
             FROM ZWAMESSAGE msg {join}
             WHERE msg.ZCHATSESSION = ?1 ORDER BY {date} IS NULL, {date}, msg.Z_PK",
            from_me = self.col("ZWAMESSAGE", "msg", "ZISFROMME"),
            kind = self.col("ZWAMESSAGE", "msg", "ZMESSAGETYPE"),
            date = self.col("ZWAMESSAGE", "msg", "ZMESSAGEDATE"),
            from = self.col("ZWAMESSAGE", "msg", "ZFROMJID"),
            text = self.col("ZWAMESSAGE", "msg", "ZTEXT"),
            starred = self.col("ZWAMESSAGE", "msg", "ZSTARRED"),
            member = member_jid,
            path = mc("ZMEDIALOCALPATH"),
            title = mc("ZTITLE"),
            size = mc("ZFILESIZE"),
            secs = mc("ZMOVIEDURATION"),
            lat = mc("ZLATITUDE"),
            lng = mc("ZLONGITUDE"),
            vname = mc("ZVCARDNAME"),
            vcard = mc("ZVCARDSTRING"),
            event = self.col("ZWAMESSAGE", "msg", "ZGROUPEVENTTYPE"),
            flags = self.col("ZWAMESSAGE", "msg", "ZFLAGS"),
            join = if has_media { "LEFT JOIN ZWAMEDIAITEM i ON i.Z_PK = msg.ZMEDIAITEM" } else { "" },
        );
        type Raw = (
            i64,
            Option<bool>,
            Option<i64>,
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<bool>,
            Option<String>,
            (Option<String>, Option<String>, Option<i64>, Option<f64>),
            (Option<f64>, Option<f64>, Option<String>, Option<String>, Option<i64>),
        );
        let rows: Vec<Raw> = {
            let mut stmt = self.db.prepare(&q).map_err(sql)?;
            stmt.query_map([chat], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    (r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?),
                    (r.get(12)?, r.get(13)?, r.get(14)?, r.get(15)?, r.get(16)?),
                ))
            })
            .and_then(|rows| rows.collect())
            .map_err(sql)?
        };
        let direct_partner = self
            .conversations
            .iter()
            .find(|c| c.id == conversation)
            .filter(|c| c.kind == ConversationKind::Direct)
            .and_then(|c| c.members.get(1).cloned());
        for (
            pk,
            from_me,
            kind,
            date,
            from,
            text,
            starred,
            member,
            (path, title, size, secs),
            (lat, lng, vname, vcard, event_type),
        ) in rows
        {
            let from_me = from_me.unwrap_or(false);
            let kind = kind.unwrap_or(0);
            let text = text.filter(|t| !t.is_empty());
            let sender = if from_me { Some(ME.to_owned()) } else { member.or_else(|| direct_partner.clone()).or(from) };
            let mut msg = Message {
                id: pk.to_string(),
                conversation: conversation.to_owned(),
                sender,
                from_me,
                time_ms: date.and_then(apple_ms),
                starred: starred.unwrap_or(false),
                ..Default::default()
            };
            let mut parts = Vec::new();
            // What `ZMESSAGETYPE` (and a notice's `ZGROUPEVENTTYPE`) means comes from msgkinds' registry
            // (decision 0020); a type it doesn't name stays an unknown part with its code.
            let voice_ext = msgkinds::media_type::sources::whatsapp::voice_note_extension();
            let opus = path.as_deref().and_then(msgkinds::media_type::name::extension).is_some_and(|e| e == voice_ext);
            let Some((kind_of, row, detail)) = resolve(kind, event_type, opus, &mut self.reading) else {
                parts.push(Part::Unknown { code: kind.to_string(), text });
                msg.parts = parts;
                if !each(msg) {
                    return Err(m::Error::Cancelled);
                }
                continue;
            };
            if let Some(mk) = MediaKind::from_kind(kind_of) {
                let view_once = row.has(Flag::ViewOnce);
                let caption = if kind_of == m::Kind::Document || view_once { None } else { text };
                let secs = secs.map(|s| s as i64).filter(|_| kind_of.has_duration());
                let mut part = if view_once {
                    Part::Media(Media { kind: mk, ..Default::default() })
                } else {
                    self.media(mk, path, title, size, secs, caption)
                };
                if let Part::Media(media) = &mut part {
                    media.view_once = view_once;
                }
                parts.push(part);
            } else {
                match kind_of {
                    m::Kind::Text => parts.push(Part::text(text.unwrap_or_default())),
                    m::Kind::Link => {
                        let url = text.clone().unwrap_or_default();
                        match msgkinds::media_type::sources::whatsapp_export::links(&url).first().copied() {
                            Some(u) => {
                                parts.push(Part::text(url.clone()));
                                parts.push(Part::Link { url: u.to_owned(), title: None });
                            }
                            None => parts.push(Part::text(url)),
                        }
                    }
                    m::Kind::Contact => {
                        let card = vcard.unwrap_or_default();
                        let phones = card
                            .lines()
                            .filter(|l| {
                                l.to_ascii_uppercase().starts_with("TEL") || l.to_ascii_uppercase().contains(".TEL")
                            })
                            .filter_map(|l| l.split_once(':').map(|(_, v)| v.trim().to_owned()))
                            .collect();
                        parts.push(Part::Contact { name: vname, phones, vcard: Some(card).filter(|c| !c.is_empty()) });
                    }
                    m::Kind::Location => parts.push(Part::Location { lat, lng, name: text, live: row.has(Flag::Live) }),
                    m::Kind::System => {
                        let event = row.event.map_or(Event::Other, Event::from);
                        parts.push(Part::System { event, text });
                    }
                    m::Kind::Deleted => msg.deleted = true,
                    m::Kind::Poll => parts.push(Part::Poll {
                        question: text.unwrap_or_default(),
                        options: Vec::new(),
                        multiple: false,
                    }),
                    m::Kind::Call => {
                        // A missed call's notice; a logged call's details live in CallHistory.sqlite (not read yet).
                        let outcome = if detail { m::CallOutcome::Missed } else { m::CallOutcome::Unknown };
                        parts.push(Part::Call { video: row.has(Flag::Video), outcome, secs: None });
                    }
                    m::Kind::Album
                    | m::Kind::StickerPack
                    | m::Kind::Event
                    | m::Kind::Payment
                    | m::Kind::Game
                    | m::Kind::Business
                    | m::Kind::Story
                    | m::Kind::Reaction => {
                        let card = m::CardKind::from_kind(kind_of).expect("a card kind");
                        parts.push(Part::Card { kind: card, title, text });
                    }
                    // Media handled above; never a registry row's kind.
                    m::Kind::Photo
                    | m::Kind::Video
                    | m::Kind::VideoNote
                    | m::Kind::Gif
                    | m::Kind::Voice
                    | m::Kind::Audio
                    | m::Kind::Sticker
                    | m::Kind::Document
                    | m::Kind::Unknown => {
                        *self.unknown.entry(kind).or_default() += 1;
                        parts.push(Part::Unknown { code: kind.to_string(), text });
                    }
                }
            }
            msg.parts = parts;
            if !each(msg) {
                return Err(m::Error::Cancelled);
            }
        }
        self.write_notes();
        Ok(())
    }
}

/// What a message's `ZMESSAGETYPE` means, from msgkinds' registry: its kind, the row that settled it, and whether that
/// row is a group event's or notice's (`ZGROUPEVENTTYPE`). `None` for a type no row names.
/// Looked up at the backup's WhatsApp version through `reading`, which counts what it couldn't answer.
fn resolve(
    kind: i64,
    event_type: Option<i64>,
    opus: bool,
    reading: &mut msgkinds::Reading,
) -> Option<(m::Kind, &'static msgkinds::Entry, bool)> {
    let entry = reading.lookup(Registry::WhatsAppIos, Code::Int(kind)).entry()?;
    // A group event (6) or a chat notice (10) says what happened in `ZGROUPEVENTTYPE` (a channel event, 55, keeps
    // its words: its codes are its own).
    let detail = match kind {
        6 => Some(Registry::WhatsAppIosSystem),
        10 => Some(Registry::WhatsAppIosNotice),
        _ => None,
    }
    .and_then(|src| Some(reading.lookup(src, Code::Int(event_type?)).entry()))
    .flatten();
    let row = detail.unwrap_or(entry);
    let mut kind = m::Kind::from(row.kind);
    // Type 3 is a voice note when its file is Opus.
    if kind == m::Kind::Audio && row.has(Flag::Refined) && opus {
        kind = m::Kind::Voice;
    }
    Some((kind, row, detail.is_some()))
}

impl IosSource {
    /// The notes say what stayed out; rewritten after each conversation, so they're complete at the end.
    fn write_notes(&mut self) {
        self.info.notes.retain(|n| n.starts_with(msgkinds::App::WhatsAppBusiness.name()));
        self.info.notes.push(
            "reactions, replies, edits and poll votes are kept by WhatsApp in a form oopz doesn't read yet".into(),
        );
        let mut unknown: Vec<_> = self.unknown.iter().collect();
        unknown.sort();
        for (code, n) in unknown {
            self.info.notes.push(format!("{n} × message type {code} kept as unknown"));
        }
        if self.missing_media > 0 {
            self.info.notes.push(format!("{} media files aren't in the backup", self.missing_media));
        }
        if self.large_media > 0 {
            self.info.notes.push(format!(
                "{} media files larger than {} MB left in the backup",
                self.large_media,
                MAX_MEDIA >> 20
            ));
        }
        self.info.notes.extend(self.reading.report().lines());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// msgkinds' registry.md, "What readers test on their side", 1: every row of the sources this adapter reads
    /// (`ZMESSAGETYPE`, and `ZGROUPEVENTTYPE` of a group event, a notice and a channel event) maps without falling
    /// to unknown.
    #[test]
    fn every_registry_row_maps() {
        let int = |e: &msgkinds::Entry| match e.code {
            Code::Int(n) => n,
            Code::Name(n) => panic!("{n}"),
        };
        for entry in Registry::WhatsAppIos.entries() {
            let (kind, row, _) = resolve(int(entry), None, false, &mut msgkinds::Reading::new(None)).expect("a row");
            assert_ne!(kind, m::Kind::Unknown, "ZMESSAGETYPE {}", int(entry));
            assert_eq!(kind, m::Kind::from(row.kind));
        }
        assert_eq!(resolve(3, None, true, &mut msgkinds::Reading::new(None)).unwrap().0, m::Kind::Voice);
        for (source, kind) in [(Registry::WhatsAppIosSystem, 6), (Registry::WhatsAppIosNotice, 10)] {
            for entry in source.entries() {
                let (got, row, detail) =
                    resolve(kind, Some(int(entry)), false, &mut msgkinds::Reading::new(None)).expect("a row");
                assert!(detail && std::ptr::eq(row, entry), "{source} {}", int(entry));
                assert_ne!(got, m::Kind::Unknown, "{source} {}", int(entry));
            }
        }
        // A type or an event no row names: unknown, or a notice that keeps its words.
        assert!(resolve(9999, None, false, &mut msgkinds::Reading::new(None)).is_none());
        let (kind, _, detail) = resolve(6, Some(9999), false, &mut msgkinds::Reading::new(None)).unwrap();
        assert_eq!((kind, detail), (m::Kind::System, false));
    }
}
