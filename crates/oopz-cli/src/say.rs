//! What the command prints, worded in the person's language ([`choose`]): `data/messages.toml` (catalog `oopz_cli`), and
//! oopz's and msgkinds' notes in the same language.

use std::sync::OnceLock;

use msgkinds::language::{LANGUAGE_LIST_VARIABLE, LOCALE_VARIABLES, LanguageError, Sources};

include!(concat!(env!("OUT_DIR"), "/messages.rs"));

/// The variable that names oopz's language, after `--lang` and before the locale.
pub const LANGUAGE_VARIABLE: &str = "OOPZ_LANG";

static LANGUAGE: OnceLock<&'static str> = OnceLock::new();

/// Decides the language once, before any output (docs/messages.md): `--lang`, [`LANGUAGE_VARIABLE`], `LANGUAGE`,
/// `LC_ALL`, `LC_MESSAGES`, `LANG`, the system's setting (Windows, macOS), English. An empty variable is unset; a
/// language oopz lacks, asked for by name, is an error, worded in the language the rest would give.
pub fn choose(flag: Option<&str>) -> Result<&'static str, String> {
    let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
    let (own, list) = (var(LANGUAGE_VARIABLE), var(LANGUAGE_LIST_VARIABLE));
    let locale = LOCALE_VARIABLES.map(|v| std::env::var(v).ok());
    let system = sys_locale::get_locale();
    let asked = [flag, own.as_deref()];
    let mut sources = Sources {
        asked: &asked,
        list: list.as_deref(),
        locale: locale.each_ref().map(Option::as_deref),
        system: system.as_deref(),
    };
    let chosen = CATALOG.language(&sources);
    sources.asked = &[];
    let fallback = CATALOG.language(&sources).unwrap_or(LANGUAGES[0]);
    let language = *LANGUAGE.get_or_init(|| chosen.clone().unwrap_or(fallback));
    let all = LANGUAGES.join(", ");
    match chosen {
        Ok(_) => Ok(language),
        Err(LanguageError::Empty) => Err(msgkinds::Note::of(Message::LanguageEmpty, [all]).text(language)),
        Err(LanguageError::Unknown(given)) => {
            Err(msgkinds::Note::of(Message::LanguageUnknown, [given, all]).text(language))
        }
    }
}

/// The language [`choose`] decided (English before it, as in unit tests): read once per run, so no line is worded in
/// another.
pub fn language() -> &'static str {
    LANGUAGE.get().copied().unwrap_or(LANGUAGES[0])
}

/// What a missing value (a date, a position, a name) shows, in every language.
pub const MISSING: &str = "-";

/// A number with one decimal (`1.5`, a speed in MB/s): `.` in every language, as numbers and dates are plain in the
/// command's output (ISO dates, no grouping) so scripts read them the same whatever the language.
pub fn decimal(x: f64) -> String {
    format!("{x:.1}")
}

/// A count and its noun (`count_chats`, `count_messages`…), worded in [`language`] in the count's form: a value of a
/// line with more than one count, as each count picks its own form.
pub fn count(amount: Message, n: impl std::fmt::Display) -> String {
    msgkinds::Note::of(amount, [n.to_string()]).text(language())
}

/// An error and its causes, `: ` between them (as anyhow's `{:#}`), each of oopz's worded by its note in
/// [`language`] and any other's as it wrote itself (an OS error, a context already worded): docs/messages.md, errors
/// row 4.
pub fn error(e: &anyhow::Error) -> String {
    e.chain().map(cause).collect::<Vec<_>>().join(": ")
}

fn cause(c: &(dyn std::error::Error + 'static)) -> String {
    let note = (c.downcast_ref::<oopz_model::Error>().map(oopz_model::Error::note))
        .or_else(|| c.downcast_ref::<oopz_core::Error>().map(|e| e.note.clone()))
        .or_else(|| c.downcast_ref::<oopz_app::AppError>().map(|e| e.note.clone()));
    note.map_or_else(|| c.to_string(), |n| n.text(language()))
}

/// A message of this catalog with its values, worded in [`language`]: `say!(Saved, path.display())`.
macro_rules! say {
    ($message:ident $(, $value:expr)* $(,)?) => {
        msgkinds::Note::of($crate::say::Message::$message, [$($value.to_string()),*]).text($crate::say::language())
    };
}
pub(crate) use say;

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/messages.md, errors row 4: each cause of the chain once, `: ` between them, an oopz error by its note.
    #[test]
    fn errors_4_each_cause_of_the_chain_once() {
        let reading = oopz_model::Error::reading(std::path::Path::new("/x/a.db"), std::io::Error::other("gone"));
        let e = anyhow::Error::from(reading).context("opening the library");
        assert_eq!(error(&e), "opening the library: reading /x/a.db: gone");
    }

    /// Each count picks its own form in a line with several (`backup_ok`, `imported_source`…).
    #[test]
    fn an_amount_takes_its_counts_form() {
        let word = |m: Message, n: &str, l: &str| msgkinds::Note::of(m, [n.to_owned()]).text(l);
        assert_eq!(word(Message::CountPeople, "1", "en"), "1 person");
        assert_eq!(word(Message::CountPeople, "2", "en"), "2 people");
        assert_eq!(word(Message::CountPeople, "1", "es"), "1 persona");
        assert_eq!(word(Message::CountConversations, "0", "es"), "0 conversaciones");
        assert_eq!(word(Message::CountMessages, "1000000", "es"), "1000000 mensajes");
        let line = msgkinds::Note::new(
            Message::BackupOk,
            ["b", "3", "1 chat", "2 messages", "2026-01-02"].map(Into::into).map(msgkinds::note::Value::Text).to_vec(),
        );
        assert_eq!(line.text("en"), "b  3 MB  ok: 1 chat, 2 messages, last 2026-01-02");
    }

    fn spanish(m: Message, values: &[&str]) -> String {
        msgkinds::Note::of(m, values.iter().map(|v| (*v).to_owned())).text("es")
    }

    /// docs/messages.md, agreement row 1: next to a path or a name (whose noun, and gender, we don't know) the
    /// Spanish takes a form without gender ("se guardó"), never "guardado" or "guardada".
    #[test]
    fn agreement_1_a_value_of_unknown_gender_takes_a_wording_without_one() {
        assert_eq!(spanish(Message::Saved, &["/x/a.html"]), "se guardó: /x/a.html");
        assert_eq!(spanish(Message::SavedBytes, &["a.jpg", "1"]), "se guardó a.jpg (1 byte)");
        assert_eq!(spanish(Message::ReceivedFile, &["a", "2", "ff"]), "se recibió a (2 bytes, sha256 ff)");
    }

    /// docs/messages.md, errors row 1: SQLite's `integrity_check` text is a value, as SQLite wrote it; the command
    /// words around it in its language.
    #[test]
    fn errors_1_sqlites_own_words_are_a_value() {
        let sqlite = "*** in database main ***\nPage 3: never used";
        assert_eq!(spanish(Message::DatabaseProblems, &[sqlite]), format!("base de datos: problemas: {sqlite}"));
        assert_eq!(
            spanish(Message::BackupDamaged, &["b.crypt15", "1", sqlite]),
            format!("b.crypt15  1 MB  DAÑADA: {sqlite}")
        );
    }

    /// Row 2, the review of oopz_cli's Spanish: a participle agrees with its own noun, written in the wording.
    #[test]
    fn agreement_2_a_participle_agrees_with_its_own_noun() {
        assert_eq!(
            spanish(Message::CopiedFolder, &["Viaje", "3 archivos", "9 bytes"]),
            "carpeta Viaje copiada: \
                    3 archivos, 9 bytes, cada tamaño comprobado"
        );
        assert_eq!(spanish(Message::LinkedInto, &["7"]), "personas unidas en el contacto 7");
        assert_eq!(spanish(Message::SelectionSaved, &["4", "1"]), "selección 4 guardada: 1 mensaje");
    }

    /// docs/messages.md, the command's language, row 11: decided once; a later choice doesn't change it.
    #[test]
    fn the_language_is_decided_once_per_run() {
        let first = choose(Some("es")).unwrap();
        assert_eq!(first, "es");
        assert_eq!(choose(Some("en")), Ok("es"));
        assert_eq!(language(), "es");
    }

    /// docs/messages.md, numbers and dates, row 1: a decimal is `1.5` in Spanish too.
    #[test]
    fn numbers_1_a_decimal_is_the_same_in_every_language() {
        assert_eq!(decimal(1.5), "1.5");
        assert_eq!(decimal(12.34), "12.3");
        let line = msgkinds::Note::of(Message::Copying, ["Media".into(), "1".into(), "2".into(), decimal(1.5)]);
        assert_eq!(line.text("es"), "copiando Media: 1 / 2 MB, 1.5 MB/s");
    }
}
