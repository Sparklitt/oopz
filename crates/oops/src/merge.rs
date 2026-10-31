//! Merging one person's one-to-one chat into the same chat in another person's backup (decision 0006). The source
//! is a [`Backup`]; the target's original file is only read, and the result is new crypt15 bytes, checked before
//! they're returned. Reports hold counts and table names only.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OpenFlags, params};
use sha2::{Digest, Sha256};

use crate::{Backup, Error, Key, Result, crypt15, media_relative};

/// Tables holding one row per message (or several, with their own `_id`), copied for every added message.
const PER_MESSAGE: &[&str] = &[
    "message_media",
    "message_quoted",
    "message_quoted_media",
    "message_details",
    "message_secret",
    "message_thumbnail",
    "message_album",
    "message_edit_info",
    "message_forwarded",
    "message_text",
    "message_link",
    "message_location",
    "message_revoked",
    "message_system",
    "message_view_once_media",
    "audio_data",
    "message_sticker_pack",
    "message_sticker_pack_stickers",
    "message_streaming_sidecar",
    "mms_thumbnail_metadata",
    "mms_metadata",
    "message_quoted_text",
    "message_quoted_call_log",
    "message_quoted_location",
    "message_quoted_vcard",
    "message_vcard",
    "message_inline_video_metadata",
    "message_system_block_contact",
    "message_system_initial_privacy_provider",
    "thread_messages",
];

/// Copied with their own rules below; with [`PER_MESSAGE`], what's never counted as left behind.
const HANDLED: &[&str] = &["message", "chat", "message_association", "message_call_log", "message_add_on"];

/// The media folders where WhatsApp keeps what the phone's owner sent in `Sent/`.
const SENT_FOLDERS: &[&str] =
    &["WhatsApp Images", "WhatsApp Video", "WhatsApp Audio", "WhatsApp Documents", "WhatsApp Animated Gifs"];

/// Merges `source_chat` of `source` into `target_chat` of the backup whose original bytes are `target`.
pub struct ChatMerge<'a> {
    pub source: &'a Backup,
    pub source_chat: i64,
    /// The target's original crypt15 file, as read from disk; only read.
    pub target: &'a [u8],
    pub target_key: &'a Key,
    pub target_chat: i64,
}

/// What a merge did and what it checked, in counts and table names only.
#[derive(Debug, Default)]
pub struct MergeReport {
    /// The target chat's messages before the merge.
    pub target_before: u64,
    /// Source messages the target chat already had (same `key_id`).
    pub already_there: u64,
    pub added: u64,
    /// Added messages newer than the target chat's first message; they still sort before it (decision 0006).
    pub added_after_target_start: u64,
    pub source_without_key: u64,
    /// The target chat's messages given new rows above the added ones.
    pub moved: u64,
    /// Rows written, per table.
    pub copied: BTreeMap<String, u64>,
    /// Source rows about added messages that weren't copied, per table.
    pub left_behind: BTreeMap<String, u64>,
    pub media_new_paths: u64,
    /// Media rows pointing at a file the target already has (same `file_hash`).
    pub media_reused: u64,
    pub media_unsafe_paths: u64,
    pub calls_reused: u64,
    /// Triggers dropped during the merge and recreated exactly after it.
    pub triggers: u64,
    /// The checks the result passed, each with what it measured.
    pub checks: Vec<(&'static str, String)>,
}

impl fmt::Display for MergeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "target chat: {} messages before", self.target_before)?;
        writeln!(f, "source chat: {} already there, {} added", self.already_there, self.added)?;
        if self.added_after_target_start > 0 {
            writeln!(f, "  added but newer than the target chat's first message: {}", self.added_after_target_start)?;
        }
        if self.source_without_key > 0 {
            writeln!(f, "  without a message id, not added: {}", self.source_without_key)?;
        }
        writeln!(f, "target chat messages moved above the added ones: {}", self.moved)?;
        writeln!(
            f,
            "media: {} new paths, {} reuse a file the target has, {} unsafe paths dropped; calls reused: {}",
            self.media_new_paths, self.media_reused, self.media_unsafe_paths, self.calls_reused
        )?;
        writeln!(f, "triggers set aside and restored: {}", self.triggers)?;
        for (table, n) in &self.copied {
            writeln!(f, "  copied {table}: {n}")?;
        }
        for (table, n) in &self.left_behind {
            writeln!(f, "  left behind {table}: {n}")?;
        }
        for (check, value) in &self.checks {
            writeln!(f, "check {check}: {value}")?;
        }
        Ok(())
    }
}

/// What a merge made: the target's new backup, its report, and the media files it expects that the target lacks.
pub struct Merged {
    /// Encrypted like the target's original, with its key.
    pub backup: Vec<u8>,
    pub report: MergeReport,
    pub media: Vec<MediaFile>,
}

/// A media file the merged backup expects: where the source keeps it and where the target will (`file_path`s).
/// Never printed: a document's name can be personal.
pub struct MediaFile {
    pub from: String,
    pub to: String,
}

impl ChatMerge<'_> {
    /// The merged backup and what it needs. Fails, writing nothing, if any check fails.
    pub fn run(&self) -> Result<Merged> {
        let source_chat = self.source.chat(self.source_chat)?;
        if !source_chat.is_one_to_one() {
            return Err(Error::Unsupported("only one-to-one chats can be merged".into()));
        }
        let original = crypt15::decrypt(self.target, self.target_key)?;
        let work = temp_copy(&original)?;
        let mut report = MergeReport::default();
        let (last_added, media) = {
            let mut db = Connection::open_with_flags(
                work.path(),
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            let source_uri = format!("file:{}?mode=ro", self.source.plain.path().display());
            db.execute("ATTACH DATABASE ?1 AS src", [source_uri])?;
            let tx = db.transaction()?;
            let done = Merger::new(&tx, self.source_chat, self.target_chat, &mut report)?.run()?;
            tx.commit()?;
            db.execute("DETACH DATABASE src", [])?;
            done
        };
        let merged = std::fs::read(work.path())
            .map_err(|source| Error::Io { context: "reading the merged database back".into(), source })?;
        let sealed = crypt15::reseal(self.target, &merged, self.target_key)?;
        report.checks = verify(&original, &sealed, self.target_key, self.target_chat, last_added, &report)?;
        Ok(Merged { backup: sealed, report, media })
    }
}

/// What [`copy_media`] did, in counts.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MediaReport {
    pub copied: u64,
    pub bytes: u64,
    /// Expected files the source's folder doesn't have (never downloaded, or deleted).
    pub missing: u64,
}

impl fmt::Display for MediaReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "media files: {} copied ({} bytes), {} missing", self.copied, self.bytes, self.missing)
    }
}

/// Copies each file from the source phone's `Media/` folder (`from_dir`, only read) into `to_dir`, laid out like the
/// target's `Media/` folder, keeping each file's modification time (galleries sort by it). `to_dir` must not exist;
/// nothing is overwritten.
pub fn copy_media(files: &[MediaFile], from_dir: &Path, to_dir: &Path) -> Result<MediaReport> {
    let err = |context: String| move |source| Error::Io { context, source };
    fs::create_dir(to_dir).map_err(err(format!("creating {}", to_dir.display())))?;
    let mut report = MediaReport::default();
    for file in files {
        let (Some(from), Some(to)) = (media_relative(&file.from), media_relative(&file.to)) else {
            report.missing += 1;
            continue;
        };
        let from = from_dir.join(from);
        let Ok(mut source) = fs::File::open(&from) else {
            report.missing += 1;
            continue;
        };
        let to = to_dir.join(to);
        fs::create_dir_all(to.parent().unwrap_or(to_dir)).map_err(err("creating a media folder".into()))?;
        let mut target = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&to)
            .map_err(err("creating a media file in the pack".into()))?;
        report.bytes += io::copy(&mut source, &mut target).map_err(err("copying a media file".into()))?;
        let modified = source.metadata().and_then(|m| m.modified()).map_err(err("reading a file's time".into()))?;
        target.set_modified(modified).map_err(err("setting a file's time".into()))?;
        report.copied += 1;
    }
    Ok(report)
}

struct Merger<'c, 'r> {
    db: &'c Connection,
    report: &'r mut MergeReport,
    source_chat: i64,
    chat: i64,
    chat_jid: i64,
    /// The `author_device_jid` the target chat uses for received (0) and sent (1) messages.
    devices: [Option<i64>; 2],
    /// Source message `_id` → (target `_id`, `from_me` in the target), for every source message in the result.
    messages: HashMap<i64, (i64, i64)>,
    calls: HashMap<i64, i64>,
    /// Source `thread_id._id` → target, for "view replies" threads.
    threads: HashMap<i64, i64>,
    add_ons: HashMap<i64, i64>,
    target_paths_by_hash: HashMap<String, String>,
    target_paths: HashSet<String>,
    /// Paths the added media rows use: new ones, and the target's own (reused), with how many rows each.
    new_paths: BTreeMap<String, u64>,
    /// One per new path, in the order first seen.
    media: Vec<MediaFile>,
    reused_paths: BTreeMap<String, u64>,
}

impl<'c, 'r> Merger<'c, 'r> {
    fn new(db: &'c Connection, source_chat: i64, chat: i64, report: &'r mut MergeReport) -> Result<Self> {
        let chat_jid: i64 = db
            .query_row(
                "SELECT c.jid_row_id FROM main.chat c JOIN main.jid j ON j._id = c.jid_row_id
                 WHERE c._id = ?1 AND (j.raw_string LIKE '%@s.whatsapp.net' OR j.raw_string LIKE '%@lid')",
                [chat],
                |r| r.get(0),
            )
            .map_err(|_| Error::Unsupported(format!("no one-to-one chat number {chat} in the target backup")))?;
        for (table,) in tables(db, "main")? {
            if table.starts_with("message_fts") {
                return Err(Error::Unsupported(format!("the target has `{table}`; its search index isn't rebuilt")));
            }
        }
        let mut devices = [None, None];
        if !columns(db, "main", "message_details")?.is_empty() {
            let mut stmt = db.prepare(
                "SELECT m.from_me, d.author_device_jid FROM main.message m
                 JOIN main.message_details d ON d.message_row_id = m._id
                 WHERE m.chat_row_id = ?1 AND d.author_device_jid IS NOT NULL
                 GROUP BY 1, 2 ORDER BY count(*) DESC",
            )?;
            let mut rows = stmt.query([chat])?;
            while let Some(r) = rows.next()? {
                let (from_me, device): (i64, i64) = (r.get(0)?, r.get(1)?);
                let slot = &mut devices[usize::from(from_me != 0)];
                slot.get_or_insert(device);
            }
        }
        let mut target_paths_by_hash = HashMap::new();
        let mut target_paths = HashSet::new();
        let media = columns(db, "main", "message_media")?;
        if media.iter().any(|c| c == "file_path") {
            let hash = if media.iter().any(|c| c == "file_hash") { "file_hash" } else { "NULL" };
            let mut stmt =
                db.prepare(&format!("SELECT file_path, {hash} FROM main.message_media WHERE file_path IS NOT NULL"))?;
            let mut rows = stmt.query([])?;
            while let Some(r) = rows.next()? {
                let (path, hash): (String, Option<String>) = (r.get(0)?, r.get(1)?);
                if let Some(hash) = hash {
                    target_paths_by_hash.entry(hash).or_insert_with(|| path.clone());
                }
                target_paths.insert(path);
            }
        }
        Ok(Self {
            db,
            report,
            source_chat,
            chat,
            chat_jid,
            devices,
            messages: HashMap::new(),
            calls: HashMap::new(),
            threads: HashMap::new(),
            add_ons: HashMap::new(),
            target_paths_by_hash,
            target_paths,
            new_paths: BTreeMap::new(),
            media: Vec::new(),
            reused_paths: BTreeMap::new(),
        })
    }

    /// Returns the last added message's new `_id` and the media files the new paths need.
    fn run(mut self) -> Result<(i64, Vec<MediaFile>)> {
        let db = self.db;
        let triggers: Vec<(String, String)> = query_all(
            db,
            "SELECT name, sql FROM main.sqlite_master WHERE type = 'trigger' AND sql IS NOT NULL ORDER BY rowid",
            [],
        )?;
        for (name, _) in &triggers {
            db.execute(&format!("DROP TRIGGER main.{}", quote(name)), [])?;
        }
        self.report.triggers = triggers.len() as u64;

        // The target chat, in order. Decision 0006 relies on sort_id = _id; refuse anything else.
        // (_id, sort_id, key_id, from_me, timestamp)
        type TargetRow = (i64, Option<i64>, Option<String>, i64, i64);
        let target: Vec<TargetRow> = query_all(
            db,
            "SELECT _id, sort_id, key_id, from_me, timestamp FROM main.message WHERE chat_row_id = ?1 ORDER BY _id",
            [self.chat],
        )?;
        if target.iter().any(|(id, sort, ..)| *sort != Some(*id)) {
            return Err(Error::Unsupported("the target chat has a sort_id unequal to its _id".into()));
        }
        self.report.target_before = target.len() as u64;
        let target_keys: HashSet<&str> = target.iter().filter_map(|t| t.2.as_deref()).collect();
        let first_target_ms = target.iter().map(|t| t.4).min();

        let source: Vec<(i64, Option<String>, i64, i64)> = query_all(
            db,
            "SELECT _id, key_id, from_me, timestamp FROM src.message WHERE chat_row_id = ?1 ORDER BY _id",
            [self.source_chat],
        )?;

        let mut next = self.first_free_id()?;
        let mut added = Vec::new();
        for (id, key, from_me, at) in &source {
            match key.as_deref().filter(|k| !k.is_empty() && *k != "-1") {
                None => self.report.source_without_key += 1,
                Some(k) if target_keys.contains(k) => self.report.already_there += 1,
                Some(_) => {
                    next += 1;
                    self.messages.insert(*id, (next, i64::from(*from_me == 0)));
                    added.push(*id);
                    if first_target_ms.is_some_and(|first| *at > first) {
                        self.report.added_after_target_start += 1;
                    }
                }
            }
        }
        self.report.added = added.len() as u64;
        let last_added = next;

        // Move the target chat above the added rows, rewriting every reference through one map.
        db.execute_batch(
            "CREATE TEMP TABLE hilo_move (old INTEGER PRIMARY KEY, new INTEGER NOT NULL);
             CREATE TEMP TABLE hilo_src (src INTEGER PRIMARY KEY, tgt INTEGER NOT NULL, added INTEGER NOT NULL);",
        )?;
        let mut moved = HashMap::new();
        {
            let mut ins = db.prepare("INSERT INTO temp.hilo_move VALUES (?1, ?2)")?;
            for (id, ..) in &target {
                next += 1;
                ins.execute([id, &next])?;
                moved.insert(*id, next);
            }
        }
        self.report.moved = moved.len() as u64;
        db.execute(
            "UPDATE main.message SET _id = (SELECT new FROM temp.hilo_move WHERE old = message._id),
                 sort_id = (SELECT new FROM temp.hilo_move WHERE old = message._id)
             WHERE _id IN (SELECT old FROM temp.hilo_move)",
            [],
        )?;
        for (table, column) in reference_columns(db, "main")? {
            let (t, c) = (quote(&table), quote(&column));
            db.execute(
                &format!(
                    "UPDATE main.{t} SET {c} = (SELECT new FROM temp.hilo_move WHERE old = {t}.{c})
                     WHERE {c} IN (SELECT old FROM temp.hilo_move)"
                ),
                [],
            )?;
        }

        // Source messages the target already had map to its (moved) rows, matched by key_id and mirrored from_me.
        let by_key: HashMap<(&str, i64), i64> = target
            .iter()
            .filter_map(|(id, _, key, from_me, _)| Some(((key.as_deref()?, *from_me), moved[id])))
            .collect();
        for (id, key, from_me, _) in &source {
            let mirrored = i64::from(*from_me == 0);
            if let Some(&tgt) = key.as_deref().and_then(|k| by_key.get(&(k, mirrored))) {
                self.messages.insert(*id, (tgt, mirrored));
            }
        }
        {
            let added: HashSet<i64> = added.iter().copied().collect();
            let mut ins = db.prepare("INSERT INTO temp.hilo_src VALUES (?1, ?2, ?3)")?;
            for (src, (tgt, _)) in &self.messages {
                ins.execute(params![src, tgt, added.contains(src)])?;
            }
        }

        const ADDED: &str = "(SELECT src FROM temp.hilo_src WHERE added)";
        self.copy_threads(ADDED)?;
        self.copy("message", &format!("_id IN {ADDED}"))?;
        for table in PER_MESSAGE {
            self.copy(table, &format!("message_row_id IN {ADDED}"))?;
        }
        self.copy("message_association", &format!("child_message_row_id IN {ADDED}"))?;
        self.copy_calls(ADDED)?;
        self.copy_reactions(ADDED)?;
        self.copy(
            "media_hash_thumbnail",
            &format!(
                "media_hash IN (SELECT file_hash FROM src.message_media WHERE message_row_id IN {ADDED})
                 AND media_hash NOT IN (SELECT media_hash FROM main.media_hash_thumbnail WHERE media_hash IS NOT NULL)"
            ),
        )?;
        self.media_refs()?;
        self.count_left_behind(ADDED)?;

        if !columns(db, "main", "sqlite_sequence")?.is_empty() {
            let names: Vec<(String,)> = query_all(db, "SELECT name FROM main.sqlite_sequence", [])?;
            for (name,) in names {
                db.execute(
                    &format!(
                        "UPDATE main.sqlite_sequence SET seq = (SELECT max(rowid) FROM main.{t})
                         WHERE name = ?1 AND seq < (SELECT max(rowid) FROM main.{t})",
                        t = quote(&name)
                    ),
                    [&name],
                )?;
            }
        }
        // Both phones' chats were created no later than their first message; older messages move that back.
        if columns(db, "main", "chat")?.iter().any(|c| c == "created_timestamp") {
            db.execute(
                "UPDATE main.chat SET created_timestamp = (SELECT min(timestamp) FROM main.message
                     WHERE chat_row_id = ?1 AND timestamp > 0)
                 WHERE _id = ?1 AND created_timestamp > (SELECT min(timestamp) FROM main.message
                     WHERE chat_row_id = ?1 AND timestamp > 0)",
                [self.chat],
            )?;
        }
        for (_, sql) in &triggers {
            db.execute_batch(sql)?;
        }
        Ok((last_added, self.media))
    }

    /// Above every message id the target has used or still refers to, so no stale reference finds a new row.
    fn first_free_id(&self) -> Result<i64> {
        let db = self.db;
        let mut max: i64 = db.query_row("SELECT coalesce(max(_id), 0) FROM main.message", [], |r| r.get(0))?;
        if !columns(db, "main", "sqlite_sequence")?.is_empty() {
            let seq: Option<i64> =
                db.query_row("SELECT seq FROM main.sqlite_sequence WHERE name = 'message'", [], |r| r.get(0)).ok();
            max = max.max(seq.unwrap_or(0));
        }
        for (table, column) in reference_columns(db, "main")? {
            let c = quote(&column);
            let m: Option<i64> = db.query_row(
                &format!("SELECT max({c}) FROM main.{} WHERE typeof({c}) = 'integer'", quote(&table)),
                [],
                |r| r.get(0),
            )?;
            max = max.max(m.unwrap_or(0));
        }
        Ok(max)
    }

    /// Copies the source rows of `table` matching `filter`, mirrored, by column name: only columns both schemas have.
    /// `_id` is kept (mapped) in `message` and renumbered elsewhere. Returns (source `_id`, new rowid) per row.
    fn copy(&mut self, table: &str, filter: &str) -> Result<Vec<(Option<i64>, i64)>> {
        let db = self.db;
        let source = columns(db, "src", table)?;
        if source.is_empty() {
            return Ok(Vec::new());
        }
        let target = columns(db, "main", table)?;
        let t = quote(table);
        if target.is_empty() {
            let n: u64 = db.query_row(&format!("SELECT count(*) FROM src.{t} WHERE {filter}"), [], |r| r.get(0))?;
            if n > 0 {
                *self.report.left_behind.entry(format!("{table} (not in the target)")).or_default() += n;
            }
            return Ok(Vec::new());
        }
        let read: Vec<String> = source.into_iter().filter(|c| target.contains(c) || c == "_id").collect();
        let write: Vec<bool> = read.iter().map(|c| target.contains(c) && (table == "message" || c != "_id")).collect();
        let id_at = read.iter().position(|c| c == "_id");
        let order = if id_at.is_some() { "ORDER BY _id" } else { "" };
        let list = |cols: &mut dyn Iterator<Item = &String>| cols.map(|c| quote(c)).collect::<Vec<_>>().join(", ");
        let mut select =
            db.prepare(&format!("SELECT {} FROM src.{t} WHERE {filter} {order}", list(&mut read.iter())))?;
        let written: Vec<&String> = read.iter().zip(&write).filter(|(_, w)| **w).map(|(c, _)| c).collect();
        let mut insert = db.prepare(&format!(
            "INSERT INTO main.{t} ({}) VALUES ({})",
            list(&mut written.iter().copied()),
            vec!["?"; written.len()].join(", ")
        ))?;

        let mut ids = Vec::new();
        let mut skipped = 0;
        let mut rows = select.query([])?;
        while let Some(r) = rows.next()? {
            let mut values = (0..read.len()).map(|i| r.get::<_, Value>(i)).collect::<rusqlite::Result<Vec<_>>>()?;
            let source_id = id_at.and_then(|i| integer(&values[i]));
            if !self.mirror(table, &read, &mut values)? {
                skipped += 1;
                continue;
            }
            insert
                .execute(rusqlite::params_from_iter(values.iter().zip(&write).filter(|(_, w)| **w).map(|(v, _)| v)))?;
            ids.push((source_id, db.last_insert_rowid()));
        }
        if !ids.is_empty() {
            *self.report.copied.entry(table.to_owned()).or_default() += ids.len() as u64;
        }
        if skipped > 0 {
            *self.report.left_behind.entry(format!("{table} (refers to a message not merged)")).or_default() += skipped;
        }
        Ok(ids)
    }

    /// Rewrites one source row as the target's owner sees it (decision 0006, step 3). `false` skips the row.
    fn mirror(&mut self, table: &str, columns: &[String], values: &mut [Value]) -> Result<bool> {
        let at = |name: &str| columns.iter().position(|c| c == name);
        let own_id = at("_id").and_then(|i| integer(&values[i]));
        // The row's from_me in the target: its own, flipped, or its message's.
        let from_me = match at("from_me").and_then(|i| integer(&values[i])) {
            Some(f) => Some(i64::from(f == 0)),
            None => {
                at("message_row_id").and_then(|i| integer(&values[i])).and_then(|m| self.messages.get(&m)).map(|m| m.1)
            }
        };
        let hash = at("file_hash").and_then(|i| match &values[i] {
            Value::Text(h) => Some(h.clone()),
            _ => None,
        });
        for (column, value) in columns.iter().zip(values.iter_mut()) {
            let n = integer(value);
            let set = !matches!(value, Value::Null);
            match column.as_str() {
                "_id" if table == "message" => *value = Value::Integer(self.new_message_id(own_id)?),
                "sort_id" if table == "message" => *value = Value::Integer(self.new_message_id(own_id)?),
                "_id" => {}
                "chat_row_id" | "parent_message_chat_row_id" if set => *value = Value::Integer(self.chat),
                "from_me" if set => *value = Value::Integer(from_me.unwrap_or(0)),
                "sender_jid_row_id" if set => *value = Value::Integer(0),
                // 6: system rows keep theirs.
                "status" if (table == "message" || table == "message_add_on") && n != Some(6) => {
                    *value = Value::Integer(if from_me == Some(1) { 13 } else { 0 });
                }
                "status" => {}
                c if is_message_reference(c) => match n {
                    None | Some(0) => {}
                    Some(id) => match self.messages.get(&id) {
                        Some((tgt, _)) => *value = Value::Integer(*tgt),
                        None => return Ok(false),
                    },
                },
                "author_device_jid" | "call_creator_device_jid_row_id" if n.is_some_and(|n| n != 0) => {
                    *value = Value::Integer(self.device(from_me)?);
                }
                "jid_row_id" if table == "call_log" && set => *value = Value::Integer(self.chat_jid),
                "call_log_row_id" if set => *value = Value::Integer(mapped(&self.calls, n, table, column)?),
                "thread_id" if table == "thread_messages" && set => {
                    *value = Value::Integer(mapped(&self.threads, n, table, column)?);
                }
                // A thread this chat doesn't have (none seen) loses the link, not the message.
                "view_replies_thread_id" if set => {
                    *value = n.and_then(|n| self.threads.get(&n)).map_or(Value::Null, |t| Value::Integer(*t));
                }
                "message_add_on_row_id" if set => *value = Value::Integer(mapped(&self.add_ons, n, table, column)?),
                "file_path" => {
                    if let Value::Text(path) = value {
                        let (path, counted) = (path.clone(), table == "message_media");
                        *value = self.place(&path, hash.as_deref(), counted)?.map_or(Value::Null, Value::Text);
                    }
                }
                c if (c.ends_with("_row_id") || c.contains("jid")) && n.is_some_and(|n| n != 0) => {
                    return Err(Error::Unsupported(format!("`{table}.{c}` holds an id the merge can't mirror")));
                }
                _ => {}
            }
        }
        Ok(true)
    }

    fn new_message_id(&self, source_id: Option<i64>) -> Result<i64> {
        source_id
            .and_then(|id| self.messages.get(&id))
            .map(|m| m.0)
            .ok_or_else(|| Error::Unsupported("a source message without a new row".into()))
    }

    fn device(&self, from_me: Option<i64>) -> Result<i64> {
        let sent = from_me == Some(1);
        self.devices[usize::from(sent)].ok_or_else(|| {
            let which = if sent { "sent" } else { "received" };
            Error::Unsupported(format!("the target chat has no author_device_jid for its {which} messages to copy"))
        })
    }

    /// The target path for a source media path: the target's own file when it has the same `file_hash`, else the
    /// mirrored folder (what one sent the other received) with a `hilo-` prefix, never an existing path.
    fn place(&mut self, path: &str, hash: Option<&str>, counted: bool) -> Result<Option<String>> {
        if let Some(theirs) = hash.and_then(|h| self.target_paths_by_hash.get(h)) {
            if counted {
                self.report.media_reused += 1;
                *self.reused_paths.entry(theirs.clone()).or_default() += 1;
            }
            return Ok(Some(theirs.clone()));
        }
        let Some(mirrored) = mirror_path(path) else {
            if counted {
                self.report.media_unsafe_paths += 1;
            }
            return Ok(None);
        };
        if self.target_paths.contains(&mirrored) {
            return Err(Error::Unsupported("a mirrored media path already exists in the target".into()));
        }
        if counted {
            let n = self.new_paths.entry(mirrored.clone()).or_default();
            if *n == 0 {
                self.report.media_new_paths += 1;
                self.media.push(MediaFile { from: path.to_owned(), to: mirrored.clone() });
            }
            *n += 1;
        }
        Ok(Some(mirrored))
    }

    /// "View replies" threads of added messages: the target's own when it has the same root `key_id` in the chat,
    /// else a mirrored copy. Before the messages, which point at them.
    fn copy_threads(&mut self, added: &str) -> Result<()> {
        let db = self.db;
        if columns(db, "src", "thread_id")?.is_empty() || columns(db, "main", "thread_id")?.is_empty() {
            return Ok(());
        }
        let mut used = Vec::new();
        if !columns(db, "src", "thread_messages")?.is_empty() {
            used.push(format!("SELECT thread_id FROM src.thread_messages WHERE message_row_id IN {added}"));
        }
        if columns(db, "src", "message")?.iter().any(|c| c == "view_replies_thread_id") {
            used.push(format!("SELECT view_replies_thread_id FROM src.message WHERE _id IN {added}"));
        }
        if used.is_empty() {
            return Ok(());
        }
        let (source, chat) = (self.source_chat, self.chat);
        let filter = format!("chat_row_id = {source} AND _id IN ({})", used.join(" UNION "));
        let existing: Vec<(i64, i64)> = query_all(
            db,
            &format!(
                "SELECT s._id, t._id FROM (SELECT _id, key_id FROM src.thread_id WHERE {filter}) s
                 JOIN main.thread_id t ON t.key_id = s.key_id AND t.chat_row_id = {chat}"
            ),
            [],
        )?;
        self.threads.extend(existing);
        let new = self.copy(
            "thread_id",
            &format!("{filter} AND key_id NOT IN (SELECT key_id FROM main.thread_id WHERE chat_row_id = {chat} AND key_id IS NOT NULL)"),
        )?;
        self.threads.extend(new.into_iter().filter_map(|(src, tgt)| Some((src?, tgt))));
        Ok(())
    }

    /// Calls of added messages: the target's own `call_log` row when it has the same `call_id`, else a mirrored copy.
    fn copy_calls(&mut self, added: &str) -> Result<()> {
        let db = self.db;
        if columns(db, "src", "message_call_log")?.is_empty() || columns(db, "src", "call_log")?.is_empty() {
            return Ok(());
        }
        let has_call_id = columns(db, "main", "call_log")?.iter().any(|c| c == "call_id")
            && columns(db, "src", "call_log")?.iter().any(|c| c == "call_id");
        let calls: Vec<(i64,)> = query_all(
            db,
            &format!("SELECT DISTINCT call_log_row_id FROM src.message_call_log WHERE message_row_id IN {added}"),
            [],
        )?;
        for (call,) in calls {
            let existing: Option<i64> = if has_call_id {
                db.query_row(
                    "SELECT t._id FROM main.call_log t JOIN src.call_log s ON s.call_id = t.call_id WHERE s._id = ?1",
                    [call],
                    |r| r.get(0),
                )
                .ok()
            } else {
                None
            };
            let new = match existing {
                Some(id) => {
                    self.report.calls_reused += 1;
                    id
                }
                None => match self.copy("call_log", &format!("_id = {call}"))?.first() {
                    Some((_, id)) => *id,
                    None => continue,
                },
            };
            self.calls.insert(call, new);
        }
        self.copy("message_call_log", &format!("message_row_id IN {added}"))?;
        Ok(())
    }

    /// Reactions on any merged message of the source chat that the target lacks (by `key_id`). Other add-ons
    /// (votes, pins…) are left behind.
    fn copy_reactions(&mut self, added: &str) -> Result<()> {
        let db = self.db;
        let tables = ["message_add_on", "message_add_on_reaction"];
        for schema in ["src", "main"] {
            for table in tables {
                if columns(db, schema, table)?.is_empty() {
                    return Ok(());
                }
            }
        }
        let others: u64 = db.query_row(
            &format!(
                "SELECT count(*) FROM src.message_add_on WHERE parent_message_row_id IN {added}
                 AND _id NOT IN (SELECT message_add_on_row_id FROM src.message_add_on_reaction)"
            ),
            [],
            |r| r.get(0),
        )?;
        if others > 0 {
            *self.report.left_behind.entry("message_add_on (not a reaction)".into()).or_default() += others;
        }
        let ids = self.copy(
            "message_add_on",
            &format!(
                "chat_row_id = {source} AND parent_message_row_id IN (SELECT src FROM temp.hilo_src)
                 AND _id IN (SELECT message_add_on_row_id FROM src.message_add_on_reaction)
                 AND key_id NOT IN (SELECT key_id FROM main.message_add_on WHERE chat_row_id = {chat} AND key_id IS NOT NULL)",
                source = self.source_chat,
                chat = self.chat
            ),
        )?;
        db.execute("CREATE TEMP TABLE hilo_add_on (src INTEGER PRIMARY KEY)", [])?;
        for (src, new) in ids {
            if let Some(src) = src {
                self.add_ons.insert(src, new);
                db.execute("INSERT INTO temp.hilo_add_on VALUES (?1)", [src])?;
            }
        }
        self.copy("message_add_on_reaction", "message_add_on_row_id IN (SELECT src FROM temp.hilo_add_on)")?;
        Ok(())
    }

    /// One `media_refs` row per new path; the target's own paths count their new messages too.
    fn media_refs(&mut self) -> Result<()> {
        let db = self.db;
        let cols = columns(db, "main", "media_refs")?;
        if !(cols.iter().any(|c| c == "path") && cols.iter().any(|c| c == "ref_count")) {
            return Ok(());
        }
        for (path, n) in &self.reused_paths {
            db.execute("UPDATE main.media_refs SET ref_count = ref_count + ?2 WHERE path = ?1", params![path, n])?;
        }
        for (path, n) in &self.new_paths {
            db.execute("INSERT INTO main.media_refs (path, ref_count) VALUES (?1, ?2)", params![path, n])?;
        }
        if !self.new_paths.is_empty() {
            *self.report.copied.entry("media_refs".into()).or_default() += self.new_paths.len() as u64;
        }
        Ok(())
    }

    /// Source rows about added messages in tables the merge doesn't copy (receipts, reporting…), per table.
    fn count_left_behind(&mut self, added: &str) -> Result<()> {
        let db = self.db;
        let mut by_table: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (table, column) in reference_columns(db, "src")? {
            if is_message_reference(&column)
                && !PER_MESSAGE.contains(&table.as_str())
                && !HANDLED.contains(&table.as_str())
            {
                by_table.entry(table).or_default().push(column);
            }
        }
        for (table, cols) in by_table {
            let cond = cols.iter().map(|c| format!("{} IN {added}", quote(c))).collect::<Vec<_>>().join(" OR ");
            let n: u64 =
                db.query_row(&format!("SELECT count(*) FROM src.{} WHERE {cond}", quote(&table)), [], |r| r.get(0))?;
            if n > 0 {
                *self.report.left_behind.entry(table).or_default() += n;
            }
        }
        Ok(())
    }
}

/// `Media/<folder>/Sent/x` ↔ `Media/<folder>/x` for folders with a `Sent/`, other folders kept; the name gets a
/// `hilo-` prefix. `None` for a path without `Media/` or one that tries to leave it.
fn mirror_path(path: &str) -> Option<String> {
    let at = path.find("Media/")? + "Media/".len();
    let (prefix, rel) = path.split_at(at);
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty() || *p == "." || *p == "..") {
        return None;
    }
    let (name, dirs) = parts.split_last()?;
    let mut dirs = dirs.to_vec();
    if dirs.len() == 2 && dirs[1] == "Sent" {
        dirs.pop();
    } else if dirs.len() == 1 && SENT_FOLDERS.contains(&dirs[0]) {
        dirs.push("Sent");
    }
    Some(format!("{prefix}{}/hilo-{name}", dirs.join("/")))
}

/// Step 8 of decision 0006, on the encrypted result against the original. Each failure names what, never a value.
fn verify(
    original: &[u8],
    sealed: &[u8],
    key: &Key,
    chat: i64,
    last_added: i64,
    report: &MergeReport,
) -> Result<Vec<(&'static str, String)>> {
    let fail = |what: String| Err(Error::MergeCheck(what));
    let merged = crypt15::decrypt(sealed, key).map_err(|e| Error::MergeCheck(format!("doesn't decrypt: {e}")))?;
    let merged_file = temp_copy(&merged)?;
    let original_file = temp_copy(original)?;
    let db = Connection::open_with_flags(
        merged_file.path(),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.execute("ATTACH DATABASE ?1 AS orig", [format!("file:{}?mode=ro", original_file.path().display())])?;
    let mut checks = vec![("decrypts with the target's key", "yes".to_owned())];

    let integrity: String = db.query_row("PRAGMA main.integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return fail("integrity_check".into());
    }
    checks.push(("integrity_check", integrity));

    let version = |schema: &str| db.query_row(&format!("PRAGMA {schema}.user_version"), [], |r| r.get::<_, i64>(0));
    if version("main")? != version("orig")? {
        return fail("user_version changed".into());
    }
    let schema_sql = |schema: &str| -> Result<Vec<(String, String, Option<String>)>> {
        query_all(&db, &format!("SELECT type, name, sql FROM {schema}.sqlite_master ORDER BY type, name"), [])
    };
    if schema_sql("main")? != schema_sql("orig")? {
        return fail("the schema (tables, indexes, triggers) changed".into());
    }
    checks.push(("schema and user_version", "unchanged".into()));

    let count = |sql: &str| db.query_row(sql, [chat], |r| r.get::<_, u64>(0));
    let after = count("SELECT count(*) FROM main.message WHERE chat_row_id = ?1")?;
    if after != report.target_before + report.added {
        return fail(format!("the chat has {after} messages, not {} + {}", report.target_before, report.added));
    }
    checks.push(("chat messages", format!("{} + {} = {after}", report.target_before, report.added)));

    let unsorted = |schema: &str| {
        db.query_row(&format!("SELECT count(*) FROM {schema}.message WHERE sort_id IS NOT _id"), [], |r| {
            r.get::<_, u64>(0)
        })
    };
    let (now, before) = (unsorted("main")?, unsorted("orig")?);
    if now != before {
        return fail(format!("{now} messages have sort_id unequal to _id (before: {before})"));
    }
    checks.push(("sort_id = _id", format!("every message (exceptions: {now}, as before)")));

    let mut dangling = 0;
    for (table, column) in reference_columns(&db, "main")? {
        let (t, c) = (quote(&table), quote(&column));
        let target = if is_sort_reference(&column) { "sort_id" } else { "_id" };
        let missing = |schema: &str| -> Result<u64> {
            Ok(db.query_row(
                &format!(
                    "SELECT count(*) FROM {schema}.{t} WHERE typeof({c}) = 'integer' AND {c} > 0
                     AND {c} NOT IN (SELECT {target} FROM {schema}.message WHERE {target} IS NOT NULL)"
                ),
                [],
                |r| r.get(0),
            )?)
        };
        let (now, before) = (missing("main")?, missing("orig")?);
        if now != before {
            return fail(format!("`{table}.{column}` refers to {now} missing messages (before: {before})"));
        }
        dangling += now;
    }
    checks.push(("references to missing messages", format!("none new ({dangling}, as before)")));

    let others = "SELECT * FROM {s}.message WHERE chat_row_id IS NOT ?1 ORDER BY _id";
    if digest(&db, &others.replace("{s}", "main"), &[&chat], true)?
        != digest(&db, &others.replace("{s}", "orig"), &[&chat], true)?
    {
        return fail("another chat's messages changed".into());
    }
    let content: Vec<String> = columns(&db, "main", "message")?
        .into_iter()
        .filter(|c| c != "_id" && c != "sort_id")
        .map(|c| quote(&c))
        .collect();
    let content = content.join(", ");
    let theirs_now =
        format!("SELECT {content} FROM main.message WHERE chat_row_id = ?1 AND _id > {last_added} ORDER BY _id");
    let theirs_before = format!("SELECT {content} FROM orig.message WHERE chat_row_id = ?1 ORDER BY _id");
    if digest(&db, &theirs_now, &[&chat], true)? != digest(&db, &theirs_before, &[&chat], true)? {
        return fail("the target chat's own messages changed or lost their order".into());
    }
    checks.push(("other chats and the chat's own messages", "identical, in order".into()));

    let referencing: HashSet<String> = reference_columns(&db, "main")?.into_iter().map(|(t, _)| t).collect();
    let mut identical = 0;
    for (table,) in tables(&db, "main")? {
        if table == "sqlite_sequence" {
            // Counters only grow; a table that never had a row gets its first one when rows are copied into it.
            let lowered: u64 = db.query_row(
                "SELECT count(*) FROM orig.sqlite_sequence o LEFT JOIN main.sqlite_sequence m ON m.name = o.name
                 WHERE m.seq IS NULL OR m.seq < o.seq",
                [],
                |r| r.get(0),
            )?;
            let new: Vec<(String,)> = query_all(
                &db,
                "SELECT name FROM main.sqlite_sequence WHERE name NOT IN (SELECT name FROM orig.sqlite_sequence)",
                [],
            )?;
            if lowered > 0 || new.iter().any(|(name,)| !report.copied.contains_key(name)) {
                return fail(
                    "`sqlite_sequence` lost or lowered a counter, or gained one for an untouched table".into(),
                );
            }
            continue;
        }
        let t = quote(&table);
        let rows =
            |schema: &str| db.query_row(&format!("SELECT count(*) FROM {schema}.{t}"), [], |r| r.get::<_, u64>(0));
        let expected = rows("orig")? + report.copied.get(&table).copied().unwrap_or(0);
        if rows("main")? != expected {
            return fail(format!("`{table}` has an unexpected number of rows"));
        }
        let written = report.copied.contains_key(&table)
            || ["message", "chat", "sqlite_sequence", "media_refs"].contains(&table.as_str());
        if !written && !referencing.contains(&table) {
            let all = format!("SELECT * FROM {{s}}.{t}");
            if digest(&db, &all.replace("{s}", "main"), &[], false)?
                != digest(&db, &all.replace("{s}", "orig"), &[], false)?
            {
                return fail(format!("`{table}` changed"));
            }
            identical += 1;
        }
    }
    if columns(&db, "main", "chat")?.iter().any(|c| c == "created_timestamp") {
        let late: u64 = db.query_row(
            "SELECT count(*) FROM main.chat WHERE _id = ?1 AND created_timestamp >
                 (SELECT min(timestamp) FROM main.message WHERE chat_row_id = ?1 AND timestamp > 0)",
            [chat],
            |r| r.get(0),
        )?;
        if late > 0 {
            return fail("the chat was created after its first message".into());
        }
        checks.push(("chat created_timestamp", "not after its first message".into()));
    }
    checks.push(("tables", format!("row counts as expected; {identical} untouched tables identical")));
    Ok(checks)
}

/// A hash of every value of every row `sql` returns, in order or as a set.
fn digest(db: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql], ordered: bool) -> Result<Vec<u8>> {
    let mut stmt = db.prepare(sql)?;
    let n = stmt.column_count();
    let mut rows = stmt.query(params)?;
    let mut hashes = Vec::new();
    while let Some(r) = rows.next()? {
        let mut h = Sha256::new();
        for i in 0..n {
            match r.get_ref(i)? {
                ValueRef::Null => h.update([0]),
                ValueRef::Integer(x) => {
                    h.update([1]);
                    h.update(x.to_le_bytes());
                }
                ValueRef::Real(x) => {
                    h.update([2]);
                    h.update(x.to_bits().to_le_bytes());
                }
                ValueRef::Text(b) | ValueRef::Blob(b) => {
                    h.update([3]);
                    h.update((b.len() as u64).to_le_bytes());
                    h.update(b);
                }
            }
        }
        hashes.push(h.finalize().to_vec());
    }
    if !ordered {
        hashes.sort();
    }
    let mut h = Sha256::new();
    for x in hashes {
        h.update(x);
    }
    Ok(h.finalize().to_vec())
}

/// A column holding a message's `_id`: `message_row_id`, or one ending in `_message_row_id`.
fn is_message_reference(column: &str) -> bool {
    column == "message_row_id" || column.ends_with("_message_row_id")
}

/// A column holding a message's `sort_id` (`chat.last_message_sort_id`, …).
fn is_sort_reference(column: &str) -> bool {
    column.ends_with("message_sort_id")
}

/// Every (table, column) in `schema` referring to a message by `_id` or `sort_id`.
fn reference_columns(db: &Connection, schema: &str) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for (table,) in tables(db, schema)? {
        for column in columns(db, schema, &table)? {
            if is_message_reference(&column) || is_sort_reference(&column) {
                out.push((table.clone(), column));
            }
        }
    }
    Ok(out)
}

/// Ordinary tables (not virtual ones) in `schema`.
fn tables(db: &Connection, schema: &str) -> Result<Vec<(String,)>> {
    query_all(
        db,
        &format!(
            "SELECT name FROM {schema}.sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_stat%'
             AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name"
        ),
        [],
    )
}

fn columns(db: &Connection, schema: &str, table: &str) -> Result<Vec<String>> {
    let mut stmt = db.prepare("SELECT name FROM pragma_table_info(?1, ?2)")?;
    let names = stmt.query_map([table, schema], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(names)
}

fn query_all<T: FromRow, P: rusqlite::Params>(db: &Connection, sql: &str, params: P) -> Result<Vec<T>> {
    let mut stmt = db.prepare(sql)?;
    let rows = stmt.query_map(params, T::from_row)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

trait FromRow: Sized {
    fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self>;
}

macro_rules! from_row {
    ($($t:ident $i:tt),+) => {
        impl<$($t: rusqlite::types::FromSql),+> FromRow for ($($t,)+) {
            fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
                Ok(($(r.get($i)?,)+))
            }
        }
    };
}
from_row!(A 0);
from_row!(A 0, B 1);
from_row!(A 0, B 1, C 2);
from_row!(A 0, B 1, C 2, D 3);
from_row!(A 0, B 1, C 2, D 3, E 4);
#[cfg(test)]
from_row!(A 0, B 1, C 2, D 3, E 4, F 5);
#[cfg(test)]
from_row!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8);

fn integer(v: &Value) -> Option<i64> {
    match v {
        Value::Integer(n) => Some(*n),
        _ => None,
    }
}

fn mapped(map: &HashMap<i64, i64>, n: Option<i64>, table: &str, column: &str) -> Result<i64> {
    n.and_then(|n| map.get(&n).copied())
        .ok_or_else(|| Error::Unsupported(format!("`{table}.{column}` points at a row that wasn't merged")))
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A private temporary file holding `bytes`, removed on drop.
fn temp_copy(bytes: &[u8]) -> Result<tempfile::NamedTempFile> {
    let io = |source| Error::Io { context: "writing a database to a temporary file".into(), source };
    let mut file = tempfile::NamedTempFile::new().map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.flush().map_err(io)?;
    Ok(file)
}

#[cfg(test)]
mod tests;
