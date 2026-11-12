//! A message's text, turned into parts: the placeholders the export writes for media, contacts, places, polls,
//! calls, deleted and edited messages (English and Spanish, and a few other languages where known), else text with
//! its links.

use msgkinds::registry::codes::whatsapp_export as code;
use std::collections::{HashMap, HashSet};

use oopz_model::{CallOutcome, Event, Media, MediaKind, Part, PollOption};

use crate::header::is_mark;

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
    let t = text.trim_start_matches(is_mark);
    let (body, edited) = match words::edited(stats.language, t) {
        Some(body) => (body, true),
        None => (t, false),
    };
    let code = words::content_in(stats.language, body);
    let row = stats.reading.lookup_name(msgkinds::Source::WhatsAppExport, code).entry();
    match code {
        code::DELETED | code::DELETED_ADMIN => return Read::Deleted { own: false },
        code::DELETED_OWN => return Read::Deleted { own: true },
        code::TEXT if words::unread_marker_in(stats.language, body) => {
            stats.reading.fallback(msgkinds::Source::WhatsAppExport)
        }
        _ => {}
    }
    let parts = parts(code, row, body, files, stats);
    Read::Parts { parts, edited }
}

fn parts(code: &str, row: Option<&msgkinds::Entry>, t: &str, files: &Files, stats: &mut Stats) -> Vec<Part> {
    let bare = t.trim().trim_matches(is_mark);
    let (first, rest) = t.split_once('\n').unwrap_or((t, ""));
    let first_bare = first.trim().trim_matches(is_mark);
    let caption = || Some(rest.trim().to_string()).filter(|c| !c.is_empty());
    let row_media = row.and_then(|e| MediaKind::from_kind(e.kind.into()));

    match code {
        code::OMITTED => {
            stats.kindless_media += 1;
            vec![media(MediaKind::Document, None, None, true)]
        }
        code::IMAGE_OMITTED
        | code::VIDEO_OMITTED
        | code::VIDEO_NOTE_OMITTED
        | code::AUDIO_OMITTED
        | code::STICKER_OMITTED
        | code::GIF_OMITTED => vec![media(row_media.unwrap_or(MediaKind::Document), None, None, true)],
        // `informe.pdf • 3 pages ‎document omitted`: the name is before the dot, or before iPhone's mark.
        code::DOCUMENT_OMITTED => {
            let before = first.trim().trim_start_matches(is_mark);
            let name = match (before.find(" • "), before.rfind(is_mark)) {
                (Some(at), _) => &before[..at],
                (None, Some(at)) if at > 0 => &before[..at],
                _ => "",
            };
            let name = Some(name.trim().to_string()).filter(|n| !n.is_empty());
            vec![media(MediaKind::Document, name, None, true)]
        }
        code::CONTACT_OMITTED => vec![Part::Contact { name: None, phones: Vec::new(), vcard: None }],
        code::VIEW_ONCE => {
            let kind = msgkinds::media_type::sources::whatsapp_export::view_once_kind(stats.language, bare);
            let mut m = media(MediaKind::from_kind(kind.into()).unwrap_or(MediaKind::Photo), None, None, true);
            if let Part::Media(m) = &mut m {
                m.view_once = true;
            }
            vec![m]
        }
        code::ATTACHED | code::CONTACT_ATTACHED => {
            // msgkinds reads the name in any language; the caption is what follows it: after iPhone's `<…: NAME>` on its
            // line, and the lines below.
            let name = msgkinds::media_type::sources::whatsapp_export::attached_name_in(stats.language, t)
                .map(|n| n.split(" • ").next().unwrap_or(n).trim());
            let caption = match first_bare.starts_with('<') {
                true => {
                    let after = first_bare.split_once('>').map_or("", |(_, more)| more.trim());
                    let c = [after, rest.trim()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
                    Some(c).filter(|c| !c.is_empty())
                }
                false => caption(),
            };
            match name {
                Some(name) if !name.is_empty() => vec![file_part(name, caption, files, stats)],
                _ => text_parts(t),
            }
        }
        code::LIVE_LOCATION => vec![Part::Location { lat: None, lng: None, name: None, live: true }],
        code::LOCATION => {
            let (lat, lng) =
                msgkinds::media_type::sources::whatsapp_export::location_in(stats.language, first_bare).unzip();
            vec![Part::Location { lat, lng, name: caption(), live: false }]
        }
        code::POLL => vec![poll(stats.language, rest)],
        code::MISSED_VOICE_CALL | code::MISSED_VIDEO_CALL | code::VOICE_CALL | code::VIDEO_CALL => {
            let outcome = if code.starts_with("missed") { CallOutcome::Missed } else { CallOutcome::Unknown };
            let video = row.is_some_and(|e| e.has(msgkinds::Flag::Video));
            vec![Part::Call { video, outcome, secs: None }]
        }
        // "Waiting for this message": WhatsApp's own line, its words kept.
        code::PENDING => vec![Part::System { event: Event::Other, text: Some(bare.to_string()) }],
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
    if msgkinds::media_type::sources::whatsapp_export::is_vcard(name) {
        let card = files.vcards.get(name);
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem).trim();
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

/// From WhatsApp's file names (`IMG-…`, `PTT-…`, iPhone's `…-PHOTO-…`), else the extension: msgkinds'
/// `whatsapp::file_name`.
fn kind_of(name: &str) -> MediaKind {
    MediaKind::from_msgkinds(msgkinds::media_type::sources::whatsapp::file_name(name).0)
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

/// `POLL:` then the question, then `OPTION: text (n votes)` lines.
fn poll(language: Option<&str>, body: &str) -> Part {
    let mut question = String::new();
    let mut options = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        let option = msgkinds::media_type::sources::whatsapp_export::poll_option_in(language, line);
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

/// The links in a text, without trailing punctuation (msgkinds' `whatsapp_export::links`).
pub fn links(text: &str) -> Vec<String> {
    msgkinds::media_type::sources::whatsapp_export::links(text).into_iter().map(str::to_owned).collect()
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
    if code == code::NOTICE {
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
    fn links() {
        assert_eq!(
            super::links("see https://example.org/a, and (http://example.com)."),
            ["https://example.org/a", "http://example.com"]
        );
    }

    #[test]
    fn polls_count_votes() {
        let Part::Poll { question, options, .. } =
            poll(None, "¿Cena?\nOPTION: Pizza (2 votes)\nOPTION: A (b) (1 vote)")
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
