//! SMS Backup & Restore's XML (decision 0015): `sms-….xml` (`<smses>`: `<sms>` and `<mms>` with their parts and
//! addresses) and `calls-….xml` (`<calls>`: `<call>`). Times are real UTC milliseconds. Conversations are keyed by
//! the other side's number in one form ([`normalize_phone`]), so the same person written `+999 9000 0002` and
//! `+99990000002` is one conversation, and a group MMS by its set of numbers.
//!
//! The app writes characters beyond the Basic Multilingual Plane (most emoji) as two UTF-16 surrogate entities
//! (`&#55357;&#56842;`), which no XML reader accepts as they are: attribute values are decoded here, pairing them.
//! A file cut short keeps what was read before the cut, and the notes say so.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use base64::Engine;
use oopz_model::{
    Adapter, App, CallOutcome, Conversation, ConversationKind, Error, Loaded, Media, MediaKind, Message, Part, Person,
    Result, Secret, Source, SourceInfo, normalize_phone,
};
use quick_xml::events::{BytesStart, Event};

pub const ID: &str = "sms-xml";
const FORMAT: &str = "SMS Backup & Restore XML";
/// The owner's person id.
pub const ME: &str = "me";

pub struct SmsXmlAdapter;

impl Adapter for SmsXmlAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn app(&self) -> App {
        App::Sms
    }

    fn secret(&self) -> Secret {
        Secret::None
    }

    fn detect(&self, path: &Path) -> bool {
        if !path.to_string_lossy().to_lowercase().ends_with(".xml") {
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else { return false };
        let mut head = Vec::new();
        if std::io::Read::read_to_end(&mut std::io::Read::take(file, 4096), &mut head).is_err() {
            return false;
        }
        let head = String::from_utf8_lossy(&head);
        head.contains("<smses") || head.contains("<calls")
    }

    fn open(&self, path: &Path, _secret: Option<&str>) -> Result<Box<dyn Source>> {
        let bytes = std::fs::read(path).map_err(|e| Error::io(path.display(), e))?;
        Ok(Box::new(read(&bytes)?))
    }
}

#[derive(PartialEq)]
enum Root {
    Messages,
    Calls,
}

#[derive(Default)]
struct Book {
    people: BTreeMap<String, Person>,
    conversations: Vec<Conversation>,
    index: HashMap<String, usize>,
    messages: Vec<Message>,
    owner_phone: Option<String>,
    unknown_types: usize,
    no_time: usize,
    unknown_elements: BTreeMap<String, usize>,
    bad_data: usize,
    empty_mms: usize,
}

/// Reads one file's bytes: `<smses>` or `<calls>`.
pub fn read(bytes: &[u8]) -> Result<Loaded> {
    let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes));
    let mut reader = quick_xml::Reader::from_str(&text);
    reader.config_mut().check_end_names = false;
    let mut book = Book::default();
    let mut root = None;
    let mut format = None;
    let mut stated: Option<usize> = None;
    let mut records = 0usize;
    let mut damaged = false;
    // The MMS being read: its attributes, parts and addresses.
    let mut mms: Option<(Attrs, Vec<Attrs>, Vec<Attrs>)> = None;
    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(e) => e,
            Err(_) if root.is_none() => return Err(Error::NotThisFormat(FORMAT)),
            Err(_) => {
                damaged = true;
                break;
            }
        };
        match event {
            Event::Comment(c) => {
                let c = String::from_utf8_lossy(&c);
                if let Some(at) = c.find("SMS Backup & Restore v") {
                    let version = c[at..].split(" on ").next().unwrap_or("").trim().to_string();
                    format = Some(version);
                }
            }
            Event::Start(e) | Event::Empty(e) if root.is_none() => {
                root = Some(match e.name().as_ref() {
                    b"smses" => Root::Messages,
                    b"calls" => Root::Calls,
                    _ => return Err(Error::NotThisFormat(FORMAT)),
                });
                stated = attrs(&e).get("count").and_then(|c| c.parse().ok());
            }
            Event::Start(e) if e.name().as_ref() == b"mms" && root == Some(Root::Messages) => {
                // An MMS left open by a broken file is kept before the next one starts.
                if let Some((a, parts, addrs)) = mms.take() {
                    records += 1;
                    book.mms(records, &a, &parts, &addrs);
                }
                mms = Some((attrs(&e), Vec::new(), Vec::new()));
            }
            Event::Start(e) | Event::Empty(e) => {
                let name = e.name();
                match (name.as_ref(), &root, mms.as_mut()) {
                    (b"part", _, Some((_, parts, _))) => parts.push(attrs(&e)),
                    (b"addr", _, Some((_, _, addrs))) => addrs.push(attrs(&e)),
                    (b"parts" | b"addrs", _, Some(_)) => {}
                    (b"sms", Some(Root::Messages), None) => {
                        records += 1;
                        book.sms(records, &attrs(&e));
                    }
                    (b"mms", Some(Root::Messages), None) => {
                        records += 1;
                        book.mms(records, &attrs(&e), &[], &[]);
                    }
                    (b"call", Some(Root::Calls), None) => {
                        records += 1;
                        book.call(records, &attrs(&e));
                    }
                    (other, ..) => {
                        let other = String::from_utf8_lossy(other).chars().take(40).collect::<String>();
                        *book.unknown_elements.entry(other).or_default() += 1;
                    }
                }
            }
            Event::End(e) if e.name().as_ref() == b"mms" => {
                if let Some((a, parts, addrs)) = mms.take() {
                    records += 1;
                    book.mms(records, &a, &parts, &addrs);
                }
            }
            _ => {}
        }
    }
    if root.is_none() {
        return Err(Error::NotThisFormat(FORMAT));
    }
    if let Some((a, parts, addrs)) = mms.take() {
        records += 1;
        book.mms(records, &a, &parts, &addrs);
    }

    let mut notes = Vec::new();
    if damaged {
        notes.push(format!("the file is cut short or damaged after {records} records: those are kept"));
    }
    if let Some(n) = stated.filter(|n| *n != records) {
        notes.push(format!("the file says it holds {n} records; {records} were read"));
    }
    if book.unknown_types > 0 {
        notes.push(format!("{} records with a type oopz doesn't know: read as received", book.unknown_types));
    }
    if book.no_time > 0 {
        notes.push(format!("{} records without a date", book.no_time));
    }
    for (name, n) in &book.unknown_elements {
        notes.push(format!("{n} × element `{name}` oopz doesn't know, skipped"));
    }
    if book.bad_data > 0 {
        notes.push(format!("{} MMS parts whose data isn't valid base64", book.bad_data));
    }
    if book.empty_mms > 0 {
        notes.push(format!("{} MMS with no part oopz reads kept as unknown", book.empty_mms));
    }
    let me = Person { id: ME.into(), phone: book.owner_phone.clone(), me: true, ..Default::default() };
    book.people.insert(ME.into(), me.clone());
    Ok(Loaded {
        info: SourceInfo {
            app: App::Sms,
            adapter: ID.into(),
            format,
            owner: Some(me),
            media_root: None,
            wall_clock: false,
            notes,
        },
        people: book.people.into_values().collect(),
        conversations: book.conversations,
        messages: book.messages,
    })
}

type Attrs = HashMap<String, String>;

fn attrs(e: &BytesStart) -> Attrs {
    let mut out = HashMap::new();
    for a in e.attributes().with_checks(false).flatten() {
        out.insert(String::from_utf8_lossy(a.key.as_ref()).into_owned(), unescape(&a.value));
    }
    out
}

/// An attribute's value: the five named entities, and numeric ones with UTF-16 surrogate pairs joined into one
/// character (a lone half becomes U+FFFD). Anything else stays as written.
pub fn unescape(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let mut high: Option<u32> = None;
    let flush = |out: &mut String, high: &mut Option<u32>| {
        if high.take().is_some() {
            out.push('\u{FFFD}');
        }
    };
    let mut rest: &str = &s;
    while let Some(at) = rest.find('&') {
        if at > 0 {
            flush(&mut out, &mut high);
            out.push_str(&rest[..at]);
        }
        let after = &rest[at + 1..];
        let Some(end) = after.find(';').filter(|e| *e <= 12) else {
            flush(&mut out, &mut high);
            out.push('&');
            rest = after;
            continue;
        };
        let name = &after[..end];
        let number = if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok()
        } else if let Some(dec) = name.strip_prefix('#') {
            dec.parse().ok()
        } else {
            None
        };
        match (number, name) {
            (Some(u @ 0xD800..=0xDBFF), _) => {
                flush(&mut out, &mut high);
                high = Some(u);
            }
            (Some(u @ 0xDC00..=0xDFFF), _) => match high.take() {
                Some(h) => {
                    let c = 0x10000 + ((h - 0xD800) << 10) + (u - 0xDC00);
                    out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
                }
                None => out.push('\u{FFFD}'),
            },
            (Some(u), _) => {
                flush(&mut out, &mut high);
                out.push(char::from_u32(u).unwrap_or('\u{FFFD}'));
            }
            (None, named) => {
                flush(&mut out, &mut high);
                match named {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "apos" => out.push('\''),
                    _ => {
                        out.push('&');
                        out.push_str(named);
                        out.push(';');
                    }
                }
            }
        }
        rest = &after[end + 1..];
    }
    if !rest.is_empty() {
        flush(&mut out, &mut high);
        out.push_str(rest);
    }
    flush(&mut out, &mut high);
    out
}

/// A value the app writes as `null` when it has none.
fn value<'a>(a: &'a Attrs, key: &str) -> Option<&'a str> {
    a.get(key).map(|v| v.trim()).filter(|v| !v.is_empty() && *v != "null")
}

/// A number in one form, or the address as written (short codes, names), or `unknown`.
fn key(address: &str) -> String {
    let address = address.trim();
    normalize_phone(address).unwrap_or_else(|| if address.is_empty() { "unknown".into() } else { address.into() })
}

fn links(text: &str) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    for word in text.split(char::is_whitespace) {
        let Some(at) = word.find("https://").or_else(|| word.find("http://")) else { continue };
        let url = word[at..].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '"', '\'']);
        if url.len() > 8 && !out.iter().any(|p| matches!(p, Part::Link { url: u, .. } if u == url)) {
            out.push(Part::Link { url: url.to_string(), title: None });
        }
    }
    out
}

impl Book {
    fn person(&mut self, id: &str, name: Option<&str>) {
        let name = name.filter(|n| *n != "(Unknown)").map(str::to_string);
        let person = self.people.entry(id.to_string()).or_insert_with(|| Person {
            id: id.to_string(),
            phone: normalize_phone(id),
            ..Default::default()
        });
        if person.name.is_none() {
            person.name = name;
        }
    }

    /// The conversation with these people (ids), made on first sight.
    fn conversation(&mut self, people: &[String], name: Option<&str>) -> String {
        let id = match people {
            [one] => one.clone(),
            many => format!("group:{}", many.join(",")),
        };
        if !self.index.contains_key(&id) {
            self.index.insert(id.clone(), self.conversations.len());
            let mut members = people.to_vec();
            members.push(ME.into());
            self.conversations.push(Conversation {
                id: id.clone(),
                kind: if people.len() > 1 { ConversationKind::Group } else { ConversationKind::Direct },
                name: name.filter(|n| *n != "(Unknown)").map(str::to_string),
                members,
                created_ms: None,
            });
        }
        id
    }

    fn time(&mut self, a: &Attrs) -> Option<i64> {
        let t = value(a, "date").and_then(|d| d.parse().ok());
        if t.is_none() {
            self.no_time += 1;
        }
        t
    }

    fn sms(&mut self, n: usize, a: &Attrs) {
        let other = key(value(a, "address").unwrap_or(""));
        let name = value(a, "contact_name");
        self.person(&other, name);
        let conversation = self.conversation(std::slice::from_ref(&other), name);
        // A type msgkinds lists: received (1) or the owner's (sent, draft, outbox, failed, queued).
        let from_me = match known(msgkinds::Source::SmsXmlSms, value(a, "type")) {
            Some(code) => code != 1,
            None => {
                self.unknown_types += 1;
                false
            }
        };
        let mut parts = Vec::new();
        if let Some(subject) = value(a, "subject") {
            parts.push(Part::text(subject));
        }
        let body = a.get("body").cloned().unwrap_or_default();
        let found = links(&body);
        parts.push(Part::text(body));
        parts.extend(found);
        let time_ms = self.time(a);
        self.messages.push(Message {
            id: format!("sms:{n}"),
            conversation,
            sender: Some(if from_me { ME.into() } else { other }),
            from_me,
            time_ms,
            parts,
            ..Default::default()
        });
    }

    /// An MMS: its box looked up in msgkinds (`Source::SmsXmlMmsBox`); its parts settle what it holds.
    fn mms(&mut self, n: usize, a: &Attrs, parts: &[Attrs], addrs: &[Attrs]) {
        let from_me = match known(msgkinds::Source::SmsXmlMmsBox, value(a, "msg_box")) {
            Some(code) => code != 1,
            None => {
                self.unknown_types += 1;
                false
            }
        };
        let from = addrs.iter().find(|x| value(x, "type") == Some("137")).and_then(|x| value(x, "address"));
        if from_me && let Some(own) = from.and_then(normalize_phone) {
            self.owner_phone.get_or_insert(own);
        }
        let mut people: Vec<String> =
            value(a, "address").unwrap_or("").split('~').filter(|s| !s.trim().is_empty()).map(key).collect();
        if people.is_empty() {
            people = addrs
                .iter()
                .filter_map(|x| value(x, "address"))
                .filter(|x| *x != "insert-address-token")
                .map(key)
                .filter(|k| Some(k) != self.owner_phone.as_ref())
                .collect();
        }
        people.sort();
        people.dedup();
        if people.is_empty() {
            people.push("unknown".into());
        }
        let names: Vec<&str> = value(a, "contact_name").map(|n| n.split(", ").collect()).unwrap_or_default();
        for (i, p) in people.clone().iter().enumerate() {
            let name = if people.len() == names.len() {
                names.get(i).copied()
            } else if people.len() == 1 {
                value(a, "contact_name")
            } else {
                None
            };
            self.person(p, name);
        }
        let conv_name = value(a, "contact_name");
        let conversation = self.conversation(&people, conv_name);
        let sender = if from_me {
            ME.to_string()
        } else {
            match from.map(key) {
                Some(k) if k != "insert-address-token" => {
                    self.person(&k, None);
                    k
                }
                _ => people[0].clone(),
            }
        };

        let mut sorted: Vec<&Attrs> = parts.iter().collect();
        sorted.sort_by_key(|p| value(p, "seq").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0));
        let mut out = Vec::new();
        if let Some(subject) = value(a, "sub") {
            out.push(Part::text(subject));
        }
        for p in sorted {
            let ct = value(p, "ct").unwrap_or("").to_ascii_lowercase();
            if ct == "application/smil" {
                continue;
            }
            let data = match value(p, "data") {
                Some(d) => match base64::engine::general_purpose::STANDARD.decode(d.as_bytes()) {
                    Ok(bytes) => Some(bytes),
                    Err(_) => {
                        self.bad_data += 1;
                        None
                    }
                },
                None => None,
            };
            let name = value(p, "cl").or_else(|| value(p, "name")).map(str::to_string);
            if ct == "text/plain" {
                let text = value(p, "text")
                    .map(str::to_string)
                    .or_else(|| data.as_ref().map(|d| String::from_utf8_lossy(d).into_owned()))
                    .unwrap_or_default();
                let found = links(&text);
                out.push(Part::text(text));
                out.extend(found);
                continue;
            }
            if ct == "text/x-vcard" || ct == "text/vcard" || ct == "text/directory" {
                let card = data
                    .as_ref()
                    .map(|d| String::from_utf8_lossy(d).into_owned())
                    .or_else(|| value(p, "text").map(str::to_string))
                    .unwrap_or_default();
                let (fn_name, phones) = vcard(&card);
                out.push(Part::Contact { name: fn_name, phones, vcard: Some(card).filter(|c| !c.is_empty()) });
                continue;
            }
            let kind = match ct.split('/').next().unwrap_or("") {
                "image" if ct == "image/gif" => MediaKind::Gif,
                "image" => MediaKind::Photo,
                "video" => MediaKind::Video,
                "audio" if ct.contains("amr") || ct.contains("3gpp") => MediaKind::Voice,
                "audio" => MediaKind::Audio,
                _ => MediaKind::Document,
            };
            out.push(Part::Media(Media {
                kind,
                name,
                mime: Some(ct).filter(|c| !c.is_empty()),
                size: data.as_ref().map(|d| d.len() as u64),
                omitted: data.is_none(),
                data,
                ..Default::default()
            }));
        }
        if out.is_empty() {
            self.empty_mms += 1;
            out.push(Part::Unknown { code: "mms".into(), text: None });
        }
        let time_ms = self.time(a);
        self.messages.push(Message {
            id: format!("mms:{n}"),
            conversation,
            sender: Some(sender),
            from_me,
            time_ms,
            parts: out,
            ..Default::default()
        });
    }

    fn call(&mut self, n: usize, a: &Attrs) {
        let other = key(value(a, "number").unwrap_or(""));
        let name = value(a, "contact_name");
        self.person(&other, name);
        let conversation = self.conversation(std::slice::from_ref(&other), name);
        let secs = value(a, "duration").and_then(|d| d.parse::<u64>().ok());
        // A type msgkinds lists (`Source::SmsXmlCall`); its direction and outcome are Android's `CallLog.Calls`.
        let (from_me, outcome) = match known(msgkinds::Source::SmsXmlCall, value(a, "type")) {
            Some(2) if secs.is_some_and(|s| s > 0) => (true, CallOutcome::Answered),
            Some(2) => (true, CallOutcome::Cancelled),
            // Incoming, or answered on another device.
            Some(1 | 7) => (false, CallOutcome::Answered),
            // Missed, or left as a voicemail.
            Some(3 | 4) => (false, CallOutcome::Missed),
            // Rejected, or blocked.
            Some(5 | 6) => (false, CallOutcome::Declined),
            _ => {
                self.unknown_types += 1;
                (false, CallOutcome::Unknown)
            }
        };
        let time_ms = self.time(a);
        self.messages.push(Message {
            id: format!("call:{n}"),
            conversation,
            sender: Some(if from_me { ME.into() } else { other }),
            from_me,
            time_ms,
            parts: vec![Part::Call { video: false, outcome, secs }],
            ..Default::default()
        });
    }
}

/// A contact card's name (`FN`) and phone numbers (`TEL`).
fn vcard(card: &str) -> (Option<String>, Vec<String>) {
    let mut name = None;
    let mut phones = Vec::new();
    for line in card.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let k = k.to_ascii_uppercase();
        let v = v.trim();
        if k == "FN" && !v.is_empty() {
            name = Some(v.to_string());
        } else if (k == "TEL" || k.starts_with("TEL;")) && !v.is_empty() {
            phones.push(v.to_string());
        }
    }
    (name, phones)
}

/// The code `v` when msgkinds' `source` lists it; `None` for a missing, unreadable or unlisted one.
fn known(source: msgkinds::Source, v: Option<&str>) -> Option<i64> {
    let code = v?.trim().parse::<i64>().ok()?;
    msgkinds::lookup(source, msgkinds::Code::Int(code), None).entry().map(|_| code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrogate_pairs_and_entities_decode() {
        assert_eq!(unescape(b"&#55357;&#56842; &amp;&lt;&#10;&#xE9;"), "😊 &<\né");
        assert_eq!(unescape(b"&#55357;x&#56842;"), "\u{FFFD}x\u{FFFD}");
        assert_eq!(unescape(b"a & b &nbsp; &#55357;"), "a & b &nbsp; \u{FFFD}");
        assert_eq!(unescape(b"&#99999999999;&#;&#1114112;"), "&#99999999999;&#;\u{FFFD}");
    }
}
