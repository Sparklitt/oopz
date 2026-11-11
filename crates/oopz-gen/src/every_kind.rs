//! One message of every kind and one notice of every event, in the one model: for the readers' tests msgkinds asks
//! for (its `docs/registry.md`, "What readers test on their side", 2 and 3): each renders and exports, a kind without
//! a duration never shows one (its file says 0 seconds, as WhatsApp's do), and an unknown code (9999) is kept.
//! Invented data only.

use oopz_model::{
    App, CallOutcome, CardKind, Conversation, ConversationKind, Event, Kind, Loaded, Media, MediaKind, Message, Part,
    Person, PollOption, SourceInfo,
};

/// The conversation every message is in.
pub const CONVERSATION: &str = "every-kind";
/// The code of the [`Kind::Unknown`] message: no source names it.
pub const UNKNOWN_CODE: &str = "9999";

/// The parts of a message of `kind`.
pub fn parts(kind: Kind) -> Vec<Part> {
    if let Some(media) = MediaKind::from_kind(kind) {
        return vec![Part::Media(Media {
            kind: media,
            file: Some(format!("{}-9999.bin", kind.code())),
            name: (kind == Kind::Document).then(|| "plan-9999.pdf".into()),
            size: Some(20_480),
            // What WhatsApp writes for a sticker or a photo: 0.
            secs: Some(if kind.has_duration() { 75 } else { 0 }),
            ..Default::default()
        })];
    }
    if let Some(card) = CardKind::from_kind(kind) {
        return vec![Part::Card { kind: card, title: Some(format!("{} 9999", kind.code())), text: None }];
    }
    match kind {
        Kind::Text => vec![Part::text("Hola 9999")],
        Kind::Link => vec![
            Part::text("https://example.org/9999"),
            Part::Link { url: "https://example.org/9999".into(), title: Some("Example".into()) },
        ],
        Kind::Location => {
            vec![Part::Location { lat: Some(40.0), lng: Some(-3.0), name: Some("Plaza".into()), live: false }]
        }
        Kind::Contact => {
            vec![Part::Contact { name: Some("Ana 9999".into()), phones: vec!["+9999000001".into()], vcard: None }]
        }
        Kind::Poll => vec![Part::Poll {
            question: "¿Cena?".into(),
            options: vec![PollOption { text: "Pizza".into(), voters: Vec::new(), votes: 2 }],
            multiple: false,
        }],
        Kind::Call => vec![Part::Call { video: true, outcome: CallOutcome::Answered, secs: Some(75) }],
        Kind::System => vec![Part::System { event: Event::Other, text: Some("Notice 9999".into()) }],
        Kind::Deleted => Vec::new(),
        Kind::Unknown => vec![Part::Unknown { code: UNKNOWN_CODE.into(), text: None }],
        // Media and cards returned above.
        Kind::Photo
        | Kind::Video
        | Kind::VideoNote
        | Kind::Gif
        | Kind::Voice
        | Kind::Audio
        | Kind::Sticker
        | Kind::Document
        | Kind::Album
        | Kind::StickerPack
        | Kind::Event
        | Kind::Payment
        | Kind::Game
        | Kind::Business
        | Kind::Story
        | Kind::Reaction => unreachable!(),
    }
}

/// A WhatsApp source with a message of each [`Kind::ALL`], in that order, then a notice with no text of its own for
/// each [`Event::ALL`].
pub fn every_kind() -> Loaded {
    let mut messages = Vec::new();
    let mut push = |parts: Vec<Part>, deleted: bool| {
        let n = messages.len() as i64;
        messages.push(Message {
            id: format!("k{n}"),
            conversation: CONVERSATION.into(),
            sender: Some("ana".into()),
            time_ms: Some(1_704_067_200_000 + n * 60_000),
            deleted,
            parts,
            ..Default::default()
        });
    };
    for kind in Kind::ALL {
        push(parts(kind), kind == Kind::Deleted);
    }
    for event in Event::ALL {
        push(vec![Part::System { event, text: None }], false);
    }
    Loaded {
        info: SourceInfo {
            app: App::WhatsApp,
            adapter: "every-kind".into(),
            format: None,
            owner: None,
            media_root: None,
            wall_clock: false,
            notes: Vec::new(),
        },
        people: vec![
            Person { id: "me".into(), me: true, ..Default::default() },
            Person { id: "ana".into(), name: Some("Ana 9999".into()), ..Default::default() },
        ],
        conversations: vec![Conversation {
            id: CONVERSATION.into(),
            kind: ConversationKind::Direct,
            name: Some("Every kind".into()),
            members: vec!["me".into(), "ana".into()],
            created_ms: None,
        }],
        messages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_message_of_each_kind_then_each_event() {
        let l = every_kind();
        assert_eq!(l.messages.len(), Kind::ALL.len() + Event::ALL.len());
        for (m, kind) in l.messages.iter().zip(Kind::ALL) {
            assert_eq!(m.kind(), kind);
        }
        for m in &l.messages[Kind::ALL.len()..] {
            assert_eq!(m.kind(), Kind::System);
        }
    }
}
