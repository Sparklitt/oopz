//! What the command prints, worded in the terminal's language: `data/messages.toml` (catalog `oopz_cli`), and
//! oopz's and msgkinds' notes in the same language.

use std::sync::OnceLock;

use msgkinds::note::LOCALE_VARIABLES;

include!(concat!(env!("OUT_DIR"), "/messages.rs"));

/// The terminal's language (msgkinds' `Catalog::locale_language`), read once.
pub fn language() -> &'static str {
    static LANGUAGE: OnceLock<&'static str> = OnceLock::new();
    LANGUAGE.get_or_init(|| {
        let settings = LOCALE_VARIABLES.map(|v| std::env::var(v).ok());
        CATALOG.locale_language(&settings.each_ref().map(Option::as_deref))
    })
}

/// A message of this catalog with its values, worded in [`language`]: `say!(Saved, path.display())`.
macro_rules! say {
    ($message:ident $(, $value:expr)* $(,)?) => {
        msgkinds::Note::of($crate::say::Message::$message, [$($value.to_string()),*]).text($crate::say::language())
    };
}
pub(crate) use say;
