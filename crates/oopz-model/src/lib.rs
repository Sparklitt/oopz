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
                Part::Card { title, text, .. } => {
                    add(title.as_deref().unwrap_or(""));
                    add(text.as_deref().unwrap_or(""));
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
    /// An album header, a sticker pack, a calendar event, a payment, a game, a business message or a story: what
    /// the source names it (`title`) and any text it had.
    Card {
        kind: CardKind,
        title: Option<String>,
        text: Option<String>,
    },
    /// The app's own notice (`docs/data-types.md` §4).
    System {
        event: Event,
        text: Option<String>,
    },
    /// What the adapter couldn't place: the app's own type code, and any text it had. Shown as a labelled card
    /// ("WhatsApp: 9999, not shown yet"), counted and exported, never dropped.
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
            Part::Card { kind, .. } => (*kind).into(),
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
/// The same vocabulary as [`msgkinds::Event`] (decision 0020), under the names oopz stored first: `topic_created` is
/// msgkinds' `topic_changed`, and `admin_changed` (an export that doesn't say which way) is kept for what it says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Encryption,
    SecurityCodeChanged,
    NumberChanged,
    DeviceChanged,
    AccountDeleted,
    ContactJoined,
    BusinessNotice,
    GroupCreated,
    /// A topic (a thread inside a group) was created, edited or closed.
    TopicCreated,
    Renamed,
    DescriptionChanged,
    PictureChanged,
    PictureRemoved,
    SettingsChanged,
    MemberAdded,
    MemberJoined,
    MemberLeft,
    MemberRemoved,
    JoinRequested,
    AdminAdded,
    AdminRemoved,
    /// Someone was made admin or stopped being one, and the source doesn't say which.
    AdminChanged,
    OwnerChanged,
    CommunityChanged,
    InviteLinkReset,
    Disappearing,
    KeptInChat,
    ChatLocked,
    Screenshot,
    Pinned,
    Unpinned,
    Blocked,
    Unblocked,
    MessageDeleted,
    HistoryCleared,
    GroupCall,
    PollClosed,
    PaymentStatus,
    MissedMessages,
    ThemeChanged,
    #[default]
    Other,
}

impl Event {
    /// Every event, [`Event::Other`] last.
    pub const ALL: [Event; 41] = [
        Event::Encryption,
        Event::SecurityCodeChanged,
        Event::NumberChanged,
        Event::DeviceChanged,
        Event::AccountDeleted,
        Event::ContactJoined,
        Event::BusinessNotice,
        Event::GroupCreated,
        Event::TopicCreated,
        Event::Renamed,
        Event::DescriptionChanged,
        Event::PictureChanged,
        Event::PictureRemoved,
        Event::SettingsChanged,
        Event::MemberAdded,
        Event::MemberJoined,
        Event::MemberLeft,
        Event::MemberRemoved,
        Event::JoinRequested,
        Event::AdminAdded,
        Event::AdminRemoved,
        Event::AdminChanged,
        Event::OwnerChanged,
        Event::CommunityChanged,
        Event::InviteLinkReset,
        Event::Disappearing,
        Event::KeptInChat,
        Event::ChatLocked,
        Event::Screenshot,
        Event::Pinned,
        Event::Unpinned,
        Event::Blocked,
        Event::Unblocked,
        Event::MessageDeleted,
        Event::HistoryCleared,
        Event::GroupCall,
        Event::PollClosed,
        Event::PaymentStatus,
        Event::MissedMessages,
        Event::ThemeChanged,
        Event::Other,
    ];

    /// The stored name (the serialized one): msgkinds' name, except `topic_created` and `admin_changed`.
    pub fn code(self) -> &'static str {
        match self {
            Event::TopicCreated => "topic_created",
            Event::AdminChanged => "admin_changed",
            e => msgkinds::Event::from(e).as_str(),
        }
    }
}

impl From<msgkinds::Event> for Event {
    fn from(e: msgkinds::Event) -> Self {
        use msgkinds::Event as E;
        match e {
            E::Encryption => Event::Encryption,
            E::SecurityCodeChanged => Event::SecurityCodeChanged,
            E::NumberChanged => Event::NumberChanged,
            E::DeviceChanged => Event::DeviceChanged,
            E::AccountDeleted => Event::AccountDeleted,
            E::ContactJoined => Event::ContactJoined,
            E::BusinessNotice => Event::BusinessNotice,
            E::GroupCreated => Event::GroupCreated,
            E::TopicChanged => Event::TopicCreated,
            E::Renamed => Event::Renamed,
            E::DescriptionChanged => Event::DescriptionChanged,
            E::PictureChanged => Event::PictureChanged,
            E::PictureRemoved => Event::PictureRemoved,
            E::SettingsChanged => Event::SettingsChanged,
            E::MemberAdded => Event::MemberAdded,
            E::MemberJoined => Event::MemberJoined,
            E::MemberLeft => Event::MemberLeft,
            E::MemberRemoved => Event::MemberRemoved,
            E::JoinRequested => Event::JoinRequested,
            E::AdminAdded => Event::AdminAdded,
            E::AdminRemoved => Event::AdminRemoved,
            E::OwnerChanged => Event::OwnerChanged,
            E::CommunityChanged => Event::CommunityChanged,
            E::InviteLinkReset => Event::InviteLinkReset,
            E::Disappearing => Event::Disappearing,
            E::KeptInChat => Event::KeptInChat,
            E::ChatLocked => Event::ChatLocked,
            E::Screenshot => Event::Screenshot,
            E::Pinned => Event::Pinned,
            E::Unpinned => Event::Unpinned,
            E::Blocked => Event::Blocked,
            E::Unblocked => Event::Unblocked,
            E::MessageDeleted => Event::MessageDeleted,
            E::HistoryCleared => Event::HistoryCleared,
            E::GroupCall => Event::GroupCall,
            E::PollClosed => Event::PollClosed,
            E::PaymentStatus => Event::PaymentStatus,
            E::MissedMessages => Event::MissedMessages,
            E::ThemeChanged => Event::ThemeChanged,
            E::Other => Event::Other,
        }
    }
}

impl From<Event> for msgkinds::Event {
    fn from(e: Event) -> Self {
        use msgkinds::Event as E;
        match e {
            Event::Encryption => E::Encryption,
            Event::SecurityCodeChanged => E::SecurityCodeChanged,
            Event::NumberChanged => E::NumberChanged,
            Event::DeviceChanged => E::DeviceChanged,
            Event::AccountDeleted => E::AccountDeleted,
            Event::ContactJoined => E::ContactJoined,
            Event::BusinessNotice => E::BusinessNotice,
            Event::GroupCreated => E::GroupCreated,
            Event::TopicCreated => E::TopicChanged,
            Event::Renamed => E::Renamed,
            Event::DescriptionChanged => E::DescriptionChanged,
            Event::PictureChanged => E::PictureChanged,
            Event::PictureRemoved => E::PictureRemoved,
            Event::SettingsChanged => E::SettingsChanged,
            Event::MemberAdded => E::MemberAdded,
            Event::MemberJoined => E::MemberJoined,
            Event::MemberLeft => E::MemberLeft,
            Event::MemberRemoved => E::MemberRemoved,
            Event::JoinRequested => E::JoinRequested,
            Event::AdminAdded => E::AdminAdded,
            Event::AdminRemoved => E::AdminRemoved,
            // msgkinds has no "either way": the closer of the two.
            Event::AdminChanged => E::AdminAdded,
            Event::OwnerChanged => E::OwnerChanged,
            Event::CommunityChanged => E::CommunityChanged,
            Event::InviteLinkReset => E::InviteLinkReset,
            Event::Disappearing => E::Disappearing,
            Event::KeptInChat => E::KeptInChat,
            Event::ChatLocked => E::ChatLocked,
            Event::Screenshot => E::Screenshot,
            Event::Pinned => E::Pinned,
            Event::Unpinned => E::Unpinned,
            Event::Blocked => E::Blocked,
            Event::Unblocked => E::Unblocked,
            Event::MessageDeleted => E::MessageDeleted,
            Event::HistoryCleared => E::HistoryCleared,
            Event::GroupCall => E::GroupCall,
            Event::PollClosed => E::PollClosed,
            Event::PaymentStatus => E::PaymentStatus,
            Event::MissedMessages => E::MissedMessages,
            Event::ThemeChanged => E::ThemeChanged,
            Event::Other => E::Other,
        }
    }
}

/// What a message is, for filters, counts and the kind chips: one per message ([`Message::kind`]). The same kinds,
/// under the same names, as [`msgkinds::Kind`] (decision 0020); kept as oopz's own type so it can be serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Text,
    Link,
    Photo,
    Video,
    VideoNote,
    Gif,
    Voice,
    Audio,
    Sticker,
    Document,
    Location,
    Contact,
    Poll,
    Call,
    Album,
    StickerPack,
    Event,
    Payment,
    Game,
    Business,
    Story,
    System,
    Deleted,
    Unknown,
}

impl Kind {
    pub const ALL: [Kind; 24] = [
        Kind::Text,
        Kind::Link,
        Kind::Photo,
        Kind::Video,
        Kind::VideoNote,
        Kind::Gif,
        Kind::Voice,
        Kind::Audio,
        Kind::Sticker,
        Kind::Document,
        Kind::Location,
        Kind::Contact,
        Kind::Poll,
        Kind::Call,
        Kind::Album,
        Kind::StickerPack,
        Kind::Event,
        Kind::Payment,
        Kind::Game,
        Kind::Business,
        Kind::Story,
        Kind::System,
        Kind::Deleted,
        Kind::Unknown,
    ];

    /// The stored name: msgkinds' own.
    pub fn code(self) -> &'static str {
        msgkinds::Kind::from(self).as_str()
    }

    pub fn from_code(code: &str) -> Option<Kind> {
        code.parse::<msgkinds::Kind>().ok().map(Kind::from)
    }

    /// A media file comes with it.
    pub fn is_media(self) -> bool {
        msgkinds::Kind::from(self).is_media()
    }

    /// It plays for a while: a sticker, a photo, a GIF, a document or an album never shows a duration.
    pub fn has_duration(self) -> bool {
        msgkinds::Kind::from(self).has_duration()
    }
}

impl From<msgkinds::Kind> for Kind {
    fn from(k: msgkinds::Kind) -> Self {
        use msgkinds::Kind as K;
        match k {
            K::Text => Kind::Text,
            K::Link => Kind::Link,
            K::Photo => Kind::Photo,
            K::Video => Kind::Video,
            K::VideoNote => Kind::VideoNote,
            K::Gif => Kind::Gif,
            K::Voice => Kind::Voice,
            K::Audio => Kind::Audio,
            K::Sticker => Kind::Sticker,
            K::Document => Kind::Document,
            K::Location => Kind::Location,
            K::Contact => Kind::Contact,
            K::Poll => Kind::Poll,
            K::Call => Kind::Call,
            K::Album => Kind::Album,
            K::StickerPack => Kind::StickerPack,
            K::Event => Kind::Event,
            K::Payment => Kind::Payment,
            K::Game => Kind::Game,
            K::Business => Kind::Business,
            K::Story => Kind::Story,
            K::System => Kind::System,
            K::Deleted => Kind::Deleted,
            K::Unknown => Kind::Unknown,
        }
    }
}

impl From<Kind> for msgkinds::Kind {
    fn from(k: Kind) -> Self {
        use msgkinds::Kind as K;
        match k {
            Kind::Text => K::Text,
            Kind::Link => K::Link,
            Kind::Photo => K::Photo,
            Kind::Video => K::Video,
            Kind::VideoNote => K::VideoNote,
            Kind::Gif => K::Gif,
            Kind::Voice => K::Voice,
            Kind::Audio => K::Audio,
            Kind::Sticker => K::Sticker,
            Kind::Document => K::Document,
            Kind::Location => K::Location,
            Kind::Contact => K::Contact,
            Kind::Poll => K::Poll,
            Kind::Call => K::Call,
            Kind::Album => K::Album,
            Kind::StickerPack => K::StickerPack,
            Kind::Event => K::Event,
            Kind::Payment => K::Payment,
            Kind::Game => K::Game,
            Kind::Business => K::Business,
            Kind::Story => K::Story,
            Kind::System => K::System,
            Kind::Deleted => K::Deleted,
            Kind::Unknown => K::Unknown,
        }
    }
}

impl From<MediaKind> for Kind {
    fn from(m: MediaKind) -> Self {
        match m {
            MediaKind::Photo => Kind::Photo,
            MediaKind::Video => Kind::Video,
            MediaKind::VideoNote => Kind::VideoNote,
            MediaKind::Gif => Kind::Gif,
            MediaKind::Sticker => Kind::Sticker,
            MediaKind::Voice => Kind::Voice,
            MediaKind::Audio => Kind::Audio,
            MediaKind::Document => Kind::Document,
        }
    }
}

impl MediaKind {
    /// The media kind of a message kind, for the kinds that come with a file.
    pub fn from_kind(kind: Kind) -> Option<MediaKind> {
        Some(match kind {
            Kind::Photo => MediaKind::Photo,
            Kind::Video => MediaKind::Video,
            Kind::VideoNote => MediaKind::VideoNote,
            Kind::Gif => MediaKind::Gif,
            Kind::Sticker => MediaKind::Sticker,
            Kind::Voice => MediaKind::Voice,
            Kind::Audio => MediaKind::Audio,
            Kind::Document => MediaKind::Document,
            _ => return None,
        })
    }
}

/// What a [`Part::Card`] is: the kinds the model holds as a title and text until each gets its own part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    /// The header above several photos or videos sent together; the items follow as their own messages.
    Album,
    StickerPack,
    /// A calendar event or reminder.
    Event,
    Payment,
    Game,
    Business,
    Story,
}

impl CardKind {
    pub const ALL: [CardKind; 7] = [
        CardKind::Album,
        CardKind::StickerPack,
        CardKind::Event,
        CardKind::Payment,
        CardKind::Game,
        CardKind::Business,
        CardKind::Story,
    ];

    /// The card kind of a message kind, for the kinds a card holds.
    pub fn from_kind(kind: Kind) -> Option<CardKind> {
        Some(match kind {
            Kind::Album => CardKind::Album,
            Kind::StickerPack => CardKind::StickerPack,
            Kind::Event => CardKind::Event,
            Kind::Payment => CardKind::Payment,
            Kind::Game => CardKind::Game,
            Kind::Business => CardKind::Business,
            Kind::Story => CardKind::Story,
            _ => return None,
        })
    }
}

impl From<CardKind> for Kind {
    fn from(c: CardKind) -> Self {
        match c {
            CardKind::Album => Kind::Album,
            CardKind::StickerPack => Kind::StickerPack,
            CardKind::Event => Kind::Event,
            CardKind::Payment => Kind::Payment,
            CardKind::Game => Kind::Game,
            CardKind::Business => Kind::Business,
            CardKind::Story => Kind::Story,
        }
    }
}

impl From<App> for msgkinds::App {
    fn from(a: App) -> Self {
        match a {
            App::WhatsApp => msgkinds::App::WhatsApp,
            App::Telegram => msgkinds::App::Telegram,
            App::Sms => msgkinds::App::Sms,
            App::Line => msgkinds::App::Line,
            App::Viber => msgkinds::App::Viber,
            App::Signal => msgkinds::App::Signal,
            App::IMessage => msgkinds::App::IMessage,
        }
    }
}

impl App {
    /// oopz's app for one of msgkinds', when oopz reads it (WhatsApp Business reads as WhatsApp).
    pub fn from_msgkinds(a: msgkinds::App) -> Option<App> {
        use msgkinds::App as A;
        match a {
            A::WhatsApp | A::WhatsAppBusiness => Some(App::WhatsApp),
            A::Telegram => Some(App::Telegram),
            A::Sms => Some(App::Sms),
            A::Line => Some(App::Line),
            A::Viber => Some(App::Viber),
            A::Signal => Some(App::Signal),
            A::IMessage => Some(App::IMessage),
            A::Messenger | A::Instagram | A::WeChat | A::Kik | A::Discord | A::Slack | A::GoogleChat | A::Hilo => None,
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

    /// Decision 0020: oopz's kinds are msgkinds' kinds, in the same order and under the same stored names.
    #[test]
    fn kinds_are_msgkinds_kinds() {
        assert_eq!(Kind::ALL.len(), msgkinds::Kind::ALL.len());
        for (k, shared) in Kind::ALL.into_iter().zip(msgkinds::Kind::ALL) {
            assert_eq!(msgkinds::Kind::from(k), shared);
            assert_eq!(Kind::from(shared), k);
            assert_eq!(serde_json::to_string(&k).unwrap(), format!("{:?}", shared.as_str()));
            assert_eq!(k.has_duration(), shared.has_duration());
        }
        for name in ["text", "link", "photo", "video", "gif", "sticker", "voice", "audio", "document", "location"] {
            assert!(Kind::from_code(name).is_some(), "{name}");
        }
        assert!(!Kind::Sticker.has_duration() && !Kind::Photo.has_duration() && !Kind::Album.has_duration());
        for c in CardKind::ALL {
            assert_eq!(CardKind::from_kind(c.into()), Some(c));
        }
    }

    /// Every msgkinds event has an oopz one and back; the stored names are oopz's, the serialized ones.
    #[test]
    fn events_map_both_ways() {
        for e in msgkinds::Event::ALL.into_iter().chain([msgkinds::Event::Other]) {
            assert_eq!(msgkinds::Event::from(Event::from(e)), e);
        }
        for e in Event::ALL {
            assert_eq!(serde_json::to_string(&e).unwrap(), format!("{:?}", e.code()));
            if !matches!(e, Event::TopicCreated | Event::AdminChanged) {
                assert_eq!(e.code(), msgkinds::Event::from(e).as_str());
            }
        }
        assert_eq!(Event::from(msgkinds::Event::TopicChanged), Event::TopicCreated);
    }

    #[test]
    fn apps_map_to_msgkinds_apps() {
        for a in App::ALL {
            let shared = msgkinds::App::from(a);
            assert_eq!(shared.as_str(), a.code());
            assert_eq!(App::from_msgkinds(shared), Some(a));
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
