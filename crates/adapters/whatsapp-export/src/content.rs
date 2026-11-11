//! A message's text, turned into parts: the placeholders the export writes for media, contacts, places, polls,
//! calls, deleted and edited messages (English and Spanish, and a few other languages where known), else text with
//! its links.

use std::collections::{HashMap, HashSet};

use oopz_model::{CallOutcome, Event, Media, MediaKind, Part, PollOption};

use crate::header::MARKS;

/// The files that came with the chat (a zip's entries, a folder's files), when known, and the contact cards' text.
#[derive(Debug, Default)]
pub struct Files {
    /// `None` for a text file alone: then no file is missing, they're all simply not here.
    pub names: Option<HashSet<String>>,
    pub vcards: HashMap<String, String>,
}

pub enum Read {
    /// "This message was deleted"; `own` for "You deleted this message".
    Deleted {
        own: bool,
    },
    Parts {
        parts: Vec<Part>,
        edited: bool,
    },
}

/// What had to be read around, for the notes.
#[derive(Debug, Default)]
pub struct Stats {
    /// Every lookup in msgkinds' registry and wording tables; its report joins the notes. The export doesn't say
    /// which WhatsApp wrote it.
    pub reading: msgkinds::Reading,
    /// The export's language, found from its lines (`msgkinds::wording::detect`); `None`: every language is tried.
    pub language: Option<&'static str>,
    pub unknown_lines: usize,
    pub kindless_media: usize,
    pub missing_files: usize,
}

impl Stats {
    pub fn notes(&self, notes: &mut Vec<String>) {
        if self.unknown_lines > 0 {
            notes.push(format!("{} lines before the first message kept as unknown", self.unknown_lines));
        }
        if self.kindless_media > 0 {
            notes.push(format!(
                "{} media left out of the export without saying what they were: counted as documents",
                self.kindless_media
            ));
        }
        if self.missing_files > 0 {
            notes.push(format!("{} files named in the chat aren't in the export", self.missing_files));
        }
        notes.extend(self.reading.report().lines());
    }
}

/// What a message's body holds. msgkinds' wording tables name it, in the export's language
/// (`media_type::sources::whatsapp_export::content_in`), and its registry row says what it is; names, places and
/// captions are taken out by punctuation, the same in every language. A body in other wording is text: kept, and
/// counted when it looks like one of WhatsApp's own `<…>` markers.
pub fn read(text: &str, files: &Files, stats: &mut Stats) -> Read {
    use msgkinds::media_type::sources::whatsapp_export as words;
    let t = text.trim_start_matches(MARKS);
    let (body, edited) = match words::edited(stats.language, t) {
        Some(body) => (body, true),
        None => (t, false),
    };
    let code = words::content_in(stats.language, body);
    let row = stats.reading.lookup_name(msgkinds::Source::WhatsAppExport, code).entry();
    match code {
        "deleted" | "deleted_admin" => return Read::Deleted { own: false },
        "deleted_own" => return Read::Deleted { own: true },
        "text" if words::unread_marker_in(stats.language, body) => {
            stats.reading.fallback(msgkinds::Source::WhatsAppExport)
        }
        _ => {}
    }
    let parts = parts(code, row, body, files, stats);
    Read::Parts { parts, edited }
}

fn parts(code: &str, row: Option<&msgkinds::Entry>, t: &str, files: &Files, stats: &mut Stats) -> Vec<Part> {
    let bare = t.trim().trim_matches(MARKS);
    let (first, rest) = t.split_once('\n').unwrap_or((t, ""));
    let first_bare = first.trim().trim_matches(MARKS);
    let caption = || Some(rest.trim().to_string()).filter(|c| !c.is_empty());
    let row_media = row.and_then(|e| MediaKind::from_kind(e.kind.into()));

    match code {
        "omitted" => {
            stats.kindless_media += 1;
            vec![media(MediaKind::Document, None, None, true)]
        }
        "image_omitted" | "video_omitted" | "video_note_omitted" | "audio_omitted" | "sticker_omitted"
        | "gif_omitted" => vec![media(row_media.unwrap_or(MediaKind::Document), None, None, true)],
        // `informe.pdf • 3 pages ‎document omitted`: the name is before the dot, or before iPhone's mark.
        "document_omitted" => {
            let before = first.trim().trim_start_matches(MARKS);
            let name = match (before.find(" • "), before.rfind(MARKS)) {
                (Some(at), _) => &before[..at],
                (None, Some(at)) if at > 0 => &before[..at],
                _ => "",
            };
            let name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
            vec![media(MediaKind::Document, name, None, true)]
        }
        "contact_omitted" => vec![Part::Contact { name: None, phones: Vec::new(), vcard: None }],
        "view_once" => {
            let kind = msgkinds::media_type::sources::whatsapp_export::view_once_kind(stats.language, bare);
            let mut m = media(MediaKind::from_kind(kind.into()).unwrap_or(MediaKind::Photo), None, None, true);
            if let Part::Media(m) = &mut m {
                m.view_once = true;
            }
            vec![m]
        }
        "attached" | "contact_attached" => {
            // iPhone: `<attached: NAME>` then a caption; Android: `NAME (file attached)`, the caption below.
            let found = if first_bare.starts_with('<') {
                first_bare.split_once(": ").and_then(|(_, after)| after.split_once('>')).map(|(name, more)| {
                    let mut c = more.trim().to_string();
                    if !rest.trim().is_empty() {
                        if !c.is_empty() {
                            c.push('\n');
                        }
                        c.push_str(rest.trim());
                    }
                    (name, Some(c).filter(|c| !c.is_empty()))
                })
            } else {
                first_bare.rfind(" (").map(|at| (first_bare[..at].split(" • ").next().unwrap_or(""), caption()))
            };
            match found.map(|(name, c)| (name.trim(), c)) {
                Some((name, c)) if !name.is_empty() => vec![file_part(name, c, files, stats)],
                _ => text_parts(t),
            }
        }
        "live_location" => vec![Part::Location { lat: None, lng: None, name: None, live: true }],
        "location" => {
            let url = first_bare.split_once(": ").map_or("", |(_, u)| u).trim();
            let (lat, lng) = coordinates(url).unzip();
            vec![Part::Location { lat, lng, name: caption(), live: false }]
        }
        "poll" => vec![poll(rest)],
        "missed_voice_call" | "missed_video_call" | "voice_call" | "video_call" => {
            let outcome = if code.starts_with("missed") { CallOutcome::Missed } else { CallOutcome::Unknown };
            let video = row.is_some_and(|e| e.has(msgkinds::Flag::Video));
            vec![Part::Call { video, outcome, secs: None }]
        }
        // "Waiting for this message": WhatsApp's own line, its words kept.
        "pending" => vec![Part::System { event: Event::Other, text: Some(bare.to_string()) }],
        _ => text_parts(t),
    }
}

fn text_parts(t: &str) -> Vec<Part> {
    let mut parts = vec![Part::text(t)];
    parts.extend(links(t).into_iter().map(|url| Part::Link { url, title: None }));
    parts
}

fn media(kind: MediaKind, name: Option<String>, caption: Option<String>, omitted: bool) -> Part {
    Part::Media(Media { kind, name, caption, omitted, ..Default::default() })
}

/// A file the message names: a contact card becomes a contact, anything else media.
fn file_part(name: &str, caption: Option<String>, files: &Files, stats: &mut Stats) -> Part {
    if let Some(names) = &files.names
        && !names.contains(name)
    {
        stats.missing_files += 1;
    }
    let lower = name.to_lowercase();
    if lower.ends_with(".vcf") {
        let card = files.vcards.get(name);
        let stem = name.get(..name.len() - 4).unwrap_or(name).trim();
        // iPhone's `00000011-Name.vcf`.
        let stem = match stem.split_once('-') {
            Some((n, s)) if n.len() == 8 && n.bytes().all(|b| b.is_ascii_digit()) => s,
            _ => stem,
        };
        let (fn_name, phones) = card.map(|c| vcard(c)).unwrap_or_default();
        return Part::Contact { name: fn_name.or_else(|| Some(stem.to_string())), phones, vcard: card.cloned() };
    }
    let kind = kind_of(name);
    Part::Media(Media {
        kind,
        file: Some(name.to_string()),
        name: (kind == MediaKind::Document).then(|| name.to_string()),
        mime: MediaKind::mime_for(name).map(str::to_string),
        caption,
        ..Default::default()
    })
}

/// From WhatsApp's file names (`IMG-…`, `PTT-…`, iPhone's `…-PHOTO-…`), else the extension.
fn kind_of(name: &str) -> MediaKind {
    let upper = name.to_uppercase();
    for (prefix, kind) in [
        ("IMG-", MediaKind::Photo),
        ("VID-", MediaKind::Video),
        ("PTT-", MediaKind::Voice),
        ("AUD-", MediaKind::Audio),
        ("STK-", MediaKind::Sticker),
        ("DOC-", MediaKind::Document),
    ] {
        if upper.starts_with(prefix) {
            return kind;
        }
    }
    for (word, kind) in [
        ("-PHOTO-", MediaKind::Photo),
        ("-VIDEO-", MediaKind::Video),
        ("-AUDIO-", MediaKind::Voice),
        ("-STICKER-", MediaKind::Sticker),
        ("-GIF-", MediaKind::Gif),
    ] {
        if upper.contains(word) {
            return kind;
        }
    }
    MediaKind::from_file_name(name)
}

/// A contact card's name (`FN`) and phone numbers (`TEL`).
fn vcard(card: &str) -> (Option<String>, Vec<String>) {
    let mut name = None;
    let mut phones = Vec::new();
    for line in card.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let key = key.to_ascii_uppercase();
        let value = value.trim();
        if key == "FN" && !value.is_empty() {
            name = Some(value.to_string());
        } else if (key == "TEL" || key.starts_with("TEL;") || key.ends_with(".TEL")) && !value.is_empty() {
            phones.push(value.rsplit(':').next().unwrap_or(value).to_string());
        }
    }
    (name, phones)
}

/// `https://maps.google.com/?q=40.4168,-3.7038` → (40.4168, -3.7038).
fn coordinates(url: &str) -> Option<(f64, f64)> {
    let (_, q) = url.split_once("q=").or_else(|| url.split_once('@'))?;
    let q = q.split(['&', ' ']).next()?;
    let (lat, lng) = q.split_once(',')?;
    let lat: f64 = lat.trim().parse().ok()?;
    let lng: f64 = lng.split(',').next()?.trim().parse().ok()?;
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng)).then_some((lat, lng))
}

/// `POLL:` then the question, then `OPTION: text (n votes)` lines.
fn poll(body: &str) -> Part {
    let mut question = String::new();
    let mut options = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        let option = ["OPTION:", "OPCIÓN:", "OPÇÃO:", "OPTION :"].iter().find_map(|p| line.strip_prefix(p));
        match option {
            Some(option) => {
                let option = option.trim();
                let (text, votes) = match option.rsplit_once(" (") {
                    Some((text, count)) if count.ends_with(')') => {
                        let n = count.split(' ').next().and_then(|n| n.parse().ok());
                        match n {
                            Some(n) => (text.to_string(), n),
                            None => (option.to_string(), 0),
                        }
                    }
                    _ => (option.to_string(), 0),
                };
                options.push(PollOption { text, voters: Vec::new(), votes });
            }
            None if options.is_empty() => {
                if !line.is_empty() {
                    if !question.is_empty() {
                        question.push('\n');
                    }
                    question.push_str(line);
                }
            }
            None => {
                // A line after the options: kept with the last option rather than lost.
                if let Some(last) = options.last_mut() {
                    last.text.push('\n');
                    last.text.push_str(line);
                }
            }
        }
    }
    Part::Poll { question, options, multiple: false }
}

/// The `http(s)://` links in a text, without trailing punctuation.
pub fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || c == '<' || c == '>') {
        let Some(at) = word.find("https://").or_else(|| word.find("http://")) else { continue };
        let url = word[at..].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '"', '\'', '”', '’']);
        if url.len() > "https://".len() && !out.iter().any(|u| u == url) {
            out.push(url.to_string());
        }
    }
    out
}

/// Text and links only: on iPhone, a line starting with U+200E that reads as plain text is the app's notice.
pub fn is_notice(parts: &[Part]) -> bool {
    parts.iter().all(|p| matches!(p, Part::Text { .. } | Part::Link { .. }))
}

/// The kind of notice, from its wording: msgkinds names it in the export's language
/// (`media_type::sources::whatsapp_export::notice_in`, Android and iPhone) and its registry row
/// (`Source::WhatsAppExport`) says what happened. Wording no table has is an `other` notice with its words, counted
/// as a fallback.
pub fn event(text: &str, stats: &mut Stats) -> Event {
    let code = msgkinds::media_type::sources::whatsapp_export::notice_in(stats.language, text);
    if code == "notice" {
        stats.reading.fallback(msgkinds::Source::WhatsAppExport);
    }
    let row = stats.reading.lookup_name(msgkinds::Source::WhatsAppExport, code).entry();
    row.and_then(|e| e.event).map_or(Event::Other, Event::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts_of(text: &str, stats: &mut Stats) -> Vec<Part> {
        match read(text, &Files::default(), stats) {
            Read::Parts { parts, .. } => parts,
            Read::Deleted { .. } => panic!("{text:?} read as deleted"),
        }
    }

    /// msgkinds' wording tables in other languages, and wording no table has: kept as text and counted.
    #[test]
    fn other_languages_and_unknown_wording() {
        let mut stats = Stats::default();
        assert!(matches!(read("Diese Nachricht wurde gelöscht.", &Files::default(), &mut stats), Read::Deleted { .. }));
        assert!(matches!(&parts_of("<Médias omis>", &mut stats)[..], [Part::Media(m)] if m.omitted));
        assert!(
            matches!(&parts_of("\u{200e}immagine omessa", &mut stats)[..], [Part::Media(m)] if m.kind == MediaKind::Photo)
        );
        assert!(matches!(
            &parts_of("Verpasster Videoanruf", &mut stats)[..],
            [Part::Call { video: true, outcome: CallOutcome::Missed, .. }]
        ));
        // Any language: the file's name comes out by its punctuation.
        assert!(matches!(
            &parts_of("IMG-20240101-WA0001.jpg (plik załączony)\nfoto", &mut stats)[..],
            [Part::Media(m)] if m.file.as_deref() == Some("IMG-20240101-WA0001.jpg") && m.caption.as_deref() == Some("foto")
        ));
        // Words that only look like a call stay text.
        assert!(matches!(&parts_of("Video call tomorrow?", &mut stats)[..], [Part::Text { .. }]));
        assert!(stats.reading.report().fallback.is_empty());
        // A marker in wording no table has stays text and is counted; so is a notice.
        assert!(matches!(&parts_of("<Mediji izpuščeni>", &mut stats)[..], [Part::Text { .. }]));
        assert_eq!(event("Ana je ustvarila skupino", &mut stats), Event::Other);
        assert_eq!(stats.reading.report().fallback, vec![(msgkinds::Source::WhatsAppExport, 2)]);
        let mut notes = Vec::new();
        stats.notes(&mut notes);
        assert!(notes.iter().any(|n| n.starts_with("2 lines in whatsapp_export")), "{notes:?}");
    }

    #[test]
    fn links_and_coordinates() {
        assert_eq!(
            links("see https://example.org/a, and (http://example.com)."),
            ["https://example.org/a", "http://example.com"]
        );
        assert_eq!(coordinates("https://maps.google.com/?q=40.4168,-3.7038"), Some((40.4168, -3.7038)));
        assert_eq!(coordinates("https://maps.google.com/?q=400,1"), None);
    }

    #[test]
    fn polls_count_votes() {
        let Part::Poll { question, options, .. } = poll("¿Cena?\nOPTION: Pizza (2 votes)\nOPTION: A (b) (1 vote)")
        else {
            panic!("a poll");
        };
        assert_eq!(question, "¿Cena?");
        assert_eq!(
            options.iter().map(|o| (o.text.as_str(), o.votes)).collect::<Vec<_>>(),
            [("Pizza", 2), ("A (b)", 1)]
        );
    }
}
