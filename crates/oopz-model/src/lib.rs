//! One model for every messaging app (decision 0013, `docs/data-types.md`): adapters turn an app's export or backup
//! into these types, and everything after that (the library, search, selection, exports, timelines) works on them
//! only. No I/O here beyond the [`Adapter`] trait's signatures.
//!
//! Ids are the app's own where it has them, and otherwise made by the adapter, stable for one file (`line:120`): a
//! message is unique by (conversation, id). Times are milliseconds since 1970 in UTC; `None` when the source has none
//! (never invented). Nothing an adapter meets is dropped: what it can't place becomes [`Part::Unknown`] with the
//! app's own code.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The apps oopz has an adapter for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum App {
    #[serde(rename = "whatsapp")]
    WhatsApp,
    Telegram,
    Sms,
    Line,
    Viber,
    Signal,
    #[serde(rename = "imessage")]
    IMessage,
}

impl App {
    pub const ALL: [App; 7] =
        [App::WhatsApp, App::Telegram, App::Sms, App::Line, App::Viber, App::Signal, App::IMessage];

    /// The stable code used in files, URLs and the CLI (`whatsapp`, `telegram`, …).
    pub fn code(self) -> &'static str {
        match self {
            App::WhatsApp => "whatsapp",
            App::Telegram => "telegram",
            App::Sms => "sms",
            App::Line => "line",
            App::Viber => "viber",
            App::Signal => "signal",
            App::IMessage => "imessage",
        }
    }

    /// The name people know the app by.
    pub fn name(self) -> &'static str {
        match self {
            App::WhatsApp => "WhatsApp",
            App::Telegram => "Telegram",
            App::Sms => "SMS",
            App::Line => "LINE",
            App::Viber => "Viber",
            App::Signal => "Signal",
            App::IMessage => "iMessage",
        }
    }

    pub fn from_code(code: &str) -> Option<App> {
        App::ALL.into_iter().find(|a| a.code() == code)
    }
}

/// Someone in a conversation, as one app knows them. People in different apps are linked into one contact by the
/// owner (the library's job), never by an adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    /// The app's id for them (a phone number, a user id, a name when the export has nothing else).
    pub id: String,
    /// In international form (`+…`) when the app gives one.
    pub phone: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    /// The name the app or the address book gives them.
    pub name: Option<String>,
    /// The owner of the source.
    pub me: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    /// One-to-one.
    #[default]
    Direct,
    Group,
    /// A channel, newsletter or broadcast that only some members post to.
    Channel,
    Broadcast,
    /// Own notes ("message yourself", Saved Messages, LINE Keep, Viber My Notes).
    Notes,
    Unknown,
}

impl ConversationKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Group => "group",
            Self::Channel => "channel",
            Self::Broadcast => "broadcast",
            Self::Notes => "notes",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "direct" => Self::Direct,
            "group" => Self::Group,
            "channel" => Self::Channel,
            "broadcast" => Self::Broadcast,
            "notes" => Self::Notes,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub kind: ConversationKind,
    pub name: Option<String>,
    /// Person ids, the owner included when the source says so.
    pub members: Vec<String>,
    pub created_ms: Option<i64>,
}

/// One message: an envelope and one or more parts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation: String,
    /// The sender's person id; `None` for the app's own notices.
    pub sender: Option<String>,
    pub from_me: bool,
    pub time_ms: Option<i64>,
    /// When it was last edited, when the source says; [`Message::edited`] is set either way.
    pub edited_ms: Option<i64>,
    pub edited: bool,
    /// Deleted for everyone: the placeholder is kept, the content isn't there.
    pub deleted: bool,
    pub forwarded: bool,
    /// Starred, saved or favourite.
    pub starred: bool,
    /// The id of the message this one answers (it may not be in the source).
    pub reply_to: Option<String>,
    pub parts: Vec<Part>,
    pub reactions: Vec<Reaction>,
}

impl Message {
    /// What the message mostly is, for filters and counts: its first media, place, poll, … part, else text.
    pub fn kind(&self) -> Kind {
        if self.deleted {
            return Kind::Deleted;
        }
        let mut kind = Kind::Text;
        for part in &self.parts {
            match part.kind() {
                Kind::Text => {}
                Kind::Link if kind == Kind::Text => kind = Kind::Link,
                Kind::Link => {}
                other => return other,
            }
        }
        if self.parts.is_empty() { Kind::Unknown } else { kind }
    }

    /// All the text a person would read: text parts, captions, poll questions and options, names of places,
    /// contacts and files; for search and plain exports.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut add = |s: &str| {
            if !s.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(s);
            }
        };
        for part in &self.parts {
            match part {
                Part::Text { text } => add(text),
                Part::Media(m) => {
                    if let Some(c) = &m.caption {
                        add(c);
                    }
                    if m.kind == MediaKind::Document
                        && let Some(n) = &m.name
                    {
                        add(n);
                    }
                }
                Part::Location { name, .. } => add(name.as_deref().unwrap_or("")),
                Part::Contact { name, .. } => add(name.as_deref().unwrap_or("")),
                Part::Poll { question, options, .. } => {
                    add(question);
                    for o in options {
                        add(&o.text);
                    }
                }
                Part::Link { url, title } => {
                    if !self.parts.iter().any(|p| matches!(p, Part::Text { text } if text.contains(url.as_str()))) {
                        add(url);
                    }
                    add(title.as_deref().unwrap_or(""));
                }
                Part::System { text, .. } | Part::Unknown { text, .. } => add(text.as_deref().unwrap_or("")),
                Part::Call { .. } => {}
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    /// The person id; `None` when the source doesn't say who.
    pub by: Option<String>,
    pub from_me: bool,
    pub emoji: String,
}

/// A part of a message (`docs/data-types.md` §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    Text {
        text: String,
    },
    Media(Media),
    Location {
        /// `None` when the source only says a place was shared (LINE's "[Location]").
        lat: Option<f64>,
        lng: Option<f64>,
        name: Option<String>,
        /// A live location shared for a while.
        live: bool,
    },
    Contact {
        name: Option<String>,
        phones: Vec<String>,
        /// The card itself, when the source kept it.
        vcard: Option<String>,
    },
    Poll {
        question: String,
        options: Vec<PollOption>,
        multiple: bool,
    },
    Call {
        video: bool,
        outcome: CallOutcome,
        secs: Option<u64>,
    },
    Link {
        url: String,
        title: Option<String>,
    },
    /// The app's own notice (`docs/data-types.md` §4).
    System {
        event: Event,
        text: Option<String>,
    },
    /// What the adapter couldn't place: the app's own type code, and any text it had.
    Unknown {
        code: String,
        text: Option<String>,
    },
}

impl Part {
    pub fn text(text: impl Into<String>) -> Self {
        Part::Text { text: text.into() }
    }

    pub fn kind(&self) -> Kind {
        match self {
            Part::Text { .. } => Kind::Text,
            Part::Media(m) => m.kind.into(),
            Part::Location { .. } => Kind::Location,
            Part::Contact { .. } => Kind::Contact,
            Part::Poll { .. } => Kind::Poll,
            Part::Call { .. } => Kind::Call,
            Part::Link { .. } => Kind::Link,
            Part::System { .. } => Kind::System,
            Part::Unknown { .. } => Kind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    pub kind: MediaKind,
    /// The file's path relative to the source's media folder, when the source points at one.
    pub file: Option<String>,
    /// The file's own name (a document's title), when the app kept it.
    pub name: Option<String>,
    pub mime: Option<String>,
    pub size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub secs: Option<u64>,
    pub caption: Option<String>,
    /// View-once media: its content is never shown or exported.
    pub view_once: bool,
    /// The source says the file wasn't included (`<Media omitted>`, a file never downloaded).
    pub omitted: bool,
    /// The file's bytes when the source holds them inline (an MMS part in an SMS backup). The library writes them
    /// to its own media folder on import and sets [`Media::file`]; never serialized.
    #[serde(skip)]
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    #[default]
    Photo,
    Video,
    /// A round video note.
    VideoNote,
    Gif,
    Sticker,
    Voice,
    Audio,
    Document,
}

impl MediaKind {
    /// From a file name's extension, for exports that only give a name.
    pub fn from_file_name(name: &str) -> Self {
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "heic" | "heif" | "avif" | "bmp" => MediaKind::Photo,
            "webp" | "tgs" => MediaKind::Sticker,
            "gif" => MediaKind::Gif,
            "mp4" | "mov" | "3gp" | "mkv" | "webm" | "avi" => MediaKind::Video,
            "opus" | "ogg" | "oga" | "amr" => MediaKind::Voice,
            "mp3" | "m4a" | "aac" | "wav" | "flac" => MediaKind::Audio,
            _ => MediaKind::Document,
        }
    }

    /// A mime type guessed from a file name, for exports that give none.
    pub fn mime_for(name: &str) -> Option<&'static str> {
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
        Some(match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "heic" => "image/heic",
            "mp4" => "video/mp4",
            "mov" => "video/quicktime",
            "3gp" => "video/3gpp",
            "webm" => "video/webm",
            "opus" | "ogg" | "oga" => "audio/ogg",
            "amr" => "audio/amr",
            "mp3" => "audio/mpeg",
            "m4a" => "audio/mp4",
            "aac" => "audio/aac",
            "pdf" => "application/pdf",
            "vcf" => "text/vcard",
            "txt" => "text/plain",
            "zip" => "application/zip",
            "apk" => "application/vnd.android.package-archive",
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollOption {
    pub text: String,
    /// Person ids of who voted for it, when the source says; else empty with `votes` counted.
    pub voters: Vec<String>,
    pub votes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallOutcome {
    Answered,
    Missed,
    Declined,
    Cancelled,
    Busy,
    Failed,
    #[default]
    Unknown,
}

/// The app's own notices (`docs/data-types.md` §4), grouped; the app's wording is kept in [`Part::System::text`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Encryption,
    SecurityCodeChanged,
    NumberChanged,
    GroupCreated,
    /// A topic (a thread inside a group) was created.
    TopicCreated,
    Renamed,
    DescriptionChanged,
    PictureChanged,
    MemberAdded,
    MemberJoined,
    MemberLeft,
    MemberRemoved,
    AdminChanged,
    SettingsChanged,
    Disappearing,
    Pinned,
    Blocked,
    MessageDeleted,
    HistoryCleared,
    #[default]
    Other,
}

/// What a message is, for filters, counts and the kind chips: one per message ([`Message::kind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Text,
    Link,
    Photo,
    Video,
    Gif,
    Sticker,
    Voice,
    Audio,
    Document,
    Location,
    Contact,
    Poll,
    Call,
    System,
    Deleted,
    Unknown,
}

impl Kind {
    pub const ALL: [Kind; 16] = [
        Kind::Text,
        Kind::Link,
        Kind::Photo,
        Kind::Video,
        Kind::Gif,
        Kind::Sticker,
        Kind::Voice,
        Kind::Audio,
        Kind::Document,
        Kind::Location,
        Kind::Contact,
        Kind::Poll,
        Kind::Call,
        Kind::System,
        Kind::Deleted,
        Kind::Unknown,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Link => "link",
            Kind::Photo => "photo",
            Kind::Video => "video",
            Kind::Gif => "gif",
            Kind::Sticker => "sticker",
            Kind::Voice => "voice",
            Kind::Audio => "audio",
            Kind::Document => "document",
            Kind::Location => "location",
            Kind::Contact => "contact",
            Kind::Poll => "poll",
            Kind::Call => "call",
            Kind::System => "system",
            Kind::Deleted => "deleted",
            Kind::Unknown => "unknown",
        }
    }

    pub fn from_code(code: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.code() == code)
    }

    /// A media file comes with it.
    pub fn is_media(self) -> bool {
        matches!(
            self,
            Kind::Photo | Kind::Video | Kind::Gif | Kind::Sticker | Kind::Voice | Kind::Audio | Kind::Document
        )
    }
}

impl From<MediaKind> for Kind {
    fn from(m: MediaKind) -> Self {
        match m {
            MediaKind::Photo => Kind::Photo,
            MediaKind::Video | MediaKind::VideoNote => Kind::Video,
            MediaKind::Gif => Kind::Gif,
            MediaKind::Sticker => Kind::Sticker,
            MediaKind::Voice => Kind::Voice,
            MediaKind::Audio => Kind::Audio,
            MediaKind::Document => Kind::Document,
        }
    }
}

/// What an opened source is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub app: App,
    /// The adapter's id (`whatsapp-export`, `telegram-json`, …).
    pub adapter: String,
    /// The format version the adapter read, when the source states one.
    pub format: Option<String>,
    /// The owner, when the source says who they are.
    pub owner: Option<Person>,
    /// The folder media file paths are relative to.
    pub media_root: Option<PathBuf>,
    /// The times are the phone's wall clock with no zone (text exports), stored as if UTC (decision 0013, point 2):
    /// shown and counted as they are, never shifted to the computer's zone.
    pub wall_clock: bool,
    /// Lines or records the adapter kept as unknown or had to read around, in plain words (no message text).
    pub notes: Vec<String>,
}

/// Why a source couldn't be opened. Never holds a key, a password or message text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The file isn't this adapter's format.
    #[error("not a {0}")]
    NotThisFormat(&'static str),
    /// A version of the format oopz hasn't seen, refused by name.
    #[error("{0}: a version oopz doesn't know")]
    UnsupportedVersion(String),
    #[error("this source needs its owner's key or password")]
    NeedsSecret,
    #[error("the key or password doesn't open it, or the file is damaged")]
    WrongSecret,
    #[error("damaged: {0}")]
    Damaged(String),
    #[error("{0}: {1}")]
    Io(String, std::io::Error),
    #[error("stopped")]
    Cancelled,
}

impl Error {
    pub fn io(context: impl std::fmt::Display, e: std::io::Error) -> Self {
        Error::Io(context.to_string(), e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// What an adapter needs besides the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Secret {
    None,
    /// WhatsApp's 64-digit key.
    Key,
    /// A backup password the owner chose.
    Password,
}

/// One app's way in: an official export, or a backup opened with the owner's own key or password; nothing else
/// (`docs/plan-all-apps.md`, "The rule every adapter follows").
pub trait Adapter: Send + Sync {
    /// Stable id (`whatsapp-export`).
    fn id(&self) -> &'static str;
    fn app(&self) -> App;
    fn secret(&self) -> Secret;
    /// The secret this particular source asks for: an iPhone backup wants its password only when it's encrypted.
    fn secret_for(&self, _path: &Path) -> Secret {
        self.secret()
    }
    /// A cheap look at a file or folder (its name, its first bytes): could this adapter open it?
    fn detect(&self, path: &Path) -> bool;
    /// Reads the source. `secret` is the owner's key or password when [`Adapter::secret`] asks for one.
    fn open(&self, path: &Path, secret: Option<&str>) -> Result<Box<dyn Source>>;
}

/// An opened source. The text exports are read whole when opened (they're small next to a backup); a backup reads
/// its messages per conversation.
pub trait Source: Send {
    fn info(&self) -> &SourceInfo;
    fn people(&self) -> &[Person];
    fn conversations(&self) -> &[Conversation];
    /// The messages of `conversation`, oldest first as the source orders them; `each` returns false to stop.
    fn messages(&mut self, conversation: &str, each: &mut dyn FnMut(Message) -> bool) -> Result<()>;
}

/// A source already read into memory: what the text-export adapters return.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub info: SourceInfo,
    pub people: Vec<Person>,
    pub conversations: Vec<Conversation>,
    /// Every message, grouped by conversation in source order.
    pub messages: Vec<Message>,
}

impl Source for Loaded {
    fn info(&self) -> &SourceInfo {
        &self.info
    }

    fn people(&self) -> &[Person] {
        &self.people
    }

    fn conversations(&self) -> &[Conversation] {
        &self.conversations
    }

    fn messages(&mut self, conversation: &str, each: &mut dyn FnMut(Message) -> bool) -> Result<()> {
        for m in self.messages.iter().filter(|m| m.conversation == conversation) {
            if !each(m.clone()) {
                return Err(Error::Cancelled);
            }
        }
        Ok(())
    }
}

/// A phone number in one form for linking: digits only, with a leading `+` kept when there was one. `None` for
/// fewer than 5 digits (short codes, "Unknown").
pub fn normalize_phone(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let digits: String = trimmed.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 5 || digits.len() > 18 {
        return None;
    }
    let plus = trimmed.starts_with('+') || trimmed.starts_with("00");
    let digits = if trimmed.starts_with("00") { digits[2..].to_string() } else { digits };
    Some(if plus { format!("+{digits}") } else { digits })
}

/// Days since 1970-01-01 for a proleptic Gregorian date, for adapters that parse dates by hand (no time-zone data).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Milliseconds since 1970 for a date and time in a fixed offset from UTC (`offset_mins` east). `None` for a day or
/// time that doesn't exist (31 February, 25:00).
pub fn civil_to_ms(y: i64, mo: u32, d: u32, h: u32, mi: u32, s: u32, offset_mins: i32) -> Option<i64> {
    if !(1..=12).contains(&mo) || d == 0 || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if d > month_days[mo as usize - 1] {
        return None;
    }
    let secs = days_from_civil(y, mo, d) * 86_400 + h as i64 * 3600 + mi as i64 * 60 + s.min(59) as i64;
    Some((secs - offset_mins as i64 * 60) * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_what_its_first_meaningful_part_is() {
        let mut m = Message { parts: vec![Part::text("hi")], ..Default::default() };
        assert_eq!(m.kind(), Kind::Text);
        m.parts.push(Part::Link { url: "https://example.org".into(), title: None });
        assert_eq!(m.kind(), Kind::Link);
        m.parts.push(Part::Media(Media { kind: MediaKind::Voice, ..Default::default() }));
        assert_eq!(m.kind(), Kind::Voice);
        m.deleted = true;
        assert_eq!(m.kind(), Kind::Deleted);
        assert_eq!(Message::default().kind(), Kind::Unknown);
    }

    #[test]
    fn text_gathers_what_a_person_reads_without_repeating_a_link() {
        let m = Message {
            parts: vec![
                Part::text("see https://example.org"),
                Part::Link { url: "https://example.org".into(), title: Some("Example".into()) },
                Part::Media(Media {
                    kind: MediaKind::Document,
                    name: Some("plan.pdf".into()),
                    caption: Some("the plan".into()),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        assert_eq!(m.text(), "see https://example.org\nExample\nthe plan\nplan.pdf");
    }

    #[test]
    fn codes_round_trip() {
        for a in App::ALL {
            assert_eq!(App::from_code(a.code()), Some(a));
        }
        for k in Kind::ALL {
            assert_eq!(Kind::from_code(k.code()), Some(k));
        }
        assert_eq!(App::from_code("myspace"), None);
        // The serialized names are the codes.
        for a in App::ALL {
            assert_eq!(serde_json::to_string(&a).unwrap(), format!("{:?}", a.code()));
        }
    }

    #[test]
    fn phones_are_normalized_for_linking() {
        assert_eq!(normalize_phone("+34 999 00 11 22").as_deref(), Some("+34999001122"));
        assert_eq!(normalize_phone("0034 999-00-11-22").as_deref(), Some("+34999001122"));
        assert_eq!(normalize_phone("(999) 555-0100").as_deref(), Some("9995550100"));
        assert_eq!(normalize_phone("1234"), None);
        assert_eq!(normalize_phone("Unknown"), None);
    }

    #[test]
    fn civil_times_are_checked_and_offset() {
        assert_eq!(civil_to_ms(1970, 1, 1, 0, 0, 0, 0), Some(0));
        assert_eq!(civil_to_ms(2024, 2, 29, 12, 0, 0, 0), Some(1_709_208_000_000));
        assert_eq!(civil_to_ms(2023, 2, 29, 12, 0, 0, 0), None);
        assert_eq!(civil_to_ms(2024, 1, 1, 1, 0, 0, 60), civil_to_ms(2024, 1, 1, 0, 0, 0, 0));
        assert_eq!(civil_to_ms(1969, 12, 31, 23, 59, 59, 0), Some(-1000));
        assert_eq!(civil_to_ms(2024, 13, 1, 0, 0, 0, 0), None);
        assert_eq!(civil_to_ms(2024, 1, 1, 24, 0, 0, 0), None);
    }

    #[test]
    fn media_kinds_from_names() {
        assert_eq!(MediaKind::from_file_name("PTT-20240101-WA0001.opus"), MediaKind::Voice);
        assert_eq!(MediaKind::from_file_name("IMG-1.JPG"), MediaKind::Photo);
        assert_eq!(MediaKind::from_file_name("STK-1.webp"), MediaKind::Sticker);
        assert_eq!(MediaKind::from_file_name("noext"), MediaKind::Document);
        assert_eq!(MediaKind::mime_for("a.PDF"), Some("application/pdf"));
    }
}
