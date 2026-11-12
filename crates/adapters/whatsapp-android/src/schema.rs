//! Detects which msgstore schema a backup has, by the tables and columns present, and builds the queries for it.
//! Anything required that's missing fails loudly, naming the table or column (never a value).

use std::collections::HashSet;

use msgkinds::Note;
use oopz_model::messages::Message as Msg;
use rusqlite::Connection;

use crate::{Error, Result};

/// The current schema (`message`, `jid`, `chat`, …), with the optional parts this backup has.
pub struct Schema {
    message: HashSet<String>,
    media: HashSet<String>,
    quoted: HashSet<String>,
    location: HashSet<String>,
    poll: HashSet<String>,
    add_on: HashSet<String>,
    chat: HashSet<String>,
    /// Which of [`OPTIONAL`]'s parts this backup has.
    has: HashSet<&'static str>,
}

/// A table and the columns needed from it.
type Needs = (&'static str, &'static [&'static str]);

/// Parts read when present: a name, and the tables and columns it needs.
const OPTIONAL: &[(&str, &[Needs])] = &[
    (
        "calls",
        &[
            ("message_call_log", &["message_row_id", "call_log_row_id"]),
            ("call_log", &["_id", "video_call", "duration"]),
        ],
    ),
    ("forwarded", &[("message_forwarded", &["message_row_id"])]),
    ("edits", &[("message_edit_info", &["message_row_id", "edited_timestamp"])]),
    ("albums", &[("message_association", &["child_message_row_id", "parent_message_row_id"])]),
    ("links", &[("message_text", &["message_row_id", "url", "page_title", "description"])]),
    (
        "reactions",
        &[
            ("message_add_on", &["_id", "parent_message_row_id", "from_me", "timestamp"]),
            ("message_add_on_reaction", &["message_add_on_row_id", "reaction"]),
        ],
    ),
    ("locations", &[("message_location", &["message_row_id", "latitude", "longitude"])]),
    ("vcards", &[("message_vcard", &["message_row_id", "vcard"])]),
    (
        "polls",
        &[("message_poll", &["message_row_id"]), ("message_poll_option", &["_id", "message_row_id", "option_name"])],
    ),
    (
        "poll_votes",
        &[
            ("message_add_on", &["_id", "parent_message_row_id", "from_me", "timestamp"]),
            ("message_add_on_poll_vote", &["message_add_on_row_id"]),
            ("message_add_on_poll_vote_selected_option", &["message_add_on_row_id", "message_poll_option_id"]),
        ],
    ),
];

const REQUIRED: &[(&str, &[&str])] = &[
    ("jid", &["_id", "raw_string"]),
    ("chat", &["_id", "jid_row_id"]),
    ("message", &["_id", "chat_row_id", "from_me", "key_id", "timestamp", "message_type", "text_data"]),
];

pub fn detect(db: &Connection) -> Result<Schema> {
    if columns(db, "message")?.is_empty() && !columns(db, "messages")?.is_empty() {
        return Err(Error::Unsupported(Note::of(Msg::WhatsappOldSchema, [])));
    }
    for (table, required) in REQUIRED {
        let present = columns(db, table)?;
        if present.is_empty() {
            return Err(Error::Unsupported(Note::of(Msg::WhatsappNoTable, [table.to_string()])));
        }
        if let Some(missing) = required.iter().find(|c| !present.contains(**c)) {
            return Err(Error::Unsupported(Note::of(Msg::WhatsappNoColumn, [table.to_string(), missing.to_string()])));
        }
    }
    let mut has = HashSet::new();
    for (part, tables) in OPTIONAL {
        let mut all = true;
        for (table, needed) in *tables {
            let present = columns(db, table)?;
            all &= needed.iter().all(|c| present.contains(*c));
        }
        if all {
            has.insert(*part);
        }
    }
    Ok(Schema {
        message: columns(db, "message")?,
        media: columns(db, "message_media")?,
        quoted: columns(db, "message_quoted")?,
        location: columns(db, "message_location")?,
        poll: columns(db, "message_poll")?,
        add_on: columns(db, "message_add_on")?,
        chat: columns(db, "chat")?,
        has,
    })
}

impl Schema {
    /// One row per message in chat `?1`, oldest first. Columns, in order: key_id, from_me, timestamp, message_type,
    /// text_data, starred, has_media, file_path, mime_type, file_size, width, height, media_duration (seconds),
    /// media_name, quoted key_id, call video_call, call duration (seconds), forwarded, edited at (ms), the album's
    /// key_id, link url, link title, link description, the message's row id, the sender's jid (`raw_string`; NULL
    /// when the row has none: mine, or a one-to-one chat's).
    pub fn messages_query(&self) -> String {
        self.messages_query_where("")
    }

    /// [`Schema::messages_query`] with `extra` (SQL over `m`, the message row, starting with `AND`) added to its
    /// condition: an importer that reads only some of a chat.
    pub fn messages_query_where(&self, extra: &str) -> String {
        let has = |set: &HashSet<String>, col: &str, expr: &str| {
            if set.contains(col) { expr.to_owned() } else { "NULL".to_owned() }
        };
        let media = |col: &str| has(&self.media, col, &format!("mm.{col}"));
        let joins = [
            (!self.media.is_empty()).then_some("LEFT JOIN message_media mm ON mm.message_row_id = m._id"),
            (!self.quoted.is_empty()).then_some("LEFT JOIN message_quoted q ON q.message_row_id = m._id"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
        format!(
            "SELECT m.key_id, m.from_me, m.timestamp, m.message_type, m.text_data, {starred}, {has_media},
                    {file_path}, {mime_type}, {file_size}, {width}, {height}, {duration}, {media_name}, {quoted},
                    {call_video}, {call_duration}, {forwarded}, {edited}, {album}, {url}, {title}, {description},
                    m._id, {sender}
             FROM message m
             {joins}
             WHERE m.chat_row_id = ?1 {extra}
             ORDER BY m.timestamp, m._id",
            starred = has(&self.message, "starred", "m.starred"),
            sender = has(
                &self.message,
                "sender_jid_row_id",
                "(SELECT raw_string FROM jid WHERE _id = m.sender_jid_row_id AND m.sender_jid_row_id > 0)"
            ),
            has_media = has(&self.media, "message_row_id", "mm.message_row_id IS NOT NULL"),
            file_path = media("file_path"),
            mime_type = media("mime_type"),
            file_size = media("file_size"),
            width = media("width"),
            height = media("height"),
            duration = media("media_duration"),
            media_name = media("media_name"),
            quoted = has(&self.quoted, "key_id", "q.key_id"),
            call_video = self.sub(
                "calls",
                "SELECT cl.video_call FROM message_call_log mcl
                JOIN call_log cl ON cl._id = mcl.call_log_row_id WHERE mcl.message_row_id = m._id"
            ),
            call_duration = self.sub(
                "calls",
                "SELECT cl.duration FROM message_call_log mcl
                JOIN call_log cl ON cl._id = mcl.call_log_row_id WHERE mcl.message_row_id = m._id"
            ),
            forwarded = self.sub("forwarded", "SELECT 1 FROM message_forwarded WHERE message_row_id = m._id"),
            edited =
                self.sub("edits", "SELECT max(edited_timestamp) FROM message_edit_info WHERE message_row_id = m._id"),
            album = self.sub(
                "albums",
                "SELECT p.key_id FROM message_association a
                JOIN message p ON p._id = a.parent_message_row_id
                WHERE a.child_message_row_id = m._id AND p.message_type = 99"
            ),
            url = self.sub("links", "SELECT url FROM message_text WHERE message_row_id = m._id"),
            title = self.sub("links", "SELECT page_title FROM message_text WHERE message_row_id = m._id"),
            description = self.sub("links", "SELECT description FROM message_text WHERE message_row_id = m._id"),
        )
    }

    /// A condition over `m` (a message row) true for messages that can change after they were sent: deleted for
    /// everyone (type 15), edited, or a poll (its votes).
    pub fn changes_later(&self) -> String {
        let mut any = vec!["m.message_type IN (15, 46)".to_owned()];
        if self.has.contains("edits") {
            any.push("m._id IN (SELECT message_row_id FROM message_edit_info)".into());
        }
        if self.has.contains("polls") {
            any.push("m._id IN (SELECT message_row_id FROM message_poll)".into());
        }
        format!("({})", any.join(" OR "))
    }

    /// Reactions to messages in chat `?1`, oldest first: the message's key_id, from_me, timestamp (ms), the emoji
    /// (empty or NULL when taken back), the sender's jid (NULL when not recorded). `None` if this backup has no
    /// reactions tables.
    pub fn reactions_query(&self) -> Option<String> {
        self.has.contains("reactions").then(|| {
            format!(
                "SELECT p.key_id, a.from_me, a.timestamp, r.reaction, {}
                 FROM message_add_on a
                 JOIN message_add_on_reaction r ON r.message_add_on_row_id = a._id
                 JOIN message p ON p._id = a.parent_message_row_id
                 WHERE p.chat_row_id = ?1
                 ORDER BY a.timestamp, a._id",
                self.add_on_sender("a")
            )
        })
    }

    /// An add-on's sender jid as a column over `alias`, or NULL when this backup doesn't record it.
    fn add_on_sender(&self, alias: &str) -> String {
        if self.add_on.contains("sender_jid_row_id") {
            format!(
                "(SELECT raw_string FROM jid WHERE _id = {alias}.sender_jid_row_id AND {alias}.sender_jid_row_id > 0)"
            )
        } else {
            "NULL".into()
        }
    }

    /// Message `?1`'s place: latitude, longitude, place name, address, and a live location's last latitude and
    /// longitude (NULL when not live or not recorded). `None` if this backup has no locations table.
    pub fn location_query(&self) -> Option<String> {
        let col = |c: &str| if self.location.contains(c) { c.to_owned() } else { "NULL".to_owned() };
        self.has.contains("locations").then(|| {
            format!(
                "SELECT latitude, longitude, {}, {}, {}, {} FROM message_location WHERE message_row_id = ?1",
                col("place_name"),
                col("place_address"),
                col("live_location_final_latitude"),
                col("live_location_final_longitude"),
            )
        })
    }

    /// Message `?1`'s vCards, in the order stored. `None` if this backup has no vCard table.
    pub fn vcards_query(&self) -> Option<&'static str> {
        self.has
            .contains("vcards")
            .then_some("SELECT vcard FROM message_vcard WHERE message_row_id = ?1 AND vcard IS NOT NULL ORDER BY rowid")
    }

    /// Whether message `?1` is a poll, and how many options a voter may choose (0: any number), when recorded.
    /// `None` if this backup has no poll tables.
    pub fn poll_query(&self) -> Option<String> {
        let count = if self.poll.contains("selectable_options_count") { "selectable_options_count" } else { "NULL" };
        self.has.contains("polls").then(|| format!("SELECT {count} FROM message_poll WHERE message_row_id = ?1"))
    }

    /// Poll `?1`'s options, in order: their row id and name.
    pub fn poll_options_query(&self) -> Option<&'static str> {
        self.has
            .contains("polls")
            .then_some("SELECT _id, option_name FROM message_poll_option WHERE message_row_id = ?1 ORDER BY _id")
    }

    /// Poll `?1`'s votes: each voter's (by `from_me` and, where recorded, sender) latest vote, as (from_me, the
    /// chosen option's row id, the sender's jid or NULL), one row per option chosen (none when the latest took the
    /// vote back). `None` if this backup has no vote tables.
    pub fn poll_votes_query(&self) -> Option<String> {
        let same_sender = if self.add_on.contains("sender_jid_row_id") {
            " AND b.sender_jid_row_id IS a.sender_jid_row_id"
        } else {
            ""
        };
        self.has.contains("poll_votes").then(|| {
            format!(
                "SELECT a.from_me, s.message_poll_option_id, {}
                 FROM message_add_on a
                 JOIN message_add_on_poll_vote_selected_option s ON s.message_add_on_row_id = a._id
                 WHERE a.parent_message_row_id = ?1 AND a._id = (
                     SELECT b._id FROM message_add_on b JOIN message_add_on_poll_vote v ON v.message_add_on_row_id = b._id
                     WHERE b.parent_message_row_id = ?1 AND b.from_me IS a.from_me{same_sender}
                     ORDER BY b.timestamp DESC, b._id DESC LIMIT 1)
                 ORDER BY a.from_me, a._id, s.message_poll_option_id",
                self.add_on_sender("a")
            )
        })
    }

    /// A chat's subject (a group's name) as a column over `c` (`chat`), or NULL when this backup has none.
    pub fn chat_subject(&self) -> &'static str {
        if self.chat.contains("subject") { "c.subject" } else { "NULL" }
    }

    /// Whether this backup has `table` with all of `columns`: for an importer's own optional parts.
    pub fn has_columns(db: &Connection, table: &str, needed: &[&str]) -> Result<bool> {
        let present = columns(db, table)?;
        Ok(!present.is_empty() && needed.iter().all(|c| present.contains(*c)))
    }

    /// `query`'s first value for the message, as a subquery (one value even if a message is linked twice), or NULL
    /// when this backup lacks `part`.
    fn sub(&self, part: &str, query: &str) -> String {
        if self.has.contains(part) { format!("({query} LIMIT 1)") } else { "NULL".into() }
    }
}

fn columns(db: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut stmt = db.prepare("SELECT name FROM pragma_table_info(?1)")?;
    let names = stmt.query_map([table], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(names)
}
