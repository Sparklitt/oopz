//! Viber's "Export message history": a zip of CSV files, one per chat. A one-to-one chat with a header, day-first
//! dates and a 24-hour clock; a group with no header, month-first two-digit-year dates and a 12-hour clock. Fields
//! with commas, quotes and newlines are quoted; media are the app's own placeholder text.

use msgkinds::registry::codes::viber_csv as viber;
use oopz_model::Kind;

use super::{Export, Manifest, civil, year_start};
use crate::corpus;
use crate::rng::Rng;

const BEN: (&str, &str) = ("Ben Ejemplo", "+99990000002");
const CARLA: (&str, &str) = ("Carla Inventado", "+99990000003");
const ME: (&str, &str) = ("Me", "");

fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

pub fn generate(seed: u64) -> Export {
    let mut rng = Rng::fork(seed, 0x71BE);
    let mut manifest = Manifest { conversations: 2, ..Default::default() };
    let odd = [
        viber::PHOTO_MESSAGE,
        viber::STICKER_MESSAGE,
        viber::VIDEO_MESSAGE,
        "hola, ¿qué tal? \"bien\"",
        "line one\nline two",
        "mira https://example.org/receta",
        "مرحبا Z̤͔ͧ̑̓ä͖̭̈̇lͮ̒ͫǫ̗̻",
    ];

    let mut direct = String::from("Date,Time,Sender,Phone,Message\n");
    let mut group = String::new();
    for (file, people, day_first) in [(&mut direct, &[ME, BEN][..], true), (&mut group, &[ME, BEN, CARLA][..], false)] {
        // From 10 January 2024: day 13 and later decide the order.
        let mut ms = year_start(2024) + 9 * 86_400_000 + 8 * 3_600_000;
        let mut rows: Vec<((&str, &str), String)> = Vec::new();
        for text in odd {
            for _ in 0..1 + rng.below(3) {
                rows.push((*rng.pick(people), {
                    let es = rng.chance(0.5);
                    corpus::sentence(&mut rng, es)
                }));
            }
            rows.push((people[1], text.to_string()));
        }
        for ((name, phone), text) in rows {
            ms += 60_000 + rng.below(10 * 3_600_000) as i64;
            let (y, mo, d, h, mi, s) = civil(ms);
            let (date, time) = if day_first {
                (format!("{d:02}/{mo:02}/{y}"), format!("{h:02}:{mi:02}:{s:02}"))
            } else {
                let h12 = if h % 12 == 0 { 12 } else { h % 12 };
                (format!("{mo}/{d}/{:02}", y % 100), format!("{h12}:{mi:02} {}", if h >= 12 { "PM" } else { "AM" }))
            };
            file.push_str(&format!("{date},{time},{},{},{}\n", field(name), field(phone), field(&text)));
            // Viber's sentences for a file it didn't attach are that kind of media (msgkinds' `viber_csv`).
            let kind = match text.as_str() {
                viber::PHOTO_MESSAGE => Kind::Photo,
                viber::STICKER_MESSAGE => Kind::Sticker,
                viber::VIDEO_MESSAGE => Kind::Video,
                t if !msgkinds::media_type::sources::whatsapp_export::links(t).is_empty() => Kind::Link,
                _ => Kind::Text,
            };
            manifest.count(kind, name == "Me");
        }
    }
    Export {
        files: vec![
            ("Viber Ben Ejemplo.csv".into(), direct.into_bytes()),
            ("Viber Fútbol los jueves.csv".into(), group.into_bytes()),
        ],
        manifest,
    }
}
