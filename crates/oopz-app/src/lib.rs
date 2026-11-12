//! The oopz desktop app's logic, without a UI toolkit (decision 0009): finding backups in a folder, checking them
//! with their owner's key, merging a lost chat into a new folder, and the Wi-Fi page a phone sends its backup to. The
//! Tauri shell in `apps/desktop` only wraps these; an Android app can wrap the same. Results hold counts, masked ids,
//! file names, chat names and, for the person's own screen, message text; never whole numbers or keys.

use serde::Serialize;

pub mod adapter;
pub mod adb;
pub mod export;
pub mod index;
pub mod receive;
pub mod session;

pub use index::Filter;

/// Every adapter oopz has, in the order detection tries them (backups first, then the text exports). The CLI and the
/// app import through this one list (docs/adapters/README.md).
pub fn adapters() -> Vec<&'static dyn oopz_model::Adapter> {
    vec![
        &adapter::WhatsAppAdapter,
        &oopz_whatsapp_ios::WHATSAPP,
        &oopz_whatsapp_ios::BUSINESS,
        &oopz_whatsapp_export::WhatsAppExportAdapter,
        &oopz_telegram_export::TelegramExportAdapter,
        &oopz_sms_xml::SmsXmlAdapter,
        &oopz_line_export::LineExportAdapter,
        &oopz_viber_export::ViberExportAdapter,
    ]
}
pub use receive::{ReceiveInfo, ReceiveStatus, Receiver};
pub use session::{BackupFile, ChatRow, MergeOutcome, MergeRequest, Missing, Session, Summary};

/// Every sentence the app's logic and its shell write (`data/messages.toml`, catalog `oopz_app`): `Message`, worded
/// through msgkinds' `Note`.
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

use messages::Message as Msg;
use msgkinds::Note;
use msgkinds::note::Value;
use oopz_model::messages::Message as ModelMsg;

/// An error for the UI: a stable `code` it words in the person's language, and a note (`note.text(language)`) of the
/// `oopz_app` catalog or another's. Never holds a key or message text. `Display` is the note's English, and so is
/// `detail` when it's serialized (`{code, detail}`, the desktop app's wire until notes travel as notes).
#[derive(Debug, Clone)]
pub struct AppError {
    /// `key_format`, `wrong_key`, `not_crypt15`, `crypt14`, `damaged`, `unsupported`, `merge_check`, `changed`,
    /// `exists`, `io`, `not_found`, `network`, `no_space`, `update`, `cancelled`.
    pub code: &'static str,
    pub note: Note,
}

impl AppError {
    pub fn new(code: &'static str, note: Note) -> Self {
        Self { code, note }
    }

    /// A message of the `oopz_app` catalog with text values.
    pub fn of(code: &'static str, message: Msg, values: impl IntoIterator<Item = String>) -> Self {
        Self::new(code, Note::of(message, values))
    }

    /// An `io` error: what was being done (a note), and the system's reason, as the system wrote it.
    pub fn io(doing: Note, e: std::io::Error) -> Self {
        Self::doing("io", doing, e)
    }

    /// An `io` error doing something to `path` (oopz's `doing_reading`, `doing_making`, `doing_writing`…).
    pub fn io_at(doing: ModelMsg, path: &std::path::Path, e: std::io::Error) -> Self {
        Self::io(Note::of(doing, [path.display().to_string()]), e)
    }

    /// What was being done (a note) and the reason it failed, in the system's or a library's own words.
    pub fn doing(code: &'static str, doing: Note, reason: impl std::fmt::Display) -> Self {
        let values = vec![Value::Note(Box::new(doing)), Value::Text(reason.to_string())];
        Self::new(code, Note::new(ModelMsg::ErrorIo, values))
    }

    /// Stopped, as asked.
    pub fn cancelled() -> Self {
        Self::new("cancelled", Note::of(ModelMsg::ErrorCancelled, []))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.note.fmt(f)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut out = s.serialize_struct("AppError", 2)?;
        out.serialize_field("code", self.code)?;
        out.serialize_field("detail", &self.note.to_string())?;
        out.end()
    }
}

impl From<oopz::Error> for AppError {
    fn from(e: oopz::Error) -> Self {
        use oopz::crypt15::Error as C;
        let code = match &e {
            oopz::Error::Crypt15(C::BadKeyFormat) => "key_format",
            oopz::Error::Crypt15(C::WrongKeyOrDamaged) => "wrong_key",
            oopz::Error::Crypt15(C::NotCrypt15(ModelMsg::NotCrypt15NoIv)) => "crypt14",
            oopz::Error::Crypt15(C::NotCrypt15(_)) => "not_crypt15",
            oopz::Error::Crypt15(C::BadContents(_)) | oopz::Error::Sqlite(_) => "damaged",
            oopz::Error::Unsupported(_) => "unsupported",
            oopz::Error::MergeCheck(_) => "merge_check",
            oopz::Error::Cancelled | oopz::Error::Crypt15(C::Cancelled) => "cancelled",
            oopz::Error::Io(..) | oopz::Error::Crypt15(C::NoRandomness | C::Write(_)) => "io",
        };
        Self::new(code, e.note())
    }
}

impl From<oopz::crypt15::Error> for AppError {
    fn from(e: oopz::crypt15::Error) -> Self {
        oopz::Error::from(e).into()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use oopz::crypt15::Error as C;

    /// docs/messages.md, errors row 6: each of the `oopz` crate's errors keeps its code (stored names the UI words),
    /// and its note.
    #[test]
    fn errors_6_the_backup_errors_keep_their_codes() {
        let io = || std::io::Error::other("x");
        let note = |m: ModelMsg| Note::of(m, []);
        let sql = rusqlite::Connection::open_in_memory().unwrap().execute("NOT SQL", []).unwrap_err();
        let codes: [(oopz::Error, &str); 13] = [
            (C::BadKeyFormat.into(), "key_format"),
            (C::WrongKeyOrDamaged.into(), "wrong_key"),
            (C::NotCrypt15(ModelMsg::NotCrypt15NoIv).into(), "crypt14"),
            (C::NotCrypt15(ModelMsg::NotCrypt15Empty).into(), "not_crypt15"),
            (C::BadContents(ModelMsg::BadContentsNoSqlite).into(), "damaged"),
            (sql.into(), "damaged"),
            (oopz::Error::Unsupported(note(ModelMsg::MergeOneToOne)), "unsupported"),
            (oopz::Error::MergeCheck(note(ModelMsg::CheckSchema)), "merge_check"),
            (oopz::Error::Cancelled, "cancelled"),
            (C::Cancelled.into(), "cancelled"),
            (oopz::Error::Io(note(ModelMsg::DoingMediaFolder), io()), "io"),
            (C::NoRandomness.into(), "io"),
            (C::Write(io()).into(), "io"),
        ];
        for (e, code) in codes {
            let words = e.note();
            let app = AppError::from(e);
            assert_eq!((app.code, app.note), (code, words));
        }
    }

    /// No error has a `source()`: each note already holds the inner error's words, so a chain (the CLI's) never says
    /// the system's reason twice.
    #[test]
    fn errors_4_no_error_repeats_its_cause() {
        use std::error::Error as _;
        let io = || std::io::Error::other("x");
        let doing = || Note::of(ModelMsg::DoingFreeSpace, []);
        assert!(oopz_model::Error::Io(doing(), io()).source().is_none());
        assert!(oopz::Error::Io(doing(), io()).source().is_none());
        assert!(oopz::Error::Crypt15(C::Write(io())).source().is_none());
        assert!(AppError::io(doing(), io()).source().is_none());
    }
}
