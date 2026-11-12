//! The phone's receive page and the computer's answers to it, worded from the `oopz_app` catalog (docs/messages.md,
//! "The receive page"): the page is served in the phone's language with its words embedded, and a refused file is
//! answered with a stable code the page words. The page keeps no words of its own.

use msgkinds::Note;

use crate::messages::{CATALOG, LANGUAGES, Message as Msg};

/// The page, with [`LANGUAGE_SLOT`] and [`WORDS_SLOT`] to fill.
const PAGE: &str = include_str!("../receive_page.html");

/// Where the page's language goes (`<html lang>`), as a BCP 47 tag.
const LANGUAGE_SLOT: &str = "{{language}}";

/// Where the page's words go: a JSON object, message name → wording, inside a `<script type="application/json">`.
const WORDS_SLOT: &str = "{{words}}";

/// The messages the page is sent: its own words and every refusal's (by these name prefixes).
const PAGE_PREFIXES: [&str; 2] = ["receive_page_", "receive_refused_"];

/// Why the computer refused a file. The answer is the HTTP status and, as its body, the message's name: a stable code
/// the page words in the phone's language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    Name,
    SizeNeeded,
    Exists,
    Busy,
    Stopped,
    Disk,
    Write,
    CutOff,
    NotFound,
}

impl Refusal {
    #[cfg(test)]
    pub(super) const ALL: [Refusal; 9] = [
        Self::Name,
        Self::SizeNeeded,
        Self::Exists,
        Self::Busy,
        Self::Stopped,
        Self::Disk,
        Self::Write,
        Self::CutOff,
        Self::NotFound,
    ];

    /// The HTTP status: unchanged from before the answers were codes.
    pub(super) fn status(self) -> u16 {
        match self {
            Self::Name | Self::CutOff => 400,
            Self::NotFound => 404,
            Self::Exists | Self::Busy => 409,
            Self::SizeNeeded => 411,
            Self::Stopped => 503,
            Self::Disk | Self::Write => 507,
        }
    }

    pub(super) fn message(self) -> Msg {
        match self {
            Self::Name => Msg::ReceiveRefusedName,
            Self::SizeNeeded => Msg::ReceiveRefusedSizeNeeded,
            Self::Exists => Msg::ReceiveRefusedExists,
            Self::Busy => Msg::ReceiveRefusedBusy,
            Self::Stopped => Msg::ReceiveRefusedStopped,
            Self::Disk => Msg::ReceiveRefusedDisk,
            Self::Write => Msg::ReceiveRefusedWrite,
            Self::CutOff => Msg::ReceiveRefusedCutOff,
            Self::NotFound => Msg::ReceiveRefusedNotFound,
        }
    }

    /// The answer's body: the message's name, never a file's name or anything the phone sent.
    pub(super) fn code(self) -> &'static str {
        self.message().as_str()
    }
}

/// The catalog's language for a phone's `Accept-Language` values (`es-419,es;q=0.9,en;q=0.8`): the first tag the
/// catalog has, read by msgkinds' rules (`Catalog::find`: `es-MX` → `es`), in the order the phone gives them (browsers
/// list them by preference; weights aren't read); none, or none the catalog has, gives the default.
pub(super) fn language<'a>(accept: impl IntoIterator<Item = &'a str>) -> &'static str {
    accept
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(|tag| tag.split(';').next().unwrap_or_default().trim())
        .filter(|tag| !tag.is_empty() && *tag != "*")
        .find_map(|tag| CATALOG.find(tag))
        .unwrap_or(LANGUAGES[0])
}

/// The page in `language`, its words embedded.
pub(super) fn page(language: &str) -> String {
    let words = sent()
        .map(|m| format!("{}:{}", json_string(m.as_str()), json_string(&Note::of(m, []).text(language))))
        .collect::<Vec<_>>()
        .join(",");
    PAGE.replace(LANGUAGE_SLOT, &language.replace('_', "-")).replace(WORDS_SLOT, &format!("{{{words}}}"))
}

/// The messages the page is sent, in the catalog's order.
pub(super) fn sent() -> impl Iterator<Item = Msg> {
    Msg::ALL.into_iter().filter(|m| PAGE_PREFIXES.iter().any(|p| m.as_str().starts_with(p)))
}

/// A JSON string that is safe inside a `<script>`: `<`, `>` and `&` as `\u` escapes (no `</script>` or `<!--` can
/// close it), U+2028 and U+2029 too (older JavaScript ends a line at them), control characters, quotes and
/// backslashes escaped.
pub(super) fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' | '>' | '&' | '\u{2028}' | '\u{2029}' => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
