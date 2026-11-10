//! The oopz desktop app's logic, without a UI toolkit (decision 0009): finding backups in a folder, checking them
//! with their owner's key, merging a lost chat into a new folder, and the Wi-Fi page a phone sends its backup to. The
//! Tauri shell in `apps/desktop` only wraps these; an Android app can wrap the same. Results hold counts, masked ids,
//! file names, chat names and, for the person's own screen, message text; never whole numbers or keys.

use serde::Serialize;

pub mod export;
pub mod index;
pub mod receive;
pub mod session;

pub use index::Filter;
pub use receive::{ReceiveInfo, ReceiveStatus, Receiver};
pub use session::{BackupFile, ChatRow, MergeOutcome, MergeRequest, Missing, Session, Summary};

/// An error for the UI: a stable `code` it words in the person's language, and an English detail. Never holds a key.
#[derive(Debug, Serialize, thiserror::Error)]
#[error("{detail}")]
pub struct AppError {
    /// `key_format`, `wrong_key`, `not_crypt15`, `crypt14`, `damaged`, `unsupported`, `merge_check`, `changed`,
    /// `exists`, `io`, `not_found`, `network`, `no_space`, `update`, `cancelled`.
    pub code: &'static str,
    pub detail: String,
}

impl AppError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self { code, detail: detail.into() }
    }

    /// An `io` error: what was being done, and the system's reason (never a key or message text).
    pub fn io(context: impl std::fmt::Display, e: std::io::Error) -> Self {
        Self::new("io", format!("{context}: {e}"))
    }
}

impl From<oopz::Error> for AppError {
    fn from(e: oopz::Error) -> Self {
        use oopz::crypt15::Error as C;
        let code = match &e {
            oopz::Error::Crypt15(C::BadKeyFormat) => "key_format",
            oopz::Error::Crypt15(C::WrongKeyOrDamaged) => "wrong_key",
            oopz::Error::Crypt15(C::NotCrypt15(why)) if why.contains("crypt14") => "crypt14",
            oopz::Error::Crypt15(C::NotCrypt15(_)) => "not_crypt15",
            oopz::Error::Crypt15(C::BadContents(_)) | oopz::Error::Sqlite(_) => "damaged",
            oopz::Error::Unsupported(_) => "unsupported",
            oopz::Error::MergeCheck(_) => "merge_check",
            oopz::Error::Cancelled | oopz::Error::Crypt15(C::Cancelled) => "cancelled",
            _ => "io",
        };
        Self::new(code, e.to_string())
    }
}

impl From<oopz::crypt15::Error> for AppError {
    fn from(e: oopz::crypt15::Error) -> Self {
        oopz::Error::from(e).into()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
