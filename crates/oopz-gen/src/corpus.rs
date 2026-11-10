//! The words, names and places the generator draws from. All invented or generic: no real person, number or address.
//! Links use only the reserved example domains (RFC 2606).

use crate::rng::Rng;

pub const FIRST_NAMES: &[&str] = &[
    "Ana", "Ben", "Carla", "Dani", "Elena", "Fran", "Gabi", "Hugo", "Inés", "Javi", "Kai", "Lucía", "Marco", "Nuria",
    "Óscar", "Paula", "Quique", "Rocío", "Sergio", "Tania", "Úrsula", "Víctor", "Wendy", "Ximena", "Yago", "Zoe",
    "Ángel", "Begoña", "Chloé", "Dmitri", "Eun-ji", "Fátima", "Göran", "Hiroshi", "Ifeoma", "Jürgen", "Kwame", "Leïla",
    "Mårten", "Nguyễn", "Øystein", "Priya", "Raúl", "Siobhán", "Thandiwe", "Uriel", "Valérie", "Wiremu", "Xiao",
];

pub const LAST_NAMES: &[&str] = &[
    "Prueba",
    "Ejemplo",
    "Inventado",
    "Ficticio",
    "Sample",
    "Placeholder",
    "Muestra",
    "Test",
    "Demo",
    "Ensayo",
    "Nadie",
    "Cualquiera",
    "Fulano",
    "Mengano",
    "Zutano",
    "Doe",
    "Roe",
    "Probeta",
];

pub const GROUP_NAMES: &[&str] = &[
    "Familia",
    "Fútbol los jueves",
    "Trabajo 🏢",
    "Viaje a la playa 🏖️",
    "Cumple sorpresa 🎂",
    "Vecinos",
    "Book club 📚",
    "Padres del cole",
    "Running 🏃",
    "Piso compartido",
    "Erasmus 2019",
    "Cena del viernes",
    "Band practice 🎸",
    "Proyecto final",
    "Primos",
    "Gym bros 💪",
    "Boda 💍",
    "Gaming 🎮",
    "Recetas",
    "Mudanza 📦",
];

const WORDS_ES: &[&str] = &[
    "hola",
    "qué",
    "tal",
    "bien",
    "vale",
    "sí",
    "no",
    "luego",
    "mañana",
    "hoy",
    "ahora",
    "casa",
    "trabajo",
    "comida",
    "cena",
    "café",
    "película",
    "serie",
    "música",
    "canción",
    "playa",
    "montaña",
    "fin",
    "de",
    "semana",
    "nos",
    "vemos",
    "llego",
    "tarde",
    "perdona",
    "gracias",
    "genial",
    "qué",
    "risa",
    "jaja",
    "jajaja",
    "oye",
    "mira",
    "esto",
    "foto",
    "te",
    "quiero",
    "mucho",
    "echo",
    "menos",
    "dónde",
    "estás",
    "cuándo",
    "vienes",
    "llámame",
    "ya",
    "voy",
    "cansado",
    "cansada",
    "feliz",
    "cumpleaños",
    "año",
    "nuevo",
    "perro",
    "gato",
    "tren",
    "avión",
    "lluvia",
    "sol",
    "frío",
    "calor",
    "pizza",
    "tortilla",
    "paella",
    "compras",
    "médico",
    "examen",
    "vacaciones",
    "domingo",
    "lunes",
    "por",
    "favor",
    "claro",
    "también",
    "nunca",
    "siempre",
    "creo",
    "que",
    "sí",
    "pues",
    "bueno",
    "madre",
    "mía",
    "increíble",
    "ánimo",
];

const WORDS_EN: &[&str] = &[
    "hey", "how", "are", "you", "good", "fine", "ok", "okay", "yes", "no", "later", "tomorrow", "today", "now", "home",
    "work", "dinner", "lunch", "coffee", "movie", "show", "song", "beach", "weekend", "see", "you", "soon", "running",
    "late", "sorry", "thanks", "great", "lol", "haha", "look", "at", "this", "photo", "love", "miss", "where", "when",
    "call", "me", "on", "my", "way", "tired", "happy", "birthday", "new", "year", "dog", "cat", "train", "flight",
    "rain", "sun", "cold", "hot", "pizza", "shopping", "doctor", "exam", "holiday", "sunday", "monday", "please",
    "sure", "too", "never", "always", "think", "so", "well", "wow", "amazing", "good", "luck", "night", "morning",
    "the", "a", "and",
];

pub const EMOJI: &[&str] = &[
    "😂",
    "❤️",
    "👍",
    "😍",
    "🙏",
    "😭",
    "🥰",
    "😊",
    "🔥",
    "🎉",
    "😅",
    "🤣",
    "👏",
    "😘",
    "🙈",
    "😎",
    "🤔",
    "😴",
    "💪",
    "✨",
    "🌙",
    "☀️",
    "🍕",
    "🎂",
    "🐶",
    "🐱",
    "⚽",
    "🎶",
    "💯",
    "🙌",
    // Edge cases: skin tones, ZWJ families and flags, keycaps, a variation selector alone.
    "👍🏽",
    "👩‍👩‍👧‍👦",
    "🏳️‍🌈",
    "🇪🇸",
    "🇬🇧",
    "1️⃣",
    "🧑🏿‍🚀",
    "❤️‍🔥",
    "🫠",
    "\u{FE0F}",
];

pub const REACTIONS: &[&str] = &["❤️", "😂", "👍", "😮", "😢", "🙏", "🔥", "🎉", "👍🏽", "🫶"];

pub const PLACES: &[(&str, &str)] = &[
    ("Plaza Inventada", "Calle Falsa 1"),
    ("Café Ejemplo", "Avenida de la Prueba 22"),
    ("Parque Ficticio", "Paseo del Test s/n"),
    ("Estación Demo", "Ronda Muestra 7"),
    ("Sample Diner", "12 Placeholder Road"),
    ("Museo de Nadie", "Camino Cualquiera 3"),
];

pub const DOCUMENTS: &[(&str, &str)] = &[
    ("Factura_2023.pdf", "application/pdf"),
    ("Contrato alquiler.pdf", "application/pdf"),
    ("Notas reunión.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
    ("Presupuesto.xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
    ("billete_tren.pdf", "application/pdf"),
    ("fotos boda.zip", "application/zip"),
    ("CV.pdf", "application/pdf"),
    ("receta abuela.txt", "text/plain"),
    ("Präsentation – final (2).pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
    ("archivo sin extensión", "application/octet-stream"),
];

pub const POLLS: &[(&str, &[&str])] = &[
    ("¿Cena?", &["Pizza", "Sushi", "Tacos"]),
    ("¿Qué día quedamos?", &["Viernes", "Sábado", "Domingo", "Ninguno 😅"]),
    ("Peli del finde", &["Comedia", "Terror", "Documental"]),
    ("Where to go?", &["Beach", "Mountains"]),
];

pub const LINKS: &[(&str, &str, &str)] = &[
    ("https://example.org/receta", "Una receta de ejemplo", "Cómo hacer una tortilla de ejemplo"),
    ("https://example.com/news/123", "Example News", "A synthetic article about nothing"),
    ("https://www.example.net/video?v=abc", "Vídeo de ejemplo", "Un vídeo que no existe"),
    ("https://example.org/a/very/long/path/that/keeps/going/and/going?with=query&and=more#fragment", "", ""),
];

pub const STICKER_PACKS: &[(&str, &str)] =
    &[("Gatitos sintéticos", "Nadie"), ("Synthetic memes", "Nobody"), ("Emojis grandes", "oopz-gen")];

/// Text that has broken chat viewers before: empty-looking, right-to-left, combining marks, markup, very long words,
/// control characters, SQL and HTML that must stay text.
pub const EDGE_TEXTS: &[&str] = &[
    " ",
    "\u{200B}",
    "مرحبا، كيف حالك؟",
    "שלום עולם",
    "Z̤͔ͧ̑̓ä͖̭̈̇lͮ̒ͫǫ̗̻",
    "<script>alert(1)</script>",
    "<b>not bold</b> &amp; &lt;",
    "'; DROP TABLE message; --",
    "line one\nline two\n\nline four",
    "tab\tseparated\tvalues",
    "*negrita* _cursiva_ ~tachado~ ```mono```",
    "@999900000002 mira esto",
    "https://example.org",
    "supercalifragilisticexpialidocious_supercalifragilisticexpialidocious_supercalifragilisticexpialidocious",
    "\u{202E}gnp.exe",
    "日本語のテキスト、絵文字🎌も",
    "Ω≈ç√∫˜µ≤≥÷",
    "null",
    "\\u0000 not a real nul",
    "🙂",
    "😂😂😂😂😂😂😂😂😂😂😂😂😂😂😂😂😂😂😂😂",
];

/// A chat message: a few words in one language, sometimes with an emoji, sometimes a question.
pub fn sentence(rng: &mut Rng, spanish: bool) -> String {
    let words = if spanish { WORDS_ES } else { WORDS_EN };
    let n = 1 + rng.below(4) + if rng.chance(0.3) { rng.below(12) } else { 0 };
    let mut out = String::new();
    for i in 0..n {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(rng.pick::<&str>(words));
    }
    if let Some(first) = out.chars().next().filter(|_| rng.chance(0.5)) {
        let upper: String = first.to_uppercase().collect();
        out.replace_range(..first.len_utf8(), &upper);
    }
    match rng.below(10) {
        0 | 1 => out.push('?'),
        2 => out.push('!'),
        3 => out.push_str("..."),
        _ => {}
    }
    if rng.chance(0.2) {
        out.push(' ');
        out.push_str(rng.pick::<&str>(EMOJI));
    }
    out
}

/// A long message (paragraphs), up to about `chars` characters.
pub fn long_text(rng: &mut Rng, spanish: bool, chars: usize) -> String {
    let mut out = String::new();
    while out.len() < chars {
        out.push_str(&sentence(rng, spanish));
        out.push_str(if rng.chance(0.15) { "\n\n" } else { ". " });
    }
    out
}

pub fn emoji_only(rng: &mut Rng) -> String {
    (0..1 + rng.below(3)).map(|_| *rng.pick(EMOJI)).collect()
}
