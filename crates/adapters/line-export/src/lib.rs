//! LINE's "Export chat history" (decision 0015): a text file that starts `[LINE] Chat history with X` (Japanese
//! `[LINE] Xとのトーク履歴`), a `Saved on:` line, then a date line per day (`2024/01/01(Mon)`, `2024.01.01 Monday`,
//! `Mon, 01/15/2024`, `2024/01/01(月)`) and a line per message: `time<TAB>name<TAB>text`, with a message of several
//! lines in double quotes (`""` for a quote), and the app's notices as `time<TAB>text`.
//!
//! Times are the phone's wall clock, read as UTC (`wall_clock`). The owner isn't written: in a one-to-one chat the
//! header names the other side, so the one other sender is the owner; otherwise the library lets the owner pick. A
//! line that fits nothing continues the message before it, or, before any message, is kept as an `Unknown` part.

use msgkinds::Note;
use msgkinds::media_type::formats::PLAIN_TEXT;
use msgkinds::media_type::name::has_format;
use msgkinds::registry::codes::line_txt as code;
use oopz_model::messages::Message as Msg;
use std::io::Read;
use std::path::Path;

use oopz_model::{
    Adapter, App, CallOutcome, Conversation, ConversationKind, Error, Event, Loaded, Media, MediaKind, Message, Part,
    Person, Result, Secret, Source, SourceInfo, civil_to_ms,
};

pub const ID: &str = "line-export";
const FORMAT: Msg = Msg::FormatLineExport;

pub struct LineExportAdapter;

impl Adapter for LineExportAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn app(&self) -> App {
        App::Line
    }

    fn secret(&self) -> Secret {
        Secret::None
    }

    fn detect(&self, path: &Path) -> bool {
        if !has_format(&path.to_string_lossy(), PLAIN_TEXT) {
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else { return false };
        let mut head = Vec::new();
        if file.take(4096).read_to_end(&mut head).is_err() {
            return false;
        }
        let head = decode(&head);
        head.trim_start().starts_with(export_mark()) || looks_like_line(&head)
    }

    fn open(&self, path: &Path, _secret: Option<&str>) -> Result<Box<dyn Source>> {
        let bytes = std::fs::read(path).map_err(|e| Error::reading(path, e))?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(Box::new(read(&name, &bytes)?))
    }
}

fn decode(bytes: &[u8]) -> String {
    msgkinds::media_type::text::decode(bytes)
}

/// LINE's shapes, from msgkinds' wording (`data/wording/line.toml`'s `[shape]`).
fn export_mark() -> &'static str {
    msgkinds::wording::shape(msgkinds::Source::LineTxt, "export_mark")[0]
}

fn call_prefix() -> &'static str {
    msgkinds::wording::shape(msgkinds::Source::LineTxt, "call_prefix")[0]
}

/// The call prefix's symbol alone (`☎`), as a line may start with it before any space.
fn phone() -> &'static str {
    call_prefix().trim_end()
}

/// No `[LINE]` header (cut off, or an old version): a date line followed by a message line in the first lines.
fn looks_like_line(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|l| !l.is_empty()).take(20).collect();
    lines.windows(2).any(|w| date_line(w[0]).is_some() && message_line(w[1]).is_some())
}

/// A date line's numbers: year, and the two others as written; `ambiguous` when they may be month/day or day/month.
#[derive(Clone, Copy)]
struct Day {
    y: i64,
    a: u32,
    b: u32,
    ambiguous: bool,
}

fn numbers(s: &str, seps: &[char]) -> Option<(Vec<u32>, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    let bytes = s.as_bytes();
    loop {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() && i - start < 4 {
            i += 1;
        }
        if i == start {
            return None;
        }
        out.push(s.get(start..i)?.parse().ok()?);
        if out.len() == 3 {
            return Some((out, i));
        }
        let c = s.get(i..)?.chars().next()?;
        if !seps.contains(&c) {
            return None;
        }
        i += c.len_utf8();
    }
}

/// `2024/01/01(Mon)`, `2024/1/1(月)`, `2024.01.01 Monday`, `2024-01-01`, `Mon, 01/15/2024`.
fn date_line(line: &str) -> Option<Day> {
    let line = line.trim();
    if line.contains('\t') || line.len() > 40 {
        return None;
    }
    if let Some((n, end)) = numbers(line, &['/', '.', '-']).filter(|(n, _)| n[0] >= 1000) {
        let rest = line.get(end..)?.trim();
        let ok = rest.is_empty()
            || (rest.starts_with('(') && rest.ends_with(')') && rest.chars().count() <= 12)
            || (rest.chars().count() <= 12 && rest.chars().all(char::is_alphabetic));
        return (ok && (1..=12).contains(&n[1])).then_some(Day { y: n[0] as i64, a: n[1], b: n[2], ambiguous: false });
    }
    let (weekday, rest) = line.split_once(", ")?;
    if weekday.is_empty() || weekday.chars().count() > 12 || !weekday.chars().all(char::is_alphabetic) {
        return None;
    }
    let (n, end) = numbers(rest.trim(), &['/', '.', '-'])?;
    (rest.trim().len() == end && n[2] >= 1000).then_some(Day { y: n[2] as i64, a: n[0], b: n[1], ambiguous: true })
}

/// `21:41`, `9:41`, `PM10:00`, `10:00 PM`, `午後10:00`, `午前9:00` on a 24-hour clock.
fn time(s: &str) -> Option<(u32, u32)> {
    let s = s.trim();
    let (pm, rest) = if let Some(r) = s.strip_prefix("PM").or_else(|| s.strip_prefix("午後")) {
        (Some(true), r)
    } else if let Some(r) = s.strip_prefix("AM").or_else(|| s.strip_prefix("午前")) {
        (Some(false), r)
    } else if let Some(r) = s.strip_suffix("PM").or_else(|| s.strip_suffix("pm")) {
        (Some(true), r)
    } else if let Some(r) = s.strip_suffix("AM").or_else(|| s.strip_suffix("am")) {
        (Some(false), r)
    } else {
        (None, s)
    };
    let (h, m) = rest.trim().split_once(':')?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 || !(h.bytes().chain(m.bytes())).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    let h = match pm {
        Some(true) if (1..12).contains(&h) => h + 12,
        Some(false) if h == 12 => 0,
        Some(_) if !(1..=12).contains(&h) => 99,
        _ => h,
    };
    Some((h, m))
}

enum Line<'a> {
    Message { time: (u32, u32), name: &'a str, text: &'a str },
    Notice { time: (u32, u32), text: &'a str },
}

fn message_line(line: &str) -> Option<Line<'_>> {
    let (t, rest) = line.split_once('\t')?;
    let time = time(t)?;
    Some(match rest.split_once('\t') {
        Some((name, text)) => Line::Message { time, name, text },
        None => Line::Notice { time, text: rest },
    })
}

/// The end of a quoted text that starts at its first character (`"…""…"`): the byte after the closing quote.
fn closing_quote(s: &str) -> Option<usize> {
    let mut chars = s.char_indices().skip(1).peekable();
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            if chars.peek().is_some_and(|(_, c)| *c == '"') {
                chars.next();
            } else {
                return Some(i + 1);
            }
        }
    }
    None
}

fn unquote(s: &str) -> String {
    s.get(1..s.len().saturating_sub(1)).unwrap_or("").replace("\"\"", "\"")
}

/// One message as written.
struct Raw {
    line: usize,
    day: Option<Day>,
    time: Option<(u32, u32)>,
    sender: Option<String>,
    text: String,
    /// Inside double quotes still open.
    open: bool,
    notice: bool,
    unknown: bool,
}

/// Reads an export's bytes; `name` is the file's name, used when the header has no chat name.
pub fn read(name: &str, bytes: &[u8]) -> Result<Loaded> {
    let text = decode(bytes);
    let lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let header = first.and_then(|i| lines[i].trim().strip_prefix(export_mark())).map(str::trim);
    if header.is_none() && !looks_like_line(&text) {
        return Err(Error::NotThisFormat(FORMAT));
    }
    let (title, direct_with) = match header {
        Some(h) => title(h),
        None => (None, None),
    };

    // A quote left open at the end of the file wasn't a quote: read again without it (after a few, without any, so
    // a hostile file can't make this slow).
    let mut no_quote = Vec::new();
    let raws = loop {
        match parse(&lines, header.is_some(), &no_quote, no_quote.len() >= 16) {
            Ok(raws) => break raws,
            Err(line) => no_quote.push(line),
        }
    };

    let mut notes = vec![Note::of(Msg::WallClockUtc, []).to_string()];
    // msgkinds' LINE rows, and the lines in wording they don't know (counted, kept as they are). One language per
    // export, found from its markers, calls and notices; then only that language's words are read.
    let mut reading = msgkinds::Reading::new(None);
    let language = {
        use msgkinds::wording::Section;
        let lines = raws.iter().map(|r| {
            let t = r.text.trim();
            if r.notice {
                (Section::Notice, t.to_string())
            } else if let Some(call) = t.strip_prefix(phone()) {
                (Section::Call, format!("{}{}", call_prefix(), call.trim()))
            } else {
                (Section::Body, t.to_string())
            }
        });
        let lines: Vec<(Section, String)> = lines.collect();
        msgkinds::wording::detect(msgkinds::Source::LineTxt, lines.iter().map(|(s, t)| (*s, t.as_str())))
    };
    reading.language(msgkinds::Source::LineTxt, language);
    let (mut day_first, mut month_first) = (false, false);
    for d in raws.iter().filter_map(|r| r.day).filter(|d| d.ambiguous) {
        day_first |= d.a > 12;
        month_first |= d.b > 12;
    }
    let ambiguous = raws.iter().any(|r| r.day.is_some_and(|d| d.ambiguous));
    let swap = day_first && !month_first;
    if ambiguous && day_first == month_first {
        notes.push(Note::of(Msg::LineDayMonthUndecided, []).to_string());
    }

    let conversation = title.clone().or_else(|| stem(name)).unwrap_or_else(|| "chat".into());
    let mut messages = Vec::new();
    let (mut unknown, mut no_day, mut bad) = (0, 0, 0);
    let mut group_event = false;
    let mut senders: Vec<String> = Vec::new();
    for raw in raws {
        let mut m =
            Message { id: format!("line:{}", raw.line), conversation: conversation.clone(), ..Default::default() };
        if raw.unknown {
            unknown += 1;
            m.parts.push(Part::Unknown { code: "line".into(), text: Some(raw.text) });
            messages.push(m);
            continue;
        }
        m.time_ms = match (raw.day, raw.time) {
            (Some(d), Some((h, mi))) => {
                let (y, mo, day) = if !d.ambiguous {
                    (d.y, d.a, d.b)
                } else if swap {
                    (d.y, d.b, d.a)
                } else {
                    (d.y, d.a, d.b)
                };
                let t = civil_to_ms(y, mo, day, h, mi, 0, 0);
                bad += t.is_none() as usize;
                t
            }
            _ => {
                no_day += 1;
                None
            }
        };
        let text = raw.text.trim_end_matches('\n').to_string();
        if raw.notice {
            if let Some(who) = unsent(language, &text) {
                m.sender = Some(who.to_string());
                m.deleted = true;
            } else {
                let event = event(language, &text, &mut reading);
                group_event |= matches!(
                    event,
                    Event::MemberJoined | Event::MemberAdded | Event::MemberLeft | Event::MemberRemoved
                );
                m.parts.push(Part::System { event, text: Some(text) });
            }
        } else {
            let sender = raw.sender.unwrap_or_default();
            if !senders.contains(&sender) {
                senders.push(sender.clone());
            }
            m.sender = Some(sender);
            m.parts = parts(language, &text, &mut reading);
        }
        messages.push(m);
    }
    if unknown > 0 {
        notes.push(Note::of(Msg::LinesBeforeFirstMessage, [unknown.to_string()]).to_string());
    }
    notes.extend(reading.report().lines());
    if no_day > 0 {
        notes.push(Note::of(Msg::MessagesBeforeDateLine, [no_day.to_string()]).to_string());
    }
    if bad > 0 {
        notes.push(Note::of(Msg::MessagesBadDate, [bad.to_string()]).to_string());
    }
    if !no_quote.is_empty() {
        notes.push(Note::of(Msg::QuotesUnclosed, [no_quote.len().to_string()]).to_string());
    }

    // In a one-to-one chat the header names the other side: the one other sender is the owner.
    let others: Vec<&String> = senders.iter().filter(|s| Some(s.as_str()) != direct_with.as_deref()).collect();
    let owner = match (&direct_with, others.as_slice()) {
        (Some(_), [one]) => {
            notes.push(Note::of(Msg::OwnerNotInHeader, []).to_string());
            Some((*one).clone())
        }
        _ => {
            notes.push(Note::of(Msg::PickOwner, []).to_string());
            None
        }
    };
    for m in &mut messages {
        m.from_me = owner.is_some() && m.sender == owner;
    }
    for m in &messages {
        if let Some(s) = &m.sender
            && !senders.contains(s)
        {
            senders.push(s.clone());
        }
    }
    let people: Vec<Person> = senders
        .iter()
        .map(|s| Person { id: s.clone(), name: Some(s.clone()), me: owner.as_ref() == Some(s), ..Default::default() })
        .collect();
    let group = header.is_some_and(|h| h.contains("Chat history in ")) || senders.len() > 2 || group_event;
    Ok(Loaded {
        info: SourceInfo {
            app: App::Line,
            adapter: ID.into(),
            format: None,
            owner: people.iter().find(|p| p.me).cloned(),
            media_root: None,
            wall_clock: true,
            notes,
        },
        conversations: vec![Conversation {
            id: conversation,
            kind: if group { ConversationKind::Group } else { ConversationKind::Direct },
            name: title,
            members: senders,
            created_ms: None,
        }],
        people,
        messages,
    })
}

/// The chat's name, and the other side's name when the header says it's one-to-one.
fn title(header: &str) -> (Option<String>, Option<String>) {
    let some = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(x) = header.strip_prefix("Chat history with ") {
        return (some(x), some(x));
    }
    if let Some(x) = header.strip_prefix("Chat history in ") {
        return (some(x), None);
    }
    if let Some(x) = header.strip_suffix("とのトーク履歴") {
        return (some(x), some(x));
    }
    if let Some(x) = header.strip_suffix("のトーク履歴") {
        return (some(x), None);
    }
    (some(header), None)
}

fn stem(name: &str) -> Option<String> {
    let s = msgkinds::media_type::name::stem(name).trim();
    let s = s.strip_prefix(export_mark()).unwrap_or(s).trim();
    Some(s.to_string()).filter(|s| !s.is_empty())
}

/// Lines into messages. `Err(line)` when the quote opened on `line` is never closed: read again with it off.
fn parse(
    lines: &[&str],
    has_header: bool,
    no_quote: &[usize],
    quotes_off: bool,
) -> std::result::Result<Vec<Raw>, usize> {
    let mut raws: Vec<Raw> = Vec::new();
    let mut day: Option<Day> = None;
    let mut header_lines = if has_header { 2 } else { 0 };
    for (i, line) in lines.iter().enumerate() {
        if let Some(last) = raws.last_mut().filter(|r| r.open) {
            last.text.push('\n');
            last.text.push_str(line);
            if closing_quote(&last.text) == Some(last.text.len()) {
                last.text = unquote(&last.text);
                last.open = false;
            }
            continue;
        }
        if header_lines > 0 && !line.trim().is_empty() {
            let t = line.trim();
            let saved_on =
                msgkinds::wording::find(msgkinds::Source::LineTxt, msgkinds::wording::Section::Title, None, t);
            if t.starts_with(export_mark()) || saved_on.is_some() {
                header_lines -= 1;
                continue;
            }
            header_lines = 0;
        }
        if let Some(d) = date_line(line) {
            day = Some(d);
            continue;
        }
        let mut raw = match message_line(line) {
            Some(Line::Message { time, name, text }) => Raw {
                line: i + 1,
                day,
                time: Some(time),
                sender: Some(name.to_string()),
                text: text.to_string(),
                open: false,
                notice: false,
                unknown: false,
            },
            Some(Line::Notice { time, text }) => Raw {
                line: i + 1,
                day,
                time: Some(time),
                sender: None,
                text: text.to_string(),
                open: false,
                notice: true,
                unknown: false,
            },
            None => {
                if let Some(last) = raws.last_mut().filter(|r| !r.unknown) {
                    last.text.push('\n');
                    last.text.push_str(line);
                } else if !line.trim().is_empty() {
                    raws.push(Raw {
                        line: i + 1,
                        day,
                        time: None,
                        sender: None,
                        text: line.to_string(),
                        open: false,
                        notice: false,
                        unknown: true,
                    });
                }
                continue;
            }
        };
        if raw.text.starts_with('"') && !quotes_off && !no_quote.contains(&(i + 1)) {
            match closing_quote(&raw.text) {
                Some(end) if end == raw.text.len() => raw.text = unquote(&raw.text),
                Some(_) => {}
                None => raw.open = true,
            }
        }
        raws.push(raw);
    }
    match raws.iter().find(|r| r.open) {
        Some(r) => Err(r.line),
        None => Ok(raws),
    }
}

/// `X unsent a message.`, `Xがメッセージの送信を取り消しました`: who (msgkinds' LINE rows whose kind is deleted).
fn unsent<'a>(language: Option<&str>, text: &'a str) -> Option<&'a str> {
    let (code, who) = msgkinds::media_type::sources::line::notice(language, text)?;
    let deleted = msgkinds::lookup_name(msgkinds::Source::LineTxt, code, None).kind() == msgkinds::Kind::Deleted;
    if deleted { who } else { None }
}

/// What a notice says happened: msgkinds' LINE rows, in the export's language (`media_type::sources::line::notice`;
/// a notice's code is LINE's own words, "joined the group."). A notice in other words is an `other` notice with its
/// words, counted in `reading`.
fn event(language: Option<&str>, text: &str, reading: &mut msgkinds::Reading) -> Event {
    match msgkinds::media_type::sources::line::notice(language, text) {
        Some((code, _)) => {
            let row = reading.lookup_name(msgkinds::Source::LineTxt, code).entry();
            row.and_then(|e| e.event).map_or(Event::Other, Event::from)
        }
        None => {
            reading.fallback(msgkinds::Source::LineTxt);
            Event::Other
        }
    }
}

/// A `[…]` marker or a ☎ line: what msgkinds' LINE rows say it is, in the export's language. One in wording they
/// don't know stays text, counted in `reading`.
fn parts(language: Option<&str>, text: &str, reading: &mut msgkinds::Reading) -> Vec<Part> {
    use msgkinds::Kind as K;
    let t = text.trim();
    if let Some((kind, rest)) = msgkinds::media_type::sources::line::marker(language, t) {
        let rest = rest.map(str::to_string);
        let media = |kind| Part::Media(Media { kind, omitted: true, ..Default::default() });
        return vec![match kind {
            K::Photo => media(MediaKind::Photo),
            K::Video => media(MediaKind::Video),
            K::Sticker => media(MediaKind::Sticker),
            K::Voice => media(MediaKind::Voice),
            K::Audio => media(MediaKind::Audio),
            K::Gif => media(MediaKind::Gif),
            K::Document => {
                Part::Media(Media { kind: MediaKind::Document, name: rest, omitted: true, ..Default::default() })
            }
            K::Contact => Part::Contact { name: rest, phones: Vec::new(), vcard: None },
            K::Location => Part::Location { lat: None, lng: None, name: rest, live: false },
            K::Album => {
                Part::Media(Media { kind: MediaKind::Photo, caption: rest, omitted: true, ..Default::default() })
            }
            // The question may follow; options and votes aren't exported.
            K::Poll => Part::Poll { question: rest.unwrap_or_default(), options: Vec::new(), multiple: false },
            // A Notes post: its text isn't in the export, the marker is all there is.
            K::Text
            | K::Link
            | K::VideoNote
            | K::Call
            | K::StickerPack
            | K::Event
            | K::Payment
            | K::Game
            | K::Business
            | K::Story
            | K::Reaction
            | K::System
            | K::Deleted
            | K::Unknown => Part::text(text),
        }];
    }
    let marker = t.starts_with('[') && t.ends_with(']') && !t.contains('\n') && t.chars().count() <= 30;
    if marker {
        reading.fallback(msgkinds::Source::LineTxt);
    }
    if let Some(call) = t.strip_prefix(phone()) {
        let (outcome, secs, video) = match msgkinds::media_type::sources::line::call(language, call) {
            Some((code, rest, video)) => match code {
                code::CALL_TIME | code::CALL_TIME_JA => (CallOutcome::Answered, duration(rest), video),
                code::MISSED_CALL | code::MISSED_CALL_JA | code::NO_ANSWER => (CallOutcome::Missed, None, video),
                code::CANCELED_CALL | code::CANCELED_CALL_JA => (CallOutcome::Cancelled, None, video),
                _ => (CallOutcome::Unknown, None, video),
            },
            None => {
                reading.fallback(msgkinds::Source::LineTxt);
                (CallOutcome::Unknown, None, false)
            }
        };
        return vec![Part::Call { video, outcome, secs }];
    }
    let mut parts = vec![Part::text(text)];
    let links = msgkinds::media_type::sources::whatsapp_export::links(text);
    parts.extend(links.into_iter().map(|url| Part::Link { url: url.to_string(), title: None }));
    parts
}

/// `0:15`, `1:02:03` in seconds.
fn duration(s: &str) -> Option<u64> {
    let mut secs = 0u64;
    for p in s.split(':') {
        secs = secs.checked_mul(60)?.checked_add(p.trim().parse().ok()?)?;
    }
    Some(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msgkinds::registry::codes::line_txt as code;

    /// Wording neither msgkinds nor oopz knows stays as it is, and is counted for the notes.
    #[test]
    fn unknown_wording_is_kept_and_counted() {
        let mut r = msgkinds::Reading::new(None);
        assert!(matches!(&parts(Some("en"), "[Foto]", &mut r)[..], [Part::Text { .. }]));
        assert!(matches!(&parts(None, "[Foto]", &mut r)[..], [Part::Media(_)]));
        assert!(matches!(&parts(None, code::PHOTO, &mut r)[..], [Part::Media(_)]));
        assert_eq!(event(None, "Ana ha lasciato il gruppo", &mut r), Event::Other);
        assert!(matches!(
            &parts(None, "☎ Llamada perdida", &mut r)[..],
            [Part::Call { outcome: CallOutcome::Unknown, .. }]
        ));
        assert_eq!(r.report().fallback, vec![(msgkinds::Source::LineTxt, 3)]);
    }

    /// Every marker, call and notice msgkinds' LINE rows have, in the words oopz reads them with.
    #[test]
    fn markers_calls_and_notices() {
        let mut r = msgkinds::Reading::new(None);
        let one = |t: &str, r: &mut msgkinds::Reading| parts(None, t, r).remove(0);
        assert!(
            matches!(one("[File] informe.pdf", &mut r), Part::Media(Media { kind: MediaKind::Document, name: Some(n), .. }) if n == "informe.pdf")
        );
        assert!(
            matches!(one("[Album] Viaje", &mut r), Part::Media(Media { kind: MediaKind::Photo, caption: Some(c), .. }) if c == "Viaje")
        );
        assert!(matches!(one("[Poll] ¿Cena?", &mut r), Part::Poll { question, .. } if question == "¿Cena?"));
        assert!(matches!(one(code::GIF, &mut r), Part::Media(Media { kind: MediaKind::Gif, .. })));
        assert!(matches!(one("[オーディオ]", &mut r), Part::Media(Media { kind: MediaKind::Audio, .. })));
        assert!(matches!(one(code::NOTE, &mut r), Part::Text { .. }));
        assert!(matches!(
            one("☎ Video call time 1:02:03", &mut r),
            Part::Call { video: true, outcome: CallOutcome::Answered, secs: Some(3723) }
        ));
        assert!(matches!(
            one("☎ 通話時間 0:15", &mut r),
            Part::Call { video: false, outcome: CallOutcome::Answered, secs: Some(15) }
        ));
        assert!(matches!(one("☎ 応答なし", &mut r), Part::Call { outcome: CallOutcome::Missed, .. }));
        assert!(matches!(one("☎ Cancelled call", &mut r), Part::Call { outcome: CallOutcome::Cancelled, .. }));
        assert!(matches!(one(code::GROUP_CALL_STARTED, &mut r), Part::Call { outcome: CallOutcome::Unknown, .. }));
        assert_eq!(event(None, "Ana changed the group's name to \"B\"", &mut r), Event::Renamed);
        assert_eq!(event(None, "Ana joined the group.", &mut r), Event::MemberJoined);
        assert_eq!(event(None, "Anaがグループに参加しました。", &mut r), Event::MemberJoined);
        assert_eq!(unsent(None, "Ana unsent a message."), Some("Ana"));
        assert_eq!(unsent(None, "Anaが送信を取り消しました"), Some("Ana"));
        assert_eq!(unsent(None, "Ana joined the group."), None);
        assert!(r.report().is_empty(), "{:?}", r.report());
    }

    #[test]
    fn dates_times_and_quotes() {
        assert!(date_line("2024/01/01(Mon)").is_some());
        assert!(date_line("2024/1/1(月)").is_some());
        assert!(date_line("2024.01.01 Monday").is_some());
        assert!(date_line("Mon, 01/15/2024").is_some_and(|d| d.ambiguous));
        assert!(date_line("2024/13/01(Mon)").is_none());
        assert!(date_line("10:00\tAna\thi").is_none());
        assert!(date_line("hello, world").is_none());
        assert_eq!(time("PM10:00"), Some((22, 0)));
        assert_eq!(time("10:00 PM"), Some((22, 0)));
        assert_eq!(time("午前12:05"), Some((0, 5)));
        assert_eq!(time("9:05"), Some((9, 5)));
        assert_eq!(time("9:5"), None);
        assert_eq!(closing_quote("\"a\"\"b\""), Some(6));
        assert_eq!(closing_quote("\"a\"\"b"), None);
        assert_eq!(unquote("\"a\"\"b\""), "a\"b");
        assert_eq!(duration("1:02:03"), Some(3723));
    }
}
