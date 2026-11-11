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
    }
}

const DELETED_OTHER: [&str; 4] =
    ["This message was deleted", "Se eliminó este mensaje", "Este mensaje fue eliminado", "Mensagem apagada"];
const DELETED_OWN: [&str; 3] = ["You deleted this message", "Eliminaste este mensaje", "Você apagou esta mensagem"];
const EDITED: [&str; 3] = ["<This message was edited>", "<Se editó este mensaje.>", "<Mensagem editada>"];
const OMITTED: [&str; 4] = ["<Media omitted>", "<Multimedia omitido>", "<Mídia oculta>", "<Medien ausgeschlossen>"];
const ATTACHED: [&str; 6] = [
    " (file attached)",
    " (archivo adjunto)",
    " (Datei angehängt)",
    " (fichier joint)",
    " (arquivo anexado)",
    " (file allegato)",
];
/// iPhone's "image omitted" and the like: the whole text.
const OMITTED_IOS: [(&str, MediaKind); 12] = [
    ("image omitted", MediaKind::Photo),
    ("video omitted", MediaKind::Video),
    ("audio omitted", MediaKind::Voice),
    ("sticker omitted", MediaKind::Sticker),
    ("gif omitted", MediaKind::Gif),
    ("document omitted", MediaKind::Document),
    ("imagen omitida", MediaKind::Photo),
    ("video omitido", MediaKind::Video),
    ("audio omitido", MediaKind::Voice),
    ("sticker omitido", MediaKind::Sticker),
    ("gif omitido", MediaKind::Gif),
    ("documento omitido", MediaKind::Document),
];

pub fn read(text: &str, files: &Files, stats: &mut Stats) -> Read {
    let t = text.trim_start_matches(MARKS);
    let bare = t.trim().trim_matches(MARKS).trim_end_matches('.');
    if DELETED_OTHER.contains(&bare) {
        return Read::Deleted { own: false };
    }
    if DELETED_OWN.contains(&bare) {
        return Read::Deleted { own: true };
    }
    let (t, edited) = strip_edited(t);
    let parts = parts(t, files, stats);
    Read::Parts { parts, edited }
}

fn strip_edited(t: &str) -> (&str, bool) {
    let trimmed = t.trim_end();
    for suffix in EDITED {
        if let Some(rest) = trimmed.strip_suffix(suffix) {
            return (rest.trim_end().trim_end_matches(MARKS).trim_end(), true);
        }
    }
    (t, false)
}

fn parts(t: &str, files: &Files, stats: &mut Stats) -> Vec<Part> {
    let single = !t.contains('\n');
    let bare = t.trim().trim_matches(MARKS);
    let lower = bare.to_lowercase();
    let (first, rest) = t.split_once('\n').unwrap_or((t, ""));
    let caption = || Some(rest.trim().to_string()).filter(|c| !c.is_empty());

    if OMITTED.contains(&bare) {
        stats.kindless_media += 1;
        return vec![media(MediaKind::Document, None, None, true)];
    }
    if single {
        for (word, kind) in OMITTED_IOS {
            if lower == word {
                return vec![media(kind, None, None, true)];
            }
            if kind == MediaKind::Document && lower.ends_with(word) {
                let name = bare
                    .get(..bare.len().saturating_sub(word.len()))
                    .unwrap_or("")
                    .split(" • ")
                    .next()
                    .unwrap_or("")
                    .trim();
                let name = Some(name.to_string()).filter(|n| !n.is_empty());
                return vec![media(kind, name, None, true)];
            }
        }
        if lower == "contact card omitted" || lower == "tarjeta de contacto omitida" {
            return vec![Part::Contact { name: None, phones: Vec::new(), vcard: None }];
        }
        if bare.chars().count() <= 80
            && ["view once", "visualización única", "ver una vez"].iter().any(|w| lower.contains(w))
        {
            let kind = if lower.contains("video") || lower.contains("vídeo") {
                MediaKind::Video
            } else if ["voice", "voz", "audio"].iter().any(|w| lower.contains(w)) {
                MediaKind::Voice
            } else {
                MediaKind::Photo
            };
            let mut m = media(kind, None, None, true);
            if let Part::Media(m) = &mut m {
                m.view_once = true;
            }
            return vec![m];
        }
    }

    let first_bare = first.trim().trim_matches(MARKS);
    for open in ["<attached: ", "<adjunto: "] {
        if let Some(after) = first_bare.strip_prefix(open)
            && let Some((name, more)) = after.split_once('>')
        {
            let mut caption_text = more.trim().to_string();
            if !rest.trim().is_empty() {
                if !caption_text.is_empty() {
                    caption_text.push('\n');
                }
                caption_text.push_str(rest.trim());
            }
            let caption = Some(caption_text).filter(|c| !c.is_empty());
            return vec![file_part(name.trim(), caption, files, stats)];
        }
    }
    for suffix in ATTACHED {
        if let Some(name) = first_bare.strip_suffix(suffix) {
            let name = name.split(" • ").next().unwrap_or(name).trim();
            if !name.is_empty() {
                return vec![file_part(name, caption(), files, stats)];
            }
        }
    }

    let first_lower = first_bare.to_lowercase();
    if ["live location shared", "ubicación en tiempo real compartida"].iter().any(|w| first_lower.starts_with(w)) {
        return vec![Part::Location { lat: None, lng: None, name: None, live: true }];
    }
    for word in ["location: ", "ubicación: ", "localização: ", "standort: "] {
        if first_lower.starts_with(word) {
            let url = first_bare.get(word.len()..).unwrap_or("").trim();
            let (lat, lng) = coordinates(url).unzip();
            return vec![Part::Location { lat, lng, name: caption(), live: false }];
        }
    }

    if ["POLL:", "ENCUESTA:", "ENQUETE:", "UMFRAGE:"].contains(&first_bare) {
        return vec![poll(rest)];
    }

    if single {
        let call = lower.split(',').next().unwrap_or("").trim();
        let missed = ["missed voice call", "missed video call", "llamada de voz perdida", "videollamada perdida"];
        let plain = ["voice call", "video call", "llamada de voz", "videollamada"];
        if missed.contains(&call) || plain.contains(&call) {
            let outcome = if missed.contains(&call) { CallOutcome::Missed } else { CallOutcome::Unknown };
            let video = call.contains("video");
            return vec![Part::Call { video, outcome, secs: None }];
        }
    }

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

/// The kind of notice, from its wording (English and Spanish).
///
/// The kind of notice, from its wording: msgkinds names it (`media_type::sources::whatsapp_export::notice`, English
/// and Spanish, Android and iPhone) and its registry row (`Source::WhatsAppExport`) says what happened.
pub fn event(text: &str) -> Event {
    let code = msgkinds::media_type::sources::whatsapp_export::notice(text);
    let row = msgkinds::lookup_name(msgkinds::Source::WhatsAppExport, code, None).entry();
    row.and_then(|e| e.event).map_or(Event::Other, Event::from)
}

#[cfg(test)]
mod tests {
    use super::*;

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
