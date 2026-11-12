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

/// A count and its noun (`count_chats`, `count_messages`…), worded in [`language`] in the count's form: a value of a
/// line with more than one count, as each count picks its own form.
pub fn count(amount: Message, n: impl std::fmt::Display) -> String {
    msgkinds::Note::of(amount, [n.to_string()]).text(language())
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

    /// Row 2, the review of oopz_cli's Spanish: a participle agrees with its own noun, written in the wording.
    #[test]
    fn agreement_2_a_participle_agrees_with_its_own_noun() {
        assert_eq!(
            spanish(Message::CopiedFolder, &["WhatsApp", "3 archivos", "9 bytes"]),
            "carpeta WhatsApp copiada: \
                    3 archivos, 9 bytes, cada tamaño comprobado"
        );
        assert_eq!(spanish(Message::LinkedInto, &["7"]), "personas unidas en el contacto 7");
        assert_eq!(spanish(Message::SelectionSaved, &["4", "1"]), "selección 4 guardada: 1 mensaje");
    }
}
