//! What to generate: how many people, chats and messages, over which dates, in which mix of message kinds. Presets
//! cover the usual cases; every field can be changed from the command line.

/// A kind of message the generator writes, with WhatsApp's `message_type` code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Text,
    EmojiOnly,
    LongText,
    Link,
    Image,
    Video,
    Voice,
    Audio,
    Document,
    Sticker,
    Gif,
    Location,
    LiveLocation,
    Contact,
    Poll,
    Call,
    Deleted,
    ViewOnce,
    VideoNote,
    Album,
}

impl Kind {
    pub const ALL: [Kind; 20] = [
        Kind::Text,
        Kind::EmojiOnly,
        Kind::LongText,
        Kind::Link,
        Kind::Image,
        Kind::Video,
        Kind::Voice,
        Kind::Audio,
        Kind::Document,
        Kind::Sticker,
        Kind::Gif,
        Kind::Location,
        Kind::LiveLocation,
        Kind::Contact,
        Kind::Poll,
        Kind::Call,
        Kind::Deleted,
        Kind::ViewOnce,
        Kind::VideoNote,
        Kind::Album,
    ];

    /// WhatsApp's `message.message_type`.
    pub fn code(self) -> i64 {
        match self {
            Kind::Text | Kind::EmojiOnly | Kind::LongText | Kind::Link => 0,
            Kind::Image => 1,
            Kind::Voice | Kind::Audio => 2,
            Kind::Video => 3,
            Kind::Contact => 4,
            Kind::Location => 5,
            Kind::Document => 9,
            Kind::Gif => 13,
            Kind::Deleted => 15,
            Kind::LiveLocation => 16,
            Kind::Sticker => 20,
            Kind::ViewOnce => 42,
            Kind::Poll => 46,
            Kind::VideoNote => 81,
            Kind::Call => 90,
            Kind::Album => 99,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::EmojiOnly => "emoji",
            Kind::LongText => "long-text",
            Kind::Link => "link",
            Kind::Image => "image",
            Kind::Video => "video",
            Kind::Voice => "voice",
            Kind::Audio => "audio",
            Kind::Document => "document",
            Kind::Sticker => "sticker",
            Kind::Gif => "gif",
            Kind::Location => "location",
            Kind::LiveLocation => "live-location",
            Kind::Contact => "contact",
            Kind::Poll => "poll",
            Kind::Call => "call",
            Kind::Deleted => "deleted",
            Kind::ViewOnce => "view-once",
            Kind::VideoNote => "video-note",
            Kind::Album => "album",
        }
    }

    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// Roughly how often each kind turns up in a busy personal chat, per 1000 messages.
    fn usual_weight(self) -> u32 {
        match self {
            Kind::Text => 700,
            Kind::EmojiOnly => 40,
            Kind::LongText => 5,
            Kind::Link => 15,
            Kind::Image => 80,
            Kind::Video => 15,
            Kind::Voice => 40,
            Kind::Audio => 3,
            Kind::Document => 6,
            Kind::Sticker => 30,
            Kind::Gif => 8,
            Kind::Location => 3,
            Kind::LiveLocation => 1,
            Kind::Contact => 2,
            Kind::Poll => 1,
            Kind::Call => 8,
            Kind::Deleted => 6,
            Kind::ViewOnce => 3,
            Kind::VideoNote => 2,
            Kind::Album => 4,
        }
    }
}

/// How often things happen to a message, each a probability per message.
#[derive(Debug, Clone)]
pub struct Rates {
    pub reply: f64,
    pub forwarded: f64,
    pub edited: f64,
    pub starred: f64,
    pub reaction: f64,
    /// A media message whose file isn't in `Media/` (deleted from the phone, never downloaded).
    pub missing_media: f64,
    /// A media file with the same bytes as an earlier one (forwarded photos, the same sticker).
    pub duplicate_media: f64,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            reply: 0.06,
            forwarded: 0.02,
            edited: 0.01,
            starred: 0.005,
            reaction: 0.05,
            missing_media: 0.05,
            duplicate_media: 0.02,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMode {
    /// Rows only: `message_media` describes files that aren't written (fast, for big databases).
    None,
    /// A small placeholder file for each media row (not a real image or sound).
    Files,
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub seed: u64,
    /// People with a one-to-one chat with the owner.
    pub contacts: u32,
    pub groups: u32,
    /// Members of each group besides the owner, at least and at most.
    pub group_size: (u32, u32),
    /// Messages in chat 1, the long conversation (one-to-one with the first contact); 0 for none.
    pub main_chat: u64,
    /// Messages in every other chat, at least and at most.
    pub per_chat: (u64, u64),
    /// The first and last message times, in ms since 1970 (UTC).
    pub start_ms: i64,
    pub end_ms: i64,
    /// Per-kind weights, in [`Kind::ALL`]'s order.
    pub mix: [u32; 20],
    pub rates: Rates,
    pub media: MediaMode,
    /// Odd but real things: empty chats, a chat with oneself, nameless contacts, strange text, unknown types,
    /// orphan rows, timestamps out of order. Each chat also starts with one message of every kind.
    pub edge_cases: bool,
    /// Write `wa.db.crypt15` (the contacts' names) beside the messages.
    pub contacts_db: bool,
    /// Also keep the unencrypted `msgstore.db` and `wa.db`.
    pub keep_plain: bool,
}

/// 2019-01-01 and 2026-11-01, UTC.
const START: i64 = 1_546_300_800_000;
const END: i64 = 1_793_491_200_000;

impl Spec {
    pub fn usual_mix() -> [u32; 20] {
        Kind::ALL.map(Kind::usual_weight)
    }

    pub fn preset(name: &str) -> Option<Spec> {
        let base = Spec {
            seed: 1,
            contacts: 8,
            groups: 3,
            group_size: (3, 8),
            main_chat: 3_000,
            per_chat: (20, 400),
            start_ms: START,
            end_ms: END,
            mix: Self::usual_mix(),
            rates: Rates::default(),
            media: MediaMode::Files,
            edge_cases: false,
            contacts_db: true,
            keep_plain: false,
        };
        Some(match name {
            // A few thousand messages: seconds to make, for clicking through the app.
            "small" => base,
            // One long conversation of a million messages over eight years, plus an ordinary phone's other chats.
            "million" => Spec {
                contacts: 40,
                groups: 12,
                group_size: (3, 40),
                main_chat: 1_000_000,
                per_chat: (30, 3_000),
                media: MediaMode::None,
                ..base
            },
            // Many chats: thousands of contacts, hundreds of groups (about 3 million messages).
            "stress" => Spec {
                contacts: 2_000,
                groups: 300,
                group_size: (2, 256),
                main_chat: 100_000,
                per_chat: (10, 2_500),
                media: MediaMode::None,
                ..base
            },
            // Everything odd, small enough to read row by row.
            "edge" => Spec {
                contacts: 6,
                groups: 3,
                main_chat: 400,
                per_chat: (0, 60),
                edge_cases: true,
                rates: Rates {
                    reply: 0.2,
                    forwarded: 0.1,
                    edited: 0.1,
                    starred: 0.05,
                    reaction: 0.25,
                    missing_media: 0.2,
                    duplicate_media: 0.1,
                },
                ..base
            },
            _ => return None,
        })
    }

    pub const PRESETS: [&str; 4] = ["small", "million", "stress", "edge"];

    /// Refuses a spec that can't be generated, naming the field.
    pub fn check(&self) -> Result<(), String> {
        if self.end_ms <= self.start_ms {
            return Err("the end date must be after the start date".into());
        }
        if self.group_size.0 > self.group_size.1 || self.per_chat.0 > self.per_chat.1 {
            return Err("a minimum is above its maximum".into());
        }
        if self.group_size.1 > 1023 {
            return Err("a WhatsApp group has at most 1024 members, the owner included".into());
        }
        if self.groups > 0 && self.group_size.0 == 0 {
            return Err("a group needs at least one member besides the owner".into());
        }
        if self.main_chat > 0 && self.contacts == 0 {
            return Err("the long conversation needs at least one contact".into());
        }
        if self.mix.iter().all(|w| *w == 0) {
            return Err("every kind has weight 0".into());
        }
        let rates = &self.rates;
        for (name, p) in [
            ("reply", rates.reply),
            ("forwarded", rates.forwarded),
            ("edited", rates.edited),
            ("starred", rates.starred),
            ("reaction", rates.reaction),
            ("missing-media", rates.missing_media),
            ("duplicate-media", rates.duplicate_media),
        ] {
            if !(0.0..=1.0).contains(&p) {
                return Err(format!("the {name} rate must be between 0 and 1"));
            }
        }
        Ok(())
    }
}
