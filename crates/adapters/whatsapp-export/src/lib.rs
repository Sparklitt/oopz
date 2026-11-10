//! WhatsApp's own "Export chat" (decision 0015): the chat as text, alone (`WhatsApp Chat with X.txt`), zipped with
//! its media (`WhatsApp Chat - X.zip`, iPhone's `_chat.txt` inside), or unpacked in a folder. Read in memory: a zip is
//! never unpacked to disk, and entries with `..` or absolute paths are ignored.
//!
//! The text has no time zone, no year format, no day/month order and no owner: times are read as if UTC (0013 point
//! 2), the day/month order is decided from the whole file (0015 point 3), and the owner is marked only where the
//! export gives them away ("You deleted this message", a sender called "You"). Every line is kept: a line that isn't
//! a message's first line continues the message before it, and lines before the first message become unknown parts.
//! What had to be read around is counted in the source's notes, never with message text.

mod content;
mod header;

use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;

use oopz_model::{
    Adapter, App, Conversation, ConversationKind, Error, Event, Loaded, Message, Part, Person, Result, Secret, Source,
    SourceInfo, civil_to_ms, normalize_phone,
};

pub use content::Files;
use header::Header;

pub const ID: &str = "whatsapp-export";
const FORMAT: &str = "WhatsApp chat export";
/// The biggest chat text or contact card read from a zip: more is a damaged or hostile file.
const MAX_ENTRY: u64 = 1 << 30;

pub struct WhatsAppExportAdapter;

impl Adapter for WhatsAppExportAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn app(&self) -> App {
        App::WhatsApp
    }

    fn secret(&self) -> Secret {
        Secret::None
    }

    fn detect(&self, path: &Path) -> bool {
        if path.is_dir() {
            return chat_file_in(path).is_some();
        }
        let name = file_name(path);
        let lower = name.to_lowercase();
        if lower.ends_with(".zip") {
            if chat_name(&name).is_some_and(|n| n != name) {
                return true;
            }
            let Ok(file) = std::fs::File::open(path) else { return false };
            let Ok(zip) = zip::ZipArchive::new(file) else { return false };
            return zip.file_names().flatten().any(|n| is_chat_text(&n));
        }
        if !lower.ends_with(".txt") {
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else { return false };
        let mut head = Vec::new();
        if file.take(4096).read_to_end(&mut head).is_err() {
            return false;
        }
        decode(&head).lines().filter(|l| !l.trim().is_empty()).take(5).any(|l| Header::parse(l).is_some())
    }

    fn open(&self, path: &Path, _secret: Option<&str>) -> Result<Box<dyn Source>> {
        let loaded = if path.is_dir() {
            read_folder(path)?
        } else {
            let bytes = std::fs::read(path).map_err(|e| Error::io(path.display(), e))?;
            let mut loaded = read(&file_name(path), &bytes)?;
            if !bytes.starts_with(b"PK") {
                loaded.info.media_root = path.parent().map(Path::to_path_buf);
            }
            loaded
        };
        Ok(Box::new(loaded))
    }
}

/// Reads an export from memory: a zip (by its first bytes) or the chat text. `name` is the file's name, which holds
/// the chat's name.
pub fn read(name: &str, bytes: &[u8]) -> Result<Loaded> {
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return read_zip(name, bytes);
    }
    read_text(chat_name(name), &decode(bytes), &Files::default())
}

/// Reads a chat's text. `files` are the media files beside it, when known (a zip or a folder).
pub fn read_text(chat: Option<String>, text: &str, files: &Files) -> Result<Loaded> {
    parse(chat, text, files)
}

fn read_zip(name: &str, bytes: &[u8]) -> Result<Loaded> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| Error::NotThisFormat(FORMAT))?;
    let mut files = Files { names: Some(HashSet::new()), ..Default::default() };
    let mut chat: Option<(String, String)> = None;
    let mut unsafe_names = 0;
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else { continue };
        if entry.is_dir() {
            continue;
        }
        let Some(path) = entry.enclosed_name() else {
            unsafe_names += 1;
            continue;
        };
        let entry_name = path.to_string_lossy().replace('\\', "/");
        let base = entry_name.rsplit('/').next().unwrap_or(&entry_name).to_string();
        let lower = base.to_lowercase();
        let is_chat = is_chat_text(&entry_name) || (chat.is_none() && lower.ends_with(".txt"));
        if is_chat || lower.ends_with(".vcf") {
            let mut raw = Vec::new();
            if (&mut entry).take(MAX_ENTRY).read_to_end(&mut raw).is_err() {
                continue;
            }
            if lower.ends_with(".vcf") {
                files.vcards.insert(base.clone(), decode(&raw));
            } else if chat.as_ref().is_none_or(|(n, _)| !is_chat_text(n)) {
                chat = Some((entry_name.clone(), decode(&raw)));
            }
        }
        if let Some(names) = files.names.as_mut() {
            names.insert(base);
        }
    }
    let Some((chat_entry, text)) = chat else { return Err(Error::NotThisFormat(FORMAT)) };
    let chat_title = chat_name(&chat_entry).filter(|_| !chat_entry.ends_with("_chat.txt")).or_else(|| chat_name(name));
    let mut loaded = parse(chat_title, &text, &files)?;
    loaded.info.notes.push("media files are inside the zip".into());
    if unsafe_names > 0 {
        loaded.info.notes.push(format!("{unsafe_names} zip entries with unsafe paths ignored"));
    }
    Ok(loaded)
}

fn read_folder(dir: &Path) -> Result<Loaded> {
    let chat_file = chat_file_in(dir).ok_or(Error::NotThisFormat(FORMAT))?;
    let bytes = std::fs::read(&chat_file).map_err(|e| Error::io(chat_file.display(), e))?;
    let mut files = Files { names: Some(HashSet::new()), ..Default::default() };
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir.display(), e))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(".vcf")
            && let Ok(raw) = std::fs::read(entry.path())
        {
            files.vcards.insert(name.clone(), decode(&raw));
        }
        if let Some(names) = files.names.as_mut() {
            names.insert(name);
        }
    }
    let name = file_name(&chat_file);
    let title = if name == "_chat.txt" { chat_name(&file_name(dir)) } else { chat_name(&name) };
    let mut loaded = parse(title, &decode(&bytes), &files)?;
    loaded.info.media_root = Some(dir.to_path_buf());
    Ok(loaded)
}

fn chat_file_in(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let names: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.iter().find(|n| is_chat_text(n)).map(|n| dir.join(n))
}

fn is_chat_text(name: &str) -> bool {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let lower = base.to_lowercase();
    lower.ends_with(".txt")
        && (base == "_chat.txt"
            || PREFIXES.iter().any(|p| lower.starts_with(&p.to_lowercase()))
            || base.contains("WhatsApp"))
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

const PREFIXES: [&str; 8] = [
    "WhatsApp Chat with ",
    "WhatsApp Chat - ",
    "Chat de WhatsApp con ",
    "Chat de WhatsApp - ",
    "WhatsApp-Chat mit ",
    "Discussion WhatsApp avec ",
    "Conversa do WhatsApp com ",
    "WhatsApp-chat met ",
];

/// The chat's name from an export's file name: `WhatsApp Chat with X.txt`, `Chat de WhatsApp con X.txt`,
/// `WhatsApp Chat - X.zip`; any other name without its extension; `None` for iPhone's `_chat.txt` or nothing.
pub fn chat_name(file: &str) -> Option<String> {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let stem = base.strip_suffix(".txt").or_else(|| base.strip_suffix(".zip")).unwrap_or(base);
    if stem.is_empty() || stem == "_chat" {
        return None;
    }
    let name = PREFIXES.iter().find_map(|p| stem.strip_prefix(p)).unwrap_or(stem);
    Some(name.trim().to_string()).filter(|n| !n.is_empty())
}

/// Text from bytes: UTF-16 when it starts with its byte-order mark, else UTF-8 (a BOM dropped, bad bytes replaced).
fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect();
        return String::from_utf16_lossy(&units);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// One message as written, before its date is read.
struct Raw {
    line: usize,
    header: Option<Header>,
    sender: Option<String>,
    text: String,
    /// iPhone writes the app's notices as the chat's own lines, starting with U+200E.
    marked: bool,
}

fn parse(chat: Option<String>, text: &str, files: &Files) -> Result<Loaded> {
    let mut raws: Vec<Raw> = Vec::new();
    let mut seen_header = false;
    let mut leading = 0;
    for (i, line) in text.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some((header, rest)) = Header::parse(line) {
            seen_header = true;
            let (sender, body) = split_sender(rest);
            let marked = body.starts_with('\u{200E}');
            raws.push(Raw { line: i + 1, header: Some(header), sender, text: body.to_string(), marked });
        } else if let Some(last) = raws.last_mut().filter(|_| seen_header) {
            last.text.push('\n');
            last.text.push_str(line);
        } else if !line.trim().is_empty() {
            leading += 1;
            if leading > 5 {
                return Err(Error::NotThisFormat(FORMAT));
            }
            raws.push(Raw { line: i + 1, header: None, sender: None, text: line.to_string(), marked: false });
        }
    }
    if !seen_header {
        return Err(Error::NotThisFormat(FORMAT));
    }

    let mut notes =
        vec!["times are the phone's local time, read as UTC: the export doesn't say its time zone".to_string()];
    let order = header::decide_order(raws.iter().filter_map(|r| r.header.as_ref()));
    if let Some(note) = order.note() {
        notes.push(note.into());
    }

    let conversation = chat.clone().unwrap_or_else(|| "chat".into());
    let mut stats = content::Stats::default();
    let mut messages = Vec::with_capacity(raws.len());
    let mut owner_hints: Vec<String> = Vec::new();
    let mut bad_dates = 0;
    let mut group_event = false;
    for raw in raws {
        let mut text = raw.text;
        while text.ends_with('\n') {
            text.pop();
        }
        let mut m =
            Message { id: format!("line:{}", raw.line), conversation: conversation.clone(), ..Default::default() };
        let Some(h) = raw.header else {
            m.parts.push(Part::Unknown { code: "line".into(), text: Some(text) });
            stats.unknown_lines += 1;
            messages.push(m);
            continue;
        };
        m.time_ms = order.date(&h).and_then(|(y, mo, d)| civil_to_ms(y, mo, d, h.hour, h.minute, h.second, 0));
        if m.time_ms.is_none() {
            bad_dates += 1;
        }
        match content::read(&text, files, &mut stats) {
            content::Read::Deleted { own } => {
                if own && let Some(s) = &raw.sender {
                    owner_hints.push(s.clone());
                }
                m.sender = raw.sender;
                m.deleted = true;
            }
            content::Read::Parts { parts, edited }
                if raw.sender.is_some() && !(raw.marked && content::is_notice(&parts)) =>
            {
                if raw.sender.as_deref().is_some_and(|s| matches!(s, "You" | "Tú" | "Tu" | "Você")) {
                    owner_hints.extend(raw.sender.clone());
                }
                m.sender = raw.sender;
                m.parts = parts;
                m.edited = edited;
            }
            content::Read::Parts { .. } => {
                let event = content::event(&text);
                group_event |= matches!(
                    event,
                    Event::GroupCreated
                        | Event::Renamed
                        | Event::DescriptionChanged
                        | Event::PictureChanged
                        | Event::MemberAdded
                        | Event::MemberJoined
                        | Event::MemberLeft
                        | Event::MemberRemoved
                        | Event::AdminChanged
                );
                m.parts.push(Part::System { event, text: Some(text.trim_start_matches(header::MARKS).to_string()) });
            }
        }
        messages.push(m);
    }
    if bad_dates > 0 {
        notes.push(format!("{bad_dates} lines with a date or time that doesn't exist: kept without a time"));
    }
    stats.notes(&mut notes);

    owner_hints.sort();
    owner_hints.dedup();
    let owner = match owner_hints.as_slice() {
        [one] => Some(one.clone()),
        [] => {
            notes.push("the export doesn't say who the owner is: pick yourself".into());
            None
        }
        _ => {
            notes.push("more than one sender looks like the owner: pick yourself".into());
            None
        }
    };

    let mut order_of: BTreeMap<String, usize> = BTreeMap::new();
    for m in &mut messages {
        if let Some(s) = &m.sender {
            m.from_me = owner.as_deref() == Some(s.as_str());
            let n = order_of.len();
            order_of.entry(s.clone()).or_insert(n);
        }
    }
    let mut senders: Vec<(usize, String)> = order_of.into_iter().map(|(s, i)| (i, s)).collect();
    senders.sort();
    let people: Vec<Person> = senders
        .iter()
        .map(|(_, s)| Person {
            id: s.clone(),
            phone: looks_like_phone(s).then(|| normalize_phone(s)).flatten(),
            name: Some(s.clone()),
            me: owner.as_deref() == Some(s.as_str()),
            ..Default::default()
        })
        .collect();
    let kind = if senders.len() > 2 || group_event { ConversationKind::Group } else { ConversationKind::Direct };
    let conversations = vec![Conversation {
        id: conversation,
        kind,
        name: chat,
        members: senders.into_iter().map(|(_, s)| s).collect(),
        created_ms: None,
    }];
    let owner_person = people.iter().find(|p| p.me).cloned();
    Ok(Loaded {
        info: SourceInfo {
            app: App::WhatsApp,
            adapter: ID.into(),
            format: None,
            owner: owner_person,
            media_root: None,
            wall_clock: true,
            notes,
        },
        people,
        conversations,
        messages,
    })
}

/// `Name: text` → the sender and the text; anything else is the app's notice. A name never holds quotes or line
/// breaks (a notice like `X changed the subject to "a: b"` does).
fn split_sender(rest: &str) -> (Option<String>, &str) {
    let (name, text) = match rest.split_once(": ") {
        Some(x) => x,
        None => match rest.strip_suffix(':') {
            Some(name) => (name, ""),
            None => return (None, rest),
        },
    };
    let name = name.trim_matches(header::MARKS);
    let ok = !name.is_empty()
        && name.chars().count() <= 100
        && !name.contains(['"', '“', '”', '«', '»'])
        && !name.trim().is_empty();
    if ok { (Some(name.to_string()), text) } else { (None, rest) }
}

/// A sender written as a number: someone not in the phone's contacts.
fn looks_like_phone(s: &str) -> bool {
    let s = s.trim_matches(header::MARKS);
    s.starts_with('+') && s.chars().all(|c| c.is_ascii_digit() || " +-()\u{A0}".contains(c))
}
