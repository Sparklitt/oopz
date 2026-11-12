//! What the app has open: backups checked with their owner's key (decrypted into private temporary copies, removed
//! when forgotten), and the merge of a chat from one into a new copy of another. Keys are taken for one call and
//! never kept: oopz asks again when it needs one.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use oopz::{Backup, ChatMerge, Key};
use rusqlite::OptionalExtension;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::index::{BuildInfo, Filter, Hit, Index, Line, Stats};
use crate::messages::Message as Msg;
use crate::{AppError, Result};
use msgkinds::Note;
use oopz_model::messages::Message as ModelMsg;

/// A file in the folder the person picked, and what kind of backup it looks like.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BackupFile {
    pub path: PathBuf,
    pub name: String,
    pub bytes: u64,
    /// `crypt15` (oopz opens it), `crypt14` or `crypt12` (older: their key file is only readable with root, so oopz
    /// doesn't open them), `not_crypt15` (named like one, but its header doesn't read).
    pub kind: &'static str,
}

/// The `*.crypt15`, `*.crypt14` and `*.crypt12` files in `path` (a backup file, a `Databases/` folder, or the folder
/// holding `Databases/`), the current backup first, then the dated ones newest first. WhatsApp's other backups in a
/// folder (`wa.db.crypt15`, the contacts, and the like) aren't listed. Only reads.
pub fn find_backups(path: &Path) -> Result<Vec<BackupFile>> {
    let mut candidates = Vec::new();
    if path.is_file() {
        candidates.push(path.to_path_buf());
    } else {
        for dir in [path.to_path_buf(), path.join("Databases")] {
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for entry in entries {
                let p = entry.map_err(|e| AppError::io_at(ModelMsg::DoingReading, &dir, e))?.path();
                let other = oopz::is_other_whatsapp_backup(&p.file_name().unwrap_or_default().to_string_lossy());
                if p.is_file() && !other {
                    candidates.push(p);
                }
            }
        }
    }
    let mut found = Vec::new();
    for p in candidates {
        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let kind = match p.extension().and_then(|e| e.to_str()) {
            Some("crypt15") if looks_like_crypt15(&p) => "crypt15",
            Some("crypt15") => "not_crypt15",
            Some("crypt14") => "crypt14",
            Some("crypt12") => "crypt12",
            _ => continue,
        };
        let bytes = fs::metadata(&p).map_err(|e| AppError::io_at(ModelMsg::DoingReading, &p, e))?.len();
        found.push(BackupFile { path: p, name, bytes, kind });
    }
    if found.is_empty() {
        return Err(AppError::of("not_found", Msg::NoBackupsIn, [path.display().to_string()]));
    }
    // `msgstore.db.crypt15` is the newest; `msgstore-YYYY-MM-DD.1.db.crypt15` sort by their date, newest first.
    found.sort_by(|a, b| {
        let current = |f: &BackupFile| !f.name.starts_with("msgstore-");
        current(b).cmp(&current(a)).then_with(|| b.name.cmp(&a.name))
    });
    Ok(found)
}

fn looks_like_crypt15(path: &Path) -> bool {
    let mut head = Vec::with_capacity(512);
    fs::File::open(path).and_then(|f| f.take(512).read_to_end(&mut head)).is_ok() && oopz::crypt15::looks_like(&head)
}

/// Bytes free for this user on the disk holding `path` (or its nearest existing parent).
pub fn free_space(path: &Path) -> Result<u64> {
    let existing = path.ancestors().find(|p| p.exists()).unwrap_or(path);
    fs4::available_space(existing).map_err(|e| AppError::io_at(ModelMsg::DoingFreeSpaceIn, path, e))
}

/// Fails with `no_space`, writing nothing, when `need` bytes don't fit at `path`.
fn ensure_space(path: &Path, need: u64) -> Result<()> {
    let free = free_space(path)?;
    if free < need {
        let mb = |b: u64| b.div_ceil(1_000_000);
        let values = [mb(need).to_string(), path.display().to_string(), mb(free).to_string()];
        return Err(AppError::of("no_space", Msg::NeedsSpace, values));
    }
    Ok(())
}

/// SHA-256 of a file, streamed, as lowercase hex: to show that an original is unchanged.
pub fn sha256_file(path: &Path) -> Result<String> {
    sha256_file_with(path, &mut |_| true)
}

/// [`sha256_file`], calling `progress(bytes read)` after each megabyte.
fn sha256_file_with(path: &Path, progress: &mut dyn FnMut(u64) -> bool) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| AppError::io_at(ModelMsg::DoingReading, path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0;
    loop {
        let n = file.read(&mut buf).map_err(|e| AppError::io_at(ModelMsg::DoingReading, path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        if !progress(done) {
            return Err(oopz::Error::Cancelled.into());
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

/// The step a check is in, from its `progress(done, total)`: the original's fingerprint (the first quarter), then
/// reading, unpacking and preparing the private copy (a quarter each, as [`Backup::open_in_with`] reports them).
pub fn check_step(done: u64, total: u64) -> &'static str {
    match done.saturating_mul(4) / total.max(1) {
        0 => "fingerprint",
        1 => "read",
        2 => "unpack",
        _ => "prepare",
    }
}

/// What a checked backup holds: counts, dates and masked ids.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// This session's number for the backup, for the merge.
    pub id: u32,
    pub name: String,
    pub bytes: u64,
    /// Of the original file when it was checked; the merge checks it again.
    pub sha256: String,
    /// SQLite's quick check passed.
    pub sound: bool,
    /// What SQLite found wrong (table and page names, never values), empty when sound.
    pub problems: String,
    pub chats: u64,
    pub messages: u64,
    pub first_ms: Option<i64>,
    pub last_ms: Option<i64>,
    pub one_to_one: u64,
    pub groups: u64,
    pub chat_list: Vec<ChatRow>,
}

/// A chat, as the Check screen shows it: never its whole number.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChatRow {
    pub number: i64,
    /// `contact`, `contact_lid`, `group`, `broadcast` or `other`.
    pub kind: &'static str,
    /// The last 3 characters of its id.
    pub ending: String,
    pub messages: u64,
    pub last_ms: Option<i64>,
    /// A group's subject, or the contact's name from `wa.db.crypt15` beside the backup; the UI can hide it.
    pub name: Option<String>,
    /// The chat's last message, for the chat list.
    pub preview: Option<Preview>,
}

/// See [`ChatRow::preview`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Preview {
    pub kind: i64,
    pub from_me: bool,
    /// At most [`PREVIEW_CHARS`] characters of its text.
    pub text: Option<String>,
}

pub const PREVIEW_CHARS: usize = 80;

/// The largest file [`Session::thumbnail`] reads.
pub const THUMBNAIL_MAX_FILE: u64 = 64 << 20;
/// The widest or tallest picture [`Session::thumbnail`] decodes.
pub const THUMBNAIL_MAX_SIDE: u32 = 16_384;
/// The most memory one decode may take.
pub const THUMBNAIL_MAX_ALLOC: u64 = 512 << 20;

/// Merge `source_chat` of checked backup `source` into `target_chat` of checked backup `target`, writing a new
/// folder in `out_parent`; with `media` (a copy of the source phone's `Media/` folder), the files the result needs.
pub struct MergeRequest<'a> {
    pub source: u32,
    pub source_chat: i64,
    pub target: u32,
    pub target_chat: i64,
    /// Asked again: oopz never keeps a key.
    pub target_key: &'a str,
    pub out_parent: &'a Path,
    pub media: Option<&'a Path>,
}

/// What two backups' copies of a chat have in common, matched by WhatsApp's message id (`key_id`), never by time or
/// text. Counts and dates only.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Missing {
    pub both: u64,
    /// Messages only in the first backup's chat, and the dates of the first and last of them.
    pub only_a: u64,
    pub only_a_first_ms: Option<i64>,
    pub only_a_last_ms: Option<i64>,
    pub only_b: u64,
    pub only_b_first_ms: Option<i64>,
    pub only_b_last_ms: Option<i64>,
}

/// What a merge wrote and checked.
#[derive(Debug, Clone, Serialize)]
pub struct MergeOutcome {
    /// The new folder, laid out like WhatsApp's: `Databases/msgstore.db.crypt15` and, with media, `Media/`.
    pub folder: PathBuf,
    pub backup: PathBuf,
    pub sha256: String,
    pub target_before: u64,
    pub already_there: u64,
    pub added: u64,
    /// The checks the result passed before it was written (decision 0006).
    pub checks: Vec<String>,
    pub media_copied: Option<u64>,
    pub media_missing: Option<u64>,
    /// The target's original file hashes the same as when it was checked, after the merge.
    pub original_untouched: bool,
}

struct Checked {
    path: PathBuf,
    backup: Backup,
    summary: Summary,
    /// Built on first use (Explore), removed with the backup.
    index: Option<Index>,
}

/// The WhatsApp version that wrote a backup, from its unencrypted header (decision 0020); `None` when it doesn't say.
pub fn version_of(path: &Path) -> Option<String> {
    use std::io::Read as _;
    let mut head = Vec::new();
    std::fs::File::open(path).and_then(|f| f.take(4096).read_to_end(&mut head)).ok()?;
    oopz::crypt15::app_version(&head)
}

/// The most search results returned, newest first (the app and the CLI alike).
pub const SEARCH_LIMIT: u32 = 200;
/// The most chat lines one call returns.
pub const LINES_MAX: u32 = 500;

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeInfo {
    /// WhatsApp's `message_type`.
    pub code: i64,
    pub messages: u64,
    pub with_text: u64,
    /// (table, messages of this type with a row in it)
    pub linked: Vec<(String, u64)>,
    /// (media type, messages), most common first
    pub mimes: Vec<(String, u64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fact {
    pub label: &'static str,
    pub value: String,
}

/// Builds a backup's search index on its own read-only connection to the decrypted copy.
pub struct IndexJob {
    source: PathBuf,
    /// The WhatsApp version that wrote the backup, for msgkinds' lookups.
    version: Option<String>,
}

impl IndexJob {
    pub fn run(&self, tmp: &Path, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<Index> {
        let db = rusqlite::Connection::open_with_flags(&self.source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(oopz::Error::from)?;
        Ok(Index::build_with(&db, tmp, progress)?.at_version(self.version.clone()))
    }
}

/// The backups checked so far. Dropping it (or [`Session::forget`]) removes their decrypted copies.
#[derive(Default)]
pub struct Session {
    checked: Vec<(u32, Checked)>,
    next_id: u32,
}

impl Session {
    /// Opens `path` with `key_text` (64 hex digits, spaces allowed), decrypting it into `tmp` (a private folder),
    /// and keeps it open for a merge. The key is dropped, wiped, when this returns.
    pub fn check(&mut self, path: &Path, key_text: &str, tmp: &Path) -> Result<Summary> {
        self.check_with(path, key_text, tmp, &mut |_, _| true)
    }

    /// [`Session::check`], calling `progress(done, total)` as it hashes the original (the first quarter) and opens it
    /// (the rest; [`check_step`] names each part). `total` stays the same during one check. When `progress` returns
    /// false the check stops with a `cancelled` error, and what it decrypted so far is removed.
    pub fn check_with(
        &mut self,
        path: &Path,
        key_text: &str,
        tmp: &Path,
        progress: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<Summary> {
        let key = Key::from_hex(key_text)?;
        let bytes = fs::metadata(path).map_err(|e| AppError::io_at(ModelMsg::DoingReading, path, e))?.len();
        // The decrypted database is larger than the compressed backup, and the search index comes next to it: about
        // four times the backup in all (an estimate; WhatsApp's databases compress about 2-3 times).
        ensure_space(tmp, 4 * bytes)?;
        let total = 4 * bytes.max(1);
        let sha256 = sha256_file_with(path, &mut |done| progress(done.min(bytes), total))?;
        let backup = Backup::open_in_with(path, &key, tmp, &mut |done, of| {
            progress(bytes + done.min(of) * 3 * bytes.max(1) / of.max(1), total)
        })?;
        let health = backup.health()?;
        let first_ms = backup
            .db()
            .query_row("SELECT min(timestamp) FROM message WHERE timestamp > 0", [], |r| r.get(0))
            .map_err(oopz::Error::from)?;
        let chats = backup.chats()?;
        // Names: a group's own subject; a contact's from wa.db beside the backup, when it opens with the same key.
        let contacts = oopz::contacts::Contacts::beside(path, &key, tmp).ok();
        let mut previews = last_messages(backup.db()).map_err(oopz::Error::from)?;
        let mut chat_list = Vec::with_capacity(chats.len());
        for c in &chats {
            let name = match (&c.subject, &contacts) {
                (Some(s), _) if !s.trim().is_empty() => Some(s.trim().to_owned()),
                (_, Some(book)) if !c.is_group() => match book.name(c.jid())? {
                    Some(name) => Some(name),
                    // A chat under WhatsApp's private id (lid): its phone id, through jid_map, is the one in contacts.
                    None => match phone_jid(backup.db(), c.row_id).map_err(oopz::Error::from)? {
                        Some(jid) => book.name(&jid)?,
                        None => None,
                    },
                },
                _ => None,
            };
            chat_list.push(ChatRow {
                number: c.row_id,
                kind: kind(c),
                ending: c.ending(),
                messages: c.messages,
                last_ms: c.last_ms,
                name,
                preview: previews.remove(&c.row_id),
            });
        }
        self.next_id += 1;
        let summary = Summary {
            id: self.next_id,
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            bytes,
            sha256,
            sound: health.check == "ok",
            problems: if health.check == "ok" { String::new() } else { health.check },
            chats: health.chats,
            messages: health.messages,
            first_ms,
            last_ms: health.last_ms,
            one_to_one: chats.iter().filter(|c| c.is_one_to_one()).count() as u64,
            groups: chats.iter().filter(|c| c.is_group()).count() as u64,
            chat_list,
        };
        self.checked
            .push((self.next_id, Checked { path: path.to_path_buf(), backup, summary: summary.clone(), index: None }));
        Ok(summary)
    }

    pub fn summaries(&self) -> Vec<Summary> {
        self.checked.iter().map(|(_, c)| c.summary.clone()).collect()
    }

    /// Every id in the backup's `jid` table by row (whole ids: for the adapter, never for the UI).
    pub fn jids(&self, id: u32) -> Result<HashMap<i64, String>> {
        let db = self.get(id)?.backup.db();
        let mut stmt =
            db.prepare("SELECT _id, raw_string FROM jid WHERE raw_string IS NOT NULL").map_err(oopz::Error::from)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .and_then(|rows| rows.collect::<rusqlite::Result<HashMap<_, _>>>())
            .map_err(oopz::Error::from)?;
        Ok(rows)
    }

    /// The whole id of each chat in a checked backup, by its row number (for the adapter, never for the UI).
    pub fn chat_jids(&self, id: u32) -> Result<HashMap<i64, String>> {
        Ok(self.get(id)?.backup.chats()?.iter().map(|c| (c.row_id, c.jid().to_owned())).collect())
    }

    /// Closes a checked backup and removes its decrypted copy.
    pub fn forget(&mut self, id: u32) {
        self.checked.retain(|(i, _)| *i != id);
    }

    fn get(&self, id: u32) -> Result<&Checked> {
        self.checked
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, c)| c)
            .ok_or_else(|| AppError::of("not_found", Msg::BackupNotOpen, [id.to_string()]))
    }

    /// Builds the search index of a checked backup in `tmp`, once; later calls return the first build's numbers.
    pub fn build_index(&mut self, id: u32, tmp: &Path) -> Result<BuildInfo> {
        self.build_index_with(id, tmp, &mut |_, _| true)
    }

    /// [`Session::build_index`], calling `progress(done, total)` as [`Index::build_with`] does.
    pub fn build_index_with(
        &mut self,
        id: u32,
        tmp: &Path,
        progress: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<BuildInfo> {
        if let Some(info) = self.index_built(id)? {
            return Ok(info);
        }
        let index = self.index_job(id)?.run(tmp, progress)?;
        self.keep_index(id, index)
    }

    /// The index's numbers, if it's built.
    pub fn index_built(&self, id: u32) -> Result<Option<BuildInfo>> {
        Ok(self.get(id)?.index.as_ref().map(|i| i.info))
    }

    /// What building a backup's index needs, so the build can run without holding the session (the app stays usable
    /// meanwhile): [`IndexJob::run`], then [`Session::keep_index`].
    pub fn index_job(&self, id: u32) -> Result<IndexJob> {
        let path = self.get(id)?.backup.db().path().map(PathBuf::from);
        let source = path.ok_or_else(|| AppError::of("not_found", Msg::NoDecryptedFile, []))?;
        let checked = self.get(id)?;
        Ok(IndexJob { source, version: version_of(&checked.path) })
    }

    /// Keeps an index built by an [`IndexJob`]. If one was kept meanwhile, that one stays and this one is removed;
    /// if the backup was forgotten meanwhile, this one is removed and the answer is `not_found`.
    pub fn keep_index(&mut self, id: u32, index: Index) -> Result<BuildInfo> {
        let checked = self
            .checked
            .iter_mut()
            .find(|(i, _)| *i == id)
            .map(|(_, c)| c)
            .ok_or_else(|| AppError::of("not_found", Msg::BackupNotOpen, [id.to_string()]))?;
        Ok(checked.index.get_or_insert(index).info)
    }

    /// Every table with its columns and row count: structure only, for a problem report.
    pub fn tables(&self, id: u32) -> Result<Vec<TableInfo>> {
        let tables = self.get(id)?.backup.tables()?;
        Ok(tables.into_iter().map(|(name, columns, rows)| TableInfo { name, columns, rows }).collect())
    }

    /// For each message type in a chat: counts and the tables its rows link to. No content.
    pub fn types(&self, id: u32, chat: i64) -> Result<Vec<TypeInfo>> {
        let profile = self.get(id)?.backup.type_profile(chat)?;
        Ok(profile
            .into_iter()
            .map(|p| TypeInfo {
                code: p.code,
                messages: p.messages,
                with_text: p.with_text,
                linked: p.linked,
                mimes: p.mimes,
            })
            .collect())
    }

    /// How WhatsApp keeps one chat: ordering ids, senders, statuses, pointers. Counts and ranges only.
    pub fn chat_facts(&self, id: u32, chat: i64) -> Result<Vec<Fact>> {
        let facts = self.get(id)?.backup.chat_facts(chat)?;
        Ok(facts.into_iter().map(|(label, value)| Fact { label, value }).collect())
    }

    fn index(&self, id: u32) -> Result<&Index> {
        self.get(id)?.index.as_ref().ok_or_else(|| AppError::of("not_found", Msg::IndexNotBuilt, []))
    }

    pub fn search(&self, id: u32, query: &str, filter: &Filter, limit: u32) -> Result<Vec<Hit>> {
        self.index(id)?.search(query, filter, limit)
    }

    /// Where a message's file is in `media` (the phone's `Media/` folder, or the `WhatsApp` folder holding it), when
    /// the backup names one inside it.
    fn media_path(&self, id: u32, message: i64, media: &Path) -> Result<Option<PathBuf>> {
        let db = self.get(id)?.backup.db();
        let file_path: Option<String> = db
            .query_row("SELECT file_path FROM message_media WHERE message_row_id = ?1", [message], |r| r.get(0))
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                // An older WhatsApp without message_media, or a value of another type: no file to show.
                rusqlite::Error::SqliteFailure(..) | rusqlite::Error::InvalidColumnType(..) => Ok(None),
                e => Err(e),
            })
            .map_err(oopz::Error::from)?;
        let Some(rel) = file_path.as_deref().and_then(oopz::media_relative) else { return Ok(None) };
        let root = if media.join("Media").is_dir() { media.join("Media") } else { media.to_path_buf() };
        Ok(Some(root.join(rel)))
    }

    /// A sticker's or a voice note's file as it is (an animated WebP keeps its frames), at most `max` bytes; `None`
    /// when the backup names no file, it isn't there, or it's larger.
    pub fn media_file(&self, id: u32, message: i64, media: &Path, max: u64) -> Result<Option<Vec<u8>>> {
        let Some(path) = self.media_path(id, message, media)? else { return Ok(None) };
        match fs::metadata(&path) {
            Ok(m) if m.is_file() && m.len() <= max => {}
            _ => return Ok(None),
        }
        Ok(fs::read(&path).ok())
    }

    /// A small JPEG of a picture message's file, at most `max` pixels on its longer side, from `media` (the phone's
    /// `Media/` folder, or the `WhatsApp` folder holding it; only read). `None` when the backup names no file, the
    /// file isn't there, or it isn't a picture oopz reads (JPEG, PNG, WebP).
    pub fn thumbnail(&self, id: u32, message: i64, media: &Path, max: u32) -> Result<Option<Vec<u8>>> {
        let Some(path) = self.media_path(id, message, media)? else { return Ok(None) };
        // A picture this large isn't a photo WhatsApp sent; don't read it into memory.
        match fs::metadata(&path) {
            Ok(m) if m.is_file() && m.len() <= THUMBNAIL_MAX_FILE => {}
            _ => return Ok(None),
        }
        let Ok(mut reader) = image::ImageReader::open(&path).and_then(|r| r.with_guessed_format()) else {
            return Ok(None);
        };
        // A small file can claim a huge picture (a decompression bomb): refuse sizes no phone photo has.
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(THUMBNAIL_MAX_SIDE);
        limits.max_image_height = Some(THUMBNAIL_MAX_SIDE);
        limits.max_alloc = Some(THUMBNAIL_MAX_ALLOC);
        reader.limits(limits);
        let Ok(picture) = reader.decode() else { return Ok(None) };
        let small = picture.thumbnail(max.max(1), max.max(1)).into_rgb8();
        let mut out = std::io::Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80)
            .encode_image(&small)
            .map_err(|e| AppError::doing("io", Note::of(Msg::DoingThumbnail, []), e))?;
        Ok(Some(out.into_inner()))
    }

    /// The position in `chat` of its first message at or after `ts` (ms): "jump to a date".
    pub fn position_at(&self, id: u32, chat: i64, ts: i64) -> Result<Option<u64>> {
        self.index(id)?.position_at(chat, ts)
    }

    pub fn stats(&self, id: u32, chat: Option<i64>) -> Result<Stats> {
        self.index(id)?.stats(chat)
    }

    pub fn chat_len(&self, id: u32, chat: i64) -> Result<u64> {
        self.index(id)?.chat_len(chat)
    }

    pub fn lines(&self, id: u32, chat: i64, from: u64, count: u32) -> Result<Vec<Line>> {
        self.index(id)?.lines(chat, from, count)
    }

    pub fn position(&self, id: u32, message: i64) -> Result<Option<(i64, u64)>> {
        self.index(id)?.position(message)
    }

    /// Saves a checked backup's stats (and, with `chat`, that chat's messages) as a web page in a new file in `out_dir`,
    /// only when the person asks ([`crate::export`]).
    pub fn export(
        &self,
        id: u32,
        chat: Option<i64>,
        labels: &crate::export::Labels,
        out_dir: &Path,
    ) -> Result<PathBuf> {
        let checked = self.get(id)?;
        crate::export::write(self.index(id)?, &checked.summary.name, chat, labels, out_dir)
    }

    /// "What's missing": chat `chat_a` of backup `a` against chat `chat_b` of backup `b` (the same chat, as each
    /// phone or each day's backup keeps it). Works for one's own older backup and for the other person's.
    pub fn missing(&self, a: u32, chat_a: i64, b: u32, chat_b: i64) -> Result<Missing> {
        let ids_a = self.get(a)?.backup.key_ids(chat_a)?;
        let ids_b = self.get(b)?.backup.key_ids(chat_b)?;
        let set_a: std::collections::HashSet<&str> = ids_a.iter().map(|(k, _)| k.as_str()).collect();
        let set_b: std::collections::HashSet<&str> = ids_b.iter().map(|(k, _)| k.as_str()).collect();
        let mut out = Missing { both: set_a.intersection(&set_b).count() as u64, ..Missing::default() };
        // Oldest first, so the first one seen is the earliest. A time of 0 is unknown, not 1970.
        let span = |ids: &[(String, i64)], other: &std::collections::HashSet<&str>| {
            let only: Vec<i64> = ids.iter().filter(|(k, _)| !other.contains(k.as_str())).map(|&(_, t)| t).collect();
            let dated = only.iter().copied().filter(|&t| t > 0);
            (only.len() as u64, dated.clone().min(), dated.max())
        };
        (out.only_a, out.only_a_first_ms, out.only_a_last_ms) = span(&ids_a, &set_b);
        (out.only_b, out.only_b_first_ms, out.only_b_last_ms) = span(&ids_b, &set_a);
        Ok(out)
    }

    /// Merges a chat into a new copy of the target (decision 0006): the result is checked before it's written, both
    /// originals are only read, and the target's original is hashed again afterwards.
    pub fn merge(&self, req: &MergeRequest<'_>) -> Result<MergeOutcome> {
        if req.source == req.target {
            return Err(AppError::of("unsupported", Msg::SameBackup, []));
        }
        let source = self.get(req.source)?;
        let target = self.get(req.target)?;
        let target_key = Key::from_hex(req.target_key)?;
        let original = fs::read(&target.path).map_err(|e| AppError::io_at(ModelMsg::DoingReading, &target.path, e))?;
        if hex::encode(Sha256::digest(&original)) != target.summary.sha256 {
            return Err(AppError::of("changed", Msg::TargetChanged, []));
        }
        let merged = ChatMerge {
            source: &source.backup,
            source_chat: req.source_chat,
            target: &original,
            target_key: &target_key,
            target_chat: req.target_chat,
        }
        .run()?;
        drop(original);

        // The exact size: the merged backup and the media files it needs (those the source folder has).
        let media_bytes: u64 = req.media.map_or(0, |dir| {
            merged
                .media
                .iter()
                .filter_map(|f| oopz::media_relative(&f.from))
                .filter_map(|rel| fs::metadata(dir.join(rel)).ok())
                .map(|m| m.len())
                .sum()
        });
        ensure_space(req.out_parent, merged.backup.len() as u64 + media_bytes)?;
        let folder = new_folder(req.out_parent)?;
        let databases = folder.join("Databases");
        fs::create_dir(&databases).map_err(|e| AppError::io_at(ModelMsg::DoingMaking, &databases, e))?;
        let backup = databases.join("msgstore.db.crypt15");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
            .and_then(|mut f| std::io::Write::write_all(&mut f, &merged.backup))
            .map_err(|e| AppError::io_at(ModelMsg::DoingWriting, &backup, e))?;
        let sha256 = sha256_file(&backup)?;
        if sha256 != hex::encode(Sha256::digest(&merged.backup)) {
            return Err(AppError::of("io", Msg::WrittenDiffers, []));
        }
        let (mut media_copied, mut media_missing) = (None, None);
        if let Some(media) = req.media {
            let report = oopz::copy_media(&merged.media, media, &folder.join("Media"))?;
            media_copied = Some(report.copied);
            media_missing = Some(report.missing);
        }
        let r = &merged.report;
        Ok(MergeOutcome {
            folder,
            backup,
            sha256,
            target_before: r.target_before,
            already_there: r.already_there,
            added: r.added,
            checks: r.checks.iter().map(|(name, value)| format!("{name}: {value}")).collect(),
            media_copied,
            media_missing,
            original_untouched: sha256_file(&target.path)? == target.summary.sha256,
        })
    }
}

/// The phone-number id behind a chat under a private id (`…@lid`), when the backup maps it (`jid_map`).
fn phone_jid(db: &rusqlite::Connection, chat: i64) -> rusqlite::Result<Option<String>> {
    if !oopz::schema::Schema::has_columns(db, "jid_map", &["lid_row_id", "jid_row_id"]).unwrap_or(false) {
        return Ok(None);
    }
    db.query_row(
        "SELECT j.raw_string FROM chat c JOIN jid_map m ON m.lid_row_id = c.jid_row_id JOIN jid j ON j._id = m.jid_row_id
         WHERE c._id = ?1 AND typeof(j.raw_string) = 'text'",
        [chat],
        |r| r.get(0),
    )
    .optional()
}

/// Each chat's newest message (by time, then row), its text cut to [`PREVIEW_CHARS`]: one pass over the messages.
fn last_messages(db: &rusqlite::Connection) -> rusqlite::Result<std::collections::HashMap<i64, Preview>> {
    let mut stmt = db.prepare(
        "SELECT chat_row_id, coalesce(message_type, 0), coalesce(from_me, 0), text_data,
             max(coalesce(timestamp, 0) * 1000000 + _id)
         FROM message WHERE chat_row_id IS NOT NULL GROUP BY chat_row_id",
    )?;
    let rows = stmt.query_map([], |r| {
        let text: Option<String> = r.get(3).or_else(|_| Ok::<_, rusqlite::Error>(None))?;
        let text = text.map(|t| t.chars().take(PREVIEW_CHARS).collect::<String>()).filter(|t| !t.is_empty());
        Ok((r.get(0)?, Preview { kind: r.get(1)?, from_me: r.get(2)?, text }))
    })?;
    rows.collect()
}

fn kind(chat: &oopz::Chat) -> &'static str {
    match chat.jid().rsplit_once('@').map(|(_, server)| server) {
        Some("s.whatsapp.net") => "contact",
        Some("lid") => "contact_lid",
        Some("g.us") => "group",
        Some("broadcast") => "broadcast",
        _ => "other",
    }
}

/// `oopz-result-N` in `parent`, the first N that doesn't exist: never an existing folder.
fn new_folder(parent: &Path) -> Result<PathBuf> {
    for n in 1..1000 {
        let folder = parent.join(format!("oopz-result-{n}"));
        match fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(AppError::io_at(ModelMsg::DoingMaking, &folder, e)),
        }
    }
    Err(AppError::of("exists", Msg::TooManyFolders, ["oopz-result".to_owned()]))
}

#[cfg(test)]
mod tests {
    use oopz::fixture::{self, BEN_CHAT, KEY_HEX, SOURCE_CHAT, TARGET_CHAT, TARGET_KEY_HEX};

    use super::*;

    #[test]
    fn finds_backups_current_first_and_names_old_formats() {
        let fx = fixture::build();
        let dir = fx.dir.path().join(msgkinds::App::WhatsApp.name());
        let db = dir.join("Databases");
        fs::create_dir_all(&db).unwrap();
        fs::copy(&fx.backup, db.join("msgstore.db.crypt15")).unwrap();
        fs::copy(&fx.backup, db.join("msgstore-2026-10-29.1.db.crypt15")).unwrap();
        fs::copy(&fx.backup, db.join("msgstore-2026-10-30.1.db.crypt15")).unwrap();
        fs::write(db.join("msgstore-2021-01-01.1.db.crypt14"), b"old").unwrap();
        fs::write(db.join("broken.crypt15"), b"").unwrap();
        fs::write(db.join("notes.txt"), b"not a backup").unwrap();
        // The contacts and stickers backups hold no messages: not listed, and never taken for the current backup.
        fs::copy(&fx.backup, db.join("wa.db.crypt15")).unwrap();
        fs::copy(&fx.backup, db.join("stickers.db.crypt15")).unwrap();

        // The WhatsApp folder or its Databases folder, the same list.
        for path in [&dir, &db] {
            let found = find_backups(path).unwrap();
            let names: Vec<_> = found.iter().map(|f| (f.name.as_str(), f.kind)).collect();
            assert_eq!(
                names,
                [
                    ("msgstore.db.crypt15", "crypt15"),
                    ("broken.crypt15", "not_crypt15"),
                    ("msgstore-2026-10-30.1.db.crypt15", "crypt15"),
                    ("msgstore-2026-10-29.1.db.crypt15", "crypt15"),
                    ("msgstore-2021-01-01.1.db.crypt14", "crypt14"),
                ]
            );
        }
        assert_eq!(find_backups(&db.join("msgstore.db.crypt15")).unwrap().len(), 1);
        assert_eq!(find_backups(fx.dir.path().join("nowhere").as_path()).unwrap_err().code, "not_found");
    }

    /// Progress only moves forward, keeps one total, and ends at it.
    fn assert_progress(seen: &[(u64, u64)]) {
        let (_, total) = *seen.last().expect("some progress");
        assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0 && w[0].1 == w[1].1), "{seen:?}");
        assert!(seen.iter().all(|&(done, of)| done <= of));
        assert_eq!(seen.last().unwrap().0, total, "{seen:?}");
    }

    #[test]
    fn free_space_reads_through_missing_folders_and_refuses_what_wont_fit() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(free_space(&tmp.path().join("not/yet")).unwrap() > 0);
        assert!(ensure_space(tmp.path(), 1).is_ok());
        assert_eq!(ensure_space(tmp.path(), u64::MAX).unwrap_err().code, "no_space");
    }

    #[test]
    fn check_and_index_report_progress() {
        let fx = fixture::build();
        let tmp = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        let mut seen = Vec::new();
        let s = session
            .check_with(&fx.backup, KEY_HEX, tmp.path(), &mut |d, t| {
                seen.push((d, t));
                true
            })
            .unwrap();
        assert_progress(&seen);
        seen.clear();
        session
            .build_index_with(s.id, tmp.path(), &mut |d, t| {
                seen.push((d, t));
                true
            })
            .unwrap();
        assert_progress(&seen);

        // A backup forgotten while its index was building keeps nothing.
        let again = session.check(&fx.backup, KEY_HEX, tmp.path()).unwrap();
        let index = session.index_job(again.id).unwrap().run(tmp.path(), &mut |_, _| true).unwrap();
        session.forget(again.id);
        assert_eq!(session.keep_index(again.id, index).unwrap_err().code, "not_found");
    }

    #[test]
    fn check_summarises_counts_and_masked_ids_only() {
        let fx = fixture::build();
        let tmp = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        let s = session.check(&fx.backup, KEY_HEX, tmp.path()).unwrap();
        assert!(s.sound, "{}", s.problems);
        assert_eq!((s.chats, s.one_to_one, s.groups), (2, 1, 1));
        let ben = s.chat_list.iter().find(|c| c.number == BEN_CHAT).unwrap();
        assert_eq!((ben.kind, ben.ending.as_str(), ben.messages), ("contact", "001", 18));
        assert!(s.first_ms.unwrap() <= s.last_ms.unwrap());
        assert_eq!(s.sha256, sha256_file(&fx.backup).unwrap());

        // What the UI receives, as JSON: never a whole number, the key or a message's text.
        let json = format!("{s:?}");
        for secret in [fixture::BEN_JID, KEY_HEX, "Hola", "playa"] {
            assert!(!json.contains(secret), "{secret:?} in the summary");
        }
        // Its decrypted copy and its search index are in `tmp`, and go when it's forgotten.
        assert_eq!(session.search(s.id, "hola", &Filter::default(), 10).unwrap_err().code, "not_found");
        let info = session.build_index(s.id, tmp.path()).unwrap();
        assert_eq!(info.messages, s.messages);
        assert!(
            !session
                .search(s.id, "hola", &Filter { chat: Some(BEN_CHAT), ..Default::default() }, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(session.stats(s.id, Some(BEN_CHAT)).unwrap().messages, 18);
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 2);
        session.forget(s.id);
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn check_names_a_wrong_key_a_bad_key_and_damage() {
        let fx = fixture::build();
        let tmp = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        let err = |s: &mut Session, path: &Path, key: &str| s.check(path, key, tmp.path()).unwrap_err();
        assert_eq!(err(&mut session, &fx.backup, TARGET_KEY_HEX).code, "wrong_key");
        assert_eq!(err(&mut session, &fx.backup, "1234").code, "key_format");

        let cut = fx.dir.path().join("cut.crypt15");
        let bytes = fs::read(&fx.backup).unwrap();
        fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
        let e = err(&mut session, &cut, KEY_HEX);
        assert_eq!(e.code, "wrong_key", "a cut-off backup fails like a wrong key: {e}");
        assert!(!e.to_string().contains(KEY_HEX));
        assert!(session.summaries().is_empty());
    }

    #[test]
    fn merge_writes_a_new_folder_and_leaves_the_originals() {
        let two = fixture::two_phones();
        let tmp = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        let source = session.check(&two.source, KEY_HEX, tmp.path()).unwrap();
        let target = session.check(&two.target, TARGET_KEY_HEX, tmp.path()).unwrap();
        let req = MergeRequest {
            source: source.id,
            source_chat: SOURCE_CHAT,
            target: target.id,
            target_chat: TARGET_CHAT,
            target_key: TARGET_KEY_HEX,
            out_parent: out.path(),
            media: Some(&two.source_media),
        };

        // What's missing, before: what the merge will add and skip.
        let before = session.missing(source.id, SOURCE_CHAT, target.id, TARGET_CHAT).unwrap();

        let done = session.merge(&req).unwrap();
        assert_eq!((before.only_a, before.both), (done.added, done.already_there), "{before:?}");
        assert!(before.only_a_first_ms <= before.only_a_last_ms && before.only_a_first_ms.is_some());
        assert!(done.original_untouched);
        assert!(done.added > 0 && done.already_there > 0, "{done:?}");
        assert!(!done.checks.is_empty());
        assert_eq!(done.folder, out.path().join("oopz-result-1"));
        assert_eq!(done.backup, done.folder.join("Databases/msgstore.db.crypt15"));
        assert!(done.media_copied.unwrap() > 0);
        assert_eq!(sha256_file(&two.target).unwrap(), target.sha256);
        assert_eq!(sha256_file(&two.source).unwrap(), source.sha256);

        // The result opens with the target's key and has the added messages.
        let mut again = Session::default();
        let merged = again.check(&done.backup, TARGET_KEY_HEX, tmp.path()).unwrap();
        let chat = merged.chat_list.iter().find(|c| c.number == TARGET_CHAT).unwrap();
        assert_eq!(chat.messages, done.target_before + done.added);
        // And after, nothing in the source's chat is missing from the result.
        let source_again = again.check(&two.source, KEY_HEX, tmp.path()).unwrap();
        let after = again.missing(source_again.id, SOURCE_CHAT, merged.id, TARGET_CHAT).unwrap();
        assert_eq!((after.only_a, after.only_b), (0, before.only_b), "{after:?}");

        // A second merge goes into a new folder, never over the first.
        let second = session.merge(&MergeRequest { media: None, ..req }).unwrap();
        assert_eq!(second.folder, out.path().join("oopz-result-2"));
        assert_eq!(fs::read(&done.backup).unwrap().len() as u64, fs::metadata(&done.backup).unwrap().len());
    }

    #[test]
    fn merge_refuses_a_wrong_key_the_same_backup_and_a_changed_original() {
        let two = fixture::two_phones();
        let tmp = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        let source = session.check(&two.source, KEY_HEX, tmp.path()).unwrap();
        let target = session.check(&two.target, TARGET_KEY_HEX, tmp.path()).unwrap();
        let req = MergeRequest {
            source: source.id,
            source_chat: SOURCE_CHAT,
            target: target.id,
            target_chat: TARGET_CHAT,
            target_key: KEY_HEX,
            out_parent: out.path(),
            media: None,
        };
        assert_eq!(session.merge(&req).unwrap_err().code, "wrong_key");
        assert_eq!(session.merge(&MergeRequest { target: source.id, ..req }).unwrap_err().code, "unsupported");
        assert_eq!(session.merge(&MergeRequest { target: 99, ..req }).unwrap_err().code, "not_found");

        // Our test's own copy changed after the check (never an original): refused.
        let mut bytes = fs::read(&two.target).unwrap();
        bytes.push(0);
        fs::write(&two.target, bytes).unwrap();
        let req = MergeRequest { target_key: TARGET_KEY_HEX, ..req };
        assert_eq!(session.merge(&req).unwrap_err().code, "changed");
        assert_eq!(fs::read_dir(out.path()).unwrap().count(), 0, "nothing written");
    }

    #[test]
    fn a_thumbnail_comes_from_the_media_folder_and_only_for_a_picture() {
        let fx = fixture::build();
        let tmp = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        let s = session.check(&fx.backup, KEY_HEX, tmp.path()).unwrap();
        // Message 3 names Media/WhatsApp Images/IMG-00000001-WA0001.jpg; put a real 1000 x 500 picture there.
        let media = tempfile::tempdir().unwrap();
        let file = media.path().join("Media/WhatsApp Images/IMG-00000001-WA0001.jpg");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        image::RgbImage::from_pixel(1000, 500, image::Rgb([20, 120, 60]))
            .save_with_format(&file, image::ImageFormat::Png)
            .unwrap();
        // The WhatsApp folder or its Media folder both work; the result is a JPEG at most 256 px wide.
        for root in [media.path().to_path_buf(), media.path().join("Media")] {
            let jpeg = session.thumbnail(s.id, 3, &root, 256).unwrap().expect("a thumbnail");
            let small = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
            assert_eq!((small.width(), small.height()), (256, 128));
        }
        // A picture claiming 20000 x 20000 pixels is refused before it's decoded.
        image::RgbImage::new(20_000, 1).save_with_format(&file, image::ImageFormat::Png).unwrap();
        assert_eq!(session.thumbnail(s.id, 3, media.path(), 256).unwrap(), None);
        // Not a picture, no file, no media row, a message that doesn't exist: nothing, never an error.
        fs::write(&file, b"not a picture").unwrap();
        assert_eq!(session.thumbnail(s.id, 3, media.path(), 256).unwrap(), None);
        assert_eq!(session.thumbnail(s.id, 5, media.path(), 256).unwrap(), None);
        assert_eq!(session.thumbnail(s.id, 1, media.path(), 256).unwrap(), None);
        assert_eq!(session.thumbnail(s.id, -7, media.path(), 256).unwrap(), None);
    }

    #[test]
    fn a_media_file_comes_as_it_is_up_to_a_size() {
        let fx = fixture::build();
        let tmp = tempfile::tempdir().unwrap();
        let mut session = Session::default();
        let s = session.check(&fx.backup, KEY_HEX, tmp.path()).unwrap();
        let media = tempfile::tempdir().unwrap();
        let file = media.path().join("Media/WhatsApp Images/IMG-00000001-WA0001.jpg");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        // An animated WebP: the chunk whose flags can say it moves, with them saying so (msgkinds' fixtures).
        let (moving, _) =
            msgkinds::media_type::signatures::WEBP_CHUNKS.into_iter().find(|(_, bit)| bit.is_some()).unwrap();
        fs::write(&file, msgkinds::media_type::fixtures::webp(moving, true)).unwrap();
        assert_eq!(session.media_file(s.id, 3, media.path(), 1 << 20).unwrap().unwrap(), fs::read(&file).unwrap());
        assert_eq!(session.media_file(s.id, 3, media.path(), 4).unwrap(), None, "larger than asked");
        assert_eq!(session.media_file(s.id, 1, media.path(), 1 << 20).unwrap(), None, "no file named");
    }

    #[test]
    fn a_private_id_chat_finds_its_phone_id_through_jid_map() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        assert_eq!(phone_jid(&db, 1).unwrap(), None, "no jid_map: no name, no error");
        db.execute_batch(
            "CREATE TABLE jid (_id INTEGER PRIMARY KEY, raw_string TEXT);
             CREATE TABLE chat (_id INTEGER PRIMARY KEY, jid_row_id INTEGER);
             CREATE TABLE jid_map (lid_row_id INTEGER PRIMARY KEY, jid_row_id INTEGER);
             INSERT INTO jid VALUES (1, '888800000001@lid'), (2, '999900000001@s.whatsapp.net'), (3, '888800000002@lid');
             INSERT INTO chat VALUES (1, 1), (2, 3);
             INSERT INTO jid_map VALUES (1, 2);",
        )
        .unwrap();
        assert_eq!(phone_jid(&db, 1).unwrap().as_deref(), Some("999900000001@s.whatsapp.net"));
        assert_eq!(phone_jid(&db, 2).unwrap(), None, "not mapped");
        assert_eq!(phone_jid(&db, 9).unwrap(), None, "no such chat");
    }
}
