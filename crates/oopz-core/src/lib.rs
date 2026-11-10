//! The oopz library (decision 0016): every source opened through an adapter (0013), in one SQLite file to read,
//! search, select, link and export from. It knows the model only; the adapters are handed to it.
//!
//! Rows hold the person's own messages, numbers and names: the library lives in the app's private folder (wiped on
//! close unless the person keeps it) or a folder the CLI is given, and nothing in it leaves the device.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use oopz_model::{self as m, Adapter, App, Kind, Source};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Transaction, params, params_from_iter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod vcard;

/// An error for the CLI and the app: a stable `code` and an English detail; never message text, a key or a number.
#[derive(Debug, Serialize, thiserror::Error)]
#[error("{detail}")]
pub struct Error {
    /// `exists`, `not_found`, `unsupported_file`, `needs_secret`, `wrong_key`, `damaged`, `unsupported`, `io`,
    /// `cancelled`, `no_space`, `library`.
    pub code: &'static str,
    pub detail: String,
}

impl Error {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self { code, detail: detail.into() }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::new("library", e.to_string())
    }
}

impl From<m::Error> for Error {
    fn from(e: m::Error) -> Self {
        let code = match &e {
            m::Error::NotThisFormat(_) => "unsupported_file",
            m::Error::UnsupportedVersion(_) => "unsupported",
            m::Error::NeedsSecret => "needs_secret",
            m::Error::WrongSecret => "wrong_key",
            m::Error::Damaged(_) => "damaged",
            m::Error::Io(..) => "io",
            m::Error::Cancelled => "cancelled",
        };
        Error::new(code, e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::new("library", e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The library's file inside its folder.
pub const FILE: &str = "library.db";
/// The schema this code writes; a newer one is refused, an older one upgraded.
const SCHEMA: i64 = 1;

/// What a filter narrows to; every part optional, empty lists mean "any". The same filter drives search, the
/// conversation list, stats, timelines, selections and exports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub apps: Vec<App>,
    pub sources: Vec<i64>,
    pub conversations: Vec<i64>,
    /// Linked contacts: their messages in groups, and their one-to-one conversations whole (both sides).
    pub contacts: Vec<i64>,
    /// People (per app), the same way as contacts.
    pub people: Vec<i64>,
    pub kinds: Vec<Kind>,
    pub from_me: Option<bool>,
    pub after_ms: Option<i64>,
    pub before_ms: Option<i64>,
    /// Words to find (accents and case ignored; the last word as a prefix).
    pub words: String,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        *self == Filter::default()
    }
}

/// A filter, plus messages added or taken out by hand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Selection {
    pub filter: Filter,
    pub add: Vec<i64>,
    pub remove: Vec<i64>,
}

/// The order messages come in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    /// Conversation by conversation, each as the source orders it.
    Conversation,
    /// Every conversation together by time (the cross-app timeline).
    Time,
    /// Newest first (search results).
    Newest,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SourceRow {
    pub id: i64,
    pub app: App,
    pub adapter: String,
    /// The file or folder's name (not its path).
    pub name: String,
    pub format: Option<String>,
    pub added_ms: i64,
    pub notes: Vec<String>,
    pub wall_clock: bool,
    pub has_media: bool,
    pub conversations: u64,
    pub people: u64,
    pub messages: u64,
    pub first_ms: Option<i64>,
    pub last_ms: Option<i64>,
    /// The owner is known (or was picked with [`Library::set_me`]).
    pub owner_known: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConversationRow {
    pub id: i64,
    pub source: i64,
    pub app: App,
    pub kind: m::ConversationKind,
    pub name: Option<String>,
    /// Names of the other members (contact names first), for a one-to-one conversation's title.
    pub members: Vec<String>,
    pub messages: u64,
    pub first_ms: Option<i64>,
    pub last_ms: Option<i64>,
    pub wall_clock: bool,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MessageRow {
    pub id: i64,
    pub conversation: i64,
    pub source: i64,
    pub app: App,
    pub pos: u64,
    pub time_ms: Option<i64>,
    pub wall_clock: bool,
    pub from_me: bool,
    pub sender: Option<i64>,
    /// The contact's name, else the app's name for them, else their id in the app.
    pub sender_name: Option<String>,
    pub kind: Kind,
    /// The message this one answers, when it's in the library.
    pub reply: Option<i64>,
    pub message: m::Message,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Hit {
    pub id: i64,
    pub conversation: i64,
    pub app: App,
    pub time_ms: Option<i64>,
    pub from_me: bool,
    pub sender_name: Option<String>,
    /// The matching words between `\u{2}` and `\u{3}`.
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PersonRow {
    pub id: i64,
    pub source: i64,
    pub app: App,
    pub key: String,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub me: bool,
    pub contact: Option<i64>,
    pub messages: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactRow {
    pub id: i64,
    pub name: String,
    /// From a vCard, or the linked people's.
    pub phones: Vec<String>,
    pub emails: Vec<String>,
    /// Linked people: (person id, app).
    pub people: Vec<(i64, App)>,
}

/// People in different sources who look like the same person.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Suggestion {
    /// `phone` or `email`.
    pub reason: &'static str,
    pub value: String,
    pub people: Vec<i64>,
    /// A contact already holding one of them (or a vCard contact with this number).
    pub contact: Option<i64>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Stats {
    pub messages: u64,
    pub from_me: u64,
    pub first_ms: Option<i64>,
    pub last_ms: Option<i64>,
    /// (`YYYY-MM`, messages)
    pub by_month: Vec<(String, u64)>,
    pub by_hour: Vec<u64>,
    /// Sunday first, 7 rows of 24.
    pub by_day_hour: Vec<Vec<u64>>,
    /// (kind, messages, media bytes)
    pub by_kind: Vec<(Kind, u64, u64)>,
    pub by_app: Vec<(App, u64)>,
    /// (person id, name, messages), most first, at most 50.
    pub by_person: Vec<(Option<i64>, Option<String>, u64)>,
    pub media_bytes: u64,
    pub voice_secs: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SavedSelection {
    pub id: i64,
    pub name: String,
    pub selection: Selection,
    pub saved_ms: i64,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn app_of(code: String) -> rusqlite::Result<App> {
    App::from_code(&code).ok_or_else(|| rusqlite::Error::InvalidColumnName(format!("app {code}")))
}

fn kind_of(code: &str) -> Kind {
    Kind::from_code(code).unwrap_or(Kind::Unknown)
}

/// FTS5 words: every word quoted, the last as a prefix when it's longer than one character.
fn fts_query(query: &str) -> Option<String> {
    let words: Vec<&str> = query.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let (last, rest) = words.split_last()?;
    let mut out: Vec<String> = rest.iter().map(|w| format!("\"{w}\"")).collect();
    out.push(if last.chars().count() > 1 { format!("\"{last}\"*") } else { format!("\"{last}\"") });
    Some(out.join(" "))
}

/// Chinese, Japanese and Korean text isn't split into words by the index: such searches look for the text itself.
fn has_cjk(s: &str) -> bool {
    s.chars().any(
        |c| matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF),
    )
}

/// SHA-256 of a file; for a folder, of its files' relative paths and sizes (a media folder would take too long).
pub fn fingerprint(path: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    if path.is_dir() {
        let mut files = Vec::new();
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let entries =
                fs::read_dir(&dir).map_err(|e| Error::new("io", format!("reading {}: {e}", dir.display())))?;
            for entry in entries.flatten() {
                let p = entry.path();
                match entry.file_type() {
                    Ok(t) if t.is_dir() => stack.push(p),
                    Ok(t) if t.is_file() => {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        files.push((p.strip_prefix(path).unwrap_or(&p).to_string_lossy().into_owned(), size));
                    }
                    _ => {}
                }
            }
        }
        files.sort();
        for (name, size) in files {
            hash.update(name.as_bytes());
            hash.update([0]);
            hash.update(size.to_le_bytes());
        }
    } else {
        let mut f = fs::File::open(path).map_err(|e| Error::new("io", format!("reading {}: {e}", path.display())))?;
        let mut buf = vec![0; 1 << 20];
        loop {
            let n = f.read(&mut buf).map_err(|e| Error::new("io", format!("reading {}: {e}", path.display())))?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
    }
    Ok(hex::encode(hash.finalize()))
}

/// The adapter for `path`: the one named, or the first whose `detect` says yes.
pub fn pick_adapter<'a>(adapters: &[&'a dyn Adapter], path: &Path, named: Option<&str>) -> Result<&'a dyn Adapter> {
    match named {
        Some(id) => adapters
            .iter()
            .find(|a| a.id() == id)
            .copied()
            .ok_or_else(|| Error::new("unsupported_file", format!("no adapter `{id}`"))),
        None => adapters.iter().find(|a| a.detect(path)).copied().ok_or_else(|| {
            Error::new("unsupported_file", format!("{} isn't a file oopz can open", display_name(path)))
        }),
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// The bytes of media a message holds, and a voice note's seconds.
fn media_facts(msg: &m::Message) -> (u64, u64) {
    let mut bytes = 0;
    let mut voice = 0;
    for p in &msg.parts {
        if let m::Part::Media(media) = p {
            bytes += media.size.unwrap_or(0);
            if matches!(media.kind, m::MediaKind::Voice | m::MediaKind::Audio) {
                voice += media.secs.unwrap_or(0);
            }
        }
    }
    (bytes, voice)
}

/// People grouped by what they share (`phone` or `email`, the value), with a vCard contact holding it.
type Matches<'a> = BTreeMap<(&'static str, String), (Vec<&'a PersonRow>, Option<i64>)>;

pub struct Library {
    db: Connection,
    dir: PathBuf,
}

/// The `WHERE` part for a filter over `msg m JOIN conv c JOIN source s`, and its values.
/// The filter's words, as a set of matching ids.
const FTS_SET: &str = "m.id IN (SELECT rowid FROM fts WHERE fts MATCH ?)";

fn narrow(filter: &Filter) -> (String, Vec<Value>) {
    let mut sql = vec!["1".to_owned()];
    let mut values: Vec<Value> = Vec::new();
    let list = |sql: &mut Vec<String>, values: &mut Vec<Value>, template: &str, items: Vec<Value>| {
        if items.is_empty() {
            return;
        }
        let marks = vec!["?"; items.len()].join(",");
        sql.push(template.replace("{}", &marks));
        values.extend(items);
    };
    list(&mut sql, &mut values, "s.app IN ({})", filter.apps.iter().map(|a| Value::Text(a.code().into())).collect());
    list(&mut sql, &mut values, "c.source IN ({})", filter.sources.iter().map(|&i| Value::Integer(i)).collect());
    list(&mut sql, &mut values, "m.conv IN ({})", filter.conversations.iter().map(|&i| Value::Integer(i)).collect());
    let who = |people_of: &str, n: usize| {
        let marks = vec!["?"; n].join(",");
        format!(
            "(m.sender IN (SELECT id FROM person WHERE {people_of} IN ({marks}))
              OR (c.kind = 'direct' AND m.conv IN (SELECT mb.conv FROM member mb JOIN person p ON p.id = mb.person
                  WHERE p.me = 0 AND p.{people_of} IN ({marks}))))"
        )
    };
    if !filter.contacts.is_empty() {
        sql.push(who("contact", filter.contacts.len()));
        for _ in 0..2 {
            values.extend(filter.contacts.iter().map(|&i| Value::Integer(i)));
        }
    }
    if !filter.people.is_empty() {
        sql.push(who("id", filter.people.len()));
        for _ in 0..2 {
            values.extend(filter.people.iter().map(|&i| Value::Integer(i)));
        }
    }
    list(&mut sql, &mut values, "m.kind IN ({})", filter.kinds.iter().map(|k| Value::Text(k.code().into())).collect());
    if let Some(me) = filter.from_me {
        sql.push("m.from_me = ?".into());
        values.push(Value::Integer(me.into()));
    }
    if let Some(t) = filter.after_ms {
        sql.push("m.ts >= ?".into());
        values.push(Value::Integer(t));
    }
    if let Some(t) = filter.before_ms {
        sql.push("m.ts < ?".into());
        values.push(Value::Integer(t));
    }
    let words = filter.words.trim();
    if has_cjk(words) {
        for w in words.split_whitespace() {
            sql.push("m.text LIKE ? ESCAPE '\\'".into());
            let escaped = w.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            values.push(Value::Text(format!("%{escaped}%")));
        }
    } else if let Some(q) = fts_query(words) {
        sql.push(FTS_SET.into());
        values.push(Value::Text(q));
    }
    (sql.join(" AND "), values)
}

const NAME_OF_SENDER: &str = "(SELECT coalesce(ct.name, p.name, p.username, p.phone, p.key) FROM person p
     LEFT JOIN contact ct ON ct.id = p.contact WHERE p.id = m.sender)";

impl Library {
    /// Opens the library in `dir` (made if missing), upgrading an older one; a newer one is refused.
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir).map_err(|e| Error::new("io", format!("making {}: {e}", dir.display())))?;
        let db = Connection::open(dir.join(FILE))?;
        // 64 MB of page cache: a large import keeps its indexes in memory instead of re-reading them.
        db.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA recursive_triggers = ON;
             PRAGMA cache_size = -65536; PRAGMA temp_store = MEMORY;",
        )?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA {
            return Err(Error::new("unsupported", "this library was made by a newer oopz"));
        }
        if version < SCHEMA {
            db.execute_batch(
                "CREATE TABLE IF NOT EXISTS source (id INTEGER PRIMARY KEY, app TEXT NOT NULL, adapter TEXT NOT NULL,
                     name TEXT NOT NULL, fingerprint TEXT NOT NULL UNIQUE, format TEXT, added_ms INTEGER NOT NULL,
                     notes TEXT NOT NULL, media_root TEXT, wall_clock INTEGER NOT NULL);
                 CREATE TABLE IF NOT EXISTS contact (id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                     phones TEXT NOT NULL DEFAULT '[]', emails TEXT NOT NULL DEFAULT '[]');
                 CREATE TABLE IF NOT EXISTS person (id INTEGER PRIMARY KEY,
                     source INTEGER NOT NULL REFERENCES source(id) ON DELETE CASCADE, key TEXT NOT NULL,
                     phone TEXT, phone_norm TEXT, username TEXT, email TEXT, name TEXT, me INTEGER NOT NULL,
                     contact INTEGER REFERENCES contact(id) ON DELETE SET NULL, UNIQUE (source, key));
                 CREATE INDEX IF NOT EXISTS person_phone ON person(phone_norm);
                 CREATE INDEX IF NOT EXISTS person_contact ON person(contact);
                 CREATE TABLE IF NOT EXISTS conv (id INTEGER PRIMARY KEY,
                     source INTEGER NOT NULL REFERENCES source(id) ON DELETE CASCADE, key TEXT NOT NULL,
                     kind TEXT NOT NULL, name TEXT, created_ms INTEGER, UNIQUE (source, key));
                 CREATE TABLE IF NOT EXISTS member (conv INTEGER NOT NULL REFERENCES conv(id) ON DELETE CASCADE,
                     person INTEGER NOT NULL REFERENCES person(id) ON DELETE CASCADE, PRIMARY KEY (conv, person));
                 CREATE TABLE IF NOT EXISTS msg (id INTEGER PRIMARY KEY,
                     conv INTEGER NOT NULL REFERENCES conv(id) ON DELETE CASCADE, pos INTEGER NOT NULL,
                     key TEXT NOT NULL, sender INTEGER REFERENCES person(id) ON DELETE SET NULL,
                     from_me INTEGER NOT NULL, ts INTEGER, kind TEXT NOT NULL, text TEXT, reply_key TEXT,
                     bytes INTEGER NOT NULL, voice INTEGER NOT NULL, month TEXT, hour INTEGER, dow INTEGER,
                     body TEXT NOT NULL);
                 CREATE INDEX IF NOT EXISTS msg_conv_pos ON msg(conv, pos);
                 CREATE INDEX IF NOT EXISTS msg_conv_key ON msg(conv, key);
                 CREATE INDEX IF NOT EXISTS msg_ts ON msg(ts);
                 CREATE INDEX IF NOT EXISTS msg_time ON msg((ts IS NULL), ts, conv, pos);
                 CREATE INDEX IF NOT EXISTS msg_newest ON msg((ts IS NULL), ts DESC, id DESC);
                 CREATE INDEX IF NOT EXISTS msg_sender ON msg(sender);
                 CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5(text, content = 'msg', content_rowid = 'id',
                     tokenize = 'unicode61 remove_diacritics 2');
                 CREATE TRIGGER IF NOT EXISTS msg_ai AFTER INSERT ON msg WHEN new.text IS NOT NULL BEGIN
                     INSERT INTO fts(rowid, text) VALUES (new.id, new.text); END;
                 CREATE TRIGGER IF NOT EXISTS msg_ad AFTER DELETE ON msg WHEN old.text IS NOT NULL BEGIN
                     INSERT INTO fts(fts, rowid, text) VALUES ('delete', old.id, old.text); END;
                 CREATE TABLE IF NOT EXISTS selection (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE,
                     json TEXT NOT NULL, saved_ms INTEGER NOT NULL);",
            )?;
            db.pragma_update(None, "user_version", SCHEMA)?;
        }
        Ok(Self { db, dir: dir.to_path_buf() })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Detects the adapter (or takes the named one), opens `path` with `secret`, and imports it.
    pub fn import_path(
        &mut self,
        adapters: &[&dyn Adapter],
        path: &Path,
        adapter: Option<&str>,
        secret: Option<&str>,
        progress: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<SourceRow> {
        let adapter = pick_adapter(adapters, path, adapter)?;
        let print = fingerprint(path)?;
        if let Some(id) = self.source_by_fingerprint(&print)? {
            return Err(Error::new(
                "exists",
                format!("{} is already in the library (source {id})", display_name(path)),
            ));
        }
        let mut source = adapter.open(path, secret)?;
        self.import(source.as_mut(), &display_name(path), &print, progress)
    }

    fn source_by_fingerprint(&self, print: &str) -> Result<Option<i64>> {
        Ok(self.db.query_row("SELECT id FROM source WHERE fingerprint = ?1", [print], |r| r.get(0)).optional()?)
    }

    /// Imports an opened source in one transaction: a failure or a stop (`progress` returning false) leaves the
    /// library as it was. `progress(done, total)` counts conversations.
    pub fn import(
        &mut self,
        source: &mut dyn Source,
        name: &str,
        fingerprint: &str,
        progress: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<SourceRow> {
        if self.source_by_fingerprint(fingerprint)?.is_some() {
            return Err(Error::new("exists", format!("{name} is already in the library")));
        }
        // Files the source holds inline go to a staging folder first, renamed to `media/<id>` once the rows are in:
        // a failure or a stop leaves no files behind either.
        let media = self.dir.join("media");
        let staging = media.join(format!(".import-{}", std::process::id()));
        let _ = fs::remove_dir_all(&staging);
        let tx = self.db.transaction()?;
        let done = import_into(&tx, source, name, fingerprint, &media, &staging, progress).and_then(|id| {
            tx.commit()?;
            Ok(id)
        });
        let id = match done {
            Ok(id) => id,
            Err(e) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
        };
        if staging.exists() {
            let target = media.join(id.to_string());
            let _ = fs::remove_dir_all(&target);
            if let Err(e) = fs::rename(&staging, &target) {
                let _ = fs::remove_dir_all(&staging);
                let _ = self.remove_source(id);
                return Err(Error::new("io", format!("keeping the source's files: {e}")));
            }
        }
        self.source(id)
    }

    pub fn sources(&self) -> Result<Vec<SourceRow>> {
        let ids: Vec<i64> = self
            .db
            .prepare("SELECT id FROM source ORDER BY id")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        ids.into_iter().map(|id| self.source(id)).collect()
    }

    pub fn source(&self, id: i64) -> Result<SourceRow> {
        self.db
            .query_row(
                "SELECT s.id, s.app, s.adapter, s.name, s.format, s.added_ms, s.notes, s.wall_clock, s.media_root,
                     (SELECT count(*) FROM conv WHERE source = s.id),
                     (SELECT count(*) FROM person WHERE source = s.id AND me = 0),
                     (SELECT count(*) FROM msg JOIN conv c ON c.id = msg.conv WHERE c.source = s.id),
                     (SELECT min(ts) FROM msg JOIN conv c ON c.id = msg.conv WHERE c.source = s.id),
                     (SELECT max(ts) FROM msg JOIN conv c ON c.id = msg.conv WHERE c.source = s.id),
                     EXISTS (SELECT 1 FROM person WHERE source = s.id AND me = 1)
                 FROM source s WHERE s.id = ?1",
                [id],
                |r| {
                    let notes: String = r.get(6)?;
                    Ok(SourceRow {
                        id: r.get(0)?,
                        app: app_of(r.get(1)?)?,
                        adapter: r.get(2)?,
                        name: r.get(3)?,
                        format: r.get(4)?,
                        added_ms: r.get(5)?,
                        notes: serde_json::from_str(&notes).unwrap_or_default(),
                        wall_clock: r.get(7)?,
                        has_media: r.get::<_, Option<String>>(8)?.is_some(),
                        conversations: r.get(9)?,
                        people: r.get(10)?,
                        messages: r.get(11)?,
                        first_ms: r.get(12)?,
                        last_ms: r.get(13)?,
                        owner_known: r.get(14)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| Error::new("not_found", format!("no source {id}")))
    }

    /// The folder a source's media file paths are relative to.
    pub fn media_root(&self, source: i64) -> Result<Option<PathBuf>> {
        Ok(self
            .db
            .query_row("SELECT media_root FROM source WHERE id = ?1", [source], |r| r.get::<_, Option<String>>(0))
            .optional()?
            .flatten()
            .map(PathBuf::from))
    }

    /// Removes a source, its rows and its media folder; contacts left with nobody and no card data go too.
    pub fn remove_source(&mut self, id: i64) -> Result<()> {
        let tx = self.db.transaction()?;
        let n = tx.execute("DELETE FROM source WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Error::new("not_found", format!("no source {id}")));
        }
        tx.execute(
            "DELETE FROM contact WHERE phones = '[]' AND emails = '[]'
                 AND NOT EXISTS (SELECT 1 FROM person WHERE contact = contact.id)",
            [],
        )?;
        tx.commit()?;
        let media = self.dir.join("media").join(id.to_string());
        if media.exists() {
            fs::remove_dir_all(&media).map_err(|e| Error::new("io", format!("removing {}: {e}", media.display())))?;
        }
        Ok(())
    }

    /// Marks `person` as the owner of their source (text exports don't say who that is): their messages become
    /// "from me".
    pub fn set_me(&mut self, person: i64) -> Result<()> {
        let tx = self.db.transaction()?;
        let source: i64 = tx
            .query_row("SELECT source FROM person WHERE id = ?1", [person], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Error::new("not_found", format!("no person {person}")))?;
        tx.execute("UPDATE person SET me = (id = ?1) WHERE source = ?2", params![person, source])?;
        tx.execute(
            "UPDATE msg SET from_me = (sender = ?1)
             WHERE conv IN (SELECT id FROM conv WHERE source = ?2) AND sender IS NOT NULL",
            params![person, source],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Conversations with at least one message matching `filter` (all of them with an empty filter), the latest
    /// first; their counts are of every message.
    pub fn conversations(&self, filter: &Filter) -> Result<Vec<ConversationRow>> {
        let (cond, values) = narrow(filter);
        let only = if filter.is_empty() {
            String::new()
        } else {
            format!(
                "AND c.id IN (SELECT DISTINCT m.conv FROM msg m JOIN conv c ON c.id = m.conv
                     JOIN source s ON s.id = c.source WHERE {cond})"
            )
        };
        let sql = format!(
            "SELECT c.id, c.source, s.app, c.kind, c.name, s.wall_clock,
                 (SELECT count(*) FROM msg WHERE conv = c.id), (SELECT min(ts) FROM msg WHERE conv = c.id),
                 (SELECT max(ts) FROM msg WHERE conv = c.id),
                 (SELECT substr(text, 1, 80) FROM msg WHERE conv = c.id ORDER BY pos DESC LIMIT 1)
             FROM conv c JOIN source s ON s.id = c.source WHERE 1 {only}
             ORDER BY (SELECT max(ts) FROM msg WHERE conv = c.id) DESC NULLS LAST, c.id"
        );
        let mut stmt = self.db.prepare(&sql)?;
        let mut rows = stmt
            .query_map(params_from_iter(values), |r| {
                Ok(ConversationRow {
                    id: r.get(0)?,
                    source: r.get(1)?,
                    app: app_of(r.get(2)?)?,
                    kind: m::ConversationKind::from_code(&r.get::<_, String>(3)?),
                    name: r.get(4)?,
                    wall_clock: r.get(5)?,
                    members: Vec::new(),
                    messages: r.get(6)?,
                    first_ms: r.get(7)?,
                    last_ms: r.get(8)?,
                    preview: r.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut members = self.db.prepare_cached(
            "SELECT coalesce(ct.name, p.name, p.username, p.phone, p.key) FROM member mb JOIN person p ON p.id = mb.person
             LEFT JOIN contact ct ON ct.id = p.contact WHERE mb.conv = ?1 AND p.me = 0 ORDER BY p.id LIMIT 12",
        )?;
        for row in &mut rows {
            row.members = members.query_map([row.id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        }
        Ok(rows)
    }

    pub fn conversation_len(&self, conversation: i64) -> Result<u64> {
        Ok(self.db.query_row("SELECT count(*) FROM msg WHERE conv = ?1", [conversation], |r| r.get(0))?)
    }

    /// `count` messages of a conversation from position `from`, as the source orders them.
    pub fn messages(&self, conversation: i64, from: u64, count: u32) -> Result<Vec<MessageRow>> {
        self.rows("m.conv = ?1 AND m.pos >= ?2 ORDER BY m.pos LIMIT ?3", params![conversation, from, count])
    }

    /// The position of the first message at or after `ts` in a conversation (the last one's when all are older).
    pub fn position_at(&self, conversation: i64, ts: i64) -> Result<Option<u64>> {
        Ok(self
            .db
            .query_row(
                "SELECT coalesce((SELECT min(pos) FROM msg WHERE conv = ?1 AND ts >= ?2),
                     (SELECT max(pos) FROM msg WHERE conv = ?1))",
                params![conversation, ts],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    /// Where a message sits: (conversation, position).
    pub fn position(&self, message: i64) -> Result<Option<(i64, u64)>> {
        Ok(self
            .db
            .query_row("SELECT conv, pos FROM msg WHERE id = ?1", [message], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    fn rows(&self, tail: &str, values: impl rusqlite::Params) -> Result<Vec<MessageRow>> {
        let sql = format!(
            "SELECT m.id, m.conv, c.source, s.app, m.pos, m.ts, s.wall_clock, m.from_me, m.sender, {NAME_OF_SENDER},
                 m.kind, (SELECT r.id FROM msg r WHERE r.conv = m.conv AND r.key = m.reply_key), m.body
             FROM msg m JOIN conv c ON c.id = m.conv JOIN source s ON s.id = c.source WHERE {tail}"
        );
        let mut stmt = self.db.prepare_cached(&sql)?;
        let rows = stmt.query_map(values, row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Messages matching `filter`, in `order`, from `offset`; at most `limit`.
    pub fn find(&self, filter: &Filter, order: Order, offset: u64, limit: u32) -> Result<Vec<MessageRow>> {
        let (cond, mut values) = narrow(filter);
        values.push(Value::Integer(limit.into()));
        values.push(Value::Integer(offset as i64));
        // The page's ids first, then their rows: the rows' columns (body, names, replies) are built for the page
        // only, never for the rows skipped or sorted.
        let order = order_by(order);
        self.rows(
            &format!(
                "m.id IN (SELECT m.id FROM msg m JOIN conv c ON c.id = m.conv JOIN source s ON s.id = c.source
                     WHERE {cond} ORDER BY {order} LIMIT ? OFFSET ?) ORDER BY {order}"
            ),
            params_from_iter(values),
        )
    }

    /// How many messages match `filter`.
    pub fn count(&self, filter: &Filter) -> Result<u64> {
        let (cond, values) = narrow(filter);
        Ok(self.db.query_row(
            &format!(
                "SELECT count(*) FROM msg m JOIN conv c ON c.id = m.conv JOIN source s ON s.id = c.source WHERE {cond}"
            ),
            params_from_iter(values),
            |r| r.get(0),
        )?)
    }

    /// Search results, newest first, with the matching words marked; with no words, the newest messages that match
    /// the rest of the filter (nothing when the filter is empty).
    pub fn search(&self, filter: &Filter, limit: u32) -> Result<Vec<Hit>> {
        if filter.is_empty() {
            return Ok(Vec::new());
        }
        let (cond, mut values) = narrow(filter);
        let fts = (!has_cjk(&filter.words)).then(|| fts_query(&filter.words)).flatten();
        // Words in a large share of the messages: walking the newest-first index and keeping the matches fills the
        // first page within a few hundred rows; reading every match's row to sort them would take seconds.
        let mut walk = "";
        if let Some(q) = &fts {
            let matches: i64 = self.db.query_row("SELECT count(*) FROM fts WHERE fts MATCH ?1", [q], |r| r.get(0))?;
            if matches > 20_000 {
                walk = "INDEXED BY msg_newest";
            }
        }
        values.push(Value::Integer(limit.into()));
        // The hits' ids first (an index walk, newest first), then names for those only; snippets after, in one pass
        // over the word index (asking it once per hit costs a pass each).
        let newest = order_by(Order::Newest);
        let ids: Vec<i64> = self
            .db
            .prepare_cached(&format!(
                "SELECT m.id FROM msg m {walk} JOIN conv c ON c.id = m.conv JOIN source s ON s.id = c.source
                 WHERE {cond} ORDER BY {newest} LIMIT ?"
            ))?
            .query_map(params_from_iter(values), |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let list = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT m.id, m.conv, s.app, m.ts, m.from_me, {NAME_OF_SENDER}, substr(m.text, 1, 120)
             FROM msg m JOIN conv c ON c.id = m.conv JOIN source s ON s.id = c.source
             WHERE m.id IN ({list}) ORDER BY {newest}"
        );
        let mut stmt = self.db.prepare(&sql)?;
        let mut hits = stmt
            .query_map([], |r| {
                Ok(Hit {
                    id: r.get(0)?,
                    conversation: r.get(1)?,
                    app: app_of(r.get(2)?)?,
                    time_ms: r.get(3)?,
                    from_me: r.get(4)?,
                    sender_name: r.get(5)?,
                    snippet: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if let (Some(q), false) = (fts, hits.is_empty()) {
            // The hits' texts in a small word index of their own (in memory): marking them in the library's index
            // would read every match's entry to reach these few.
            self.db.execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS temp.hit USING fts5(text, tokenize = 'unicode61 remove_diacritics 2');
                 DELETE FROM temp.hit;",
            )?;
            self.db.execute(
                &format!("INSERT INTO temp.hit (rowid, text) SELECT id, text FROM msg WHERE id IN ({list})"),
                [],
            )?;
            let mut stmt = self
                .db
                .prepare("SELECT rowid, snippet(hit, 0, char(2), char(3), '…', 12) FROM temp.hit WHERE hit MATCH ?1")?;
            let snippets: HashMap<i64, String> =
                stmt.query_map([q], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            for h in &mut hits {
                if let Some(s) = snippets.get(&h.id) {
                    h.snippet = s.clone();
                }
            }
        }
        Ok(hits)
    }

    /// The ids a selection holds: its filter's messages, plus those added, minus those taken out; in `order`.
    pub fn selected(&self, selection: &Selection, order: Order) -> Result<Vec<i64>> {
        let (cond, mut values) = narrow(&selection.filter);
        let add = selection.add.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let remove = selection.remove.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let base = if selection.filter.is_empty() { "0".to_owned() } else { format!("({cond})") };
        if selection.filter.is_empty() {
            values.clear();
        }
        let sql = format!(
            "SELECT m.id FROM msg m JOIN conv c ON c.id = m.conv JOIN source s ON s.id = c.source
             WHERE ({base} OR m.id IN ({add})) AND m.id NOT IN ({remove}) ORDER BY {}",
            order_by(order)
        );
        let mut stmt = self.db.prepare(&sql)?;
        let ids = stmt.query_map(params_from_iter(values), |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// The messages with these ids, in the order given.
    pub fn messages_by_id(&self, ids: &[i64]) -> Result<Vec<MessageRow>> {
        let mut out = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(",");
            let mut rows = self.rows(&format!("m.id IN ({marks})"), params_from_iter(chunk.iter()))?;
            let at: HashMap<i64, usize> = chunk.iter().enumerate().map(|(i, &id)| (id, i)).collect();
            rows.sort_by_key(|r| at.get(&r.id).copied().unwrap_or(usize::MAX));
            out.extend(rows);
        }
        Ok(out)
    }

    pub fn save_selection(&mut self, name: &str, selection: &Selection) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::new("library", "a selection needs a name"));
        }
        self.db.execute(
            "INSERT INTO selection (name, json, saved_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT (name) DO UPDATE SET json = excluded.json, saved_ms = excluded.saved_ms",
            params![name, serde_json::to_string(selection)?, now_ms()],
        )?;
        Ok(self.db.query_row("SELECT id FROM selection WHERE name = ?1", [name], |r| r.get(0))?)
    }

    pub fn selections(&self) -> Result<Vec<SavedSelection>> {
        let mut stmt = self.db.prepare("SELECT id, name, json, saved_ms FROM selection ORDER BY name")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(id, name, json, saved_ms)| {
                Ok(SavedSelection { id, name, selection: serde_json::from_str(&json)?, saved_ms })
            })
            .collect()
    }

    pub fn delete_selection(&mut self, id: i64) -> Result<()> {
        self.db.execute("DELETE FROM selection WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Counts for the messages matching `filter` (hours and days in the computer's time zone, or as written for a
    /// source whose times are the phone's wall clock).
    pub fn stats(&self, filter: &Filter) -> Result<Stats> {
        let (cond, values) = narrow(filter);
        let from = "FROM msg m JOIN conv c ON c.id = m.conv JOIN source s ON s.id = c.source";
        let mut s = Stats { by_hour: vec![0; 24], by_day_hour: vec![vec![0; 24]; 7], ..Default::default() };
        // One pass over the messages, grouped finely, summed here: each further pass would read every row again.
        let mut stmt = self.db.prepare(&format!(
            "SELECT m.month, m.dow, m.hour, m.kind, s.app, m.sender, count(*), coalesce(sum(m.from_me), 0),
                 coalesce(sum(m.bytes), 0), coalesce(sum(m.voice), 0), min(m.ts), max(m.ts)
             {from} WHERE {cond} GROUP BY m.month, m.dow, m.hour, m.kind, s.app, m.sender"
        ))?;
        let mut months: BTreeMap<String, u64> = BTreeMap::new();
        let mut kinds: HashMap<String, (u64, u64)> = HashMap::new();
        let mut apps: HashMap<String, u64> = HashMap::new();
        let mut senders: HashMap<Option<i64>, u64> = HashMap::new();
        let mut rows = stmt.query(params_from_iter(values.iter()))?;
        while let Some(r) = rows.next()? {
            let n: u64 = r.get(6)?;
            let bytes: u64 = r.get(8)?;
            s.messages += n;
            s.from_me += r.get::<_, u64>(7)?;
            s.media_bytes += bytes;
            s.voice_secs += r.get::<_, u64>(9)?;
            if let Some(t) = r.get::<_, Option<i64>>(10)? {
                s.first_ms = Some(s.first_ms.map_or(t, |f| f.min(t)));
            }
            if let Some(t) = r.get::<_, Option<i64>>(11)? {
                s.last_ms = Some(s.last_ms.map_or(t, |l| l.max(t)));
            }
            if let Some(month) = r.get::<_, Option<String>>(0)? {
                *months.entry(month).or_default() += n;
            }
            if let (Some(dow), Some(hour)) = (r.get::<_, Option<usize>>(1)?, r.get::<_, Option<usize>>(2)?)
                && let (Some(day), true) = (s.by_day_hour.get_mut(dow), hour < 24)
            {
                day[hour] += n;
                s.by_hour[hour] += n;
            }
            let k = kinds.entry(r.get(3)?).or_default();
            k.0 += n;
            k.1 += bytes;
            *apps.entry(r.get(4)?).or_default() += n;
            *senders.entry(r.get(5)?).or_default() += n;
        }
        s.by_month = months.into_iter().collect();
        let mut by_kind: Vec<(Kind, u64, u64)> = kinds.into_iter().map(|(k, (n, b))| (kind_of(&k), n, b)).collect();
        by_kind.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        s.by_kind = by_kind;
        let mut by_app: Vec<(App, u64)> =
            apps.into_iter().map(|(a, n)| Ok((app_of(a)?, n))).collect::<rusqlite::Result<_>>()?;
        by_app.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        s.by_app = by_app;
        let mut top: Vec<(Option<i64>, u64)> = senders.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        top.truncate(50);
        let mut name = self.db.prepare(&format!("SELECT {NAME_OF_SENDER} FROM (SELECT ?1 AS sender) m"))?;
        for (sender, n) in top {
            let who = match sender {
                Some(_) => name.query_row([sender], |r| r.get::<_, Option<String>>(0))?,
                None => None,
            };
            s.by_person.push((sender, who, n));
        }
        Ok(s)
    }

    /// People per app (in `source`, or every source), the most messages first.
    pub fn people(&self, source: Option<i64>) -> Result<Vec<PersonRow>> {
        let mut stmt = self.db.prepare(
            "SELECT p.id, p.source, s.app, p.key, p.name, p.phone, p.username, p.email, p.me, p.contact,
                 (SELECT count(*) FROM msg WHERE sender = p.id) AS n
             FROM person p JOIN source s ON s.id = p.source WHERE ?1 IS NULL OR p.source = ?1
             ORDER BY p.me DESC, n DESC, p.id",
        )?;
        let rows = stmt
            .query_map([source], |r| {
                Ok(PersonRow {
                    id: r.get(0)?,
                    source: r.get(1)?,
                    app: app_of(r.get(2)?)?,
                    key: r.get(3)?,
                    name: r.get(4)?,
                    phone: r.get(5)?,
                    username: r.get(6)?,
                    email: r.get(7)?,
                    me: r.get(8)?,
                    contact: r.get(9)?,
                    messages: r.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn contacts(&self) -> Result<Vec<ContactRow>> {
        let mut stmt =
            self.db.prepare("SELECT id, name, phones, emails FROM contact ORDER BY name COLLATE NOCASE, id")?;
        let mut rows = stmt
            .query_map([], |r| {
                let (phones, emails): (String, String) = (r.get(2)?, r.get(3)?);
                Ok(ContactRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    phones: serde_json::from_str(&phones).unwrap_or_default(),
                    emails: serde_json::from_str(&emails).unwrap_or_default(),
                    people: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut people = self.db.prepare_cached(
            "SELECT p.id, s.app, p.phone, p.email FROM person p JOIN source s ON s.id = p.source WHERE p.contact = ?1
             ORDER BY p.id",
        )?;
        for c in &mut rows {
            for row in people.query_map([c.id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    app_of(r.get(1)?)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })? {
                let (id, app, phone, email) = row?;
                c.people.push((id, app));
                if let Some(p) = phone.filter(|p| !c.phones.contains(p)) {
                    c.phones.push(p);
                }
                if let Some(e) = email.filter(|e| !c.emails.iter().any(|x| x.eq_ignore_ascii_case(e))) {
                    c.emails.push(e);
                }
            }
        }
        Ok(rows)
    }

    /// Links people into one contact: the contact one of them is already in (the oldest), or a new one named
    /// `name` (else the first one's name). Returns the contact's id.
    pub fn link(&mut self, people: &[i64], name: Option<&str>) -> Result<i64> {
        if people.is_empty() {
            return Err(Error::new("library", "nobody to link"));
        }
        let tx = self.db.transaction()?;
        let marks = vec!["?"; people.len()].join(",");
        let found: i64 = tx.query_row(
            &format!("SELECT count(*) FROM person WHERE id IN ({marks})"),
            params_from_iter(people.iter()),
            |r| r.get(0),
        )?;
        if found as usize != people.len() {
            return Err(Error::new("not_found", "someone to link isn't in the library"));
        }
        let existing: Option<i64> = tx.query_row(
            &format!("SELECT min(contact) FROM person WHERE id IN ({marks})"),
            params_from_iter(people.iter()),
            |r| r.get(0),
        )?;
        let contact = match existing {
            Some(c) => c,
            None => {
                let fallback: String = tx.query_row(
                    "SELECT coalesce(name, username, phone, email, key) FROM person WHERE id = ?1",
                    [people[0]],
                    |r| r.get(0),
                )?;
                let name = name.map(str::trim).filter(|n| !n.is_empty()).map_or(fallback, str::to_owned);
                tx.execute("INSERT INTO contact (name) VALUES (?1)", [name])?;
                tx.last_insert_rowid()
            }
        };
        // Everyone already linked to the contacts being merged comes along.
        tx.execute(
            &format!(
                "UPDATE person SET contact = ?1 WHERE id IN ({marks})
                    OR contact IN (SELECT contact FROM person WHERE id IN ({marks}) AND contact IS NOT NULL)"
            ),
            params_from_iter(std::iter::once(contact).chain(people.iter().copied()).chain(people.iter().copied())),
        )?;
        if let (Some(name), Some(_)) = (name.map(str::trim).filter(|n| !n.is_empty()), existing) {
            tx.execute("UPDATE contact SET name = ?1 WHERE id = ?2", params![name, contact])?;
        }
        tx.execute(
            "DELETE FROM contact WHERE id != ?1 AND phones = '[]' AND emails = '[]'
                 AND NOT EXISTS (SELECT 1 FROM person WHERE contact = contact.id)",
            [contact],
        )?;
        tx.commit()?;
        Ok(contact)
    }

    /// Takes someone out of their contact.
    pub fn unlink(&mut self, person: i64) -> Result<()> {
        self.db.execute("UPDATE person SET contact = NULL WHERE id = ?1", [person])?;
        Ok(())
    }

    pub fn rename_contact(&mut self, contact: i64, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::new("library", "a contact needs a name"));
        }
        if self.db.execute("UPDATE contact SET name = ?1 WHERE id = ?2", params![name, contact])? == 0 {
            return Err(Error::new("not_found", format!("no contact {contact}")));
        }
        Ok(())
    }

    /// Deletes a contact; its people stay, unlinked.
    pub fn delete_contact(&mut self, contact: i64) -> Result<()> {
        self.db.execute("DELETE FROM contact WHERE id = ?1", [contact])?;
        Ok(())
    }

    /// Contacts from a vCard file: each card a new contact with its numbers and emails (none linked: the
    /// suggestions show who matches). Returns how many were added.
    pub fn import_vcards(&mut self, text: &str) -> Result<usize> {
        let cards = vcard::parse(text);
        let tx = self.db.transaction()?;
        for card in &cards {
            tx.execute(
                "INSERT INTO contact (name, phones, emails) VALUES (?1, ?2, ?3)",
                params![card.name, serde_json::to_string(&card.phones)?, serde_json::to_string(&card.emails)?],
            )?;
        }
        tx.commit()?;
        Ok(cards.len())
    }

    /// People who look like the same person: the same number or email in different sources, or a vCard contact's
    /// number; skipped when they're already all in one contact.
    pub fn suggestions(&self) -> Result<Vec<Suggestion>> {
        let people = self.people(None)?;
        let contacts = self.contacts()?;
        let mut by: Matches<'_> = BTreeMap::new();
        for p in people.iter().filter(|p| !p.me) {
            if let Some(n) = p.phone.as_deref().and_then(m::normalize_phone) {
                by.entry(("phone", n)).or_default().0.push(p);
            }
            if let Some(e) = p.email.as_deref().map(|e| e.trim().to_lowercase()).filter(|e| e.contains('@')) {
                by.entry(("email", e)).or_default().0.push(p);
            }
        }
        for c in &contacts {
            for n in c.phones.iter().filter_map(|p| m::normalize_phone(p)) {
                if let Some(entry) = by.get_mut(&("phone", n)) {
                    entry.1.get_or_insert(c.id);
                }
            }
            for e in &c.emails {
                if let Some(entry) = by.get_mut(&("email", e.trim().to_lowercase())) {
                    entry.1.get_or_insert(c.id);
                }
            }
        }
        let mut out = Vec::new();
        for ((reason, value), (group, card)) in by {
            let sources: std::collections::BTreeSet<i64> = group.iter().map(|p| p.source).collect();
            let contacts_of: std::collections::BTreeSet<Option<i64>> = group.iter().map(|p| p.contact).collect();
            let linked_already = contacts_of.len() == 1
                && contacts_of.iter().next().copied().flatten().is_some()
                && (card.is_none() || contacts_of.contains(&card));
            if linked_already || (sources.len() < 2 && card.is_none()) {
                continue;
            }
            let contact = group.iter().find_map(|p| p.contact).or(card);
            let name = contact
                .and_then(|c| contacts.iter().find(|x| x.id == c).map(|x| x.name.clone()))
                .or_else(|| group.iter().find_map(|p| p.name.clone()));
            out.push(Suggestion { reason, value, people: group.iter().map(|p| p.id).collect(), contact, name });
        }
        Ok(out)
    }

    /// Links a suggestion's people into its contact (a vCard contact when there is one).
    pub fn accept(&mut self, suggestion: &Suggestion) -> Result<i64> {
        let contact = self.link(&suggestion.people, suggestion.name.as_deref())?;
        if let Some(card) = suggestion.contact.filter(|&c| c != contact) {
            // A vCard contact absorbs the people's contact: its card data is the richer.
            let tx = self.db.transaction()?;
            tx.execute("UPDATE person SET contact = ?1 WHERE contact = ?2", params![card, contact])?;
            tx.execute("DELETE FROM contact WHERE id = ?1", [contact])?;
            tx.commit()?;
            return Ok(card);
        }
        Ok(contact)
    }
}

fn order_by(order: Order) -> &'static str {
    match order {
        Order::Conversation => "m.conv, m.pos",
        // `(m.ts IS NULL)` first is "nulls last", written so the msg_time and msg_newest indexes serve it.
        Order::Time => "(m.ts IS NULL), m.ts, m.conv, m.pos",
        Order::Newest => "(m.ts IS NULL), m.ts DESC, m.id DESC",
    }
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MessageRow> {
    let body: String = r.get(12)?;
    let message: m::Message = serde_json::from_str(&body)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(12, rusqlite::types::Type::Text, Box::new(e)))?;
    Ok(MessageRow {
        id: r.get(0)?,
        conversation: r.get(1)?,
        source: r.get(2)?,
        app: app_of(r.get(3)?)?,
        pos: r.get(4)?,
        time_ms: r.get(5)?,
        wall_clock: r.get(6)?,
        from_me: r.get(7)?,
        sender: r.get(8)?,
        sender_name: r.get(9)?,
        kind: kind_of(&r.get::<_, String>(10)?),
        reply: r.get(11)?,
        message,
    })
}

fn import_into(
    tx: &Transaction<'_>,
    source: &mut dyn Source,
    name: &str,
    fingerprint: &str,
    media: &Path,
    staging: &Path,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<i64> {
    let info = source.info().clone();
    let mut inline_files = 0u64;
    tx.execute(
        "INSERT INTO source (app, adapter, name, fingerprint, format, added_ms, notes, media_root, wall_clock)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            info.app.code(),
            info.adapter,
            name,
            fingerprint,
            info.format,
            now_ms(),
            serde_json::to_string(&info.notes)?,
            info.media_root.as_ref().map(|p| p.to_string_lossy().into_owned()),
            info.wall_clock
        ],
    )?;
    let source_id = tx.last_insert_rowid();
    let mut people: HashMap<String, i64> = HashMap::new();
    {
        let mut insert = tx.prepare(
            "INSERT INTO person (source, key, phone, phone_norm, username, email, name, me)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT (source, key) DO NOTHING",
        )?;
        let owner = info.owner.iter();
        for p in source.people().iter().chain(owner) {
            if people.contains_key(&p.id) {
                continue;
            }
            insert.execute(params![
                source_id,
                p.id,
                p.phone,
                p.phone.as_deref().and_then(m::normalize_phone),
                p.username,
                p.email,
                p.name,
                p.me
            ])?;
            let id =
                tx.query_row("SELECT id FROM person WHERE source = ?1 AND key = ?2", params![source_id, p.id], |r| {
                    r.get(0)
                })?;
            people.insert(p.id.clone(), id);
        }
    }
    let conversations = source.conversations().to_vec();
    let total = conversations.len() as u64;
    let mut insert_conv =
        tx.prepare("INSERT INTO conv (source, key, kind, name, created_ms) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    let mut insert_member = tx.prepare("INSERT OR IGNORE INTO member (conv, person) VALUES (?1, ?2)")?;
    let mut insert_person = tx.prepare("INSERT INTO person (source, key, me) VALUES (?1, ?2, 0)")?;
    let mut insert_msg = tx.prepare(
        "INSERT INTO msg (conv, pos, key, sender, from_me, ts, kind, text, reply_key, bytes, voice, month, hour, dow, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
             CASE WHEN ?6 IS NULL THEN NULL WHEN ?12 THEN strftime('%Y-%m', ?6 / 1000, 'unixepoch')
                  ELSE strftime('%Y-%m', ?6 / 1000, 'unixepoch', 'localtime') END,
             CASE WHEN ?6 IS NULL THEN NULL WHEN ?12 THEN CAST(strftime('%H', ?6 / 1000, 'unixepoch') AS INTEGER)
                  ELSE CAST(strftime('%H', ?6 / 1000, 'unixepoch', 'localtime') AS INTEGER) END,
             CASE WHEN ?6 IS NULL THEN NULL WHEN ?12 THEN CAST(strftime('%w', ?6 / 1000, 'unixepoch') AS INTEGER)
                  ELSE CAST(strftime('%w', ?6 / 1000, 'unixepoch', 'localtime') AS INTEGER) END,
             ?13)",
    )?;
    for (done, conv) in conversations.iter().enumerate() {
        if !progress(done as u64, total) {
            return Err(Error::new("cancelled", "stopped"));
        }
        insert_conv.execute(params![source_id, conv.id, conv.kind.code(), conv.name, conv.created_ms])?;
        let conv_id = tx.last_insert_rowid();
        let mut person_id = |key: &str, people: &mut HashMap<String, i64>| -> Result<i64> {
            if let Some(&id) = people.get(key) {
                return Ok(id);
            }
            // Someone the source names in a message but not among its people (a sender who left, a deleted
            // account): kept, with their id as their name.
            insert_person.execute(params![source_id, key])?;
            let id = tx.last_insert_rowid();
            people.insert(key.to_owned(), id);
            Ok(id)
        };
        for member in &conv.members {
            let id = person_id(member, &mut people)?;
            insert_member.execute(params![conv_id, id])?;
        }
        let mut pos = 0u64;
        let mut failed: Option<Error> = None;
        let streamed = source.messages(&conv.id, &mut |mut msg| {
            let step = (|| -> Result<()> {
                if info.media_root.is_none() {
                    for part in &mut msg.parts {
                        if let m::Part::Media(media_part) = part {
                            keep_inline(media_part, staging, &mut inline_files)?;
                        }
                    }
                }
                let sender = match &msg.sender {
                    Some(key) => Some(person_id(key, &mut people)?),
                    None => None,
                };
                if let (Some(id), m::ConversationKind::Group | m::ConversationKind::Channel) = (sender, conv.kind) {
                    insert_member.execute(params![conv_id, id])?;
                }
                let text = msg.text();
                let (bytes, voice) = media_facts(&msg);
                insert_msg.execute(params![
                    conv_id,
                    pos,
                    msg.id,
                    sender,
                    msg.from_me,
                    msg.time_ms,
                    msg.kind().code(),
                    (!text.is_empty()).then_some(text),
                    msg.reply_to,
                    bytes,
                    voice,
                    info.wall_clock,
                    serde_json::to_string(&msg)?
                ])?;
                pos += 1;
                Ok(())
            })();
            match step {
                Ok(()) => !pos.is_multiple_of(512) || progress(done as u64, total),
                Err(e) => {
                    failed = Some(e);
                    false
                }
            }
        });
        if let Some(e) = failed {
            return Err(e);
        }
        match streamed {
            Ok(()) => {}
            Err(m::Error::Cancelled) => return Err(Error::new("cancelled", "stopped")),
            Err(e) => return Err(e.into()),
        }
    }
    // A source read as it streams learns some notes only at the end (what it couldn't read).
    tx.execute(
        "UPDATE source SET notes = ?1 WHERE id = ?2",
        params![serde_json::to_string(&source.info().notes)?, source_id],
    )?;
    if inline_files > 0 {
        let root = media.join(source_id.to_string());
        tx.execute(
            "UPDATE source SET media_root = ?1 WHERE id = ?2",
            params![root.to_string_lossy().into_owned(), source_id],
        )?;
    }
    progress(total, total);
    Ok(source_id)
}

/// Writes a media part's inline bytes into `staging` and points [`m::Media::file`] at the copy. The name is the part's
/// own (only letters, digits, `.`, `-`, `_`), after a counter so two parts never share a file.
fn keep_inline(media: &mut m::Media, staging: &Path, count: &mut u64) -> Result<()> {
    let Some(data) = media.data.take() else { return Ok(()) };
    if media.view_once || media.file.is_some() {
        return Ok(());
    }
    let own: String = media
        .name
        .as_deref()
        .and_then(|n| n.rsplit(['/', '\\']).next())
        .unwrap_or("")
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .take(80)
        .collect();
    let own = own.trim_start_matches('.');
    let own = if own.is_empty() { format!("{:?}", media.kind).to_lowercase() } else { own.to_owned() };
    *count += 1;
    let file = format!("{count:06}-{own}");
    fs::create_dir_all(staging).map_err(|e| Error::new("io", format!("a folder for the source's files: {e}")))?;
    // Room for the file and 256 MB to spare, so the library never fills the disk.
    let free = fs4::available_space(staging).map_err(|e| Error::new("io", format!("reading the free space: {e}")))?;
    if free < data.len() as u64 + (256 << 20) {
        return Err(Error::new("no_space", "not enough free space for the source's media files"));
    }
    fs::write(staging.join(&file), &data).map_err(|e| Error::new("io", format!("writing {file}: {e}")))?;
    media.file = Some(file);
    media.omitted = false;
    Ok(())
}

#[cfg(test)]
mod tests;
