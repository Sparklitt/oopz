//! Telegram Desktop's "Export Telegram data" as JSON (`result.json`): the owner, two address-book contacts and five
//! chats (a personal chat with every kind of message, a supergroup with service messages, Saved Messages, a
//! channel and the Replies feed), with media that was and wasn't included.

use msgkinds::media_type::formats;
use msgkinds::registry::codes::telegram_json as tg;
use msgkinds::registry::codes::telegram_json_action as action;
use msgkinds::registry::codes::telegram_json_entity as entity;
use msgkinds::registry::codes::telegram_json_media as media;
use oopz_model::Kind;
use serde_json::{Value, json};

use super::{Export, Manifest, civil, year_start};
use crate::corpus;
use crate::rng::Rng;

pub const OWNER_ID: u64 = 1_000_001;
const BEN: u64 = 1_000_002;
const CARLA: u64 = 1_000_003;
const NOT_INCLUDED: &str = "(File not included. Change data exporting settings to download.)";

struct Chat {
    messages: Vec<Value>,
    next_id: u64,
    ms: i64,
}

impl Chat {
    fn new(start_ms: i64) -> Self {
        Chat { messages: Vec::new(), next_id: 1, ms: start_ms }
    }

    /// Adds a message from `from` (a user id, or none for a channel post), with `fields` merged in.
    fn push(&mut self, rng: &mut Rng, manifest: &mut Manifest, kind: Kind, from: Option<u64>, fields: Value) {
        self.ms += 30_000 + rng.below(6 * 3_600_000) as i64;
        let (y, mo, d, h, mi, s) = civil(self.ms);
        let mut m = json!({
            "id": self.next_id,
            "type": tg::MESSAGE,
            "date": format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}"),
            "date_unixtime": (self.ms / 1000).to_string(),
            "text": "",
            "text_entities": [],
        });
        if let Some(id) = from {
            m["from"] = json!(name(id));
            m["from_id"] = json!(format!("user{id}"));
        }
        if let (Value::Object(m), Value::Object(f)) = (&mut m, fields) {
            m.extend(f);
        }
        self.next_id += 1;
        manifest.count(kind, from == Some(OWNER_ID));
        self.messages.push(m);
    }

    fn service(&mut self, rng: &mut Rng, manifest: &mut Manifest, kind: Kind, actor: u64, fields: Value) {
        let mut f = json!({ "type": tg::SERVICE, "actor": name(actor), "actor_id": format!("user{actor}") });
        if let (Value::Object(m), Value::Object(x)) = (&mut f, fields) {
            m.extend(x);
        }
        self.push(rng, manifest, kind, None, f);
        // `push` counts a service message as not mine: count the owner's own.
        if actor == OWNER_ID {
            manifest.from_me += 1;
        }
    }
}

fn name(id: u64) -> &'static str {
    match id {
        OWNER_ID => "Ana Prueba",
        BEN => "Ben Ejemplo",
        CARLA => "Carla Inventado",
        _ => "Dani Ficticio",
    }
}

fn fillers(chat: &mut Chat, rng: &mut Rng, manifest: &mut Manifest, people: &[u64]) {
    for _ in 0..1 + rng.below(3) {
        let from = *rng.pick(people);
        let text = {
            let es = rng.chance(0.5);
            corpus::sentence(rng, es)
        };
        chat.push(
            rng,
            manifest,
            Kind::Text,
            Some(from),
            json!({ "text": text.clone(), "text_entities": [{"type": entity::PLAIN, "text": text}] }),
        );
    }
}

pub fn generate(seed: u64) -> Export {
    let mut rng = Rng::fork(seed, 0x7E1E);
    let mut manifest = Manifest { conversations: 5, ..Default::default() };
    let m = &mut manifest;
    let me = OWNER_ID;

    let mut personal = Chat::new(year_start(2024));
    let duo = [me, BEN];
    let specials: Vec<(Kind, u64, Value)> = vec![
        (Kind::Link, me, json!({ "text": ["mira ", {"type": entity::LINK, "text": "https://example.org/receta"}] })),
        (
            Kind::Link,
            BEN,
            json!({ "text": [{"type": entity::TEXT_LINK, "text": "esto", "href": "https://example.com/news/123"}, " 👀"] }),
        ),
        (
            Kind::Photo,
            BEN,
            json!({ (tg::PHOTO): "photos/photo_1@01-01-2024_10-00-00.jpg", "photo_file_size": 52011, "width": 1280, "height": 960, "text": "la playa" }),
        ),
        (Kind::Photo, me, json!({ (tg::PHOTO): NOT_INCLUDED, "width": 800, "height": 600 })),
        (
            Kind::Voice,
            me,
            json!({ (tg::FILE): "voice_messages/audio_1@01-01-2024_10-05-00.ogg", "media_type": media::VOICE_MESSAGE, "mime_type": formats::OGG_AUDIO, "duration_seconds": 7, "file_size": 20480 }),
        ),
        (
            Kind::VideoNote,
            BEN,
            json!({ (tg::FILE): "round_video_messages/file_1@01-01-2024.mp4", "media_type": media::VIDEO_MESSAGE, "mime_type": formats::MP4, "duration_seconds": 12, "width": 384, "height": 384 }),
        ),
        (
            Kind::Video,
            me,
            json!({ (tg::FILE): NOT_INCLUDED, "media_type": media::VIDEO_FILE, "mime_type": formats::MP4, "duration_seconds": 61, "width": 1920, "height": 1080, "file_name": "IMG_0001.MP4" }),
        ),
        (
            Kind::Gif,
            BEN,
            json!({ (tg::FILE): "video_files/gif_1.mp4", "media_type": media::ANIMATION, "mime_type": formats::MP4, "duration_seconds": 3, "width": 320, "height": 240 }),
        ),
        (
            Kind::Sticker,
            BEN,
            json!({ (tg::FILE): "stickers/sticker.webp", "media_type": media::STICKER, "sticker_emoji": "😺", "mime_type": formats::WEBP, "width": 512, "height": 512 }),
        ),
        (
            Kind::Audio,
            me,
            json!({ (tg::FILE): "files/canción.mp3", "media_type": media::AUDIO_FILE, "mime_type": formats::MP3, "duration_seconds": 215, "performer": "Nadie", "title": "Canción de prueba" }),
        ),
        (
            Kind::Document,
            BEN,
            json!({ (tg::FILE): "files/Factura_2023.pdf", "file_name": "Factura_2023.pdf", "mime_type": formats::PDF, "file_size": 120345, "text": "la factura" }),
        ),
        (Kind::Location, me, json!({ (tg::LOCATION_INFORMATION): {"latitude": 40.4168, "longitude": -3.7038} })),
        (
            Kind::Location,
            BEN,
            json!({ (tg::LOCATION_INFORMATION): {"latitude": 41.3874, "longitude": 2.1686}, (tg::LIVE_LOCATION_PERIOD_SECONDS): 900 }),
        ),
        (
            Kind::Poll,
            BEN,
            json!({ (tg::POLL): {"question": "¿Cena?", "closed": false, "total_voters": 2, "answers": [{"text": "Pizza", "voters": 2, "chosen": true}, {"text": "Sushi", "voters": 0, "chosen": false}]} }),
        ),
        (
            Kind::Contact,
            me,
            json!({ (tg::CONTACT_INFORMATION): {"first_name": "Carla", "last_name": "Inventado", "phone_number": "+99990000003"}, "contact_vcard": "contacts/contact_1.vcard" }),
        ),
        (Kind::Text, me, json!({ "text": "contesto a algo que no está", "reply_to_message_id": 999_999 })),
        (Kind::Text, BEN, json!({ "text": "reenviado", "forwarded_from": "Canal de Pruebas" })),
        (
            Kind::Text,
            me,
            json!({ "text": "editado", "edited": "2024-01-20T10:00:00", "edited_unixtime": "1705744800" }),
        ),
        (
            Kind::Text,
            BEN,
            json!({ "text": "reacciona", "reactions": [{"type": "emoji", "count": 3, "emoji": "❤", "recent": [{"from": "Ana Prueba", "from_id": "user1000001", "date": "2024-01-20T10:00:00"}]}] }),
        ),
        (Kind::Text, me, json!({ "text": "مرحبا، كيف حالك؟ Z̤͔ͧ̑̓ä͖̭̈̇lͮ̒ͫǫ̗̻" })),
        (Kind::Unknown, BEN, json!({ "type": "something_new", "text": "" })),
        // Only `text_entities`, no `text`.
        (Kind::Text, BEN, json!({ "text": "", "text_entities": [{"type": entity::PLAIN, "text": "solo entidades"}] })),
        (
            Kind::Unknown,
            BEN,
            json!({ (tg::INVOICE_INFORMATION): {"title": "Entrada", "description": "Concierto", "amount": 145, "currency": "USD"} }),
        ),
        (
            Kind::Link,
            BEN,
            json!({ "via_bot": "@juegobot", (tg::GAME_TITLE): "Tetrominó", "game_description": "un juego", "game_link": "https://example.org/juego" }),
        ),
        (
            Kind::Unknown,
            BEN,
            json!({ (tg::GIVEAWAY_INFORMATION): {"quantity": 3, "months": 6, "until_date": "2024-03-01T00:00:00", "channels": [777]} }),
        ),
    ];
    for (kind, from, fields) in specials {
        fillers(&mut personal, &mut rng, m, &duo);
        personal.push(&mut rng, m, kind, Some(from), fields);
    }
    personal.service(
        &mut rng,
        m,
        Kind::Call,
        me,
        json!({ "action": action::PHONE_CALL, "duration_seconds": 95, "discard_reason": "hangup", "text": "" }),
    );
    personal.service(
        &mut rng,
        m,
        Kind::Call,
        BEN,
        json!({ "action": action::PHONE_CALL, "discard_reason": "missed", "text": "" }),
    );
    fillers(&mut personal, &mut rng, m, &duo);

    let mut group = Chat::new(year_start(2024) + 86_400_000);
    let all = [me, BEN, CARLA];
    group.service(
        &mut rng,
        m,
        Kind::System,
        BEN,
        json!({ "action": action::CREATE_GROUP, "title": "Fútbol", "members": ["Ben Ejemplo", "Ana Prueba"], "text": "" }),
    );
    group.service(
        &mut rng,
        m,
        Kind::System,
        BEN,
        json!({ "action": action::INVITE_MEMBERS, "members": ["Carla Inventado"], "text": "" }),
    );
    fillers(&mut group, &mut rng, m, &all);
    group.service(
        &mut rng,
        m,
        Kind::System,
        CARLA,
        json!({ "action": action::EDIT_GROUP_TITLE, "title": "Fútbol los jueves", "text": "" }),
    );
    group.service(&mut rng, m, Kind::System, me, json!({ "action": action::PIN_MESSAGE, "message_id": 3, "text": "" }));
    fillers(&mut group, &mut rng, m, &all);
    // Dani is in no member list: a sender the export doesn't otherwise know.
    group.push(&mut rng, m, Kind::Text, Some(1_000_004), json!({ "text": "hola, soy nuevo" }));
    group.service(
        &mut rng,
        m,
        Kind::System,
        BEN,
        json!({ "action": action::REMOVE_MEMBERS, "members": ["Dani Ficticio"], "text": "" }),
    );
    group.service(&mut rng, m, Kind::System, BEN, json!({ "action": action::BOOST_APPLY, "boosts": 1, "text": "" }));
    // An action no msgkinds row names (a newer Telegram): kept as unknown with its code.
    group.service(&mut rng, m, Kind::Unknown, BEN, json!({ "action": "something_newer", "text": "" }));
    group.service(
        &mut rng,
        m,
        Kind::System,
        CARLA,
        json!({ "action": action::TOPIC_CREATED, "title": "Partidos", "text": "" }),
    );
    group.service(
        &mut rng,
        m,
        Kind::System,
        CARLA,
        json!({ "action": action::TOPIC_EDIT, "new_title": "Partidos 2024", "text": "" }),
    );
    group.service(
        &mut rng,
        m,
        Kind::System,
        me,
        json!({ "action": action::EDIT_CHAT_THEME, "emoticon": "⚽", "text": "" }),
    );
    group.service(
        &mut rng,
        m,
        Kind::Game,
        BEN,
        json!({ "action": action::SCORE_IN_GAME, "game_message_id": 2, "score": 120, "text": "" }),
    );
    group.service(
        &mut rng,
        m,
        Kind::Payment,
        me,
        json!({ "action": action::SEND_PAYMENT, "amount": 500, "currency": "JPY", "invoice_message_id": 4, "text": "" }),
    );
    group.service(
        &mut rng,
        m,
        Kind::System,
        BEN,
        json!({ "action": action::GROUP_CALL_SCHEDULED, "schedule_date": 1_706_000_000, "text": "" }),
    );
    group.service(
        &mut rng,
        m,
        Kind::Call,
        BEN,
        json!({ "action": action::GROUP_CALL, "duration_seconds": 600, "text": "" }),
    );

    // Telegram's own feed of replies to the owner's comments.
    let mut replies = Chat::new(year_start(2024) + 4 * 86_400_000);
    replies.push(&mut rng, m, Kind::Text, Some(CARLA), json!({ "text": "respuesta a tu comentario" }));

    let mut saved = Chat::new(year_start(2024) + 2 * 86_400_000);
    saved.push(&mut rng, m, Kind::Text, Some(me), json!({ "text": "nota para mí" }));
    fillers(&mut saved, &mut rng, m, &[me]);

    let mut channel = Chat::new(year_start(2024) + 3 * 86_400_000);
    for _ in 0..3 {
        let text = corpus::sentence(&mut rng, true);
        channel.push(
            &mut rng,
            m,
            Kind::Text,
            None,
            json!({ "from": "Canal de Pruebas", "from_id": "channel777", "text": text }),
        );
    }

    let export = json!({
        "about": "Here is the data you requested. Remember: Telegram is ad free, it doesn't use your data for ad targeting and doesn't sell it to others.",
        "personal_information": {
            "user_id": OWNER_ID,
            "first_name": "Ana",
            "last_name": "Prueba",
            "phone_number": "+999 9000 0001",
            "username": "@ana_prueba",
            "bio": ""
        },
        "contacts": {
            "about": "This is your contact list.",
            "list": [
                {"first_name": "Ben", "last_name": "Ejemplo", "phone_number": "+99990000002", "date": "2023-01-01T10:00:00", "date_unixtime": "1672567200"},
                {"first_name": "Carla", "last_name": "", "phone_number": "+99990000003", "date": "2023-01-01T10:00:00", "date_unixtime": "1672567200"}
            ]
        },
        "chats": {
            "about": "This page lists all chats from this export.",
            "list": [
                {"name": "Ben Ejemplo", "type": "personal_chat", "id": BEN, "messages": personal.messages},
                {"name": "Fútbol los jueves", "type": "private_supergroup", "id": 1_777_000_001u64, "messages": group.messages},
                {"type": "saved_messages", "id": OWNER_ID, "messages": saved.messages},
                {"name": "Canal de Pruebas", "type": "public_channel", "id": 777, "messages": channel.messages},
                {"name": "Replies", "type": "replies", "id": 708_513, "messages": replies.messages}
            ]
        }
    });
    let text = serde_json::to_string_pretty(&export).unwrap_or_default();
    Export { files: vec![("result.json".into(), text.into_bytes())], manifest }
}
