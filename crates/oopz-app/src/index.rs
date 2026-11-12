//! A search index and stats over a checked backup (decision 0009): a private SQLite file next to the backup's
//! decrypted copy, removed with it, holding one row per message (time, side, type, text, media size) and an FTS5
//! full-text index that ignores accents (`canción` finds `cancion`). Built once per backup, in seconds; a search then
//! takes milliseconds. Message text is for the person's own screen only: never logged, never in an error.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::Result;

/// WhatsApp's message type for a poll.
const POLL: i64 = 46;

/// A search result: where the message is, and a short piece of its text around the match, with the matched words
/// between `\u{2}` and `\u{3}` (the UI turns them into highlights without parsing HTML).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Hit {
    pub id: i64,
    pub chat: i64,
    pub ts: i64,
    pub from_me: bool,
    pub snippet: String,
}

/// One message as the chat viewer shows it.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Line {
    pub id: i64,
    pub ts: i64,
    pub from_me: bool,
    /// WhatsApp's message type code (0 text, 1 photo, 2 audio, 3 video, 9 document, 20 sticker, …).
    pub kind: i64,
    pub text: Option<String>,
    /// The last 3 characters of the sender's id, in a group.
    pub sender: Option<String>,
    /// The sender's row in WhatsApp's `jid` table, for the adapter (never sent to the UI).
    #[serde(skip)]
    pub sender_row: Option<i64>,
    /// The message this one answers, as WhatsApp kept a copy of it.
    pub reply: Option<Reply>,
    /// (emoji, from me), oldest first; a removed reaction isn't here.
    pub reactions: Vec<(String, bool)>,
    pub edited: bool,
    /// WhatsApp's forward score: 0 not forwarded, 1 to 4 forwarded, 5 or more "forwarded many times".
    pub forwarded: u8,
    /// A poll's options: (name, votes), in their order.
    pub poll: Vec<(String, u64)>,
    pub call: Option<Call>,
    pub media: Option<Media>,
    /// A system message's `message_system.action_type`, for the adapter (never sent to the UI).
    #[serde(skip)]
    pub action: Option<i64>,
}

/// See [`Line::reply`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Reply {
    /// The answered message, when it's still in the backup (the viewer can jump to it).
    pub id: Option<i64>,
    pub from_me: bool,
    pub kind: i64,
    pub text: Option<String>,
}

/// See [`Line::call`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Call {
    pub video: bool,
    /// WhatsApp's call result code (5 answered, 2 missed, 3 declined, …).
    pub result: Option<i64>,
    pub secs: Option<i64>,
}

/// See [`Line::media`]: what the backup says about a file, whether or not the file is at hand.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Media {
    pub mime: Option<String>,
    /// A document's file name.
    pub name: Option<String>,
    pub size: Option<i64>,
    pub secs: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// WhatsApp's `file_path` (`Media/WhatsApp Stickers/…`), for the adapter (never sent to the UI).
    #[serde(skip)]
    pub file: Option<String>,
}

/// What a search is narrowed to; every part optional. With no words, the newest messages that match the rest.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Filter {
    pub chat: Option<i64>,
    /// WhatsApp message type codes; empty means any.
    pub kinds: Vec<i64>,
    pub from_me: Option<bool>,
    /// The last 3 characters of a sender's id (as [`Stats::senders`] gives them).
    pub sender: Option<String>,
    pub after_ms: Option<i64>,
    pub before_ms: Option<i64>,
}

impl Filter {
    fn narrows(&self) -> bool {
        self.chat.is_some()
            || !self.kinds.is_empty()
            || self.from_me.is_some()
            || self.sender.is_some()
            || self.after_ms.is_some()
            || self.before_ms.is_some()
    }
}

/// Counts for a chat, or for the whole backup.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Stats {
    pub messages: u64,
    pub from_me: u64,
    pub from_others: u64,
    pub first_ms: Option<i64>,
    pub last_ms: Option<i64>,
    /// (`YYYY-MM` in this computer's time zone, messages)
    pub by_month: Vec<(String, u64)>,
    /// Messages per hour of the day, this computer's time zone.
    pub by_hour: Vec<u64>,
    /// Messages per day of the week (Sunday first) and hour: 7 rows of 24.
    pub by_day_hour: Vec<Vec<u64>>,
    /// (message type code, messages, media bytes)
    pub by_kind: Vec<(i64, u64, u64)>,
    /// Voice notes and audio, in seconds.
    pub audio_seconds: u64,
    /// The most active senders in a group: (last 3 characters of their id, messages), most first.
    pub senders: Vec<(String, u64)>,
}

/// How long the index took to build, for the plan's numbers.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BuildInfo {
    pub messages: u64,
    pub with_text: u64,
    pub millis: u64,
}

pub struct Index {
    db: Connection,
    _file: tempfile::NamedTempFile,
    pub info: BuildInfo,
    /// The WhatsApp version that wrote the backup (msgkinds' lookups are at it); `None`: not known.
    version: Option<String>,
}

impl Index {
    /// Builds the index from a backup's database (read-only) into a new private file in `dir`.
    pub fn build(source: &Connection, dir: &Path) -> Result<Self> {
        Self::build_with(source, dir, &mut |_, _| true)
    }

    /// [`Index::build`], calling `progress(done, total)`: copying the messages is the first half, the stats the next
    /// quarter, the full-text index the last.
    pub fn build_with(source: &Connection, dir: &Path, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<Self> {
        let started = Instant::now();
        let file = tempfile::Builder::new()
            .prefix("index-")
            .tempfile_in(dir)
            .map_err(|e| crate::AppError::io("creating the search index", e))?;
        let mut db = Connection::open(file.path()).map_err(oopz::Error::from)?;
        build_into(source, &mut db, progress)?;
        let (messages, with_text) = db
            .query_row("SELECT count(*), count(text) FROM msg", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(oopz::Error::from)?;
        let info = BuildInfo { messages, with_text, millis: started.elapsed().as_millis() as u64 };
        Ok(Self { db, _file: file, info, version: None })
    }

    /// The same index, its lookups at the WhatsApp `version` that wrote the backup.
    pub fn at_version(mut self, version: Option<String>) -> Self {
        self.version = version;
        self
    }

    /// Messages matching every word of `query` (the last word as a prefix: `pla` finds `playa`), newest first, in
    /// `chat` or everywhere; at most `limit`.
    /// `filter` narrows it (with no words, the newest messages that match the filter; nothing when it narrows nothing).
    pub fn search(&self, query: &str, filter: &Filter, limit: u32) -> Result<Vec<Hit>> {
        let fts = fts_query(query);
        if fts.is_none() && !filter.narrows() {
            return Ok(Vec::new());
        }
        let narrow = "(?2 IS NULL OR m.chat = ?2)
             AND (?4 IS NULL OR m.kind IN (SELECT value FROM json_each(?4)))
             AND (?5 IS NULL OR m.from_me = ?5)
             AND (?6 IS NULL OR m.sender IN (SELECT row FROM who WHERE ending = ?6))
             AND (?7 IS NULL OR m.ts >= ?7) AND (?8 IS NULL OR m.ts < ?8)";
        let sql = if fts.is_some() {
            format!(
                "SELECT m.id, m.chat, m.ts, m.from_me, snippet(fts, 0, char(2), char(3), '…', 12)
                 FROM fts JOIN msg m ON m.id = fts.rowid
                 WHERE fts MATCH ?1 AND {narrow} ORDER BY fts.rowid DESC LIMIT ?3"
            )
        } else {
            format!(
                "SELECT m.id, m.chat, m.ts, m.from_me, coalesce(substr(m.text, 1, 80), '')
                 FROM msg m WHERE ?1 IS NULL AND {narrow} ORDER BY m.ts DESC, m.id DESC LIMIT ?3"
            )
        };
        let kinds = (!filter.kinds.is_empty())
            .then(|| format!("[{}]", filter.kinds.iter().map(i64::to_string).collect::<Vec<_>>().join(",")));
        let mut stmt = self.db.prepare_cached(&sql).map_err(oopz::Error::from)?;
        let hits = stmt
            .query_map(
                params![
                    fts,
                    filter.chat,
                    limit,
                    kinds,
                    filter.from_me,
                    filter.sender,
                    filter.after_ms,
                    filter.before_ms
                ],
                |r| Ok(Hit { id: r.get(0)?, chat: r.get(1)?, ts: r.get(2)?, from_me: r.get(3)?, snippet: r.get(4)? }),
            )
            .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            .map_err(oopz::Error::from)?;
        Ok(hits)
    }

    /// How many messages a chat has, for the viewer's scroll bar.
    pub fn chat_len(&self, chat: i64) -> Result<u64> {
        Ok(self
            .db
            .query_row("SELECT count(*) FROM msg WHERE chat = ?1", [chat], |r| r.get(0))
            .map_err(oopz::Error::from)?)
    }

    /// `count` messages of a chat from position `from` (oldest first): the rows a virtualized list has on screen.
    pub fn lines(&self, chat: i64, from: u64, count: u32) -> Result<Vec<Line>> {
        self.lines_inner(chat, from, count).map_err(|e| oopz::Error::from(e).into())
    }

    fn lines_inner(&self, chat: i64, from: u64, count: u32) -> rusqlite::Result<Vec<Line>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT m.id, m.ts, m.from_me, m.kind, m.text, w.ending, m.size, m.secs, x.id IS NOT NULL,
                 x.reply_id, x.reply_from_me, x.reply_kind, x.reply_text, x.reply_key IS NOT NULL, x.fwd, x.edited,
                 x.mime, x.name, x.w, x.h, x.call_video, x.call_result, x.call_secs, m.sender, x.action, x.file
             FROM msg m LEFT JOIN who w ON w.row = m.sender LEFT JOIN extra x ON x.id = m.id
             WHERE m.chat = ?1 AND m.pos >= ?3 ORDER BY m.pos LIMIT ?2",
        )?;
        let mut lines = stmt
            .query_map(params![chat, count, from], |r| {
                let (size, secs): (Option<i64>, Option<i64>) = (r.get(6)?, r.get(7)?);
                let extra: bool = r.get(8)?;
                let mime: Option<String> = r.get(16)?;
                let media = if mime.is_some() || size.is_some() {
                    Some(Media {
                        mime,
                        name: r.get(17)?,
                        size,
                        secs,
                        width: r.get(18)?,
                        height: r.get(19)?,
                        file: r.get(25)?,
                    })
                } else {
                    None
                };
                let call = match r.get::<_, Option<bool>>(20)? {
                    Some(video) => Some(Call { video, result: r.get(21)?, secs: r.get(22)? }),
                    None => None,
                };
                Ok(Line {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    from_me: r.get(2)?,
                    kind: r.get(3)?,
                    text: r.get(4)?,
                    sender: r.get(5)?,
                    sender_row: r.get(23)?,
                    reply: if extra && r.get(13)? {
                        Some(Reply {
                            id: r.get(9)?,
                            from_me: r.get::<_, Option<bool>>(10)?.unwrap_or(false),
                            kind: r.get::<_, Option<i64>>(11)?.unwrap_or(0),
                            text: r.get(12)?,
                        })
                    } else {
                        None
                    },
                    reactions: Vec::new(),
                    edited: r.get::<_, Option<bool>>(15)?.unwrap_or(false),
                    forwarded: r.get::<_, Option<i64>>(14)?.unwrap_or(0).clamp(0, 255) as u8,
                    poll: Vec::new(),
                    call,
                    media,
                    action: r.get(24)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut reactions = self.db.prepare_cached("SELECT emoji, from_me FROM react WHERE msg = ?1 ORDER BY rowid")?;
        let mut poll = self.db.prepare_cached("SELECT name, votes FROM poll WHERE msg = ?1 ORDER BY ord")?;
        for line in &mut lines {
            // A sticker, a photo or a document has no duration, whatever WhatsApp wrote (it writes 0): msgkinds says
            // which kinds play.
            if let Some(media) = &mut line.media
                && !msgkinds::lookup(
                    msgkinds::Source::WhatsAppAndroid,
                    msgkinds::Code::Int(line.kind),
                    self.version.as_deref(),
                )
                .kind()
                .has_duration()
            {
                media.secs = None;
            }
            line.reactions =
                reactions.query_map([line.id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            if line.kind == POLL {
                line.poll =
                    poll.query_map([line.id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
            }
        }
        Ok(lines)
    }

    /// The position of the first message of `chat` at or after `ts` (ms), for "jump to a date"; the last message's
    /// when all are older, `None` when the chat is empty.
    pub fn position_at(&self, chat: i64, ts: i64) -> Result<Option<u64>> {
        let at = self
            .db
            .query_row(
                "SELECT coalesce((SELECT min(pos) FROM msg WHERE chat = ?1 AND ts >= ?2),
                     (SELECT max(pos) FROM msg WHERE chat = ?1))",
                params![chat, ts],
                |r| r.get(0),
            )
            .optional()
            .map_err(oopz::Error::from)?
            .flatten();
        Ok(at)
    }

    /// Where message `id` sits in its chat (for jumping from a search result to the viewer).
    pub fn position(&self, id: i64) -> Result<Option<(i64, u64)>> {
        let found = self.db.query_row("SELECT chat, pos FROM msg WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)));
        match found {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(oopz::Error::from(e).into()),
        }
    }

    /// Counts for `chat`, or the whole backup.
    pub fn stats(&self, chat: Option<i64>) -> Result<Stats> {
        self.stats_inner(chat).map_err(|e| oopz::Error::from(e).into())
    }

    /// From the `agg` tables made at build time: a whole backup's stats read thousands of rows, not a million.
    fn stats_inner(&self, chat: Option<i64>) -> rusqlite::Result<Stats> {
        let mut s = Stats { by_hour: vec![0; 24], by_day_hour: vec![vec![0; 24]; 7], ..Default::default() };
        let filter = "(?1 IS NULL OR chat = ?1)";
        (s.messages, s.from_me, s.first_ms, s.last_ms, s.audio_seconds) = self.db.query_row(
            &format!(
                "SELECT coalesce(sum(n), 0), coalesce(sum(n * from_me), 0), min(first), max(last), coalesce(sum(secs), 0)
                 FROM agg WHERE {filter}"
            ),
            [chat],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        s.from_others = s.messages - s.from_me;
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT month, sum(n) FROM agg WHERE {filter} AND month IS NOT NULL GROUP BY month ORDER BY month"
        ))?;
        s.by_month = stmt.query_map([chat], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT hour, sum(n) FROM agg WHERE {filter} AND hour IS NOT NULL GROUP BY hour"
        ))?;
        for row in stmt.query_map([chat], |r| Ok((r.get::<_, usize>(0)?, r.get::<_, u64>(1)?)))? {
            let (hour, n) = row?;
            if let Some(slot) = s.by_hour.get_mut(hour) {
                *slot = n;
            }
        }
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT dow, hour, sum(n) FROM agg WHERE {filter} AND hour IS NOT NULL GROUP BY dow, hour"
        ))?;
        for row in
            stmt.query_map([chat], |r| Ok((r.get::<_, usize>(0)?, r.get::<_, usize>(1)?, r.get::<_, u64>(2)?)))?
        {
            let (dow, hour, n) = row?;
            if let Some(slot) = s.by_day_hour.get_mut(dow).and_then(|d| d.get_mut(hour)) {
                *slot = n;
            }
        }
        let mut stmt = self.db.prepare_cached(&format!(
            "SELECT kind, sum(n), sum(bytes) FROM agg WHERE {filter} GROUP BY kind ORDER BY 2 DESC"
        ))?;
        s.by_kind =
            stmt.query_map([chat], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut stmt = self.db.prepare_cached(
            "SELECT w.ending, sum(a.n) FROM agg_sender a JOIN who w ON w.row = a.sender
             WHERE (?1 IS NULL OR a.chat = ?1) GROUP BY a.sender ORDER BY 2 DESC LIMIT 20",
        )?;
        s.senders = stmt.query_map([chat], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(s)
    }
}

/// Copies what the index needs from `source` (whatever columns this WhatsApp version has) and builds FTS5 over it.
fn build_into(
    source: &Connection,
    db: &mut Connection,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> std::result::Result<(), oopz::Error> {
    let stop = || oopz::Error::Cancelled;
    let messages: u64 =
        source.query_row("SELECT count(*) FROM message WHERE chat_row_id IS NOT NULL", [], |r| r.get(0))?;
    let total = 4 * messages.max(1);
    if !progress(0, total) {
        return Err(stop());
    }
    db.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA cache_size = -32768;
         CREATE TABLE msg (id INTEGER PRIMARY KEY, chat INTEGER NOT NULL, ts INTEGER NOT NULL, from_me INTEGER NOT NULL,
             kind INTEGER NOT NULL, sender INTEGER, text TEXT, size INTEGER, secs INTEGER,
             -- The message's place in its chat, oldest first: the viewer reads a screenful by it, without OFFSET.
             pos INTEGER NOT NULL);
         CREATE TABLE who (row INTEGER PRIMARY KEY, ending TEXT NOT NULL);
         -- What the viewer shows besides the text, for the messages that have any of it.
         CREATE TABLE extra (id INTEGER PRIMARY KEY, reply_key TEXT, reply_id INTEGER, reply_from_me INTEGER,
             reply_kind INTEGER, reply_text TEXT, fwd INTEGER, edited INTEGER, mime TEXT, name TEXT, w INTEGER,
             h INTEGER, call_video INTEGER, call_result INTEGER, call_secs INTEGER, action INTEGER,
             file TEXT);
         CREATE TABLE react (msg INTEGER NOT NULL, emoji TEXT NOT NULL, from_me INTEGER NOT NULL);
         CREATE TABLE poll (msg INTEGER NOT NULL, ord INTEGER NOT NULL, name TEXT, votes INTEGER NOT NULL);",
    )?;
    let has = |table: &str, column: &str| -> rusqlite::Result<bool> {
        source
            .query_row("SELECT count(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2", [table, column], |r| r.get(0))
    };
    let col = |table: &str, column: &str, alias: &str| -> rusqlite::Result<String> {
        Ok(if has(table, column)? { format!("{alias}.{column}") } else { "NULL".into() })
    };
    let media = has("message_media", "message_row_id")?;
    let (size, secs) = if media {
        (col("message_media", "file_size", "mm")?, col("message_media", "media_duration", "mm")?)
    } else {
        ("NULL".into(), "NULL".into())
    };
    // Each side table joins only when this WhatsApp version has it; a missing column reads as NULL.
    let (mut joins, mut extras) = (String::new(), Vec::new());
    let sides: [(&str, &str, &str, &str, &[&str]); 7] = [
        ("message_quoted", "q", "message_row_id", "m._id", &["key_id", "from_me", "message_type", "text_data"]),
        ("message_forwarded", "f", "message_row_id", "m._id", &["forward_score"]),
        ("message_edit_info", "e", "message_row_id", "m._id", &["edited_timestamp"]),
        (
            "message_media",
            "mx",
            "message_row_id",
            "m._id",
            &["mime_type", "media_name", "width", "height", "file_path"],
        ),
        ("message_call_log", "mc", "message_row_id", "m._id", &[]),
        ("call_log", "c", "_id", "mc.call_log_row_id", &["video_call", "call_result", "duration"]),
        // What a system message (type 7) says happened: msgkinds' `whatsapp_android_system` codes.
        ("message_system", "ms", "message_row_id", "m._id", &["action_type"]),
    ];
    for (table, alias, key, other, cols) in sides {
        let there = has(table, key)? && (alias != "c" || has("message_call_log", "call_log_row_id")?);
        if there {
            joins += &format!(" LEFT JOIN {table} {alias} ON {alias}.{key} = {other}");
        }
        for c in cols {
            extras.push(if there { col(table, c, alias)? } else { "NULL".into() });
        }
    }
    // A reply can quote a message of another chat (WhatsApp's "reply privately"): its key is looked up there.
    extras.push(if has("message_quoted", "message_row_id")? {
        col("message_quoted", "parent_message_chat_row_id", "q")?
    } else {
        "NULL".into()
    });
    let sql = format!(
        "SELECT m._id, m.chat_row_id, coalesce(m.timestamp, 0), coalesce(m.from_me, 0), coalesce(m.message_type, 0),
            {}, nullif(m.text_data, ''), {size}, {secs}, {}
         FROM message m {}{joins}
         WHERE m.chat_row_id IS NOT NULL
         ORDER BY m.chat_row_id, coalesce(m.timestamp, 0), m._id",
        col("message", "sender_jid_row_id", "m")?,
        extras.join(", "),
        if media { "LEFT JOIN message_media mm ON mm.message_row_id = m._id" } else { "" },
    );
    let tx = db.transaction()?;
    {
        let mut insert = tx.prepare("INSERT INTO msg VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;
        let mut insert_extra = tx.prepare(
            "INSERT INTO extra (id, reply_key, reply_from_me, reply_kind, reply_text, fwd, edited, mime, name, w, h,
                 file, call_video, call_result, call_secs, action)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        )?;
        // (chat, quoted key) → the replies quoting it, to find the quoted message's row afterwards.
        let mut quoted: HashMap<(i64, String), Vec<i64>> = HashMap::new();
        let mut read = source.prepare(&sql)?;
        let mut rows = read.query([])?;
        let (mut chat, mut pos, mut copied) = (None, 0i64, 0u64);
        while let Some(r) = rows.next()? {
            let x: Vec<rusqlite::types::Value> = (9..24).map(|i| r.get(i)).collect::<rusqlite::Result<_>>()?;
            if x.iter().any(|v| *v != rusqlite::types::Value::Null) {
                let id: i64 = r.get(0)?;
                let reply_key: Option<String> = r.get(9)?;
                if let Some(key) = &reply_key {
                    let in_chat = r.get::<_, Option<i64>>(24)?.unwrap_or(r.get(1)?);
                    quoted.entry((in_chat, key.clone())).or_default().push(id);
                }
                // An edit row means edited; a forward score of 0 means not forwarded.
                let edited = x[5] != rusqlite::types::Value::Null;
                insert_extra.execute(params![
                    id, reply_key, x[1], x[2], x[3], x[4], edited, x[6], x[7], x[8], x[9], x[10], x[11], x[12], x[13],
                    x[14]
                ])?;
            }
            copied += 1;
            if copied % 4096 == 0 && !progress(2 * copied.min(messages), total) {
                return Err(stop());
            }
            let this: i64 = r.get(1)?;
            if chat != Some(this) {
                (chat, pos) = (Some(this), 0);
            }
            insert.execute(params![
                r.get::<_, i64>(0)?,
                this,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<i64>>(8)?,
                pos,
            ])?;
            pos += 1;
        }
        if !progress(2 * messages.max(1), total) {
            return Err(stop());
        }
        // The answered messages' rows, by their key in the same chat: one pass over the keys, only when replies exist.
        if !quoted.is_empty() && has("message", "key_id")? {
            let mut set = tx.prepare("UPDATE extra SET reply_id = ?2 WHERE id = ?1")?;
            let mut read = source.prepare("SELECT _id, chat_row_id, key_id FROM message WHERE key_id IS NOT NULL")?;
            let mut rows = read.query([])?;
            while let Some(r) = rows.next()? {
                let Ok(key) = r.get::<_, String>(2) else { continue };
                if let Some(replies) = quoted.get(&(r.get(1)?, key)) {
                    let target: i64 = r.get(0)?;
                    for reply in replies {
                        set.execute([reply, &target])?;
                    }
                }
            }
        }
        if has("message_add_on", "parent_message_row_id")? && has("message_add_on_reaction", "reaction")? {
            let mut insert = tx.prepare("INSERT INTO react VALUES (?1, ?2, ?3)")?;
            // Each person's latest reaction only: a changed one replaces theirs, an empty one takes it back.
            let sender = col("message_add_on", "sender_jid_row_id", "a")?;
            let mut read = source.prepare(&format!(
                "WITH latest AS (
                     SELECT max(a._id) AS id FROM message_add_on a
                     JOIN message_add_on_reaction r ON r.message_add_on_row_id = a._id
                     WHERE a.parent_message_row_id IS NOT NULL
                     GROUP BY a.parent_message_row_id, coalesce(a.from_me, 0), {sender})
                 SELECT a.parent_message_row_id, r.reaction, coalesce(a.from_me, 0)
                 FROM latest JOIN message_add_on a ON a._id = latest.id
                 JOIN message_add_on_reaction r ON r.message_add_on_row_id = a._id
                 WHERE typeof(r.reaction) = 'text' AND r.reaction != ''
                 ORDER BY a.parent_message_row_id, a._id"
            ))?;
            let mut rows = read.query([])?;
            while let Some(r) = rows.next()? {
                insert.execute(params![r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?])?;
            }
        }
        if has("message_poll_option", "message_row_id")? {
            // Votes counted from the chosen options when the backup has them, else WhatsApp's own total.
            // Each voter's latest vote only (a changed vote is a new one), when the backup links votes to voters.
            let latest_votes = has("message_add_on_poll_vote_selected_option", "message_add_on_row_id")?
                && has("message_add_on_poll_vote", "message_add_on_row_id")?
                && has("message_add_on", "parent_message_row_id")?;
            let sender = col("message_add_on", "sender_jid_row_id", "a")?;
            let latest = format!(
                "(SELECT count(*) FROM message_add_on_poll_vote_selected_option s
                  WHERE s.message_poll_option_id = o._id AND s.message_add_on_row_id IN (
                      SELECT max(a._id) FROM message_add_on a
                      JOIN message_add_on_poll_vote v ON v.message_add_on_row_id = a._id
                      WHERE a.parent_message_row_id = o.message_row_id
                      GROUP BY coalesce(a.from_me, 0), {sender}))"
            );
            let votes = if latest_votes && has("message_add_on_poll_vote_selected_option", "message_poll_option_id")? {
                latest.as_str()
            } else if has("message_add_on_poll_vote_selected_option", "message_poll_option_id")? {
                "(SELECT count(*) FROM message_add_on_poll_vote_selected_option s WHERE s.message_poll_option_id = o._id)"
            } else if has("message_poll_option", "vote_total")? {
                "coalesce(o.vote_total, 0)"
            } else {
                "0"
            };
            let mut insert = tx.prepare("INSERT INTO poll VALUES (?1, ?2, ?3, ?4)")?;
            let mut read = source.prepare(&format!(
                "SELECT o.message_row_id, o._id, {}, {votes} FROM message_poll_option o WHERE o.message_row_id IS NOT NULL",
                col("message_poll_option", "option_name", "o")?
            ))?;
            let mut rows = read.query([])?;
            while let Some(r) = rows.next()? {
                insert.execute(params![
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?
                ])?;
            }
        }
        // Senders, masked: only the last 3 characters of each id are kept, never the number.
        let mut insert = tx.prepare("INSERT INTO who VALUES (?1, ?2)")?;
        let mut read = source.prepare("SELECT _id, raw_string FROM jid WHERE raw_string IS NOT NULL")?;
        let mut rows = read.query([])?;
        let mut seen = HashMap::new();
        while let Some(r) = rows.next()? {
            let raw: String = r.get(1)?;
            seen.insert(r.get::<_, i64>(0)?, masked_ending(&raw));
        }
        for (row, ending) in seen {
            insert.execute(params![row, ending])?;
        }
    }
    tx.execute_batch(
        "CREATE UNIQUE INDEX msg_chat ON msg (chat, pos);
         CREATE INDEX react_msg ON react (msg);
         CREATE INDEX poll_msg ON poll (msg);
         -- Stats by chat, month and hour in this computer's time zone at build time, type and side.
         CREATE TABLE agg AS SELECT chat,
             CASE WHEN ts > 0 THEN strftime('%Y-%m', ts / 1000, 'unixepoch', 'localtime') END AS month,
             CASE WHEN ts > 0 THEN CAST(strftime('%H', ts / 1000, 'unixepoch', 'localtime') AS INTEGER) END AS hour,
             CASE WHEN ts > 0 THEN CAST(strftime('%w', ts / 1000, 'unixepoch', 'localtime') AS INTEGER) END AS dow,
             kind, from_me, count(*) AS n, coalesce(sum(size), 0) AS bytes,
             coalesce(sum(CASE WHEN kind = 2 THEN secs END), 0) AS secs, min(nullif(ts, 0)) AS first, max(ts) AS last
             FROM msg GROUP BY 1, 2, 3, 4, 5, 6;
         CREATE INDEX msg_chat_ts ON msg (chat, ts);
         CREATE INDEX agg_chat ON agg (chat);
         CREATE TABLE agg_sender AS SELECT chat, sender, count(*) AS n FROM msg
             WHERE from_me = 0 AND sender IS NOT NULL GROUP BY chat, sender;
         CREATE INDEX agg_sender_chat ON agg_sender (chat);",
    )?;
    if !progress(3 * messages.max(1), total) {
        return Err(stop());
    }
    tx.execute_batch(
        "CREATE VIRTUAL TABLE fts USING fts5(text, content = 'msg', content_rowid = 'id',
             tokenize = 'unicode61 remove_diacritics 2');
         INSERT INTO fts (rowid, text) SELECT id, text FROM msg WHERE text IS NOT NULL;
         INSERT INTO fts (fts) VALUES ('optimize');",
    )?;
    tx.commit()?;
    db.execute_batch("ANALYZE")?;
    progress(total, total);
    Ok(())
}

/// The person's words as an FTS5 query: each word quoted (so `AND`, `*` or `"` typed are just text), all required,
/// the last one as a prefix when it has two letters or more (a one-letter prefix matches most of a backup and took
/// over 50 ms on a million messages). `None` when there's no word.
fn fts_query(query: &str) -> Option<String> {
    let words: Vec<&str> = query.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let (last, rest) = words.split_last()?;
    let mut out: Vec<String> = rest.iter().map(|w| format!("\"{w}\"")).collect();
    out.push(if last.chars().count() > 1 { format!("\"{last}\"*") } else { format!("\"{last}\"") });
    Some(out.join(" "))
}

/// The last 3 characters of an id's number: `user[.agent][:device]@server` keeps `user` only, so a device id
/// (`10000000001.0:0@s.whatsapp.net`) ends like its number, not `0:0`.
fn masked_ending(raw: &str) -> String {
    let user = raw.split(['@', '.', ':']).next().unwrap_or_default();
    user.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use oopz::Backup;
    use oopz::fixture::{self, BEN_CHAT, GROUP_CHAT, KEY_HEX};

    use super::*;

    fn index() -> (fixture::Fixture, tempfile::TempDir, Index) {
        let fx = fixture::build();
        let backup = Backup::open(&fx.backup, &oopz::Key::from_hex(KEY_HEX).unwrap()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let index = Index::build(backup.db(), dir.path()).unwrap();
        (fx, dir, index)
    }

    #[test]
    fn search_ignores_accents_and_case_and_finds_prefixes() {
        let (_fx, _dir, index) = index();
        assert!(index.info.messages > 0 && index.info.with_text > 0);
        // The fixture's invented texts include "Hola" and "playa".
        for query in ["hola", "HOLA", "pla", "Playa"] {
            let hits = index.search(query, &Filter::default(), 50).unwrap();
            assert!(!hits.is_empty(), "{query}");
            assert!(hits.iter().all(|h| h.snippet.contains('\u{2}')), "{query}: highlighted");
        }
        assert!(
            index
                .search("hola", &Filter { chat: Some(BEN_CHAT), ..Default::default() }, 50)
                .unwrap()
                .iter()
                .all(|h| h.chat == BEN_CHAT)
        );
        assert!(index.search("zzzz", &Filter::default(), 50).unwrap().is_empty());
        // Operators typed by the person are just text, never an FTS5 syntax error.
        for odd in ["\"", "AND", "a OR", "*", "NEAR(", "-", "  "] {
            assert!(index.search(odd, &Filter::default(), 50).is_ok(), "{odd}");
        }
    }

    #[test]
    fn the_viewer_pages_through_a_chat_and_jumps_to_a_hit() {
        let (_fx, _dir, index) = index();
        let len = index.chat_len(BEN_CHAT).unwrap();
        assert_eq!(len, 18);
        let first = index.lines(BEN_CHAT, 0, 10).unwrap();
        let rest = index.lines(BEN_CHAT, 10, 10).unwrap();
        assert_eq!((first.len(), rest.len()), (10, 8));
        assert!(first.windows(2).all(|w| w[0].ts <= w[1].ts));
        let hit = &index.search("playa", &Filter { chat: Some(BEN_CHAT), ..Default::default() }, 1).unwrap()[0];
        let (chat, at) = index.position(hit.id).unwrap().unwrap();
        assert_eq!(chat, BEN_CHAT);
        assert_eq!(index.lines(BEN_CHAT, at, 1).unwrap()[0].id, hit.id);
        assert_eq!(index.position(-1).unwrap(), None);
    }

    #[test]
    fn stats_count_by_month_hour_kind_and_sender_without_numbers() {
        let (_fx, _dir, index) = index();
        let all = index.stats(None).unwrap();
        let ben = index.stats(Some(BEN_CHAT)).unwrap();
        assert_eq!(ben.messages, 18);
        assert_eq!(ben.from_me + ben.from_others, ben.messages);
        assert_eq!(ben.by_hour.len(), 24);
        assert_eq!(ben.by_hour.iter().sum::<u64>(), ben.by_month.iter().map(|(_, n)| n).sum::<u64>());
        assert_eq!(ben.by_kind.iter().map(|(_, n, _)| n).sum::<u64>(), 18);
        assert!(all.messages > ben.messages);
        let group = index.stats(Some(GROUP_CHAT)).unwrap();
        assert!(group.senders.iter().all(|(ending, _)| ending.chars().count() <= 3));
        assert!(!format!("{all:?}{group:?}").contains(fixture::BEN_JID));
    }

    #[test]
    fn a_device_id_ends_like_its_number() {
        assert_eq!(masked_ending("10000000001.0:0@s.whatsapp.net"), "001");
        assert_eq!(masked_ending("10000000002:1@s.whatsapp.net"), "002");
        assert_eq!(masked_ending("120000000000000001@g.us"), "001");
    }

    #[test]
    fn words_become_a_safe_fts_query() {
        assert_eq!(fts_query("hola  pla").as_deref(), Some("\"hola\" \"pla\"*"));
        assert_eq!(fts_query("\"a\" OR bc*").as_deref(), Some("\"a\" \"OR\" \"bc\"*"));
        assert_eq!(fts_query("¿qué?").as_deref(), Some("\"qué\"*"));
        assert_eq!(fts_query(" - * "), None);
        assert_eq!(fts_query("hola m").as_deref(), Some("\"hola\" \"m\""));
    }

    /// A source with just the tables these cases need, invented rows only.
    fn edge_source(side_tables: bool) -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE jid (_id INTEGER PRIMARY KEY, raw_string TEXT);
             CREATE TABLE message (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, key_id TEXT,
                 sender_jid_row_id INTEGER, timestamp INTEGER, message_type INTEGER, text_data TEXT);
             INSERT INTO jid VALUES (1, '999900000001@s.whatsapp.net'), (2, '999900000002@s.whatsapp.net');
             INSERT INTO message VALUES
                 (1, 10, 0, 'k1', 1, 1000, 0, 'hola'),
                 (2, 10, 1, 'k2', NULL, 2000, 0, 'respuesta'),
                 (3, 20, 1, 'k3', NULL, 3000, 0, 'en otro chat'),
                 (4, 10, 1, 'k4', NULL, 4000, 46, '¿cuándo?'),
                 (5, 10, 0, 'k5', 1, 5000, 0, 'a nothing');",
        )
        .unwrap();
        if side_tables {
            db.execute_batch(
                "CREATE TABLE message_quoted (message_row_id INTEGER PRIMARY KEY, parent_message_chat_row_id INTEGER,
                     from_me INTEGER, key_id TEXT, message_type INTEGER, text_data TEXT);
                 -- 2 answers 1 in the same chat; 3 answers 1 from another chat; 5 answers a deleted message.
                 INSERT INTO message_quoted VALUES (2, 10, 0, 'k1', 0, 'hola'), (3, 10, 0, 'k1', 0, 'hola'),
                     (5, 10, 1, 'gone', 0, 'borrado');
                 CREATE TABLE message_add_on (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER,
                     key_id TEXT, sender_jid_row_id INTEGER, parent_message_row_id INTEGER, timestamp INTEGER);
                 CREATE TABLE message_add_on_reaction (message_add_on_row_id INTEGER PRIMARY KEY, reaction TEXT);
                 -- On 1: person 2 reacts then changes it; I react then take it back; person 1's reaction has no value.
                 INSERT INTO message_add_on VALUES (1, 10, 0, 'a1', 2, 1, 1), (2, 10, 0, 'a2', 2, 1, 2),
                     (3, 10, 1, 'a3', NULL, 1, 3), (4, 10, 1, 'a4', NULL, 1, 4), (5, 10, 0, 'a5', 1, 1, 5);
                 INSERT INTO message_add_on_reaction VALUES (1, '👍'), (2, '❤️'), (3, '😂'), (4, ''), (5, NULL);
                 CREATE TABLE message_poll_option (_id INTEGER PRIMARY KEY, message_row_id INTEGER, option_name TEXT,
                     vote_total INTEGER);
                 INSERT INTO message_poll_option VALUES (1, 4, 'lunes', 9), (2, 4, 'martes', 9);
                 CREATE TABLE message_add_on_poll_vote (message_add_on_row_id INTEGER PRIMARY KEY);
                 CREATE TABLE message_add_on_poll_vote_selected_option (message_add_on_row_id INTEGER,
                     message_poll_option_id INTEGER);
                 -- Person 2 votes lunes, then changes to martes; I vote martes.
                 INSERT INTO message_add_on VALUES (6, 10, 0, 'v1', 2, 4, 6), (7, 10, 0, 'v2', 2, 4, 7),
                     (8, 10, 1, 'v3', NULL, 4, 8);
                 INSERT INTO message_add_on_poll_vote VALUES (6), (7), (8);
                 INSERT INTO message_add_on_poll_vote_selected_option VALUES (6, 1), (7, 2), (8, 2);",
            )
            .unwrap();
        }
        db
    }

    #[test]
    fn changed_reactions_and_votes_count_once_and_replies_find_their_message_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::build(&edge_source(true), dir.path()).unwrap();
        let chat = index.lines(10, 0, 10).unwrap();
        let first = &chat[0];
        // Person 2's latest (❤️) only; mine was taken back; one with no value isn't shown.
        assert_eq!(first.reactions, vec![("❤️".to_owned(), false)]);
        assert_eq!(chat[1].reply.as_ref().unwrap().id, Some(1));
        let other = index.lines(20, 0, 1).unwrap();
        assert_eq!(other[0].reply.as_ref().unwrap().id, Some(1), "a reply from another chat");
        let gone = chat.iter().find(|l| l.id == 5).unwrap().reply.clone().unwrap();
        assert_eq!((gone.id, gone.text.as_deref()), (None, Some("borrado")), "the quoted copy stays");
        let poll = chat.iter().find(|l| l.id == 4).unwrap();
        assert_eq!(poll.poll, vec![("lunes".to_owned(), 0), ("martes".to_owned(), 2)]);
    }

    #[test]
    fn a_backup_without_side_tables_still_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::build(&edge_source(false), dir.path()).unwrap();
        let lines = index.lines(10, 0, 10).unwrap();
        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| l.reply.is_none() && l.reactions.is_empty() && l.poll.is_empty()));
        assert!(lines.iter().all(|l| l.media.is_none() && l.call.is_none() && !l.edited && l.forwarded == 0));
    }

    #[test]
    fn jump_to_a_date_and_filters_at_the_edges() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::build(&edge_source(true), dir.path()).unwrap();
        assert_eq!(index.position_at(10, i64::MIN).unwrap(), Some(0));
        assert_eq!(index.position_at(10, 4000).unwrap(), Some(2), "exactly at a message's time");
        assert_eq!(index.position_at(10, i64::MAX).unwrap(), Some(3), "after the last: the last");
        assert_eq!(index.position_at(99, 0).unwrap(), None, "no such chat");
        let polls = Filter { kinds: vec![46], ..Default::default() };
        assert_eq!(index.search("", &polls, 10).unwrap().len(), 1);
        let empty_range = Filter { after_ms: Some(5000), before_ms: Some(5000), ..Default::default() };
        assert!(index.search("", &empty_range, 10).unwrap().is_empty());
        let person = Filter { sender: Some("001".into()), ..Default::default() };
        assert_eq!(index.search("", &person, 10).unwrap().len(), 2);
        assert!(
            index.search("hola", &Filter { sender: Some("002".into()), ..Default::default() }, 10).unwrap().is_empty()
        );
    }

    #[test]
    fn an_index_build_stops_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let stopped = Index::build_with(&edge_source(true), dir.path(), &mut |_, _| false);
        assert_eq!(stopped.err().map(|e| e.code), Some("cancelled"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "nothing left behind");
    }
}
