//! WhatsApp Android as an adapter of the one model (decisions 0013, 0014): a crypt15 backup opened with its owner's
//! key, read through the same session and index the app uses, so every extra the viewer shows (replies, reactions,
//! edits, forwards, polls, calls, media facts, names) reaches the library too. The decrypted copy and the index live
//! in a private temporary folder removed when the source is dropped.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use oopz_model as m;
use oopz_model::{Adapter, App, Error, Secret, Source};

use crate::index::Line;
use crate::session::Session;

/// The id the owner has in a WhatsApp source.
pub const ME: &str = "me";

/// Rows read from the index at a time while streaming a chat.
const PAGE: u32 = 2_000;

pub struct WhatsAppAdapter;

impl Adapter for WhatsAppAdapter {
    fn id(&self) -> &'static str {
        "whatsapp-android"
    }

    fn app(&self) -> App {
        App::WhatsApp
    }

    fn secret(&self) -> Secret {
        Secret::Key
    }

    fn detect(&self, path: &Path) -> bool {
        if path.is_dir() {
            return path.join("Databases/msgstore.db.crypt15").is_file() || path.join("msgstore.db.crypt15").is_file();
        }
        path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".crypt15"))
    }

    fn open(&self, path: &Path, secret: Option<&str>) -> m::Result<Box<dyn Source>> {
        let key = secret.filter(|k| !k.trim().is_empty()).ok_or(Error::NeedsSecret)?;
        let file = if path.is_dir() {
            [path.join("Databases/msgstore.db.crypt15"), path.join("msgstore.db.crypt15")]
                .into_iter()
                .find(|p| p.is_file())
                .ok_or(Error::NotThisFormat("WhatsApp backup folder"))?
        } else {
            path.to_path_buf()
        };
        Ok(Box::new(WhatsAppSource::open(&file, key)?))
    }
}

pub struct WhatsAppSource {
    session: Session,
    id: u32,
    info: m::SourceInfo,
    people: Vec<m::Person>,
    conversations: Vec<m::Conversation>,
    /// Sender rows to person ids.
    jids: HashMap<i64, String>,
    /// Chat rows to the other person's id, in one-to-one chats.
    direct: HashMap<i64, String>,
    // Last: dropped after the session closed its files.
    _tmp: tempfile::TempDir,
}

fn model_error(e: crate::AppError) -> Error {
    match e.code {
        "key_format" | "wrong_key" => Error::WrongSecret,
        "not_crypt15" => Error::NotThisFormat("WhatsApp crypt15 backup"),
        "crypt14" | "unsupported" => Error::UnsupportedVersion(e.detail),
        "damaged" => Error::Damaged(e.detail),
        "cancelled" => Error::Cancelled,
        _ => Error::Io(e.detail, std::io::Error::other(e.code)),
    }
}

/// `9999…@s.whatsapp.net` → `+9999…`; other ids (groups, private `lid` ids) have no phone number.
fn phone_of(jid: &str) -> Option<String> {
    let user = jid.strip_suffix("@s.whatsapp.net")?;
    let user = user.split(':').next().unwrap_or(user);
    m::normalize_phone(&format!("+{user}"))
}

impl WhatsAppSource {
    pub fn open(path: &Path, key: &str) -> m::Result<Self> {
        let tmp = tempfile::Builder::new()
            .prefix("oopz-wa-")
            .tempdir()
            .map_err(|e| Error::io("making a private folder", e))?;
        let mut session = Session::default();
        let summary = session.check(path, key, tmp.path()).map_err(model_error)?;
        session.build_index(summary.id, tmp.path()).map_err(model_error)?;
        let jids = session.jids(summary.id).map_err(model_error)?;
        let chat_jids = session.chat_jids(summary.id).map_err(model_error)?;
        let book =
            oopz::Key::from_hex(key).ok().and_then(|k| oopz::contacts::Contacts::beside(path, &k, tmp.path()).ok());
        let name_of = |jid: &str| book.as_ref().and_then(|b| b.name(jid).ok().flatten());

        let mut people = vec![m::Person { id: ME.into(), me: true, ..Default::default() }];
        let mut known = BTreeSet::new();
        let mut add = |people: &mut Vec<m::Person>, jid: &str, name: Option<String>| {
            if known.insert(jid.to_owned()) {
                people.push(m::Person { id: jid.to_owned(), phone: phone_of(jid), name, ..Default::default() });
            }
        };
        let mut conversations = Vec::new();
        let mut direct = HashMap::new();
        for row in &summary.chat_list {
            let jid = chat_jids.get(&row.number).cloned().unwrap_or_default();
            let kind = match row.kind {
                "contact" | "contact_lid" => m::ConversationKind::Direct,
                "group" => m::ConversationKind::Group,
                "broadcast" => m::ConversationKind::Broadcast,
                _ => m::ConversationKind::Unknown,
            };
            let mut members = Vec::new();
            if kind == m::ConversationKind::Direct && !jid.is_empty() {
                add(&mut people, &jid, row.name.clone());
                direct.insert(row.number, jid.clone());
                members = vec![ME.to_owned(), jid];
            }
            conversations.push(m::Conversation {
                id: row.number.to_string(),
                kind,
                name: row.name.clone(),
                members,
                created_ms: None,
            });
        }
        for jid in jids.values() {
            if jid.ends_with("@s.whatsapp.net") || jid.ends_with("@lid") {
                add(&mut people, jid, name_of(jid));
            }
        }
        let info = m::SourceInfo {
            app: App::WhatsApp,
            adapter: "whatsapp-android".into(),
            format: Some("crypt15".into()),
            owner: Some(people[0].clone()),
            media_root: path.parent().and_then(Path::parent).map(|p| p.join("Media")).filter(|p| p.is_dir()),
            notes: if summary.sound { Vec::new() } else { vec![format!("SQLite's check found: {}", summary.problems)] },
        };
        Ok(Self { session, id: summary.id, info, people, conversations, jids, direct, _tmp: tmp })
    }
}

impl Source for WhatsAppSource {
    fn info(&self) -> &m::SourceInfo {
        &self.info
    }

    fn people(&self) -> &[m::Person] {
        &self.people
    }

    fn conversations(&self) -> &[m::Conversation] {
        &self.conversations
    }

    fn messages(&mut self, conversation: &str, each: &mut dyn FnMut(m::Message) -> bool) -> m::Result<()> {
        let chat: i64 = conversation.parse().map_err(|_| Error::Damaged(format!("no chat {conversation}")))?;
        let mut from = 0;
        loop {
            let lines = self.session.lines(self.id, chat, from, PAGE).map_err(model_error)?;
            let n = lines.len() as u64;
            for line in lines {
                let sender = if line.from_me {
                    Some(ME.to_owned())
                } else {
                    line.sender_row.and_then(|r| self.jids.get(&r).cloned()).or_else(|| self.direct.get(&chat).cloned())
                };
                if !each(message(conversation, line, sender)) {
                    return Err(Error::Cancelled);
                }
            }
            if n < PAGE as u64 {
                return Ok(());
            }
            from += n;
        }
    }
}

/// WhatsApp's result codes for a call.
fn outcome(code: Option<i64>) -> m::CallOutcome {
    match code {
        Some(5) => m::CallOutcome::Answered,
        Some(2) => m::CallOutcome::Missed,
        Some(3) => m::CallOutcome::Declined,
        Some(4) => m::CallOutcome::Cancelled,
        Some(1) => m::CallOutcome::Busy,
        Some(6) => m::CallOutcome::Failed,
        _ => m::CallOutcome::Unknown,
    }
}

/// The links in a text, for link parts.
fn links(text: &str) -> Vec<m::Part> {
    text.split_whitespace()
        .filter(|w| w.starts_with("https://") || w.starts_with("http://"))
        .map(|w| m::Part::Link { url: w.trim_end_matches([',', '.', ')', '!', '?']).to_owned(), title: None })
        .collect()
}

/// One index line as the model's message. The type codes are WhatsApp's (`oopz-gen`'s spec lists them).
fn message(conversation: &str, line: Line, sender: Option<String>) -> m::Message {
    let text = line.text.clone().filter(|t| !t.is_empty());
    let media_kind = match line.kind {
        1 | 42 => Some(m::MediaKind::Photo),
        2 => Some(if line.media.as_ref().and_then(|x| x.mime.as_deref()).is_some_and(|t| t.contains("ogg")) {
            m::MediaKind::Voice
        } else {
            m::MediaKind::Audio
        }),
        3 | 43 => Some(m::MediaKind::Video),
        9 => Some(m::MediaKind::Document),
        13 => Some(m::MediaKind::Gif),
        20 => Some(m::MediaKind::Sticker),
        81 => Some(m::MediaKind::VideoNote),
        _ => None,
    };
    let mut parts = Vec::new();
    let mut deleted = false;
    match (line.kind, media_kind) {
        (_, Some(kind)) => {
            let facts = line.media.clone();
            let view_once = matches!(line.kind, 42 | 43);
            parts.push(m::Part::Media(m::Media {
                kind,
                file: None,
                name: facts.as_ref().and_then(|x| x.name.clone()),
                mime: facts.as_ref().and_then(|x| x.mime.clone()),
                size: facts.as_ref().and_then(|x| x.size).and_then(|s| u64::try_from(s).ok()),
                width: facts.as_ref().and_then(|x| x.width).and_then(|s| u32::try_from(s).ok()),
                height: facts.as_ref().and_then(|x| x.height).and_then(|s| u32::try_from(s).ok()),
                secs: facts.as_ref().and_then(|x| x.secs).and_then(|s| u64::try_from(s).ok()),
                // A view-once message's content is never kept.
                caption: if view_once { None } else { text.clone() },
                view_once,
                omitted: false,
            }));
        }
        (0, None) => {
            if let Some(t) = &text {
                parts.push(m::Part::text(t.clone()));
                parts.extend(links(t));
            }
        }
        (4, None) => parts.push(m::Part::Contact { name: text.clone(), phones: Vec::new(), vcard: None }),
        (5 | 16, None) => {
            parts.push(m::Part::Location { lat: None, lng: None, name: text.clone(), live: line.kind == 16 })
        }
        (15, None) => deleted = true,
        (46, None) => parts.push(m::Part::Poll {
            question: text.clone().unwrap_or_default(),
            options: line
                .poll
                .iter()
                .map(|(name, votes)| m::PollOption { text: name.clone(), voters: Vec::new(), votes: *votes })
                .collect(),
            multiple: false,
        }),
        (90, None) => {
            let call = line.call.clone();
            parts.push(m::Part::Call {
                video: call.as_ref().is_some_and(|c| c.video),
                outcome: outcome(call.as_ref().and_then(|c| c.result)),
                secs: call.as_ref().and_then(|c| c.secs).and_then(|s| u64::try_from(s).ok()),
            });
        }
        (7, None) => parts.push(m::Part::System { event: m::Event::Other, text: text.clone() }),
        (code, None) => parts.push(m::Part::Unknown { code: code.to_string(), text: text.clone() }),
    }
    m::Message {
        id: line.id.to_string(),
        conversation: conversation.to_owned(),
        sender,
        from_me: line.from_me,
        time_ms: (line.ts > 0).then_some(line.ts),
        edited_ms: None,
        edited: line.edited,
        deleted,
        forwarded: line.forwarded > 0,
        starred: false,
        reply_to: line.reply.as_ref().and_then(|r| r.id).map(|id| id.to_string()),
        parts,
        reactions: line
            .reactions
            .iter()
            .map(|(emoji, from_me)| m::Reaction {
                by: from_me.then(|| ME.to_owned()),
                from_me: *from_me,
                emoji: emoji.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phones_come_only_from_phone_ids() {
        assert_eq!(phone_of("999912345678@s.whatsapp.net").as_deref(), Some("+999912345678"));
        assert_eq!(phone_of("999912345678:12@s.whatsapp.net").as_deref(), Some("+999912345678"));
        assert_eq!(phone_of("12345678901234@lid"), None);
        assert_eq!(phone_of("999912345-1600000000@g.us"), None);
    }

    #[test]
    fn detect_by_name_or_folder() {
        assert!(WhatsAppAdapter.detect(Path::new("msgstore-2024-01-01.1.db.crypt15")));
        assert!(!WhatsAppAdapter.detect(Path::new("msgstore.db.crypt14")));
        assert!(!WhatsAppAdapter.detect(Path::new("chat.txt")));
    }

    #[test]
    fn a_key_is_asked_for() {
        let e = WhatsAppAdapter.open(Path::new("msgstore.db.crypt15"), None).err().unwrap();
        assert!(matches!(e, Error::NeedsSecret));
        let e = WhatsAppAdapter.open(Path::new("msgstore.db.crypt15"), Some("  ")).err().unwrap();
        assert!(matches!(e, Error::NeedsSecret));
    }
}
