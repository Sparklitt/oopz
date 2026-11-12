//! WhatsApp's "Export chat" text: one group chat in four layouts (Android day first, US month first with a 12-hour
//! clock, Spanish with "p. m." and CRLF, iPhone in brackets with U+200E marks and a BOM), with every placeholder the
//! export writes, multi-line messages, system lines and the media files beside the text.

use oopz_model::Kind;

use super::{Export, Manifest, civil, year_start};
use crate::corpus;
use crate::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `31/12/2023, 21:41 - Name: text`
    Android,
    /// `12/31/23, 9:41 PM - Name: text` (U+202F before PM)
    Us,
    /// `31/12/23, 9:41 p. m. - Name: text` (U+00A0 before p. m.), Spanish placeholders, CRLF
    Spanish,
    /// `[31.12.23, 21:41:05] Name: text`, U+200E marks, a BOM
    Ios,
}

impl Layout {
    pub const ALL: [Layout; 4] = [Layout::Android, Layout::Us, Layout::Spanish, Layout::Ios];

    pub fn name(self) -> &'static str {
        match self {
            Layout::Android => "android",
            Layout::Us => "us",
            Layout::Spanish => "es",
            Layout::Ios => "ios",
        }
    }

    /// The chat file's name, as the app names it.
    pub fn file_name(self) -> String {
        match self {
            Layout::Android | Layout::Us => format!("WhatsApp Chat with {GROUP}.txt"),
            Layout::Spanish => format!("Chat de WhatsApp con {GROUP}.txt"),
            Layout::Ios => msgkinds::media_type::sources::whatsapp::IOS_CHAT_TEXT.into(),
        }
    }
}

/// The group's name, and its owner (who wrote "You deleted this message").
pub const GROUP: &str = "Fútbol los jueves";
pub const OWNER: &str = "Ana Prueba";
const PEOPLE: [&str; 3] = [OWNER, "Ben Ejemplo", "Carla Inventado"];

enum Content {
    Text(String),
    Link(String),
    Omitted,
    /// Android's file name, iPhone's file name, a caption.
    Attached(&'static str, &'static str, Option<&'static str>),
    Contact,
    Location,
    Poll,
    Deleted,
    Edited(&'static str),
    ViewOnce,
    MissedCall,
    Encrypted,
    Created,
    Added,
    Renamed,
}

impl Content {
    fn kind(&self, layout: Layout) -> Kind {
        match self {
            Content::Text(_) | Content::Edited(_) => Kind::Text,
            Content::Link(_) => Kind::Link,
            Content::Omitted if layout == Layout::Ios => Kind::Photo,
            Content::Omitted => Kind::Document,
            Content::Attached(name, ..) => match &name[..3] {
                "IMG" => Kind::Photo,
                "PTT" => Kind::Voice,
                "VID" => Kind::Video,
                "STK" => Kind::Sticker,
                _ => Kind::Document,
            },
            Content::Contact => Kind::Contact,
            Content::Location => Kind::Location,
            Content::Poll => Kind::Poll,
            Content::Deleted => Kind::Deleted,
            Content::ViewOnce => Kind::Photo,
            Content::MissedCall => Kind::Call,
            Content::Encrypted | Content::Created | Content::Added | Content::Renamed => Kind::System,
        }
    }

    fn is_system(&self) -> bool {
        matches!(self, Content::Encrypted | Content::Created | Content::Added | Content::Renamed)
    }
}

/// The chat in `layout`: the text file first, then the media files it names.
pub fn generate(seed: u64, layout: Layout) -> Export {
    let mut rng = Rng::fork(seed, 0x57A7);
    let specials: Vec<(usize, Content)> = vec![
        (1, Content::Attached("IMG-20231212-WA0001.jpg", "00000005-PHOTO-2023-12-12-10-00-00.jpg", Some("mira"))),
        (0, Content::Attached("PTT-20231212-WA0002.opus", "00000006-AUDIO-2023-12-12-10-05-00.opus", None)),
        (2, Content::Attached("VID-20231213-WA0003.mp4", "00000007-VIDEO-2023-12-13-11-00-00.mp4", None)),
        (1, Content::Attached("STK-20231213-WA0004.webp", "00000008-STICKER-2023-12-13-11-05-00.webp", None)),
        (2, Content::Attached("Factura_2023.pdf", "00000009-Factura_2023.pdf", Some("la factura"))),
        (0, Content::Contact),
        (1, Content::Omitted),
        (2, Content::Location),
        (1, Content::Poll),
        (2, Content::Deleted),
        (0, Content::Deleted),
        (1, Content::Edited("nos vemos a las 8")),
        (2, Content::ViewOnce),
        (0, Content::Link("mira https://example.org/receta".into())),
        (1, Content::MissedCall),
        (2, Content::Renamed),
        (2, Content::Text("مرحبا، كيف حالك؟".into())),
        (0, Content::Text("Z̤͔ͧ̑̓ä͖̭̈̇lͮ̒ͫǫ̗̻ שלום".into())),
        (1, Content::Text("line one\nline two\n\nline four".into())),
        (0, Content::Text(corpus::long_text(&mut rng, true, 70_000).trim_end().to_string())),
    ];
    let mut script: Vec<(usize, Content)> = vec![(1, Content::Encrypted), (1, Content::Created), (1, Content::Added)];
    for special in specials {
        for _ in 0..1 + rng.below(3) {
            let mut text = {
                let es = rng.chance(0.5);
                corpus::sentence(&mut rng, es)
            };
            if rng.chance(0.15) {
                text.push('\n');
                text.push_str(&corpus::sentence(&mut rng, true));
            }
            script.push((rng.below(3) as usize, Content::Text(text)));
        }
        script.push(special);
    }

    // From 10 December 2023, so both day-first and month-first dates have a number over 12.
    let mut ms = year_start(2023) + (343 * 24 + 8) * 3_600_000;
    let mut manifest = Manifest { conversations: 1, ..Default::default() };
    let mut out = String::new();
    if layout == Layout::Ios {
        out.push('\u{FEFF}');
    }
    let newline = if layout == Layout::Spanish { "\r\n" } else { "\n" };
    for (who, content) in &script {
        ms += 60_000 + rng.below(8 * 3_600_000) as i64;
        let from_me = *who == 0 && !content.is_system();
        manifest.count(content.kind(layout), from_me);
        let body = body(content, *who, layout);
        if content.is_system() {
            if layout == Layout::Ios {
                out.push_str(&format!("{}{GROUP}: \u{200E}{body}", header(ms, layout)));
            } else {
                out.push_str(&format!("{}{body}", header(ms, layout)));
            }
        } else {
            let mark = if body.starts_with(lrm()) { lrm() } else { "" };
            out.push_str(&format!("{mark}{}{}: {body}", header(ms, layout), PEOPLE[*who]));
        }
        out.push('\n');
    }
    let out = if newline == "\n" { out } else { out.replace('\n', newline) };

    let mut files = vec![(layout.file_name(), out.into_bytes())];
    for (_, content) in &script {
        match content {
            Content::Attached(android, ios, _) => {
                let name = if layout == Layout::Ios { ios } else { android };
                files.push((name.to_string(), format!("oopz-gen placeholder {name}").into_bytes()));
            }
            Content::Contact => files.push((contact_file(layout).into(), VCARD.as_bytes().to_vec())),
            _ => {}
        }
    }
    Export { files, manifest }
}

const VCARD: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Ben Ejemplo\r\nTEL;type=CELL:+99990000002\r\nEND:VCARD\r\n";

/// iPhone's left-to-right mark, the first of the marks WhatsApp writes (msgkinds' wording `[shape] marks`).
fn lrm() -> &'static str {
    msgkinds::wording::shape(msgkinds::Source::WhatsAppExport, "marks")[0]
}

fn contact_file(layout: Layout) -> &'static str {
    if layout == Layout::Ios { "00000011-Ben Ejemplo.vcf" } else { "Ben Ejemplo.vcf" }
}

fn header(ms: i64, layout: Layout) -> String {
    let (y, mo, d, h, mi, s) = civil(ms);
    let (h12, pm) = (if h % 12 == 0 { 12 } else { h % 12 }, h >= 12);
    match layout {
        Layout::Android => format!("{d:02}/{mo:02}/{y}, {h:02}:{mi:02} - "),
        Layout::Us => format!("{mo}/{d}/{:02}, {h12}:{mi:02}\u{202F}{} - ", y % 100, if pm { "PM" } else { "AM" }),
        Layout::Spanish => {
            format!("{d}/{mo}/{:02}, {h12}:{mi:02}\u{00A0}{} - ", y % 100, if pm { "p. m." } else { "a. m." })
        }
        Layout::Ios => format!("[{d:02}.{mo:02}.{:02}, {h:02}:{mi:02}:{s:02}] ", y % 100),
    }
}

fn body(content: &Content, who: usize, layout: Layout) -> String {
    let es = layout == Layout::Spanish;
    let ios = layout == Layout::Ios;
    let pick = |en: &str, sp: &str| if es { sp.to_string() } else { en.to_string() };
    let lrm = if ios { lrm() } else { "" };
    match content {
        Content::Text(t) | Content::Link(t) => t.clone(),
        Content::Omitted if ios => "\u{200E}image omitted".into(),
        Content::Omitted => pick("<Media omitted>", "<Multimedia omitido>"),
        Content::Attached(android, ios_name, caption) => {
            let mut s = if ios {
                format!("\u{200E}<attached: {ios_name}>")
            } else {
                let pages = if msgkinds::media_type::name::has_format(android, msgkinds::media_type::formats::PDF) {
                    pick(" • 2 pages", " • 2 páginas")
                } else {
                    String::new()
                };
                format!("{android}{pages} {}", pick("(file attached)", "(archivo adjunto)"))
            };
            if let Some(c) = caption {
                s.push('\n');
                s.push_str(c);
            }
            s
        }
        Content::Contact if ios => format!("\u{200E}<attached: {}>", contact_file(layout)),
        Content::Contact => format!("{} {}", contact_file(layout), pick("(file attached)", "(archivo adjunto)")),
        Content::Location => {
            let word = if ios {
                "Location"
            } else if es {
                "ubicación"
            } else {
                "location"
            };
            format!("{word}: https://maps.google.com/?q=40.416800,-3.703800")
        }
        Content::Poll if es => {
            "ENCUESTA:\n¿Cena?\nOPCIÓN: Pizza (2 votos)\nOPCIÓN: Sushi (1 voto)\nOPCIÓN: Tacos (0 votos)".into()
        }
        Content::Poll => {
            "POLL:\n¿Cena?\nOPTION: Pizza (2 votes)\nOPTION: Sushi (1 vote)\nOPTION: Tacos (0 votes)".into()
        }
        Content::Deleted if who == 0 => {
            format!(
                "{lrm}{}",
                pick(
                    if ios { "You deleted this message." } else { "You deleted this message" },
                    "Eliminaste este mensaje."
                )
            )
        }
        Content::Deleted => format!(
            "{lrm}{}",
            pick(
                if ios { "This message was deleted." } else { "This message was deleted" },
                "Se eliminó este mensaje."
            )
        ),
        Content::Edited(t) => format!("{t} {lrm}{}", pick("<This message was edited>", "<Se editó este mensaje.>")),
        Content::ViewOnce => format!("{lrm}{}", pick("View once photo omitted", "Foto de visualización única omitida")),
        Content::MissedCall => format!("{lrm}{}", pick("Missed voice call", "Llamada de voz perdida")),
        Content::Encrypted => pick(
            "Messages and calls are end-to-end encrypted. No one outside of this chat, not even WhatsApp, can read or listen to them. Tap to learn more.",
            "Los mensajes y las llamadas están cifrados de extremo a extremo. Nadie fuera de este chat, ni siquiera WhatsApp, puede leerlos ni escucharlos. Toca para obtener más información.",
        ),
        Content::Created if ios => format!("Ben Ejemplo created group “{GROUP}”"),
        Content::Created => {
            pick(&format!("Ben Ejemplo created group \"{GROUP}\""), &format!("Ben Ejemplo creó el grupo \"{GROUP}\""))
        }
        Content::Added => pick("Ben Ejemplo added you", "Ben Ejemplo te añadió"),
        Content::Renamed if ios => "Carla Inventado changed the subject to “Fútbol: los jueves”".into(),
        Content::Renamed => pick(
            "Carla Inventado changed the subject to \"Fútbol: los jueves\"",
            "Carla Inventado cambió el asunto a \"Fútbol: los jueves\"",
        ),
    }
}
