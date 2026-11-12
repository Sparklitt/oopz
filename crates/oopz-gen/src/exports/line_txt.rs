//! LINE's "Export chat history" text: a one-to-one chat in three English layouts (`2024/01/01(Mon)` with a 24-hour
//! clock, `2024.01.01 Monday` with `10:00 PM`, `Mon, 01/15/2024` with `PM10:00`) and a Japanese group
//! (`2024/01/01(月)`, BOM, CRLF), with every placeholder, calls, unsent messages and quoted multi-line messages.

use msgkinds::registry::codes::line_txt as line;
use oopz_model::Kind;

use super::{Export, Manifest, civil, weekday, year_start};
use crate::corpus;
use crate::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    En,
    EnDots,
    EnUs,
    Ja,
}

impl Layout {
    pub const ALL: [Layout; 4] = [Layout::En, Layout::EnDots, Layout::EnUs, Layout::Ja];

    pub fn name(self) -> &'static str {
        match self {
            Layout::En => "en",
            Layout::EnDots => "en-dots",
            Layout::EnUs => "en-us",
            Layout::Ja => "ja",
        }
    }
}

const PEOPLE: [&str; 3] = ["Ana Prueba", "Ben Ejemplo", "Carla Inventado"];
const DAYS_EN: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const DAYS_JA: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];

enum Content {
    Text(String),
    /// A placeholder: English, Japanese, its kind.
    Placeholder(&'static str, &'static str, Kind),
    /// A call line: English, Japanese.
    Call(&'static str, &'static str),
    Unsent,
    System,
}

pub fn generate(seed: u64, layout: Layout) -> Export {
    let mut rng = Rng::fork(seed, 0x11E0);
    let ja = layout == Layout::Ja;
    // The Japanese export is a group whose owner isn't written; the English ones are with Ben, so Ana is the owner.
    let people: &[usize] = if ja { &[0, 1, 2] } else { &[0, 1] };
    let specials = vec![
        Content::Placeholder(line::PHOTO, line::PHOTO_JA, Kind::Photo),
        Content::Placeholder(line::STICKER, line::STICKER_JA, Kind::Sticker),
        Content::Placeholder(line::VIDEO, line::VIDEO_JA, Kind::Video),
        Content::Placeholder(line::FILE, line::FILE_JA, Kind::Document),
        Content::Placeholder(line::VOICE_MESSAGE, line::VOICE_MESSAGE_JA, Kind::Voice),
        Content::Placeholder(line::CONTACT, line::CONTACT_JA, Kind::Contact),
        Content::Placeholder(line::LOCATION, line::LOCATION_JA, Kind::Location),
        Content::Placeholder(line::ALBUM, line::ALBUM_JA, Kind::Photo),
        Content::Call("☎ Call time 0:15", "☎ 通話時間 0:15"),
        Content::Call(line::MISSED_CALL, line::MISSED_CALL_JA),
        Content::Call(line::CANCELED_CALL, "☎ キャンセル"),
        Content::Unsent,
        Content::System,
        Content::Text("mira https://example.org/receta".into()),
        Content::Text("he said \"hi\"\nand then\n\n\"bye\"".into()),
        Content::Text("مرحبا Z̤͔ͧ̑̓ä͖̭̈̇lͮ̒ͫǫ̗̻ 日本語".into()),
    ];
    let mut manifest = Manifest { conversations: 1, ..Default::default() };
    let mut out = String::new();
    if ja {
        out.push('\u{FEFF}');
        out.push_str("[LINE] フットサル部のトーク履歴\n保存日時：2024/02/01 10:00\n\n");
    } else {
        out.push_str("[LINE] Chat history with Ben Ejemplo\nSaved on: 2024/02/01 10:00\n\n");
    }
    // From 10 January 2024, so a month-first date shows a day over 12.
    let mut ms = year_start(2024) + 9 * 86_400_000 + 8 * 3_600_000;
    let mut day = -1;
    for special in specials {
        let mut script: Vec<(usize, Content)> = Vec::new();
        for _ in 0..1 + rng.below(3) {
            let who = *rng.pick(people);
            script.push((
                who,
                Content::Text({
                    let es = rng.chance(0.5);
                    corpus::sentence(&mut rng, es)
                }),
            ));
        }
        script.push((1, special));
        for (who, content) in script {
            ms += 60_000 + rng.below(8 * 3_600_000) as i64;
            if ms.div_euclid(86_400_000) != day {
                day = ms.div_euclid(86_400_000);
                out.push_str(&date_line(ms, layout));
                out.push('\n');
            }
            let time = time(ms, layout);
            let name = PEOPLE[who];
            let (kind, line) = match content {
                Content::Text(t) => {
                    let kind = if t.contains("https://") { Kind::Link } else { Kind::Text };
                    let t = if t.contains('\n') || t.starts_with('"') {
                        format!("\"{}\"", t.replace('"', "\"\""))
                    } else {
                        t
                    };
                    (kind, format!("{time}\t{name}\t{t}"))
                }
                Content::Placeholder(en, jp, kind) => (kind, format!("{time}\t{name}\t{}", if ja { jp } else { en })),
                Content::Call(en, jp) => (Kind::Call, format!("{time}\t{name}\t{}", if ja { jp } else { en })),
                Content::Unsent if ja => (Kind::Deleted, format!("{time}\t{name}が送信を取り消しました")),
                Content::Unsent => (Kind::Deleted, format!("{time}\t{name} unsent a message.")),
                Content::System if ja => (Kind::System, format!("{time}\tCarla Inventadoがグループに参加しました。")),
                Content::System => (Kind::System, format!("{time}\t{name} changed the chat background.")),
            };
            let system = matches!(kind, Kind::System);
            manifest.count(kind, !ja && who == 0 && !system);
            out.push_str(&line);
            out.push('\n');
        }
    }
    let out = if ja { out.replace('\n', "\r\n") } else { out };
    let name = if ja { "[LINE] フットサル部のトーク.txt" } else { "[LINE] Chat with Ben Ejemplo.txt" };
    Export { files: vec![(name.into(), out.into_bytes())], manifest }
}

fn date_line(ms: i64, layout: Layout) -> String {
    let (y, mo, d, ..) = civil(ms);
    let wd = weekday(ms);
    match layout {
        Layout::En => format!("{y}/{mo:02}/{d:02}({})", &DAYS_EN[wd][..3]),
        Layout::EnDots => format!("{y}.{mo:02}.{d:02} {}", DAYS_EN[wd]),
        Layout::EnUs => format!("{}, {mo:02}/{d:02}/{y}", &DAYS_EN[wd][..3]),
        Layout::Ja => format!("{y}/{mo:02}/{d:02}({})", DAYS_JA[wd]),
    }
}

fn time(ms: i64, layout: Layout) -> String {
    let (_, _, _, h, mi, _) = civil(ms);
    let (h12, pm) = (if h % 12 == 0 { 12 } else { h % 12 }, h >= 12);
    match layout {
        Layout::En | Layout::Ja => format!("{h:02}:{mi:02}"),
        Layout::EnDots => format!("{h12:02}:{mi:02} {}", if pm { "PM" } else { "AM" }),
        Layout::EnUs => format!("{}{h12:02}:{mi:02}", if pm { "PM" } else { "AM" }),
    }
}
