//! Viber's "Export message history" (decision 0015): a zip of CSV files, one per chat, or one CSV. Each row is a
//! message: date, time, sender, phone, text; the header (when there is one) names the columns in any order and case,
//! and with none they're read in that order. Media are the app's own sentences ("Photo message"): msgkinds' `viber_csv` rows read them as a file not attached.
//!
//! Times are the phone's wall clock, read as UTC (`wall_clock`); the day/month order is decided from each file (a
//! first number over 12: day first; a second: month first; else day first, noted). The sender "Me" is the owner. A
//! zip is read in memory; entries with `..` or absolute paths are ignored.

use msgkinds::Note;
use msgkinds::media_type::formats::{CSV, ZIP};
use msgkinds::media_type::name::has_format;
use msgkinds::media_type::sources::viber_csv;
use msgkinds::note::Value;
use msgkinds::wording::codes::viber_csv as col;
use oopz_model::messages::Message as Msg;
use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use oopz_model::{
    Adapter, App, Conversation, ConversationKind, Error, Loaded, Media, MediaKind, Message, Part, Person, Result,
    Secret, Source, SourceInfo, civil_to_ms, normalize_phone,
};

pub const ID: &str = "viber-csv";
const FORMAT: &str = "Viber message history (CSV)";
const MAX_ENTRY: u64 = 1 << 30;
/// The owner's person id.
pub const ME: &str = "me";
/// The owner's name as the export shows it (in English), and as the person is named.
const ME_NAME: &str = "Me";

pub struct ViberExportAdapter;

impl Adapter for ViberExportAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn app(&self) -> App {
        App::Viber
    }

    fn secret(&self) -> Secret {
        Secret::None
    }

    fn detect(&self, path: &Path) -> bool {
        let name = path.to_string_lossy();
        let Ok(file) = std::fs::File::open(path) else { return false };
        if has_format(&name, ZIP) {
            let Ok(zip) = zip::ZipArchive::new(file) else { return false };
            let names: Vec<String> = zip.file_names().flatten().map(|n| n.into_owned()).collect();
            return !names.is_empty() && names.iter().all(|n| has_format(n, CSV) || n.ends_with('/'));
        }
        if !has_format(&name, CSV) {
            return false;
        }
        let mut head = Vec::new();
        if file.take(4096).read_to_end(&mut head).is_err() {
            return false;
        }
        let text = decode(&head);
        let first = text.lines().next().unwrap_or("");
        let columns: Vec<_> = first.split([',', ';', '\t']).filter_map(viber_csv::column).collect();
        (columns.contains(&col::DATE) && columns.contains(&col::MESSAGE))
            || date(first.split([',', ';', '\t']).next().unwrap_or("")).is_some()
    }

    fn open(&self, path: &Path, _secret: Option<&str>) -> Result<Box<dyn Source>> {
        let bytes = std::fs::read(path).map_err(|e| Error::io(path.display(), e))?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Box::new(read(&name, &bytes)?))
    }
}

fn decode(bytes: &[u8]) -> String {
    msgkinds::media_type::text::decode(bytes)
}

/// Reads an export from memory: a zip of CSV files (by its first bytes) or one CSV named `name`.
pub fn read(name: &str, bytes: &[u8]) -> Result<Loaded> {
    let mut book = Book::default();
    if msgkinds::media_type::sniff(bytes, msgkinds::media_type::Hint::None).is_some_and(|s| s.format.mime == ZIP) {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| Error::NotThisFormat(FORMAT))?;
        let mut refused = 0;
        for i in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(i) else { continue };
            let Some(path) = entry.enclosed_name() else {
                book.unsafe_names += 1;
                continue;
            };
            let entry_name = path.to_string_lossy().into_owned();
            if entry.is_dir() || !has_format(&entry_name, CSV) {
                continue;
            }
            let mut raw = Vec::new();
            if (&mut entry).take(MAX_ENTRY).read_to_end(&mut raw).is_err() {
                refused += 1;
                continue;
            }
            if book.csv(&entry_name, &decode(&raw)).is_err() {
                refused += 1;
            }
        }
        if book.conversations.is_empty() {
            return Err(Error::NotThisFormat(FORMAT));
        }
        if refused > 0 {
            book.notes.push(Note::of(Msg::ViberNotCsv, [refused.to_string()]).to_string());
        }
    } else {
        book.csv(name, &decode(bytes))?;
    }
    Ok(book.finish())
}

#[derive(Default)]
struct Book {
    people: BTreeMap<String, Person>,
    conversations: Vec<Conversation>,
    messages: Vec<Message>,
    notes: Vec<String>,
    unsafe_names: usize,
    me_seen: bool,
    /// msgkinds' `viber_csv` rows; its report joins the notes. The CSV doesn't say which Viber wrote it.
    reading: msgkinds::Reading,
}

#[derive(Clone, Copy, Default)]
struct Columns {
    date: Option<usize>,
    time: Option<usize>,
    sender: Option<usize>,
    phone: Option<usize>,
    message: Option<usize>,
}

impl Columns {
    fn from_header(fields: &[String]) -> Option<Columns> {
        let mut c = Columns::default();
        for (i, f) in fields.iter().enumerate() {
            let slot = match viber_csv::column(f) {
                Some(col::DATE) => &mut c.date,
                Some(col::TIME) => &mut c.time,
                Some(col::PHONE) => &mut c.phone,
                Some(col::SENDER) => &mut c.sender,
                Some(col::MESSAGE) => &mut c.message,
                _ => continue,
            };
            slot.get_or_insert(i);
        }
        (c.date.is_some() && c.message.is_some()).then_some(c)
    }

    /// No header: date, time, sender, phone, message; four fields leave out the phone.
    fn positional(len: usize) -> Columns {
        if len == 4 {
            Columns { date: Some(0), time: Some(1), sender: Some(2), phone: None, message: Some(3) }
        } else {
            Columns { date: Some(0), time: Some(1), sender: Some(2), phone: Some(3), message: Some(4) }
        }
    }
}

/// A date's three numbers as written: `31/12/2023`, `12/31/23`, `31.12.2023`, `2023-12-31`.
#[derive(Clone, Copy)]
struct DateRaw {
    a: u32,
    b: u32,
    c: u32,
    year_first: bool,
    two_digit_year: bool,
}

fn date(s: &str) -> Option<DateRaw> {
    let s = s.trim().trim_matches('"').trim_end_matches('.');
    let mut parts = s.split(['/', '.', '-']);
    let (a, b, c) = (parts.next()?.trim(), parts.next()?.trim(), parts.next()?.trim());
    if parts.next().is_some() {
        return None;
    }
    let num = |x: &str| {
        (!x.is_empty() && x.len() <= 4 && x.bytes().all(|b| b.is_ascii_digit())).then(|| x.parse().ok()).flatten()
    };
    let (na, nb, nc): (u32, u32, u32) = (num(a)?, num(b)?, num(c)?);
    let year_first = a.len() == 4;
    if !year_first && c.len() != 2 && c.len() != 4 || b.len() > 2 {
        return None;
    }
    Some(DateRaw { a: na, b: nb, c: nc, year_first, two_digit_year: !year_first && c.len() == 2 })
}

/// `21:41`, `21:41:05`, `9:41 PM`, `9:41 p. m.` on a 24-hour clock (hour 99 for a 12-hour time that doesn't exist).
fn time(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim();
    let digits_end = s.find(|c: char| !(c.is_ascii_digit() || c == ':')).unwrap_or(s.len());
    let (clock, marker) = s.split_at(digits_end);
    let mut it = clock.split(':');
    let h: u32 = it.next()?.parse().ok()?;
    let m = it.next()?;
    if m.len() != 2 {
        return None;
    }
    let m: u32 = m.parse().ok()?;
    let sec: u32 = match it.next() {
        Some(x) => x.parse().ok()?,
        None => 0,
    };
    let marker: String = marker.chars().filter(|c| c.is_alphabetic()).collect::<String>().to_lowercase();
    let h = match marker.as_str() {
        "" => h,
        "pm" if (1..12).contains(&h) => h + 12,
        "am" if h == 12 => 0,
        "am" | "pm" if (1..=12).contains(&h) => h,
        _ => 99,
    };
    Some((h, m, sec))
}

impl Book {
    /// One CSV file: one conversation. What a message column holds comes from msgkinds (`viber_csv`): text, a link,
    /// or one of Viber's sentences for a file it didn't attach ("Photo message").
    fn csv(&mut self, name: &str, text: &str) -> Result<()> {
        let first_line = text.lines().next().unwrap_or("");
        let delimiter =
            b",;\t".iter().copied().max_by_key(|d| first_line.bytes().filter(|b| b == d).count()).unwrap_or(b',');
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(text.as_bytes());
        let mut rows: Vec<(usize, Vec<String>)> = Vec::new();
        let mut broken = 0;
        for record in reader.records() {
            match record {
                Ok(r) => {
                    let line = r.position().map(|p| p.line() as usize).unwrap_or(rows.len() + 1);
                    rows.push((line, r.iter().map(str::to_string).collect()));
                }
                Err(_) => broken += 1,
            }
        }
        let Some((_, first)) = rows.first() else { return Err(Error::NotThisFormat(FORMAT)) };
        let (columns, skip) = match Columns::from_header(first) {
            Some(c) => (Some(c), 1),
            None if first.first().is_some_and(|f| date(f).is_some()) => (None, 0),
            None => return Err(Error::NotThisFormat(FORMAT)),
        };

        let stem = msgkinds::media_type::name::stem(name);
        let mut id = stem.to_string();
        let mut n = 1;
        while self.conversations.iter().any(|c| c.id == id) {
            n += 1;
            id = format!("{stem}#{n}");
        }
        let title = stem.strip_prefix("Viber ").unwrap_or(stem).trim().to_string();

        // The day/month order of this file.
        let dates: Vec<Option<DateRaw>> = rows
            .iter()
            .skip(skip)
            .map(|(_, f)| {
                let c = columns.unwrap_or_else(|| Columns::positional(f.len()));
                c.date.and_then(|i| f.get(i)).and_then(|d| date(d.split_whitespace().next().unwrap_or("")))
            })
            .collect();
        let (mut day, mut month) = (false, false);
        for d in dates.iter().flatten().filter(|d| !d.year_first) {
            day |= d.a > 12;
            month |= d.b > 12;
        }
        let month_first = month && !day;
        if day == month && dates.iter().flatten().any(|d| !d.year_first) {
            self.notes.push(in_chat(&title, Note::of(Msg::DayMonthUndecidedDayFirst, [])));
        }

        let (mut short, mut no_time) = (0, 0);
        let mut members: Vec<String> = Vec::new();
        for ((line, fields), d) in rows.into_iter().skip(skip).zip(dates) {
            let c = columns.unwrap_or_else(|| Columns::positional(fields.len()));
            let get = |i: Option<usize>| i.and_then(|i| fields.get(i)).map(|s| s.trim()).unwrap_or("");
            let mut m = Message { id: format!("line:{line}"), conversation: id.clone(), ..Default::default() };
            if fields.len() < 3 {
                short += 1;
                m.parts.push(Part::Unknown { code: "row".into(), text: Some(fields.join(",")) });
                self.messages.push(m);
                continue;
            }
            let date_field = get(c.date);
            let time_field = match get(c.time) {
                "" => date_field.split_once(' ').map(|(_, t)| t).unwrap_or(""),
                t => t,
            };
            m.time_ms = match (d, time(time_field)) {
                (Some(d), Some((h, mi, s))) => {
                    let (y, mo, dd) = if d.year_first {
                        (d.a as i64, d.b, d.c)
                    } else {
                        let y = if d.two_digit_year { 2000 + d.c as i64 } else { d.c as i64 };
                        if month_first { (y, d.a, d.b) } else { (y, d.b, d.a) }
                    };
                    civil_to_ms(y, mo, dd, h, mi, s, 0)
                }
                _ => None,
            };
            no_time += m.time_ms.is_none() as usize;
            let sender = get(c.sender);
            let phone = get(c.phone);
            let from_me = viber_csv::is_owner(sender);
            let person = if from_me {
                self.me_seen = true;
                ME.to_string()
            } else {
                let normalized = normalize_phone(phone);
                let pid = normalized
                    .clone()
                    .unwrap_or_else(|| if sender.is_empty() { "unknown".into() } else { sender.into() });
                let p = self.people.entry(pid.clone()).or_insert_with(|| Person {
                    id: pid.clone(),
                    phone: normalized,
                    ..Default::default()
                });
                if p.name.is_none() && !sender.is_empty() {
                    p.name = Some(sender.to_string());
                }
                pid
            };
            if !members.contains(&person) {
                members.push(person.clone());
            }
            m.sender = Some(person);
            m.from_me = from_me;
            // Columns past the message are the message's own commas, unquoted.
            let mut text = get(c.message).to_string();
            if columns.is_none()
                && let Some(i) = c.message
            {
                for extra in fields.iter().skip(i + 1) {
                    text.push(delimiter as char);
                    text.push_str(extra);
                }
            }
            let code = msgkinds::media_type::sources::viber_csv::content(&text);
            let row = self.reading.lookup_name(msgkinds::Source::ViberCsv, code).entry();
            if let Some(e) = row.filter(|e| e.has(msgkinds::Flag::Omitted))
                && let Some(kind) = MediaKind::from_kind(e.kind.into())
            {
                m.parts.push(Part::Media(Media { kind, omitted: true, ..Default::default() }));
                self.messages.push(m);
                continue;
            }
            m.parts.push(Part::text(text.clone()));
            let links = msgkinds::media_type::sources::whatsapp_export::links(&text);
            m.parts.extend(links.into_iter().map(|url| Part::Link { url: url.to_string(), title: None }));
            self.messages.push(m);
        }
        if short > 0 {
            self.notes.push(in_chat(&title, Note::of(Msg::RowsTooShort, [short.to_string()])));
        }
        if no_time > 0 {
            self.notes.push(in_chat(&title, Note::of(Msg::RowsBadDate, [no_time.to_string()])));
        }
        if broken > 0 {
            self.notes.push(in_chat(&title, Note::of(Msg::RowsUnsplit, [broken.to_string()])));
        }
        let kind = if members.len() > 2 { ConversationKind::Group } else { ConversationKind::Direct };
        self.conversations.push(Conversation {
            id,
            kind,
            name: Some(title).filter(|t| !t.is_empty()),
            members,
            created_ms: None,
        });
        Ok(())
    }

    fn finish(mut self) -> Loaded {
        let mut notes = vec![Note::of(Msg::WallClockUtc, []).to_string()];
        notes.append(&mut self.notes);
        if self.unsafe_names > 0 {
            notes.push(Note::of(Msg::UnsafeZipEntries, [self.unsafe_names.to_string()]).to_string());
        }
        notes.extend(self.reading.report().lines());
        let owner =
            self.me_seen.then(|| Person { id: ME.into(), name: Some(ME_NAME.into()), me: true, ..Default::default() });
        match &owner {
            Some(me) => {
                self.people.insert(ME.into(), me.clone());
            }
            None => notes.push(Note::of(Msg::ViberNoMe, [ME_NAME.to_owned()]).to_string()),
        }
        Loaded {
            info: SourceInfo {
                app: App::Viber,
                adapter: ID.into(),
                format: None,
                owner,
                media_root: None,
                wall_clock: true,
                notes,
            },
            people: self.people.into_values().collect(),
            conversations: self.conversations,
            messages: self.messages,
        }
    }
}

/// A note about one chat: its title, then the note (worded in the same language).
fn in_chat(title: &str, note: Note) -> String {
    Note::new(Msg::InChat, vec![Value::Text(title.to_owned()), Value::Note(Box::new(note))]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_times() {
        assert!(date("31/12/2023").is_some_and(|d| d.a == 31 && !d.two_digit_year));
        assert!(date("1/2/24").is_some_and(|d| d.two_digit_year));
        assert!(date("2023-12-31").is_some_and(|d| d.year_first));
        assert!(date("hello").is_none());
        assert!(date("1/2/345").is_none());
        assert_eq!(time("21:41"), Some((21, 41, 0)));
        assert_eq!(time("9:41 PM"), Some((21, 41, 0)));
        assert_eq!(time("12:05\u{A0}a. m."), Some((0, 5, 0)));
        assert_eq!(time("13:00 PM"), Some((99, 0, 0)));
        assert_eq!(time("9:5"), None);
    }
}
