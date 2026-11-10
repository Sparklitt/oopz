//! Telegram Desktop's JSON export (decision 0015): `result.json` from "Export chat history" (one chat) or "Export
//! Telegram data" (`chats.list`, `contacts.list`, `personal_information`), alone, in its folder, or zipped. A zip is
//! read in memory; entries with `..` or absolute paths are ignored.
//!
//! Times come from `date_unixtime` (real UTC); an export without it gives only `date`, the computer's wall clock, read
//! as UTC and flagged (`wall_clock`). Every message is kept: a type, media type or service action oopz doesn't know
//! becomes an `Unknown` part with Telegram's own code, counted in the notes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use oopz_model::{
    Adapter, App, CallOutcome, Conversation, ConversationKind, Error, Event, Loaded, Media, MediaKind, Message, Part,
    Person, PollOption, Reaction, Result, Secret, Source, SourceInfo, civil_to_ms, normalize_phone,
};
use serde_json::{Map, Value};

pub const ID: &str = "telegram-json";
const FORMAT: &str = "Telegram JSON export";
const MAX_ENTRY: u64 = 4 << 30;
/// Reactions counted without who: at most this many are listed per emoji.
const MAX_ANONYMOUS_REACTIONS: u64 = 1000;

pub struct TelegramExportAdapter;

impl Adapter for TelegramExportAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn app(&self) -> App {
        App::Telegram
    }

    fn secret(&self) -> Secret {
        Secret::None
    }

    fn detect(&self, path: &Path) -> bool {
        if path.is_dir() {
            return result_in(path).is_some();
        }
        let lower = path.to_string_lossy().to_lowercase();
        if lower.ends_with(".zip") {
            let Ok(file) = std::fs::File::open(path) else { return false };
            let Ok(zip) = zip::ZipArchive::new(file) else { return false };
            return zip.file_names().flatten().any(|n| n.ends_with("result.json"));
        }
        if !lower.ends_with(".json") {
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else { return false };
        let mut head = Vec::new();
        if file.take(4096).read_to_end(&mut head).is_err() {
            return false;
        }
        let head = String::from_utf8_lossy(&head);
        head.trim_start_matches('\u{FEFF}').trim_start().starts_with('{')
            && ["\"personal_information\"", "\"chats\"", "\"messages\""].iter().any(|k| head.contains(k))
    }

    fn open(&self, path: &Path, _secret: Option<&str>) -> Result<Box<dyn Source>> {
        let (json, root) = if path.is_dir() {
            let file = result_in(path).ok_or(Error::NotThisFormat(FORMAT))?;
            let root = file.parent().map(Path::to_path_buf);
            (std::fs::read(&file).map_err(|e| Error::io(file.display(), e))?, root)
        } else {
            let bytes = std::fs::read(path).map_err(|e| Error::io(path.display(), e))?;
            if bytes.starts_with(b"PK") {
                (from_zip(&bytes)?, None)
            } else {
                (bytes, path.parent().map(Path::to_path_buf))
            }
        };
        let mut loaded = read(&json)?;
        loaded.info.media_root = root;
        if loaded.info.media_root.is_none() {
            loaded.info.notes.push("media files are inside the zip".into());
        }
        Ok(Box::new(loaded))
    }
}

fn result_in(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("result.json");
    if direct.is_file() {
        return Some(direct);
    }
    // Telegram writes `ChatExport_2024-01-01/result.json` or `DataExport_…/result.json`.
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path().join("result.json")).find(|p| p.is_file())
}

/// `result.json` from a zip (the shallowest one).
pub fn from_zip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| Error::NotThisFormat(FORMAT))?;
    let mut best: Option<(usize, usize)> = None;
    for i in 0..zip.len() {
        let Ok(entry) = zip.by_index(i) else { continue };
        let Some(path) = entry.enclosed_name() else { continue };
        if path.file_name().is_some_and(|n| n == "result.json") {
            let depth = path.components().count();
            if best.is_none_or(|(d, _)| depth < d) {
                best = Some((depth, i));
            }
        }
    }
    let (_, i) = best.ok_or(Error::NotThisFormat(FORMAT))?;
    let entry = zip.by_index(i).map_err(|e| Error::Damaged(e.to_string()))?;
    let mut out = Vec::new();
    entry.take(MAX_ENTRY).read_to_end(&mut out).map_err(|e| Error::io("result.json in the zip", e))?;
    Ok(out)
}

#[derive(Default)]
struct Reader {
    owner: Option<String>,
    people: BTreeMap<String, Person>,
    notes: Notes,
}

#[derive(Default)]
struct Notes {
    unix_times: usize,
    wall_times: usize,
    no_time: usize,
    unknown: BTreeMap<String, usize>,
    duplicate_ids: usize,
    not_objects: usize,
    owner_inferred: bool,
}

/// Reads `result.json`'s bytes.
pub fn read(bytes: &[u8]) -> Result<Loaded> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::NotThisFormat(FORMAT))?;
    let Some(top) = value.as_object() else { return Err(Error::NotThisFormat(FORMAT)) };

    let mut r = Reader::default();
    if let Some(me) = top.get("personal_information").and_then(Value::as_object) {
        let id = me.get("user_id").and_then(id_of).map(|id| format!("user{id}"));
        if let Some(id) = id {
            let name = full_name(me);
            let phone = me.get("phone_number").and_then(Value::as_str).and_then(normalize_phone);
            let username = me.get("username").and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string);
            r.people
                .insert(id.clone(), Person { id: id.clone(), phone, username, name, me: true, ..Default::default() });
            r.owner = Some(id);
        }
    }

    let chats: Vec<&Value> = if let Some(chats) = top.get("chats") {
        let Some(list) = chats.get("list").and_then(Value::as_array) else {
            return Err(Error::UnsupportedVersion("Telegram export whose chats aren't a list".into()));
        };
        let left = top.get("left_chats").and_then(|l| l.get("list")).and_then(Value::as_array);
        list.iter().chain(left.into_iter().flatten()).collect()
    } else if let Some(messages) = top.get("messages") {
        if !messages.is_array() {
            return Err(Error::UnsupportedVersion("Telegram export whose messages aren't a list".into()));
        }
        vec![&value]
    } else {
        return Err(Error::NotThisFormat(FORMAT));
    };

    if let Some(list) = top.get("contacts").and_then(|c| c.get("list")).and_then(Value::as_array) {
        for c in list.iter().filter_map(Value::as_object) {
            let Some(phone) = c.get("phone_number").and_then(Value::as_str).and_then(normalize_phone) else { continue };
            let id = format!("phone:{phone}");
            if r.people.values().any(|p| p.phone.as_deref() == Some(phone.as_str())) {
                continue;
            }
            r.people.insert(id.clone(), Person { id, phone: Some(phone), name: full_name(c), ..Default::default() });
        }
    }

    let mut conversations = Vec::new();
    let mut messages = Vec::new();
    for (i, chat) in chats.into_iter().enumerate() {
        let Some(chat) = chat.as_object() else {
            r.notes.not_objects += 1;
            continue;
        };
        let (conversation, mut list) = r.chat(i, chat);
        conversations.push(conversation);
        messages.append(&mut list);
    }
    if let Some(owner) = &r.owner {
        for m in &mut messages {
            m.from_me = m.sender.as_deref() == Some(owner.as_str());
            for reaction in &mut m.reactions {
                reaction.from_me = reaction.by.as_deref() == Some(owner.as_str());
            }
        }
        if let Some(p) = r.people.get_mut(owner) {
            p.me = true;
        }
    }

    let n = &r.notes;
    let mut notes = Vec::new();
    let wall_clock = n.wall_times > 0 && n.unix_times == 0;
    if n.wall_times > 0 {
        notes.push(format!(
            "{} times have no zone (no date_unixtime): the computer's wall clock, read as UTC",
            n.wall_times
        ));
    }
    if n.no_time > 0 {
        notes.push(format!("{} messages without a date", n.no_time));
    }
    for (code, count) in &n.unknown {
        notes.push(format!("{count} × `{code}` kept as unknown"));
    }
    if n.duplicate_ids > 0 {
        notes.push(format!("{} repeated message ids made unique with #2, #3…", n.duplicate_ids));
    }
    if n.not_objects > 0 {
        notes.push(format!("{} chats or messages that weren't objects skipped", n.not_objects));
    }
    if n.owner_inferred {
        notes.push("the owner is inferred from the one-to-one chat (the other side is the chat's id)".into());
    }
    if r.owner.is_none() {
        notes.push("the export doesn't say who the owner is: pick yourself".into());
    }
    let owner = r.owner.as_ref().and_then(|o| r.people.get(o)).cloned();
    Ok(Loaded {
        info: SourceInfo {
            app: App::Telegram,
            adapter: ID.into(),
            format: None,
            owner,
            media_root: None,
            wall_clock,
            notes,
        },
        people: r.people.into_values().collect(),
        conversations,
        messages,
    })
}

impl Reader {
    fn chat(&mut self, index: usize, chat: &Map<String, Value>) -> (Conversation, Vec<Message>) {
        let id = chat.get("id").and_then(id_of).unwrap_or_else(|| format!("chat:{index}"));
        let code = chat.get("type").and_then(Value::as_str).unwrap_or("");
        let kind = match code {
            "personal_chat" | "bot_chat" => ConversationKind::Direct,
            "private_group" | "private_supergroup" | "public_supergroup" => ConversationKind::Group,
            "private_channel" | "public_channel" => ConversationKind::Channel,
            "saved_messages" => ConversationKind::Notes,
            other => {
                *self.notes.unknown.entry(format!("chat type {other}")).or_default() += 1;
                ConversationKind::Unknown
            }
        };
        let name = chat
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| (kind == ConversationKind::Notes).then(|| "Saved Messages".to_string()));
        // A one-chat export of a personal chat says nothing about its owner, but the chat's id is the other side's.
        if self.owner.is_none() && code == "personal_chat" {
            let other = format!("user{id}");
            let me = chat
                .get("messages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|m| m.get("from_id").and_then(Value::as_str))
                .find(|f| *f != other && f.starts_with("user"));
            if let Some(me) = me {
                self.owner = Some(me.to_string());
                self.notes.owner_inferred = true;
            }
        }

        let mut members: Vec<String> = Vec::new();
        let mut seen_ids: HashMap<String, usize> = HashMap::new();
        let mut created_ms = None;
        let mut out = Vec::new();
        let list = chat.get("messages").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        for (i, m) in list.iter().enumerate() {
            let Some(m) = m.as_object() else {
                self.notes.not_objects += 1;
                continue;
            };
            let mut message = self.message(&id, i, m);
            let n = seen_ids.entry(message.id.clone()).or_default();
            *n += 1;
            if *n > 1 {
                message.id = format!("{}#{n}", message.id);
                self.notes.duplicate_ids += 1;
            }
            if let Some(s) = &message.sender
                && !members.contains(s)
            {
                members.push(s.clone());
            }
            if created_ms.is_none() && matches!(&message.parts[..], [Part::System { event: Event::GroupCreated, .. }]) {
                created_ms = message.time_ms;
            }
            out.push(message);
        }
        (Conversation { id, kind, name, members, created_ms }, out)
    }

    fn person(&mut self, id: Option<&Value>, name: Option<&Value>) -> Option<String> {
        let name = name.and_then(Value::as_str).map(str::to_string);
        let id = match id.and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => format!("name:{}", name.as_deref()?),
        };
        let person = self.people.entry(id.clone()).or_insert_with(|| Person { id: id.clone(), ..Default::default() });
        if person.name.is_none() {
            person.name = name;
        }
        Some(id)
    }

    fn message(&mut self, chat: &str, index: usize, m: &Map<String, Value>) -> Message {
        let id = m.get("id").and_then(id_of).unwrap_or_else(|| format!("index:{index}"));
        let mut out = Message { id, conversation: chat.to_string(), ..Default::default() };
        out.time_ms = self.time(m.get("date_unixtime"), m.get("date"), true);
        if m.contains_key("edited") || m.contains_key("edited_unixtime") {
            out.edited = true;
            out.edited_ms = self.time(m.get("edited_unixtime"), m.get("edited"), false);
        }
        out.reply_to = m.get("reply_to_message_id").and_then(id_of);
        out.forwarded = m.get("forwarded_from").is_some_and(|f| !f.is_null());
        let kind = m.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "message" => {
                out.sender = self.person(m.get("from_id"), m.get("from"));
                out.parts = self.parts(m);
            }
            "service" => {
                out.sender = self.person(m.get("actor_id"), m.get("actor"));
                out.parts = vec![self.service(m)];
            }
            other => {
                let code = if other.is_empty() { "no type" } else { other };
                *self.notes.unknown.entry(format!("message type {code}")).or_default() += 1;
                out.sender = self.person(m.get("from_id"), m.get("from"));
                let text = text_of(m.get("text")).0;
                out.parts = vec![Part::Unknown { code: code.to_string(), text: Some(text).filter(|t| !t.is_empty()) }];
            }
        }
        out.reactions = reactions(m.get("reactions"));
        out
    }

    /// Milliseconds from `date_unixtime` (seconds, a string or a number), else from `date` read as UTC.
    fn time(&mut self, unix: Option<&Value>, wall: Option<&Value>, count: bool) -> Option<i64> {
        if let Some(secs) = unix.and_then(int_of) {
            self.notes.unix_times += count as usize;
            return secs.checked_mul(1000);
        }
        let parsed = wall.and_then(Value::as_str).and_then(wall_time);
        if count {
            match parsed {
                Some(_) => self.notes.wall_times += 1,
                None => self.notes.no_time += 1,
            }
        }
        parsed
    }

    fn parts(&mut self, m: &Map<String, Value>) -> Vec<Part> {
        let (text, links) = text_of(m.get("text"));
        let mut parts = Vec::new();
        let mut media = None;
        if let Some(photo) = m.get("photo").and_then(Value::as_str) {
            let mut x = Media { kind: MediaKind::Photo, ..file_facts(m, photo) };
            x.size = m.get("photo_file_size").and_then(int_of).and_then(|n| u64::try_from(n).ok()).or(x.size);
            media = Some(x);
        } else if m.contains_key("file") || m.contains_key("media_type") {
            let kind = match m.get("media_type").and_then(Value::as_str) {
                Some("voice_message") => MediaKind::Voice,
                Some("video_message") => MediaKind::VideoNote,
                Some("video_file") => MediaKind::Video,
                Some("animation") => MediaKind::Gif,
                Some("sticker") => MediaKind::Sticker,
                Some("audio_file") => MediaKind::Audio,
                None => MediaKind::Document,
                Some(other) => {
                    *self.notes.unknown.entry(format!("media type {other}")).or_default() += 1;
                    return vec![Part::Unknown { code: other.to_string(), text: Some(text).filter(|t| !t.is_empty()) }];
                }
            };
            let file = m.get("file").and_then(Value::as_str).unwrap_or("(File not included.)");
            media = Some(Media { kind, ..file_facts(m, file) });
        }
        if let Some(mut x) = media {
            x.caption = Some(text.clone()).filter(|t| !t.is_empty());
            parts.push(Part::Media(x));
        }
        if let Some(loc) = m.get("location_information").and_then(Value::as_object) {
            let lat = loc.get("latitude").and_then(Value::as_f64);
            let lng = loc.get("longitude").and_then(Value::as_f64);
            let name = ["place_name", "title", "address"]
                .iter()
                .find_map(|k| m.get(*k).and_then(Value::as_str))
                .map(str::to_string);
            let live = m.contains_key("live_location_period_seconds");
            parts.push(Part::Location { lat, lng, name, live });
        }
        if let Some(poll) = m.get("poll").and_then(Value::as_object) {
            let question = poll.get("question").map(|q| text_of(Some(q)).0).unwrap_or_default();
            let options = poll
                .get("answers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_object)
                .map(|a| PollOption {
                    text: a.get("text").map(|t| text_of(Some(t)).0).unwrap_or_default(),
                    voters: Vec::new(),
                    votes: a.get("voters").and_then(int_of).and_then(|n| u64::try_from(n).ok()).unwrap_or(0),
                })
                .collect();
            parts.push(Part::Poll { question, options, multiple: false });
        }
        if let Some(c) = m.get("contact_information").and_then(Value::as_object) {
            let phones = c.get("phone_number").and_then(Value::as_str).map(|p| vec![p.to_string()]).unwrap_or_default();
            parts.push(Part::Contact { name: full_name(c), phones, vcard: None });
        }
        if parts.is_empty() {
            parts.push(Part::text(text));
        } else if !matches!(parts[0], Part::Media(_)) && !text.is_empty() {
            parts.insert(0, Part::text(text));
        }
        parts.extend(links);
        parts
    }

    fn service(&mut self, m: &Map<String, Value>) -> Part {
        let action = m.get("action").and_then(Value::as_str).unwrap_or("");
        let members: Vec<&str> =
            m.get("members").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
        let title = m.get("title").and_then(Value::as_str);
        let text = if !members.is_empty() {
            Some(members.join(", "))
        } else {
            title.map(str::to_string).or_else(|| Some(text_of(m.get("text")).0).filter(|t| !t.is_empty()))
        };
        let secs = m.get("duration_seconds").and_then(int_of).and_then(|n| u64::try_from(n).ok());
        let event = match action {
            "phone_call" => {
                let outcome = match m.get("discard_reason").and_then(Value::as_str) {
                    Some("missed") => CallOutcome::Missed,
                    Some("busy") => CallOutcome::Busy,
                    Some("disconnect") => CallOutcome::Failed,
                    Some("hangup") if secs.is_some_and(|s| s > 0) => CallOutcome::Answered,
                    Some("hangup") => CallOutcome::Cancelled,
                    None if secs.is_some_and(|s| s > 0) => CallOutcome::Answered,
                    _ => CallOutcome::Unknown,
                };
                return Part::Call {
                    video: m.get("is_video").and_then(Value::as_bool).unwrap_or(false),
                    outcome,
                    secs,
                };
            }
            "group_call" => return Part::Call { video: false, outcome: CallOutcome::Unknown, secs },
            "create_group" | "create_channel" | "migrate_from_group" => Event::GroupCreated,
            "invite_members" => Event::MemberAdded,
            "join_group_by_link" | "join_group_by_request" => Event::MemberJoined,
            "remove_members" => Event::MemberRemoved,
            "edit_group_title" => Event::Renamed,
            "edit_group_photo" | "delete_group_photo" => Event::PictureChanged,
            "pin_message" => Event::Pinned,
            "clear_history" => Event::HistoryCleared,
            "set_messages_ttl" => Event::Disappearing,
            "migrate_to_supergroup" => Event::SettingsChanged,
            other => {
                let code = if other.is_empty() { "service" } else { other };
                *self.notes.unknown.entry(format!("action {code}")).or_default() += 1;
                return Part::Unknown { code: code.to_string(), text };
            }
        };
        Part::System { event, text }
    }
}

/// The facts of a `photo` or `file`: its path, or `omitted` when Telegram didn't include it.
fn file_facts(m: &Map<String, Value>, file: &str) -> Media {
    let num = |k: &str| m.get(k).and_then(int_of);
    let omitted = file.starts_with('(');
    Media {
        file: (!omitted && !file.is_empty()).then(|| file.to_string()),
        name: m.get("file_name").and_then(Value::as_str).map(str::to_string),
        mime: m.get("mime_type").and_then(Value::as_str).map(str::to_string),
        size: num("file_size").and_then(|n| u64::try_from(n).ok()),
        width: num("width").and_then(|n| u32::try_from(n).ok()),
        height: num("height").and_then(|n| u32::try_from(n).ok()),
        secs: num("duration_seconds").and_then(|n| u64::try_from(n).ok()),
        view_once: m.contains_key("self_destruct_period_seconds"),
        omitted,
        ..Default::default()
    }
}

/// `text` as a string or a list of strings and entities: the plain text, and its links as parts.
fn text_of(text: Option<&Value>) -> (String, Vec<Part>) {
    let mut out = String::new();
    let mut links = Vec::new();
    match text {
        Some(Value::String(s)) => out.push_str(s),
        Some(Value::Array(items)) => {
            for item in items {
                match item {
                    Value::String(s) => out.push_str(s),
                    Value::Object(e) => {
                        let t = e.get("text").and_then(Value::as_str).unwrap_or("");
                        out.push_str(t);
                        match e.get("type").and_then(Value::as_str) {
                            Some("link") if !t.is_empty() => links.push(Part::Link { url: t.to_string(), title: None }),
                            Some("text_link") => {
                                if let Some(href) = e.get("href").and_then(Value::as_str) {
                                    let title = Some(t.to_string()).filter(|t| !t.is_empty());
                                    links.push(Part::Link { url: href.to_string(), title });
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    (out, links)
}

fn reactions(value: Option<&Value>) -> Vec<Reaction> {
    let mut out = Vec::new();
    for r in value.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        let emoji = ["emoji", "document_id", "type"]
            .iter()
            .find_map(|k| r.get(*k).and_then(|v| v.as_str().map(str::to_string).or_else(|| id_of(v))))
            .unwrap_or_default();
        let recent: Vec<Option<String>> = r
            .get("recent")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|x| x.get("from_id").and_then(Value::as_str).map(str::to_string))
            .collect();
        let count = r.get("count").and_then(int_of).and_then(|n| u64::try_from(n).ok()).unwrap_or(0);
        let anonymous = count.saturating_sub(recent.len() as u64).min(MAX_ANONYMOUS_REACTIONS);
        let mut seen = HashSet::new();
        for by in recent {
            if by.as_ref().is_none_or(|b| seen.insert(b.clone())) {
                out.push(Reaction { by, from_me: false, emoji: emoji.clone() });
            }
        }
        for _ in 0..anonymous {
            out.push(Reaction { by: None, from_me: false, emoji: emoji.clone() });
        }
    }
    out
}

fn full_name(o: &Map<String, Value>) -> Option<String> {
    let part = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let name = format!("{} {}", part("first_name"), part("last_name")).trim().to_string();
    Some(name).filter(|n| !n.is_empty())
}

/// An id written as a number or a string.
fn id_of(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn int_of(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// `2024-01-31T21:41:05` as if UTC.
fn wall_time(s: &str) -> Option<i64> {
    let (date, time) = s.split_once('T')?;
    let mut d = date.splitn(3, '-').map(|x| x.parse::<i64>().ok());
    let (y, mo, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.splitn(3, ':').map(|x| x.get(..2).unwrap_or(x).parse::<u32>().ok());
    let (h, mi, sec) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    civil_to_ms(y, u32::try_from(mo).ok()?, u32::try_from(day).ok()?, h, mi, sec, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_times_and_text_entities() {
        assert_eq!(wall_time("1970-01-01T00:00:01"), Some(1000));
        assert_eq!(wall_time("1969-12-31T23:59:59"), Some(-1000));
        assert_eq!(wall_time("2024-02-30T00:00:00"), None);
        assert_eq!(wall_time("nonsense"), None);
        let v: Value = serde_json::from_str(r#"["a ", {"type": "bold", "text": "b"}, {"type": "text_link", "text": "c", "href": "https://example.org"}]"#)
            .expect("json");
        let (text, links) = text_of(Some(&v));
        assert_eq!(text, "a bc");
        assert_eq!(links, vec![Part::Link { url: "https://example.org".into(), title: Some("c".into()) }]);
    }
}
