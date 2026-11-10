//! `msgstore.db`: the tables oopz reads and merges, shaped like a current Android backup (decision 0006's measured
//! columns), and the rows for each message.

use std::collections::VecDeque;

use anyhow::Result;
use rusqlite::{Connection, params};

use crate::corpus;
use crate::media::{File, Media, MediaRow};
use crate::rng::{Rng, mix};
use crate::spec::{Kind, Spec};
use crate::world::{ChatKind, World};

pub const SCHEMA: &str = "
CREATE TABLE props (_id INTEGER PRIMARY KEY AUTOINCREMENT, key TEXT UNIQUE, value TEXT);
CREATE TABLE jid (_id INTEGER PRIMARY KEY AUTOINCREMENT, user TEXT NOT NULL, server TEXT NOT NULL, agent INTEGER,
    device INTEGER, type INTEGER, raw_string TEXT);
CREATE UNIQUE INDEX jid_key_index ON jid (raw_string);
CREATE TABLE jid_map (lid_row_id INTEGER PRIMARY KEY, jid_row_id INTEGER);
CREATE TABLE chat (_id INTEGER PRIMARY KEY AUTOINCREMENT, jid_row_id INTEGER UNIQUE, hidden INTEGER, subject TEXT,
    created_timestamp INTEGER, display_message_row_id INTEGER, last_message_row_id INTEGER,
    last_read_message_row_id INTEGER, last_read_receipt_sent_message_row_id INTEGER,
    last_important_message_row_id INTEGER, archived INTEGER, sort_timestamp INTEGER, group_type INTEGER,
    last_message_reaction_row_id INTEGER, last_read_message_sort_id INTEGER, display_message_sort_id INTEGER,
    last_message_sort_id INTEGER, last_read_receipt_sent_message_sort_id INTEGER);
CREATE TABLE message (_id INTEGER PRIMARY KEY AUTOINCREMENT, chat_row_id INTEGER NOT NULL, from_me INTEGER NOT NULL,
    key_id TEXT, sender_jid_row_id INTEGER, status INTEGER, broadcast INTEGER, recipient_count INTEGER,
    origination_flags INTEGER, origin INTEGER, timestamp INTEGER, received_timestamp INTEGER,
    receipt_server_timestamp INTEGER, message_type INTEGER, text_data TEXT, starred INTEGER, lookup_tables INTEGER,
    message_add_on_flags INTEGER, sort_id INTEGER NOT NULL DEFAULT 0, view_mode INTEGER DEFAULT 0,
    translated_text TEXT, view_replies_thread_id INTEGER);
CREATE TABLE message_media (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER, autotransfer_retry_enabled INTEGER,
    multicast_id TEXT, media_job_uuid TEXT, transferred INTEGER, transcoded INTEGER, file_path TEXT, file_size INTEGER,
    suspicious_content INTEGER, trim_from INTEGER, trim_to INTEGER, face_x INTEGER, face_y INTEGER, media_key BLOB,
    media_key_timestamp INTEGER, width INTEGER, height INTEGER, has_streaming_sidecar INTEGER, gif_attribution INTEGER,
    thumbnail_height_width_ratio REAL, direct_path TEXT, first_scan_sidecar BLOB, first_scan_length INTEGER,
    message_url TEXT, mime_type TEXT, file_length INTEGER, media_name TEXT, file_hash TEXT, media_duration INTEGER,
    page_count INTEGER, enc_file_hash TEXT, partial_media_hash TEXT, partial_media_enc_hash TEXT, is_animated_sticker
    INTEGER, original_file_hash TEXT, mute_video INTEGER, media_caption TEXT, is_offloaded INTEGER DEFAULT 0);
CREATE TABLE message_quoted (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER,
    parent_message_chat_row_id INTEGER, from_me INTEGER, sender_jid_row_id INTEGER, key_id TEXT, timestamp INTEGER,
    message_type INTEGER, text_data TEXT);
CREATE TABLE message_forwarded (message_row_id INTEGER PRIMARY KEY, forward_score INTEGER, forward_origin INTEGER);
CREATE TABLE message_edit_info (message_row_id INTEGER PRIMARY KEY, original_key_id TEXT, edited_timestamp INTEGER,
    sender_timestamp INTEGER);
CREATE TABLE message_text (message_row_id INTEGER PRIMARY KEY, description TEXT, page_title TEXT, url TEXT,
    font_style INTEGER, text_color INTEGER, background_color INTEGER, preview_type INTEGER);
CREATE TABLE message_association (_id INTEGER PRIMARY KEY AUTOINCREMENT, child_message_row_id INTEGER,
    parent_message_row_id INTEGER, association_type INTEGER);
CREATE TABLE message_album (message_row_id INTEGER PRIMARY KEY, image_count INTEGER, video_count INTEGER);
CREATE TABLE message_system (message_row_id INTEGER PRIMARY KEY, action_type INTEGER);
CREATE TABLE message_system_chat_participant (_id INTEGER PRIMARY KEY AUTOINCREMENT, message_row_id INTEGER,
    user_jid_row_id INTEGER);
CREATE TABLE message_system_value_change (message_row_id INTEGER PRIMARY KEY, old_data TEXT);
CREATE TABLE call_log (_id INTEGER PRIMARY KEY AUTOINCREMENT, jid_row_id INTEGER, from_me INTEGER, call_id TEXT,
    transaction_id INTEGER, timestamp INTEGER, video_call INTEGER, duration INTEGER, call_result INTEGER,
    bytes_transferred INTEGER, group_jid_row_id INTEGER, is_joinable_group_call INTEGER,
    call_creator_device_jid_row_id INTEGER);
CREATE TABLE message_call_log (message_row_id INTEGER PRIMARY KEY, call_log_row_id INTEGER);
CREATE TABLE message_add_on (_id INTEGER PRIMARY KEY AUTOINCREMENT, chat_row_id INTEGER NOT NULL,
    from_me INTEGER NOT NULL, key_id TEXT NOT NULL, sender_jid_row_id INTEGER, parent_message_row_id INTEGER,
    timestamp INTEGER, status INTEGER, message_add_on_type INTEGER);
CREATE TABLE message_add_on_reaction (message_add_on_row_id INTEGER PRIMARY KEY, reaction TEXT,
    sender_timestamp INTEGER);
CREATE TABLE message_location (message_row_id INTEGER PRIMARY KEY, chat_row_id INTEGER, latitude REAL,
    longitude REAL, place_name TEXT, place_address TEXT, url TEXT, live_location_share_duration INTEGER,
    live_location_sequence_number INTEGER, live_location_final_latitude REAL, live_location_final_longitude REAL,
    live_location_final_timestamp INTEGER, map_download_status INTEGER);
CREATE TABLE message_vcard (_id INTEGER PRIMARY KEY AUTOINCREMENT, message_row_id INTEGER, vcard TEXT);
CREATE TABLE message_poll (message_row_id INTEGER PRIMARY KEY, poll_type INTEGER, content_type INTEGER,
    selectable_options_count INTEGER);
CREATE TABLE message_poll_option (_id INTEGER PRIMARY KEY AUTOINCREMENT, message_row_id INTEGER, option_name TEXT,
    vote_total INTEGER);
CREATE TABLE message_add_on_poll_vote (message_add_on_row_id INTEGER PRIMARY KEY, sender_timestamp INTEGER);
CREATE TABLE message_add_on_poll_vote_selected_option (_id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_add_on_row_id INTEGER, message_poll_option_id INTEGER);
CREATE TABLE message_sticker_pack (message_row_id INTEGER PRIMARY KEY, sticker_pack_id TEXT, name TEXT,
    publisher TEXT);
CREATE TABLE group_participant_user (_id INTEGER PRIMARY KEY AUTOINCREMENT, group_jid_row_id INTEGER,
    user_jid_row_id INTEGER, rank INTEGER, pending INTEGER);
CREATE TABLE group_past_participant_user (_id INTEGER PRIMARY KEY AUTOINCREMENT, group_jid_row_id INTEGER,
    user_jid_row_id INTEGER, is_leave INTEGER, timestamp INTEGER);
CREATE TABLE receipt_user (_id INTEGER PRIMARY KEY AUTOINCREMENT, message_row_id INTEGER,
    receipt_user_jid_row_id INTEGER, receipt_timestamp INTEGER, read_timestamp INTEGER, played_timestamp INTEGER);
";

/// Built once the rows are in: faster than keeping them up to date row by row.
const INDEXES: &str = "
CREATE UNIQUE INDEX message_key_index ON message (chat_row_id, from_me, key_id);
CREATE INDEX message_sort_index ON message (chat_row_id, sort_id);
CREATE INDEX message_add_on_parent ON message_add_on (parent_message_row_id);
CREATE INDEX message_association_child ON message_association (child_message_row_id);
";

/// WhatsApp's system `action_type`s the generator writes.
mod action {
    pub const ENCRYPTED: i64 = 18;
    pub const RENAMED: i64 = 1;
    pub const LEFT: i64 = 5;
    pub const ICON: i64 = 6;
    pub const CREATED: i64 = 11;
    pub const ADDED: i64 = 12;
    pub const REMOVED: i64 = 14;
    pub const ADMIN: i64 = 15;
    pub const DESCRIPTION: i64 = 27;
    /// Not a code WhatsApp uses: a reader must show it as unknown, not fail.
    pub const UNKNOWN: i64 = 9_999;
}

/// A message a later one may reply to or react to.
#[derive(Clone)]
struct Recent {
    key: String,
    from_me: bool,
    sender: i64,
    ts: i64,
    code: i64,
    text: Option<String>,
}

/// What one chat's writer remembers between messages: little, so a million-message chat needs no more memory.
pub struct ChatState {
    pub rng: Rng,
    pub row: i64,
    jid_row: i64,
    members: Vec<usize>,
    group: bool,
    recent: VecDeque<Recent>,
    /// The last author, to write runs of messages by the same person.
    last_author: usize,
    pub last_message: i64,
    pub last_ts: i64,
    /// For `sort_id`, which WhatsApp keeps in time order within a chat.
    pub written: u64,
}

/// Counts for the manifest.
#[derive(Default)]
pub struct Counts {
    pub messages: u64,
    pub by_kind: std::collections::BTreeMap<&'static str, u64>,
    pub system: u64,
    pub reactions: u64,
    pub replies: u64,
    pub edits: u64,
    pub forwarded: u64,
    pub starred: u64,
    pub poll_votes: u64,
    pub orphans: u64,
}

pub struct Writer<'w> {
    db: &'w Connection,
    spec: &'w Spec,
    world: &'w World,
    pub media: Media,
    /// Person index → their jid row (and lid row when they have one).
    jid_rows: Vec<(i64, Option<i64>)>,
    next_message: i64,
    next_add_on: i64,
    next_call: i64,
    next_option: i64,
    pub counts: Counts,
}

impl<'w> Writer<'w> {
    /// Writes the people and chats, and returns the writer and each chat's state.
    pub fn new(db: &'w Connection, spec: &'w Spec, world: &'w World, media: Media) -> Result<(Self, Vec<ChatState>)> {
        db.execute_batch(SCHEMA)?;
        db.execute_batch(
            "INSERT INTO props (key, value) VALUES ('generator', 'oopz-gen: synthetic, not a real backup');",
        )?;
        let mut add_jid = db
            .prepare("INSERT INTO jid (user, server, agent, device, type, raw_string) VALUES (?1, ?2, 0, 0, ?3, ?4)")?;
        let mut jid = |raw: &str| -> Result<i64> {
            let (user, server) = raw.split_once('@').unwrap_or((raw, ""));
            let kind = match server {
                "g.us" => 1,
                "lid" => 17,
                "broadcast" => 5,
                _ => 0,
            };
            add_jid.execute(params![user, server, kind, raw])?;
            Ok(db.last_insert_rowid())
        };
        let mut jid_rows = Vec::with_capacity(world.people.len());
        for p in &world.people {
            let row = jid(&p.jid)?;
            let lid = p.lid.as_deref().map(&mut jid).transpose()?;
            if let Some(lid) = lid {
                db.execute("INSERT INTO jid_map VALUES (?1, ?2)", [lid, row])?;
            }
            jid_rows.push((row, lid));
        }
        let mut chats = Vec::with_capacity(world.chats.len());
        for (i, chat) in world.chats.iter().enumerate() {
            let (jid_row, subject) = match &chat.kind {
                ChatKind::Direct(n) => (jid_rows[*n].0, None),
                ChatKind::Own => (jid_rows[0].0, None),
                ChatKind::Status => (jid("status@broadcast")?, None),
                ChatKind::Group { jid: g, subject, members } => {
                    let row = jid(g)?;
                    for (rank, m) in members.iter().enumerate() {
                        // The owner made the group; the first member after is an admin. Some members are filed under
                        // their lid, as WhatsApp does now.
                        let (user, lid) = jid_rows[*m];
                        let user = if *m == 0 { 0 } else { lid.unwrap_or(user) };
                        db.execute(
                            "INSERT INTO group_participant_user (group_jid_row_id, user_jid_row_id, rank, pending)
                             VALUES (?1, ?2, ?3, 0)",
                            params![row, user, [2, 1].get(rank).copied().unwrap_or(0)],
                        )?;
                    }
                    (row, subject.as_deref())
                }
            };
            db.execute(
                "INSERT INTO chat (_id, jid_row_id, hidden, subject, created_timestamp, archived, group_type)
                 VALUES (?1, ?2, 0, ?3, ?4, ?5, 0)",
                params![i as i64 + 1, jid_row, subject, chat.start_ms, (i % 17 == 16) as i64],
            )?;
            chats.push(ChatState {
                rng: Rng::fork(spec.seed, 1_000 + i as u64),
                row: i as i64 + 1,
                jid_row,
                members: chat.members(),
                group: chat.is_group(),
                recent: VecDeque::new(),
                last_author: 0,
                last_message: 0,
                last_ts: chat.start_ms,
                written: 0,
            });
        }
        drop(add_jid);
        let writer = Writer {
            db,
            spec,
            world,
            media,
            jid_rows,
            next_message: 1,
            next_add_on: 1,
            next_call: 1,
            next_option: 1,
            counts: Counts::default(),
        };
        Ok((writer, chats))
    }

    fn key_id(&self, row: i64) -> String {
        // Android key ids: 32 uppercase hex digits. A bijection of the row, so never two alike.
        format!("{:016X}{:016X}", mix(row as u64 ^ self.spec.seed), mix(!(row as u64) ^ self.spec.seed))
    }

    /// The sender column for `author` in this chat: 0 for the owner and in one-to-one chats, else their jid row
    /// (or lid row, for some).
    fn sender(&self, chat: &ChatState, author: usize, rng: &mut Rng) -> i64 {
        if author == 0 || !chat.group {
            return 0;
        }
        let (jid, lid) = self.jid_rows[author];
        match lid {
            Some(lid) if rng.chance(0.7) => lid,
            _ => jid,
        }
    }

    fn insert_message(
        &mut self,
        chat: &mut ChatState,
        ts: i64,
        (from_me, sender): (bool, i64),
        code: i64,
        text: Option<&str>,
        key: Option<String>,
    ) -> Result<i64> {
        let row = self.next_message;
        self.next_message += 1;
        let key = key.or_else(|| Some(self.key_id(row)));
        let status = if from_me { 13 } else { 0 };
        let starred = chat.rng.chance(self.spec.rates.starred) && code != 7;
        self.counts.starred += starred as u64;
        self.db
            .prepare_cached(
                "INSERT INTO message (_id, chat_row_id, from_me, key_id, sender_jid_row_id, status, broadcast,
                     recipient_count, origination_flags, origin, timestamp, received_timestamp,
                     receipt_server_timestamp, message_type, text_data, starred, lookup_tables, message_add_on_flags,
                     sort_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0, 0, ?7, ?8, ?7, ?9, ?10, ?11, 0, 0, ?1)",
            )?
            .execute(params![row, chat.row, from_me, key, sender, status, ts, ts + 900, code, text, starred])?;
        chat.last_message = row;
        chat.written += 1;
        self.counts.messages += 1;
        Ok(row)
    }

    /// The chat's first rows: the encryption notice, and for a group its creation and members.
    pub fn open_chat(&mut self, chat: &mut ChatState, index: usize) -> Result<()> {
        let ts = chat.last_ts;
        let row = self.insert_message(chat, ts, (false, 0), 7, None, None)?;
        self.system(row, action::ENCRYPTED)?;
        if chat.group {
            let row = self.insert_message(chat, ts + 1, (true, 0), 7, None, Some(String::new()))?;
            self.db.execute("UPDATE message SET key_id = NULL WHERE _id = ?1", [row])?;
            self.system(row, action::CREATED)?;
            if let ChatKind::Group { subject, .. } = &self.world.chats[index].kind {
                self.db.execute("INSERT INTO message_system_value_change VALUES (?1, ?2)", params![row, subject])?;
            }
            let row = self.insert_message(chat, ts + 2, (true, 0), 7, None, Some("-1".into()))?;
            self.system(row, action::ADDED)?;
            for m in chat.members.clone().into_iter().skip(1) {
                let user = self.jid_rows[m].0;
                self.db.execute(
                    "INSERT INTO message_system_chat_participant (message_row_id, user_jid_row_id) VALUES (?1, ?2)",
                    [row, user],
                )?;
            }
        }
        chat.last_ts = ts + 3;
        Ok(())
    }

    fn system(&mut self, row: i64, action: i64) -> Result<()> {
        self.db.prepare_cached("INSERT INTO message_system VALUES (?1, ?2)")?.execute([row, action])?;
        self.counts.system += 1;
        Ok(())
    }

    /// The next author: often the same person again, as people write in runs.
    fn author(&self, chat: &mut ChatState) -> usize {
        let members = &chat.members;
        if members.len() == 1 || chat.rng.chance(0.45) {
            return chat.last_author;
        }
        let pick = if chat.group {
            // A few people write most of a group's messages.
            let weights: Vec<u32> = (0..members.len()).map(|i| 1000 / (i as u32 + 1)).collect();
            members[chat.rng.weighted(&weights)]
        } else {
            *chat.rng.pick(members)
        };
        if pick == chat.last_author && members.len() == 2 { members[(members[0] == pick) as usize] } else { pick }
    }

    /// One message of `kind` (or a kind drawn from the mix), with what may come with it.
    pub fn message(&mut self, chat: &mut ChatState, ts: i64, kind: Option<Kind>) -> Result<()> {
        let author = self.author(chat);
        chat.last_author = author;
        let mut rng = Rng::new(chat.rng.next_u64());
        let kind = kind.unwrap_or_else(|| Kind::ALL[rng.weighted(&self.spec.mix)]);
        *self.counts.by_kind.entry(kind.name()).or_default() += 1;
        let from_me = author == 0;
        let sender = self.sender(chat, author, &mut rng);
        let spanish = self.world.people[author].spanish;
        let rates = self.spec.rates.clone();

        let text: Option<String> = match kind {
            Kind::Text if self.spec.edge_cases && rng.chance(0.15) => Some((*rng.pick(corpus::EDGE_TEXTS)).into()),
            Kind::Text => Some(corpus::sentence(&mut rng, spanish)),
            Kind::EmojiOnly => Some(corpus::emoji_only(&mut rng)),
            Kind::LongText => {
                let chars =
                    if self.spec.edge_cases && rng.chance(0.3) { 65_536 } else { rng.range(600, 4_000) as usize };
                Some(corpus::long_text(&mut rng, spanish, chars))
            }
            Kind::Link => Some(format!("{} {}", corpus::sentence(&mut rng, spanish), rng.pick(corpus::LINKS).0)),
            Kind::Image | Kind::Video if rng.chance(0.2) => Some(corpus::sentence(&mut rng, spanish)),
            Kind::Contact => {
                let p = &self.world.people[rng.below(self.world.people.len() as u64) as usize];
                Some(p.display_name.clone().or_else(|| p.wa_name.clone()).unwrap_or_else(|| p.number.clone()))
            }
            Kind::Poll => Some(rng.pick(corpus::POLLS).0.to_owned()),
            Kind::Location if rng.chance(0.5) => Some(rng.pick(corpus::PLACES).0.to_owned()),
            _ => None,
        };
        let code = kind.code();
        let row = self.insert_message(chat, ts, (from_me, sender), code, text.as_deref(), None)?;

        // Media.
        let media_kind = !matches!(
            kind,
            Kind::Text
                | Kind::EmojiOnly
                | Kind::LongText
                | Kind::Link
                | Kind::Location
                | Kind::LiveLocation
                | Kind::Contact
                | Kind::Poll
                | Kind::Call
                | Kind::Deleted
                | Kind::Album
        );
        if media_kind {
            let doc = (kind == Kind::Document).then(|| *rng.pick(corpus::DOCUMENTS));
            let missing = rng.chance(rates.missing_media) || kind == Kind::ViewOnce;
            let dup = rng.chance(rates.duplicate_media);
            let file = if missing {
                File::Missing
            } else if dup {
                File::Same
            } else {
                File::New
            };
            let m = self.media.row(&mut rng, kind, ts, from_me, doc, file)?;
            self.media_row(row, chat.row, &m, text.as_deref())?;
            if kind == Kind::Sticker && rng.chance(0.05) {
                let (name, publisher) = *rng.pick(corpus::STICKER_PACKS);
                self.db.execute(
                    "INSERT INTO message_sticker_pack VALUES (?1, ?2, ?3, ?4)",
                    params![row, format!("pack-{}", rng.below(1000)), name, publisher],
                )?;
            }
        }
        match kind {
            Kind::Link => {
                let (url, title, description) = *rng.pick(corpus::LINKS);
                self.db
                    .prepare_cached("INSERT INTO message_text (message_row_id, description, page_title, url) VALUES (?1, ?2, ?3, ?4)")?
                    .execute(params![row, (!description.is_empty()).then_some(description), (!title.is_empty()).then_some(title), url])?;
            }
            Kind::Location | Kind::LiveLocation => {
                let (place, address) = *rng.pick(corpus::PLACES);
                let (lat, lng) = (rng.unit() * 140.0 - 70.0, rng.unit() * 340.0 - 170.0);
                let live = kind == Kind::LiveLocation;
                self.db
                    .prepare_cached(
                        "INSERT INTO message_location (message_row_id, chat_row_id, latitude, longitude, place_name,
                         place_address, live_location_share_duration, live_location_final_latitude,
                         live_location_final_longitude, live_location_final_timestamp)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    )?
                    .execute(params![
                        row,
                        chat.row,
                        lat,
                        lng,
                        (!live && text.is_some()).then_some(place),
                        (!live && text.is_some()).then_some(address),
                        live.then_some(900),
                        live.then_some(lat + 0.01),
                        live.then_some(lng + 0.01),
                        live.then_some(ts + 900_000),
                    ])?;
            }
            Kind::Contact => {
                let number = format!("9999{:08}", rng.below(100_000_000));
                let name = text.clone().unwrap_or_default();
                let vcard = format!(
                    "BEGIN:VCARD\nVERSION:3.0\nN:;{name};;;\nFN:{name}\nitem1.TEL;waid={number}:+{number}\nitem1.X-ABLabel:Móvil\nEND:VCARD"
                );
                self.db
                    .prepare_cached("INSERT INTO message_vcard (message_row_id, vcard) VALUES (?1, ?2)")?
                    .execute(params![row, vcard])?;
            }
            Kind::Poll => self.poll(chat, row, ts, &mut rng)?,
            Kind::Call => {
                let video = rng.chance(0.3);
                let missed = rng.chance(0.25);
                let duration = if missed { 0 } else { rng.range(5, 5_400) as i64 };
                let call = self.next_call;
                self.next_call += 1;
                let jid = self.jid_rows[if from_me { chat.members.get(1).copied().unwrap_or(0) } else { author }].0;
                self.db
                    .prepare_cached(
                        "INSERT INTO call_log (_id, jid_row_id, from_me, call_id, timestamp, video_call, duration,
                         call_result, group_jid_row_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    )?
                    .execute(params![
                        call,
                        jid,
                        from_me,
                        self.key_id(-call),
                        ts,
                        video,
                        duration,
                        if missed { 2 } else { 5 },
                        if chat.group { chat.jid_row } else { 0 }
                    ])?;
                self.db.prepare_cached("INSERT INTO message_call_log VALUES (?1, ?2)")?.execute([row, call])?;
            }
            Kind::Album => {
                let photos = rng.range(2, 8) as i64;
                let videos = if rng.chance(0.2) { 1 } else { 0 };
                self.db.execute("INSERT INTO message_album VALUES (?1, ?2, ?3)", [row, photos, videos])?;
                for i in 0..photos + videos {
                    let child_kind = if i < photos { Kind::Image } else { Kind::Video };
                    let at = ts + 1_000 * (i + 1);
                    let child = self.insert_message(chat, at, (from_me, sender), child_kind.code(), None, None)?;
                    let missing = rng.chance(rates.missing_media);
                    let named = if i < photos { Kind::Album } else { Kind::Video };
                    let file = if missing { File::Missing } else { File::New };
                    let m = self.media.row(&mut rng, named, at, from_me, None, file)?;
                    self.media_row(child, chat.row, &m, None)?;
                    self.db.execute(
                        "INSERT INTO message_association (child_message_row_id, parent_message_row_id, association_type)
                         VALUES (?1, ?2, 1)",
                        [child, row],
                    )?;
                }
            }
            _ => {}
        }

        // Things that happen to a message.
        if !chat.recent.is_empty() && kind != Kind::Deleted && rng.chance(rates.reply) {
            let to = chat.recent[rng.below(chat.recent.len() as u64) as usize].clone();
            self.db
                .prepare_cached("INSERT INTO message_quoted VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?
                .execute(params![row, chat.row, to.from_me, to.sender, to.key, to.ts, to.code, to.text])?;
            self.counts.replies += 1;
        }
        if kind != Kind::Deleted && kind != Kind::Call && rng.chance(rates.forwarded) {
            let score = if rng.chance(0.1) { 5 } else { 1 };
            self.db.prepare_cached("INSERT INTO message_forwarded VALUES (?1, ?2, 0)")?.execute([row, score])?;
            self.counts.forwarded += 1;
        }
        if matches!(kind, Kind::Text | Kind::Link | Kind::LongText) && from_me && rng.chance(rates.edited) {
            let at = ts + rng.range(5_000, 900_000) as i64;
            let key = self.key_id(row);
            self.db
                .prepare_cached("INSERT INTO message_edit_info VALUES (?1, ?2, ?3, ?3)")?
                .execute(params![row, key, at])?;
            self.counts.edits += 1;
        }
        if kind != Kind::Deleted && rng.chance(rates.reaction) {
            self.reactions(chat, row, author, ts, &mut rng)?;
        }

        let recent = Recent {
            key: self.key_id(row),
            from_me,
            sender,
            ts,
            code,
            text: text.map(|t| t.chars().take(200).collect()),
        };
        if chat.recent.len() == 32 {
            chat.recent.pop_front();
        }
        chat.recent.push_back(recent);
        chat.rng = Rng::new(chat.rng.next_u64() ^ rng.next_u64());

        // Groups change now and then.
        if chat.group && chat.members.len() > 2 && rng.chance(0.002) {
            self.group_event(chat, ts + 1, &mut rng)?;
        }
        Ok(())
    }

    fn media_row(&mut self, row: i64, chat: i64, m: &MediaRow, caption: Option<&str>) -> Result<()> {
        self.db
            .prepare_cached(
                "INSERT INTO message_media (message_row_id, chat_row_id, transferred, file_path, file_size, width,
                     height, mime_type, media_name, file_hash, media_duration, media_caption)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?
            .execute(params![
                row,
                chat,
                m.file_path.is_some(),
                m.file_path,
                m.size,
                m.width,
                m.height,
                m.mime,
                m.name,
                m.hash,
                m.duration,
                caption
            ])?;
        Ok(())
    }

    fn add_on(&mut self, chat: &ChatState, parent: i64, who: usize, ts: i64, kind: i64, rng: &mut Rng) -> Result<i64> {
        let id = self.next_add_on;
        self.next_add_on += 1;
        let sender = self.sender(chat, who, rng);
        self.db
            .prepare_cached(
                "INSERT INTO message_add_on (_id, chat_row_id, from_me, key_id, sender_jid_row_id,
                     parent_message_row_id, timestamp, status, message_add_on_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?
            .execute(params![
                id,
                chat.row,
                who == 0,
                self.key_id(-(1 << 40) - id),
                sender,
                parent,
                ts,
                if who == 0 { 13 } else { 0 },
                kind
            ])?;
        Ok(id)
    }

    fn reactions(&mut self, chat: &ChatState, row: i64, author: usize, ts: i64, rng: &mut Rng) -> Result<()> {
        let others: Vec<usize> = chat.members.iter().copied().filter(|m| *m != author).collect();
        if others.is_empty() {
            return Ok(());
        }
        for _ in 0..rng.range(1, others.len().min(4) as u64) {
            let who = *rng.pick(&others);
            let at = ts + rng.range(1_000, 3_600_000) as i64;
            let id = self.add_on(chat, row, who, at, 56, rng)?;
            let emoji = *rng.pick(corpus::REACTIONS);
            self.db
                .prepare_cached("INSERT INTO message_add_on_reaction VALUES (?1, ?2, ?3)")?
                .execute(params![id, emoji, at])?;
            self.counts.reactions += 1;
            // Taken back, or changed.
            if rng.chance(0.08) {
                let later = at + 60_000;
                let id = self.add_on(chat, row, who, later, 56, rng)?;
                let emoji = if rng.chance(0.5) { "" } else { *rng.pick(corpus::REACTIONS) };
                self.db
                    .prepare_cached("INSERT INTO message_add_on_reaction VALUES (?1, ?2, ?3)")?
                    .execute(params![id, emoji, later])?;
                self.counts.reactions += 1;
            }
        }
        Ok(())
    }

    fn poll(&mut self, chat: &ChatState, row: i64, ts: i64, rng: &mut Rng) -> Result<()> {
        let (question, options) = *rng.pick(corpus::POLLS);
        let _ = question;
        let single = rng.chance(0.5);
        self.db.execute("INSERT INTO message_poll VALUES (?1, 0, 0, ?2)", [row, if single { 1 } else { 0 }])?;
        let first = self.next_option;
        for name in options {
            self.db.execute(
                "INSERT INTO message_poll_option (_id, message_row_id, option_name, vote_total) VALUES (?1, ?2, ?3, 0)",
                params![self.next_option, row, name],
            )?;
            self.next_option += 1;
        }
        for who in chat.members.clone() {
            if !rng.chance(0.7) {
                continue;
            }
            // Some voters change their mind: only the latest vote counts.
            for round in 0..if rng.chance(0.15) { 2 } else { 1 } {
                let at = ts + (round + 1) * rng.range(10_000, 7_200_000) as i64;
                let id = self.add_on(chat, row, who, at, 66, rng)?;
                self.db.execute("INSERT INTO message_add_on_poll_vote VALUES (?1, ?2)", [id, at])?;
                let picks = if single { 1 } else { rng.range(0, options.len() as u64) };
                let mut chosen = Vec::new();
                for _ in 0..picks {
                    let o = first + rng.below(options.len() as u64) as i64;
                    if !chosen.contains(&o) {
                        chosen.push(o);
                        self.db.execute(
                            "INSERT INTO message_add_on_poll_vote_selected_option
                                 (message_add_on_row_id, message_poll_option_id) VALUES (?1, ?2)",
                            [id, o],
                        )?;
                    }
                }
                self.counts.poll_votes += 1;
            }
        }
        self.db.execute(
            "UPDATE message_poll_option SET vote_total = (SELECT count(*) FROM message_add_on_poll_vote_selected_option
                 WHERE message_poll_option_id = message_poll_option._id) WHERE message_row_id = ?1",
            [row],
        )?;
        Ok(())
    }

    /// A rename, a new description or icon, an admin, someone leaving or removed.
    fn group_event(&mut self, chat: &mut ChatState, ts: i64, rng: &mut Rng) -> Result<()> {
        let who = chat.members[1 + rng.below(chat.members.len() as u64 - 1) as usize];
        let sender = self.sender(chat, who, rng);
        let event = *rng.pick(&[
            action::RENAMED,
            action::DESCRIPTION,
            action::ICON,
            action::ADMIN,
            action::LEFT,
            action::REMOVED,
        ]);
        let row = self.insert_message(chat, ts, (false, sender), 7, None, None)?;
        self.system(row, event)?;
        match event {
            action::RENAMED => {
                self.db.execute(
                    "INSERT INTO message_system_value_change VALUES (?1, ?2)",
                    params![row, "Nombre anterior"],
                )?;
            }
            action::LEFT | action::REMOVED => {
                let user = self.jid_rows[who].0;
                self.db.execute(
                    "INSERT INTO message_system_chat_participant (message_row_id, user_jid_row_id) VALUES (?1, ?2)",
                    [row, user],
                )?;
                self.db.execute(
                    "INSERT INTO group_past_participant_user (group_jid_row_id, user_jid_row_id, is_leave, timestamp)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![chat.jid_row, user, event == action::LEFT, ts],
                )?;
                self.db.execute(
                    "DELETE FROM group_participant_user WHERE group_jid_row_id = ?1 AND user_jid_row_id IN (?2, ?3)",
                    params![chat.jid_row, user, self.jid_rows[who].1],
                )?;
                chat.members.retain(|m| *m != who);
                if chat.last_author == who {
                    chat.last_author = 0;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Rows real backups have and readers trip on, written once into chat `chat`: a reaction and a reply whose
    /// message isn't there, an unknown type, an unknown system event, a sender-less group row, a row with no time,
    /// a media row with no path, a path leaving `Media/`, an empty text, a NULL text in a text message, two rows
    /// at the same millisecond, a message dated before the chat and one in the future.
    pub fn orphans(&mut self, chat: &mut ChatState) -> Result<()> {
        let ts = chat.last_ts;
        let mut rng = Rng::new(chat.rng.next_u64());
        let row =
            self.insert_message(chat, ts, (false, 0), 0, Some("tiene una reacción a un mensaje borrado"), None)?;
        let who = chat.members[1.min(chat.members.len() - 1)];
        self.add_on(chat, row + 10_000_000, who, ts, 56, &mut rng)
            .and_then(|id| Ok(self.db.execute("INSERT INTO message_add_on_reaction VALUES (?1, '👻', 0)", [id])?))?;
        self.db.execute(
            "INSERT INTO message_quoted VALUES (?1, ?2, ?2, 0, 0, 'NO-SUCH-KEY', 0, 0, NULL)",
            [row, chat.row],
        )?;
        self.insert_message(chat, ts + 1, (false, 0), 11, None, None)?;
        self.insert_message(chat, ts + 2, (true, 0), 12_345, Some("tipo desconocido"), None)?;
        let r = self.insert_message(chat, ts + 3, (false, 0), 7, None, None)?;
        self.system(r, action::UNKNOWN)?;
        let r = self.insert_message(chat, ts + 4, (false, 0), 7, None, None)?;
        self.db.execute("INSERT INTO message_system VALUES (?1, NULL)", [r])?;
        self.insert_message(chat, ts + 5, (false, 0), 0, Some(""), None)?;
        self.insert_message(chat, ts + 5, (false, 0), 0, None, None)?;
        let r = self.insert_message(chat, ts + 6, (false, 0), 1, None, None)?;
        self.db.execute("INSERT INTO message_media (message_row_id, chat_row_id) VALUES (?1, ?2)", [r, chat.row])?;
        let r = self.insert_message(chat, ts + 7, (false, 0), 9, None, None)?;
        self.db.execute(
            "INSERT INTO message_media (message_row_id, chat_row_id, file_path, mime_type, file_size)
             VALUES (?1, ?2, 'Media/../../outside.pdf', 'application/pdf', 10)",
            [r, chat.row],
        )?;
        let r = self.insert_message(chat, 0, (false, 0), 0, Some("sin fecha"), None)?;
        self.db.execute("UPDATE message SET timestamp = NULL WHERE _id = ?1", [r])?;
        self.insert_message(chat, 86_400_000, (false, 0), 0, Some("de 1970"), None)?;
        self.insert_message(chat, 4_102_444_800_000, (true, 0), 0, Some("desde 2100"), None)?;
        self.insert_message(chat, ts + 8, (false, 0), 0, Some("misma clave"), Some("DUPLICATE-KEY".into()))?;
        self.insert_message(chat, ts + 9, (true, 0), 0, Some("misma clave, mía"), Some("DUPLICATE-KEY".into()))?;
        self.counts.orphans += 16;
        chat.last_ts = ts + 10;
        Ok(())
    }

    /// Fills in each chat's last-message columns, and builds the indexes.
    pub fn finish(&mut self, chats: &[ChatState]) -> Result<()> {
        for c in chats {
            let last = (c.last_message > 0).then_some(c.last_message);
            self.db.execute(
                "UPDATE chat SET display_message_row_id = ?2, last_message_row_id = ?2, last_read_message_row_id = ?2,
                     last_read_receipt_sent_message_row_id = ?2, last_important_message_row_id = ?2,
                     last_message_sort_id = ?2, display_message_sort_id = ?2, last_read_message_sort_id = ?2,
                     last_read_receipt_sent_message_sort_id = ?2, sort_timestamp = ?3 WHERE _id = ?1",
                params![c.row, last, c.last_ts],
            )?;
        }
        self.db.execute_batch(INDEXES)?;
        Ok(())
    }
}
