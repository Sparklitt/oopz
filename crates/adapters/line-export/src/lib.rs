//! LINE's "Export chat history" (decision 0015): a text file that starts `[LINE] Chat history with X` (Japanese
//! `[LINE] Xとのトーク履歴`), a `Saved on:` line, then a date line per day (`2024/01/01(Mon)`, `2024.01.01 Monday`,
//! `Mon, 01/15/2024`, `2024/01/01(月)`) and a line per message: `time<TAB>name<TAB>text`, with a message of several
//! lines in double quotes (`""` for a quote), and the app's notices as `time<TAB>text`.
//!
//! Times are the phone's wall clock, read as UTC (`wall_clock`). The owner isn't written: in a one-to-one chat the
//! header names the other side, so the one other sender is the owner; otherwise the library lets the owner pick. A
//! line that fits nothing continues the message before it, or, before any message, is kept as an `Unknown` part.

use std::io::Read;
use std::path::Path;

use oopz_model::{
    Adapter, App, CallOutcome, Conversation, ConversationKind, Error, Event, Loaded, Media, MediaKind, Message, Part,
    Person, Result, Secret, Source, SourceInfo, civil_to_ms,
};

pub const ID: &str = "line-export";
const FORMAT: &str = "LINE chat history";

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
        if !path.to_string_lossy().to_lowercase().ends_with(".txt") {
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else { return false };
        let mut head = Vec::new();
        if file.take(4096).read_to_end(&mut head).is_err() {
            return false;
        }
        let head = decode(&head);
        head.trim_start().starts_with("[LINE]") || looks_like_line(&head)
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
    let header = first.and_then(|i| lines[i].trim().strip_prefix("[LINE]")).map(str::trim);
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

    let mut notes = vec!["times are the phone's local time, read as UTC: the export doesn't say its time zone".into()];
    let (mut day_first, mut month_first) = (false, false);
    for d in raws.iter().filter_map(|r| r.day).filter(|d| d.ambiguous) {
        day_first |= d.a > 12;
        month_first |= d.b > 12;
    }
    let ambiguous = raws.iter().any(|r| r.day.is_some_and(|d| d.ambiguous));
    let swap = day_first && !month_first;
    if ambiguous && day_first == month_first {
        notes.push("no date decides the day/month order of `Mon, 01/02/2024` lines: read as month first".into());
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
            if let Some(who) = unsent(&text) {
                m.sender = Some(who.to_string());
                m.deleted = true;
            } else {
                let event = event(&text);
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
            m.parts = parts(&text);
        }
        messages.push(m);
    }
    if unknown > 0 {
        notes.push(format!("{unknown} lines before the first message kept as unknown"));
    }
    if no_day > 0 {
        notes.push(format!("{no_day} messages before any date line: kept without a time"));
    }
    if bad > 0 {
        notes.push(format!("{bad} messages with a date or time that doesn't exist: kept without a time"));
    }
    if !no_quote.is_empty() {
        notes.push(format!("{} quotes never closed: read as plain text", no_quote.len()));
    }

    // In a one-to-one chat the header names the other side: the one other sender is the owner.
    let others: Vec<&String> = senders.iter().filter(|s| Some(s.as_str()) != direct_with.as_deref()).collect();
    let owner = match (&direct_with, others.as_slice()) {
        (Some(_), [one]) => {
            notes.push("the owner is the sender the header doesn't name".into());
            Some((*one).clone())
        }
        _ => {
            notes.push("the export doesn't say who the owner is: pick yourself".into());
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
    let s = name.strip_suffix(".txt").unwrap_or(name).trim();
    let s = s.strip_prefix("[LINE]").unwrap_or(s).trim();
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
            if t.starts_with("[LINE]") || t.starts_with("Saved on") || t.starts_with("保存日時") {
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

/// `X unsent a message.`, `Xが送信を取り消しました`: who.
fn unsent(text: &str) -> Option<&str> {
    let t = text.trim().trim_end_matches(['.', '。']);
    t.strip_suffix(" unsent a message")
        .or_else(|| t.strip_suffix("が送信を取り消しました"))
        .map(str::trim)
        .filter(|w| !w.is_empty())
}

/// What a notice says happened: msgkinds' LINE rows first (`Source::LineTxt`: a notice's code is the end of its
/// line, "joined the group."), then the words oopz met that msgkinds doesn't list yet (no public export shows them).
fn event(text: &str) -> Event {
    let line = text.trim();
    let listed = msgkinds::Source::LineTxt
        .entries()
        .iter()
        .find(|e| e.kind == msgkinds::Kind::System && matches!(e.code, msgkinds::Code::Name(w) if line.ends_with(w)));
    if let Some(e) = listed {
        return e.event.map_or(Event::Other, Event::from);
    }
    let t = text.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| t.contains(w));
    if has(&["joined", "参加"]) {
        Event::MemberJoined
    } else if has(&["invited", "招待"]) {
        Event::MemberAdded
    } else if has(&["removed", "削除"]) {
        Event::MemberRemoved
    } else if has(&[" left", "退会"]) {
        Event::MemberLeft
    } else if has(&["group name", "グループ名"]) {
        Event::Renamed
    } else if has(&["group photo", "group profile photo", "グループ画像"]) {
        Event::PictureChanged
    } else if has(&["announcement", "アナウンス"]) {
        Event::Pinned
    } else {
        Event::Other
    }
}

/// `[Photo]` and the like: the kind, or a contact or a place.
const PLACEHOLDERS: [(&str, &str); 8] = [
    ("[Photo]", "[写真]"),
    ("[Sticker]", "[スタンプ]"),
    ("[Video]", "[動画]"),
    ("[File]", "[ファイル]"),
    ("[Voice message]", "[ボイスメッセージ]"),
    ("[Contact]", "[連絡先]"),
    ("[Location]", "[位置情報]"),
    ("[Album]", "[アルバム]"),
];

fn parts(text: &str) -> Vec<Part> {
    let t = text.trim();
    for (i, (en, ja)) in PLACEHOLDERS.iter().enumerate() {
        let Some(rest) = t.strip_prefix(en).or_else(|| t.strip_prefix(ja)) else { continue };
        let rest = Some(rest.trim().to_string()).filter(|r| !r.is_empty());
        let media = |kind| Part::Media(Media { kind, omitted: true, ..Default::default() });
        return vec![match i {
            0 => media(MediaKind::Photo),
            1 => media(MediaKind::Sticker),
            2 => media(MediaKind::Video),
            3 => Part::Media(Media { kind: MediaKind::Document, name: rest, omitted: true, ..Default::default() }),
            4 => media(MediaKind::Voice),
            5 => Part::Contact { name: rest, phones: Vec::new(), vcard: None },
            6 => Part::Location { lat: None, lng: None, name: rest, live: false },
            _ => Part::Media(Media { kind: MediaKind::Photo, caption: rest, omitted: true, ..Default::default() }),
        }];
    }
    if let Some(call) = t.strip_prefix('☎') {
        let call = call.trim();
        let video = call.to_lowercase().contains("video") || call.contains("ビデオ");
        let (outcome, secs) = if let Some(d) =
            ["Call time", "Video call time", "通話時間", "ビデオ通話時間"].iter().find_map(|p| call.strip_prefix(p))
        {
            (CallOutcome::Answered, duration(d.trim()))
        } else if ["Missed call", "不在着信", "No answer", "応答なし"].iter().any(|p| call.starts_with(p)) {
            (CallOutcome::Missed, None)
        } else if ["Canceled call", "Cancelled call", "キャンセル"].iter().any(|p| call.starts_with(p)) {
            (CallOutcome::Cancelled, None)
        } else {
            (CallOutcome::Unknown, None)
        };
        return vec![Part::Call { video, outcome, secs }];
    }
    let mut parts = vec![Part::text(text)];
    for word in text.split(char::is_whitespace) {
        if let Some(at) = word.find("https://").or_else(|| word.find("http://")) {
            let url = word[at..].trim_end_matches(['.', ',', ')', '!', '?', '"', '」', '。']);
            if url.len() > 8 {
                parts.push(Part::Link { url: url.to_string(), title: None });
            }
        }
    }
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
