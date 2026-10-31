//! A search index and stats over a checked backup (decision 0009): a private SQLite file next to the backup's
//! decrypted copy, removed with it, holding one row per message (time, side, type, text, media size) and an FTS5
//! full-text index that ignores accents (`canción` finds `cancion`). Built once per backup, in seconds; a search then
//! takes milliseconds. Message text is for the person's own screen only: never logged, never in an error.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Result;

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
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Line {
    pub id: i64,
    pub ts: i64,
    pub from_me: bool,
    /// WhatsApp's message type code (0 text, 1 photo, 2 audio, 3 video, 9 document, 20 sticker, …).
    pub kind: i64,
    pub text: Option<String>,
    /// The last 3 characters of the sender's id, in a group.
    pub sender: Option<String>,
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
}

impl Index {
    /// Builds the index from a backup's database (read-only) into a new private file in `dir`.
    pub fn build(source: &Connection, dir: &Path) -> Result<Self> {
        let started = Instant::now();
        let file = tempfile::Builder::new()
            .prefix("index-")
            .tempfile_in(dir)
            .map_err(|e| crate::AppError::io("creating the search index", e))?;
        let mut db = Connection::open(file.path()).map_err(oopz::Error::from)?;
        build_into(source, &mut db).map_err(oopz::Error::from)?;
        let (messages, with_text) = db
            .query_row("SELECT count(*), count(text) FROM msg", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(oopz::Error::from)?;
        let info = BuildInfo { messages, with_text, millis: started.elapsed().as_millis() as u64 };
        Ok(Self { db, _file: file, info })
    }

    /// Messages matching every word of `query` (the last word as a prefix: `pla` finds `playa`), newest first, in
    /// `chat` or everywhere; at most `limit`.
    pub fn search(&self, query: &str, chat: Option<i64>, limit: u32) -> Result<Vec<Hit>> {
        let Some(fts) = fts_query(query) else { return Ok(Vec::new()) };
        let mut stmt = self
            .db
            .prepare_cached(
                "SELECT m.id, m.chat, m.ts, m.from_me, snippet(fts, 0, char(2), char(3), '…', 12)
                 FROM fts JOIN msg m ON m.id = fts.rowid
                 WHERE fts MATCH ?1 AND (?2 IS NULL OR m.chat = ?2)
                 ORDER BY fts.rowid DESC LIMIT ?3",
            )
            .map_err(oopz::Error::from)?;
        let hits = stmt
            .query_map(params![fts, chat, limit], |r| {
                Ok(Hit { id: r.get(0)?, chat: r.get(1)?, ts: r.get(2)?, from_me: r.get(3)?, snippet: r.get(4)? })
            })
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
        let mut stmt = self
            .db
            .prepare_cached(
                "SELECT m.id, m.ts, m.from_me, m.kind, m.text, w.ending FROM msg m LEFT JOIN who w ON w.row = m.sender
                 WHERE m.chat = ?1 AND m.pos >= ?3 ORDER BY m.pos LIMIT ?2",
            )
            .map_err(oopz::Error::from)?;
        let lines = stmt
            .query_map(params![chat, count, from], |r| {
                Ok(Line {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    from_me: r.get(2)?,
                    kind: r.get(3)?,
                    text: r.get(4)?,
                    sender: r.get(5)?,
                })
            })
            .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            .map_err(oopz::Error::from)?;
        Ok(lines)
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
        let mut s = Stats { by_hour: vec![0; 24], ..Default::default() };
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
fn build_into(source: &Connection, db: &mut Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA cache_size = -32768;
         CREATE TABLE msg (id INTEGER PRIMARY KEY, chat INTEGER NOT NULL, ts INTEGER NOT NULL, from_me INTEGER NOT NULL,
             kind INTEGER NOT NULL, sender INTEGER, text TEXT, size INTEGER, secs INTEGER,
             -- The message's place in its chat, oldest first: the viewer reads a screenful by it, without OFFSET.
             pos INTEGER NOT NULL);
         CREATE TABLE who (row INTEGER PRIMARY KEY, ending TEXT NOT NULL);",
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
    let sql = format!(
        "SELECT m._id, m.chat_row_id, coalesce(m.timestamp, 0), coalesce(m.from_me, 0), coalesce(m.message_type, 0),
            {}, nullif(m.text_data, ''), {size}, {secs}
         FROM message m {}
         WHERE m.chat_row_id IS NOT NULL
         ORDER BY m.chat_row_id, coalesce(m.timestamp, 0), m._id",
        col("message", "sender_jid_row_id", "m")?,
        if media { "LEFT JOIN message_media mm ON mm.message_row_id = m._id" } else { "" },
    );
    let tx = db.transaction()?;
    {
        let mut insert = tx.prepare("INSERT INTO msg VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;
        let mut read = source.prepare(&sql)?;
        let mut rows = read.query([])?;
        let (mut chat, mut pos) = (None, 0i64);
        while let Some(r) = rows.next()? {
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
        // Senders, masked: only the last 3 characters of each id are kept, never the number.
        let mut insert = tx.prepare("INSERT INTO who VALUES (?1, ?2)")?;
        let mut read = source.prepare("SELECT _id, raw_string FROM jid WHERE raw_string IS NOT NULL")?;
        let mut rows = read.query([])?;
        let mut seen = HashMap::new();
        while let Some(r) = rows.next()? {
            let raw: String = r.get(1)?;
            let user = raw.split_once('@').map_or(raw.as_str(), |(u, _)| u);
            let ending: String = user.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
            seen.insert(r.get::<_, i64>(0)?, ending);
        }
        for (row, ending) in seen {
            insert.execute(params![row, ending])?;
        }
    }
    tx.execute_batch(
        "CREATE UNIQUE INDEX msg_chat ON msg (chat, pos);
         -- Stats by chat, month and hour in this computer's time zone at build time, type and side.
         CREATE TABLE agg AS SELECT chat,
             CASE WHEN ts > 0 THEN strftime('%Y-%m', ts / 1000, 'unixepoch', 'localtime') END AS month,
             CASE WHEN ts > 0 THEN CAST(strftime('%H', ts / 1000, 'unixepoch', 'localtime') AS INTEGER) END AS hour,
             kind, from_me, count(*) AS n, coalesce(sum(size), 0) AS bytes,
             coalesce(sum(CASE WHEN kind = 2 THEN secs END), 0) AS secs, min(nullif(ts, 0)) AS first, max(ts) AS last
             FROM msg GROUP BY 1, 2, 3, 4, 5;
         CREATE INDEX agg_chat ON agg (chat);
         CREATE TABLE agg_sender AS SELECT chat, sender, count(*) AS n FROM msg
             WHERE from_me = 0 AND sender IS NOT NULL GROUP BY chat, sender;
         CREATE INDEX agg_sender_chat ON agg_sender (chat);
         CREATE VIRTUAL TABLE fts USING fts5(text, content = 'msg', content_rowid = 'id',
             tokenize = 'unicode61 remove_diacritics 2');
         INSERT INTO fts (rowid, text) SELECT id, text FROM msg WHERE text IS NOT NULL;
         INSERT INTO fts (fts) VALUES ('optimize');",
    )?;
    tx.commit()?;
    db.execute_batch("ANALYZE")?;
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
            let hits = index.search(query, None, 50).unwrap();
            assert!(!hits.is_empty(), "{query}");
            assert!(hits.iter().all(|h| h.snippet.contains('\u{2}')), "{query}: highlighted");
        }
        assert!(index.search("hola", Some(BEN_CHAT), 50).unwrap().iter().all(|h| h.chat == BEN_CHAT));
        assert!(index.search("zzzz", None, 50).unwrap().is_empty());
        // Operators typed by the person are just text, never an FTS5 syntax error.
        for odd in ["\"", "AND", "a OR", "*", "NEAR(", "-", "  "] {
            assert!(index.search(odd, None, 50).is_ok(), "{odd}");
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
        let hit = &index.search("playa", Some(BEN_CHAT), 1).unwrap()[0];
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
    fn words_become_a_safe_fts_query() {
        assert_eq!(fts_query("hola  pla").as_deref(), Some("\"hola\" \"pla\"*"));
        assert_eq!(fts_query("\"a\" OR bc*").as_deref(), Some("\"a\" \"OR\" \"bc\"*"));
        assert_eq!(fts_query("¿qué?").as_deref(), Some("\"qué\"*"));
        assert_eq!(fts_query(" - * "), None);
        assert_eq!(fts_query("hola m").as_deref(), Some("\"hola\" \"m\""));
    }
}
