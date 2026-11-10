//! oopz: a person's own WhatsApp backups (decision 0007). Opens a crypt15 backup with its owner's key, reads its
//! structure, and merges a chat back into another backup (decision 0006). Output is counts and masked ids only.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Component, Path};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

pub mod contacts;
pub mod crypt15;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixture;
pub mod merge;
pub mod schema;

pub use crypt15::Key;
pub use merge::{ChatMerge, MediaFile, MediaReport, MergeReport, Merged, copy_media};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Crypt15(#[from] crypt15::Error),
    #[error("unsupported WhatsApp database: {0}")]
    Unsupported(String),
    #[error("reading the WhatsApp database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("the merged backup failed a check, nothing was written: {0}")]
    MergeCheck(String),
    #[error("{context}: {source}")]
    Io { context: String, source: std::io::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

/// A decrypted backup, opened read-only from a private temporary copy that's removed when this is dropped. The
/// original file is only read.
pub struct Backup {
    db: Connection,
    schema: schema::Schema,
    plain: tempfile::NamedTempFile,
}

impl Backup {
    pub fn open(path: &Path, key: &Key) -> Result<Self> {
        Self::open_in(path, key, &std::env::temp_dir())
    }

    /// Opens it with its decrypted copy in `dir` (an app's private cache: Android has no shared temporary folder).
    /// Memory holds the backup file once, never the whole database: it's decrypted in place and streamed to disk.
    pub fn open_in(path: &Path, key: &Key, dir: &Path) -> Result<Self> {
        Self::open_in_with(path, key, dir, &mut |_, _| {})
    }

    /// [`Backup::open_in`], calling `progress(done, total)` as it reads the file, decompresses it and indexes the copy
    /// for reading ([`index_for_reading`]): on a phone, a backup of a million messages takes a while, and the bar moves.
    pub fn open_in_with(path: &Path, key: &Key, dir: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<Self> {
        let io = |source| Error::Io { context: format!("reading {}", path.display()), source };
        // Reading, decompressing and indexing weigh about the same; each a third of the bar.
        let len = fs::metadata(path).map_err(io)?.len().max(1);
        let total = 3 * len;
        let mut file = Vec::with_capacity(len as usize);
        {
            let mut input = fs::File::open(path).map_err(io)?;
            let mut buf = vec![0u8; 4 << 20];
            loop {
                let n = std::io::Read::read(&mut input, &mut buf).map_err(io)?;
                if n == 0 {
                    break;
                }
                file.extend_from_slice(&buf[..n]);
                progress((file.len() as u64).min(len), total);
            }
        }
        let io = |source| Error::Io { context: "writing the decrypted database to a temporary file".into(), source };
        let mut plain = tempfile::NamedTempFile::new_in(dir).map_err(io)?;
        {
            let mut out = std::io::BufWriter::with_capacity(1 << 20, plain.as_file_mut());
            crypt15::decrypt_to_with(file, key, &mut out, &mut |done, of| {
                progress(len + done * len / of.max(1), total);
            })?;
            out.flush().map_err(io)?;
        }
        index_for_reading(plain.path(), &mut |done, of| progress(2 * len + done * len / of.max(1), total))?;
        let db = Connection::open_with_flags(plain.path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // Our reads go through a million rows and their linked tables: more pages kept than SQLite's 2 MB.
        db.pragma_update(None, "cache_size", -16_384)?;
        let schema = schema::detect(&db)?;
        Ok(Self { db, schema, plain })
    }

    /// Whether the database inside is sound (SQLite's `quick_check`) and how much it holds. Counts and a date only.
    pub fn health(&self) -> Result<Health> {
        let mut stmt = self.db.prepare("PRAGMA quick_check")?;
        let problems: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let (chats, messages, last_ms) = self.db.query_row(
            "SELECT (SELECT count(*) FROM chat), (SELECT count(*) FROM message), (SELECT max(timestamp) FROM message)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        Ok(Health { check: problems.join("; "), chats, messages, last_ms })
    }

    /// Every chat, most recent first.
    pub fn chats(&self) -> Result<Vec<Chat>> {
        let mut stmt = self.db.prepare(&format!(
            "SELECT c._id, j.raw_string, count(m._id), max(m.timestamp), {}
             FROM chat c JOIN jid j ON j._id = c.jid_row_id LEFT JOIN message m ON m.chat_row_id = c._id
             GROUP BY c._id ORDER BY max(m.timestamp) DESC",
            self.schema.chat_subject()
        ))?;
        let chats = stmt
            .query_map([], |r| {
                Ok(Chat {
                    row_id: r.get(0)?,
                    jid: r.get(1)?,
                    messages: r.get(2)?,
                    last_ms: r.get(3)?,
                    subject: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(chats)
    }

    /// WhatsApp's id and time (ms) of every message in a chat, oldest first. Ids, not content: they let two backups
    /// of the same chat be matched. A row with no time reads 0 (unknown).
    pub fn key_ids(&self, chat_row_id: i64) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.db.prepare(
            "SELECT key_id, coalesce(timestamp, 0) FROM message WHERE chat_row_id = ?1 AND key_id IS NOT NULL
             ORDER BY coalesce(timestamp, 0), _id",
        )?;
        let ids = stmt.query_map([chat_row_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// How WhatsApp keeps a chat's messages: ordering ids, senders, statuses, the chat row's pointers. Counts, ranges
    /// and column names only (never text, ids or numbers), to merge one backup's chat into another's safely. A fact
    /// whose table or column this backup lacks reads "n/a".
    pub fn chat_facts(&self, chat_row_id: i64) -> Result<Vec<(&'static str, String)>> {
        const FACTS: &[(&str, &str)] = &[
            ("messages", "SELECT count(*) FROM message WHERE chat_row_id = ?1"),
            ("from_me 0 / 1", "SELECT sum(from_me = 0) || ' / ' || sum(from_me = 1) FROM message WHERE chat_row_id = ?1"),
            ("_id range", "SELECT min(_id) || ' .. ' || max(_id) FROM message WHERE chat_row_id = ?1"),
            ("sort_id range", "SELECT min(sort_id) || ' .. ' || max(sort_id) FROM message WHERE chat_row_id = ?1"),
            ("sort_id = _id", "SELECT sum(sort_id = _id) FROM message WHERE chat_row_id = ?1"),
            ("sort_id null", "SELECT sum(sort_id IS NULL) FROM message WHERE chat_row_id = ?1"),
            (
                "sort_id order breaks time order",
                "SELECT count(*) FROM (SELECT timestamp, lag(timestamp) OVER (ORDER BY sort_id) AS prev
                 FROM message WHERE chat_row_id = ?1) WHERE timestamp < prev",
            ),
            (
                "_id order breaks time order",
                "SELECT count(*) FROM (SELECT timestamp, lag(timestamp) OVER (ORDER BY _id) AS prev
                 FROM message WHERE chat_row_id = ?1) WHERE timestamp < prev",
            ),
            ("all messages: _id max / sort_id max", "SELECT max(_id) || ' / ' || max(sort_id) FROM message"),
            (
                "sender_jid_row_id by from_me (0: null|0|other, 1: null|0|other)",
                "SELECT sum(from_me = 0 AND sender_jid_row_id IS NULL) || '|' || sum(from_me = 0 AND sender_jid_row_id = 0)
                 || '|' || sum(from_me = 0 AND sender_jid_row_id > 0) || ', ' || sum(from_me = 1 AND sender_jid_row_id IS NULL)
                 || '|' || sum(from_me = 1 AND sender_jid_row_id = 0) || '|' || sum(from_me = 1 AND sender_jid_row_id > 0)
                 FROM message WHERE chat_row_id = ?1",
            ),
            (
                "status by from_me (status:count)",
                "SELECT group_concat(f || ':' || status || ':' || n, ' ') FROM (SELECT from_me AS f, status, count(*) AS n
                 FROM message WHERE chat_row_id = ?1 GROUP BY 1, 2)",
            ),
            (
                "origin, broadcast, recipient_count values",
                "SELECT group_concat(DISTINCT origin) || ' | ' || group_concat(DISTINCT broadcast) || ' | '
                 || group_concat(DISTINCT recipient_count) FROM message WHERE chat_row_id = ?1",
            ),
            (
                "received_timestamp / receipt_server_timestamp set",
                "SELECT sum(received_timestamp > 0) || ' / ' || sum(receipt_server_timestamp > 0) FROM message
                 WHERE chat_row_id = ?1",
            ),
            (
                "key_id lengths (length:count)",
                "SELECT group_concat(l || ':' || n, ' ') FROM (SELECT length(key_id) AS l, count(*) AS n FROM message
                 WHERE chat_row_id = ?1 GROUP BY 1)",
            ),
            ("lookup_tables values", "SELECT group_concat(DISTINCT lookup_tables) FROM message WHERE chat_row_id = ?1"),
            (
                "message_details author_device_jid by from_me (0: null|set, 1: null|set)",
                "SELECT sum(m.from_me = 0 AND d.author_device_jid IS NULL) || '|' || sum(m.from_me = 0 AND d.author_device_jid IS NOT NULL)
                 || ', ' || sum(m.from_me = 1 AND d.author_device_jid IS NULL) || '|' || sum(m.from_me = 1 AND d.author_device_jid IS NOT NULL)
                 FROM message m JOIN message_details d ON d.message_row_id = m._id WHERE m.chat_row_id = ?1",
            ),
            (
                "message_secret rows",
                "SELECT count(*) FROM message_secret s JOIN message m ON m._id = s.message_row_id WHERE m.chat_row_id = ?1",
            ),
            (
                "media: has file_path / transferred / has media_key / has direct_path",
                "SELECT sum(mm.file_path IS NOT NULL) || ' / ' || sum(mm.transferred = 1) || ' / ' || sum(mm.media_key IS NOT NULL)
                 || ' / ' || sum(mm.direct_path IS NOT NULL) FROM message_media mm JOIN message m ON m._id = mm.message_row_id
                 WHERE m.chat_row_id = ?1",
            ),
            (
                "media file_path roots (first folder:count)",
                "SELECT group_concat(r || ':' || n, ' ') FROM (SELECT substr(mm.file_path, 1, instr(mm.file_path || '/', '/') - 1) AS r,
                 count(*) AS n FROM message_media mm JOIN message m ON m._id = mm.message_row_id WHERE m.chat_row_id = ?1
                 GROUP BY 1)",
            ),
            ("chat jid server", "SELECT j.server FROM chat c JOIN jid j ON j._id = c.jid_row_id WHERE c._id = ?1"),
            (
                "chat jid in jid_map (as lid / as phone)",
                "SELECT (SELECT count(*) FROM jid_map WHERE lid_row_id = c.jid_row_id) || ' / '
                 || (SELECT count(*) FROM jid_map WHERE jid_row_id = c.jid_row_id) FROM chat c WHERE c._id = ?1",
            ),
            (
                "chat columns pointing at this chat's messages",
                "SELECT group_concat(name, ' ') FROM pragma_table_info('chat') p WHERE p.name LIKE '%message_row_id'
                 AND (SELECT count(*) FROM chat c JOIN message m ON m._id = (SELECT CASE p.name
                   WHEN 'display_message_row_id' THEN c.display_message_row_id WHEN 'last_message_row_id' THEN c.last_message_row_id
                   WHEN 'last_read_message_row_id' THEN c.last_read_message_row_id
                   WHEN 'last_read_receipt_sent_message_row_id' THEN c.last_read_receipt_sent_message_row_id
                   WHEN 'last_important_message_row_id' THEN c.last_important_message_row_id END)
                 WHERE c._id = ?1) > 0",
            ),
            (
                "chat sort ids vs max sort_id (display/last/last_read)",
                "SELECT (c.display_message_sort_id = mx) || '/' || (c.last_message_sort_id = mx) || '/'
                 || (c.last_read_message_sort_id = mx) FROM chat c, (SELECT max(sort_id) AS mx FROM message WHERE chat_row_id = ?1)
                 WHERE c._id = ?1",
            ),
            ("chat created_timestamp vs first message", "SELECT (c.created_timestamp <= (SELECT min(timestamp) FROM message WHERE chat_row_id = ?1)) FROM chat c WHERE c._id = ?1"),
            ("reactions (add-ons) on this chat", "SELECT count(*) FROM message_add_on WHERE chat_row_id = ?1"),
            (
                // IN builds the chat's key_id set once; a correlated lookup per reply took hours on a million messages.
                "quoted rows / with parent in chat",
                "SELECT count(*) || ' / ' || sum(q.key_id IN (SELECT key_id FROM message WHERE chat_row_id = ?1))
                 FROM message_quoted q JOIN message m ON m._id = q.message_row_id WHERE m.chat_row_id = ?1",
            ),
            ("receipt_user rows", "SELECT count(*) FROM receipt_user r JOIN message m ON m._id = r.message_row_id WHERE m.chat_row_id = ?1"),
            ("props keys", "SELECT group_concat(key, ' ') FROM props"),
        ];
        let mut out = Vec::new();
        for (label, sql) in FACTS {
            // A missing table or column means this WhatsApp version doesn't have it: worth knowing, not an error.
            let read = |r: &rusqlite::Row<'_>| {
                r.get::<_, Option<String>>(0).or_else(|_| r.get::<_, Option<i64>>(0).map(|n| n.map(|n| n.to_string())))
            };
            // Facts about the whole database take no chat number.
            let value = if sql.contains("?1") {
                self.db.query_row(sql, [chat_row_id], read)
            } else {
                self.db.query_row(sql, [], read)
            };
            let value = value.map(|v| v.unwrap_or_else(|| "-".into())).unwrap_or_else(|_| "n/a".into());
            out.push((*label, value));
        }
        Ok(out)
    }

    /// For each `message_type` in a chat (-1: none recorded): how many messages, how many have text, and how many have a row in each
    /// table that points at messages, plus their media types. Structure and counts only, to map type codes without
    /// reading content.
    pub fn type_profile(&self, chat_row_id: i64) -> Result<Vec<TypeProfile>> {
        let mut profiles: BTreeMap<i64, TypeProfile> = BTreeMap::new();
        let mut stmt = self.db.prepare(
            "SELECT coalesce(message_type, -1), count(*), count(text_data) FROM message WHERE chat_row_id = ?1 GROUP BY 1",
        )?;
        let mut rows = stmt.query([chat_row_id])?;
        while let Some(r) = rows.next()? {
            let code = r.get(0)?;
            profiles
                .insert(code, TypeProfile { code, messages: r.get(1)?, with_text: r.get(2)?, ..Default::default() });
        }

        let mut stmt = self.db.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
        let tables = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for table in tables {
            let has_link: bool = self.db.query_row(
                "SELECT count(*) > 0 FROM pragma_table_info(?1) WHERE name = 'message_row_id'",
                [&table],
                |r| r.get(0),
            )?;
            if !has_link {
                continue;
            }
            let quoted = table.replace('"', "\"\"");
            let mut stmt = self.db.prepare(&format!(
                "SELECT coalesce(m.message_type, -1), count(DISTINCT m._id) FROM message m JOIN \"{quoted}\" x
                 ON x.message_row_id = m._id WHERE m.chat_row_id = ?1 GROUP BY 1"
            ))?;
            let mut rows = stmt.query([chat_row_id])?;
            while let Some(r) = rows.next()? {
                if let Some(p) = profiles.get_mut(&r.get(0)?) {
                    p.linked.push((table.clone(), r.get(1)?));
                }
            }
        }

        if self.db.query_row(
            "SELECT count(*) > 0 FROM pragma_table_info('message_media') WHERE name = 'mime_type'",
            [],
            |r| r.get::<_, bool>(0),
        )? {
            let mut stmt = self.db.prepare(
                "SELECT coalesce(m.message_type, -1), coalesce(mm.mime_type, '(none)'), count(*) FROM message m
                 JOIN message_media mm ON mm.message_row_id = m._id WHERE m.chat_row_id = ?1
                 GROUP BY 1, 2 ORDER BY 3 DESC",
            )?;
            let mut rows = stmt.query([chat_row_id])?;
            while let Some(r) = rows.next()? {
                if let Some(p) = profiles.get_mut(&r.get(0)?) {
                    p.mimes.push((r.get(1)?, r.get(2)?));
                }
            }
        }
        Ok(profiles.into_values().collect())
    }

    /// Every table with its columns and row count: the backup's structure, no values. For mapping new tables.
    pub fn tables(&self) -> Result<Vec<(String, Vec<String>, u64)>> {
        let mut stmt = self.db.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
        let names = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut out = Vec::new();
        for name in names {
            let mut stmt = self.db.prepare("SELECT name FROM pragma_table_info(?1)")?;
            let columns = stmt.query_map([&name], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            let quoted = name.replace('"', "\"\"");
            let rows = self.db.query_row(&format!("SELECT count(*) FROM \"{quoted}\""), [], |r| r.get(0))?;
            out.push((name, columns, rows));
        }
        Ok(out)
    }

    /// One chat, as [`Backup::chats`] has it: only its own messages counted (every chat's took a pass over them all,
    /// and an import asks often).
    pub fn chat(&self, row_id: i64) -> Result<Chat> {
        self.db
            .query_row(
                &format!(
                    "SELECT c._id, j.raw_string, (SELECT count(*) FROM message m WHERE m.chat_row_id = c._id),
                        (SELECT max(timestamp) FROM message m WHERE m.chat_row_id = c._id), {}
                     FROM chat c JOIN jid j ON j._id = c.jid_row_id WHERE c._id = ?1",
                    self.schema.chat_subject()
                ),
                [row_id],
                |r| {
                    Ok(Chat {
                        row_id: r.get(0)?,
                        jid: r.get(1)?,
                        messages: r.get(2)?,
                        last_ms: r.get(3)?,
                        subject: r.get(4)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| Error::Unsupported(format!("no chat number {row_id} in this backup")))
    }

    /// The decrypted database, read-only, for an importer that translates it (decision 0007).
    pub fn db(&self) -> &Connection {
        &self.db
    }

    /// The queries for this backup's schema.
    pub fn schema(&self) -> &schema::Schema {
        &self.schema
    }
}

/// The indexes an import's reads go by: (table, columns). WhatsApp's own database may lack some (a version's
/// schema, a table it never indexed), and each one lacking turns a read into a pass over a whole table per message: an
/// import of a million messages then took most of an hour.
const READ_INDEXES: &[(&str, &[&str])] = &[
    ("message", &["chat_row_id", "timestamp", "_id"]),
    ("message_media", &["message_row_id"]),
    ("message_quoted", &["message_row_id"]),
    ("message_call_log", &["message_row_id"]),
    ("message_forwarded", &["message_row_id"]),
    ("message_edit_info", &["message_row_id"]),
    ("message_association", &["child_message_row_id"]),
    ("message_text", &["message_row_id"]),
    ("message_vcard", &["message_row_id"]),
    ("message_location", &["message_row_id"]),
    ("message_poll", &["message_row_id"]),
    ("message_poll_option", &["message_row_id"]),
    ("message_add_on", &["parent_message_row_id"]),
    ("message_add_on_reaction", &["message_add_on_row_id"]),
    ("message_system", &["message_row_id"]),
];

/// Adds the [`READ_INDEXES`] the decrypted copy at `path` lacks (a copy of ours, removed after: the original is
/// never written). An index is lacking unless one starts with the same columns (or the column is the table's row
/// id). Seconds on a phone, once; `progress(done, total)` by index.
fn index_for_reading(path: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<()> {
    let db = Connection::open(path)?;
    // A copy we throw away: no journal to write, nothing to sync.
    db.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA cache_size = -16384;")?;
    normalize_types(&db)?;
    let total = READ_INDEXES.len() as u64;
    for (n, (table, columns)) in READ_INDEXES.iter().enumerate() {
        progress(n as u64, total);
        let present: Vec<(String, bool)> = db
            .prepare("SELECT name, pk = 1 AND upper(type) = 'INTEGER' FROM pragma_table_info(?1)")?
            .query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if !columns.iter().all(|c| present.iter().any(|(name, _)| name == c)) {
            continue;
        }
        let row_id = columns.len() == 1 && present.iter().any(|(name, pk)| *pk && name == columns[0]);
        let indexed = db
            .prepare(
                "SELECT group_concat(i.name, ',') FROM pragma_index_list(?1) l, pragma_index_info(l.name) i
                 GROUP BY l.name",
            )?
            .query_map([table], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .iter()
            .any(|cols| {
                let cols: Vec<&str> = cols.split(',').collect();
                // The message index serves the chat's order only with its time second.
                let wanted = if *table == "message" { &columns[..2] } else { &columns[..1] };
                cols.len() >= wanted.len() && cols[..wanted.len()] == *wanted
            });
        if !row_id && !indexed {
            db.execute_batch(&format!("CREATE INDEX hilo_read_{table} ON {table} ({})", columns.join(", ")))?;
        }
    }
    progress(total, total);
    Ok(())
}

/// Columns oopz reads as numbers, and as text, by table.
const TYPED: &[(&str, &[&str], &[&str])] = &[
    (
        "message",
        &["chat_row_id", "from_me", "timestamp", "message_type", "starred", "sender_jid_row_id", "sort_id"],
        &["key_id", "text_data"],
    ),
    ("message_media", &["file_size", "width", "height", "media_duration"], &["file_path", "mime_type", "media_name"]),
    ("message_add_on", &["chat_row_id", "from_me", "parent_message_row_id", "timestamp"], &["key_id"]),
    ("call_log", &["video_call", "duration"], &[]),
    ("chat", &["jid_row_id"], &["subject"]),
    ("jid", &[], &["raw_string"]),
];

/// A value of the wrong type in a column oopz reads (one damaged row, an odd WhatsApp version) would stop every read
/// of the backup. In this private copy (the original is never changed), a number column's odd value becomes a number
/// (text that isn't one becomes 0, which oopz reads as unknown for a time), and a text column's bytes become NULL.
/// One pass over each table.
fn normalize_types(db: &Connection) -> Result<()> {
    for (table, numbers, texts) in TYPED {
        let present: Vec<String> = db
            .prepare("SELECT name FROM pragma_table_info(?1)")?
            .query_map([table], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let numbers: Vec<&str> = numbers.iter().copied().filter(|c| present.iter().any(|p| p == c)).collect();
        let texts: Vec<&str> = texts.iter().copied().filter(|c| present.iter().any(|p| p == c)).collect();
        if numbers.is_empty() && texts.is_empty() {
            continue;
        }
        let set = numbers
            .iter()
            .map(|c| format!("\"{c}\" = CASE WHEN typeof(\"{c}\") IN ('integer', 'null') THEN \"{c}\" ELSE CAST(\"{c}\" AS INTEGER) END"))
            .chain(texts.iter().map(|c| format!("\"{c}\" = CASE WHEN typeof(\"{c}\") = 'blob' THEN NULL ELSE \"{c}\" END")))
            .collect::<Vec<_>>()
            .join(", ");
        let odd = numbers
            .iter()
            .map(|c| format!("typeof(\"{c}\") NOT IN ('integer', 'null')"))
            .chain(texts.iter().map(|c| format!("typeof(\"{c}\") = 'blob'")))
            .collect::<Vec<_>>()
            .join(" OR ");
        // OR IGNORE: a row the change would make clash with another (a unique index) is left as it was.
        db.execute(&format!("UPDATE OR IGNORE \"{table}\" SET {set} WHERE {odd}"), [])?;
    }
    Ok(())
}

/// See [`Backup::health`].
#[derive(Debug, PartialEq, Eq)]
pub struct Health {
    /// `ok`, or what SQLite found wrong (table and page names, never values).
    pub check: String,
    pub chats: u64,
    pub messages: u64,
    pub last_ms: Option<i64>,
}

/// See [`Backup::type_profile`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TypeProfile {
    pub code: i64,
    pub messages: u64,
    pub with_text: u64,
    /// (table, messages of this type with a row in it)
    pub linked: Vec<(String, u64)>,
    /// (media type, messages), most common first
    pub mimes: Vec<(String, u64)>,
}

/// A chat in a backup. Holds the full WhatsApp id (a phone number); only [`Chat::masked`] is for display.
pub struct Chat {
    pub row_id: i64,
    jid: String,
    pub messages: u64,
    pub last_ms: Option<i64>,
    /// A group's name as of the backup (never logged).
    pub subject: Option<String>,
}

impl Chat {
    /// The kind of chat and the last 3 characters of its id: enough to recognise, never the whole number.
    pub fn masked(&self) -> String {
        let (_, server) = self.jid.split_once('@').unwrap_or((&self.jid, ""));
        let kind = match server {
            "s.whatsapp.net" => "contact",
            // WhatsApp's private id for a contact; its digits aren't a phone number.
            "lid" => "contact (lid)",
            "g.us" => "group",
            "broadcast" => "broadcast",
            _ => "other",
        };
        format!("{kind} ••••{}", self.ending())
    }

    /// The last 3 characters of its id, for an app to word as [`Chat::masked`] does: never the whole number.
    pub fn ending(&self) -> String {
        let user = self.jid.split_once('@').map_or(self.jid.as_str(), |(user, _)| user);
        user.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect()
    }

    /// The full WhatsApp id, a phone number for a contact: for matching and origins only, never for display.
    pub fn jid(&self) -> &str {
        &self.jid
    }

    pub fn is_one_to_one(&self) -> bool {
        self.jid.ends_with("@s.whatsapp.net") || self.jid.ends_with("@lid")
    }

    pub fn is_group(&self) -> bool {
        self.jid.ends_with("@g.us")
    }
}

impl fmt::Debug for Chat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Chat({}, {})", self.row_id, self.masked())
    }
}

/// Whether `name` is one of WhatsApp's other encrypted backups (contacts, chat settings, stickers, wallpaper,
/// business data): they sit next to the message backups, end in `.crypt15` too, and hold no messages.
pub fn is_other_whatsapp_backup(name: &str) -> bool {
    const OTHERS: [&str; 5] = ["wa.db.", "chatsettingsbackup.", "commerce_backup.", "stickers.", "wallpapers."];
    let name = name.to_ascii_lowercase();
    OTHERS.iter().any(|o| name.starts_with(o))
}

/// A database `file_path` (relative `Media/…` or an old absolute path) relative to the `Media/` folder, or `None`
/// if it has no `Media/` or tries to leave it.
pub fn media_relative(file_path: &str) -> Option<&Path> {
    let (_, rel) = file_path.split_once("Media/")?;
    let rel = Path::new(rel);
    rel.components().all(|c| matches!(c, Component::Normal(_))).then_some(rel)
}

#[cfg(test)]
mod tests;
