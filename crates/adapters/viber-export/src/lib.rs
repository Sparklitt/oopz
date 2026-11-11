//! Viber's "Export message history" (decision 0015): a zip of CSV files, one per chat, or one CSV. Each row is a
//! message: date, time, sender, phone, text; the header (when there is one) names the columns in any order and case,
//! and with none they're read in that order. Media are the app's own words ("Photo message") and kept as text.
//!
//! Times are the phone's wall clock, read as UTC (`wall_clock`); the day/month order is decided from each file (a
//! first number over 12: day first; a second: month first; else day first, noted). The sender "Me" is the owner. A
//! zip is read in memory; entries with `..` or absolute paths are ignored.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use oopz_model::{
    Adapter, App, Conversation, ConversationKind, Error, Loaded, Message, Part, Person, Result, Secret, Source,
    SourceInfo, civil_to_ms, normalize_phone,
};

pub const ID: &str = "viber-csv";
const FORMAT: &str = "Viber message history (CSV)";
const MAX_ENTRY: u64 = 1 << 30;
/// The owner's person id.
pub const ME: &str = "me";

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
        let lower = path.to_string_lossy().to_lowercase();
        let Ok(file) = std::fs::File::open(path) else { return false };
        if lower.ends_with(".zip") {
            let Ok(zip) = zip::ZipArchive::new(file) else { return false };
            let names: Vec<String> = zip.file_names().flatten().map(|n| n.to_lowercase()).collect();
            return !names.is_empty() && names.iter().all(|n| n.ends_with(".csv") || n.ends_with('/'));
        }
        if !lower.ends_with(".csv") {
            return false;
        }
        let mut head = Vec::new();
        if file.take(4096).read_to_end(&mut head).is_err() {
            return false;
        }
        let text = decode(&head);
        let first = text.lines().next().unwrap_or("");
        let lower = first.to_lowercase();
        (lower.contains("date") && (lower.contains("message") || lower.contains("text")))
            || date(first.split([',', ';', '\t']).next().unwrap_or("")).is_some()
    }

    fn open(&self, path: &Path, _secret: Option<&str>) -> Result<Box<dyn Source>> {
        let bytes = std::fs::read(path).map_err(|e| Error::io(path.display(), e))?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Box::new(read(&name, &bytes)?))
    }
}

fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)).into_owned()
}

/// Reads an export from memory: a zip of CSV files (by its first bytes) or one CSV named `name`.
pub fn read(name: &str, bytes: &[u8]) -> Result<Loaded> {
    let mut book = Book::default();
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| Error::NotThisFormat(FORMAT))?;
        let mut refused = 0;
        for i in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(i) else { continue };
            let Some(path) = entry.enclosed_name() else {
                book.unsafe_names += 1;
                continue;
            };
            let entry_name = path.to_string_lossy().into_owned();
            if entry.is_dir() || !entry_name.to_lowercase().ends_with(".csv") {
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
            book.notes.push(format!("{refused} files in the zip aren't Viber CSV: skipped"));
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
            let f = f.trim().trim_start_matches('\u{FEFF}').to_lowercase();
            let slot = if f.contains("date") {
                &mut c.date
            } else if f.contains("time") {
                &mut c.time
            } else if f.contains("phone") || f.contains("number") {
                &mut c.phone
            } else if f.contains("sender") || f.contains("name") {
                &mut c.sender
            } else if f.contains("message") || f.contains("text") {
                &mut c.message
            } else {
                continue;
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
    /// One CSV file: one conversation.
    /// TODO(msgkinds): msgkinds lists Viber Desktop's database (`Source::ViberDesktop`), not this CSV export yet:
    /// switch once it has a `viber_csv` source; a row neither reads stays an unknown part.
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

        let stem = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let stem = stem.strip_suffix(".csv").or_else(|| stem.strip_suffix(".CSV")).unwrap_or(stem);
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
            self.notes.push(format!("{title}: no date decides the day/month order: read as day first"));
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
            let from_me = ["me", "yo", "ich", "moi", "eu", "я"].contains(&sender.to_lowercase().as_str());
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
            m.parts.push(Part::text(text.clone()));
            for word in text.split(char::is_whitespace) {
                if let Some(at) = word.find("https://").or_else(|| word.find("http://")) {
                    let url = word[at..].trim_end_matches(['.', ',', ')', '!', '?', '"', '\'']);
                    if url.len() > 8 {
                        m.parts.push(Part::Link { url: url.to_string(), title: None });
                    }
                }
            }
            self.messages.push(m);
        }
        if short > 0 {
            self.notes.push(format!("{title}: {short} rows too short to be a message kept as unknown"));
        }
        if no_time > 0 {
            self.notes
                .push(format!("{title}: {no_time} rows with a date or time oopz can't read: kept without a time"));
        }
        if broken > 0 {
            self.notes.push(format!("{title}: {broken} rows the CSV reader couldn't split"));
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
        let mut notes =
            vec!["times are the phone's local time, read as UTC: the export doesn't say its time zone".to_string()];
        notes.append(&mut self.notes);
        if self.unsafe_names > 0 {
            notes.push(format!("{} zip entries with unsafe paths ignored", self.unsafe_names));
        }
        let owner =
            self.me_seen.then(|| Person { id: ME.into(), name: Some("Me".into()), me: true, ..Default::default() });
        match &owner {
            Some(me) => {
                self.people.insert(ME.into(), me.clone());
            }
            None => notes.push("no row is from \"Me\": pick yourself".into()),
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
