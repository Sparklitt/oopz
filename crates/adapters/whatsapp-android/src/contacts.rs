//! WhatsApp's contacts backup (`wa.db.crypt15`, beside `msgstore.db.crypt15`), opened with the same key as the
//! messages (decision 0034, point 6): only the name of a jid asked for is ever read. Its shape is not yet checked on a
//! real backup, so it's guarded: a file that doesn't open with the key, or has no `wa_contacts (jid, display_name,
//! wa_name)`, gives no names, never an error.

use std::fs;
use std::io::Write;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::crypt15::{self, Key};

/// An opened contacts file, its decrypted copy removed when dropped.
pub struct Contacts {
    db: Connection,
    _plain: tempfile::NamedTempFile,
}

/// Why there are no names: for a report's counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoContacts {
    /// No `wa.db.crypt15` beside the backup.
    Missing,
    /// It doesn't open with this key (or is damaged): never forced.
    WrongKey,
    /// It opened, but not in the shape we read.
    UnknownShape,
}

impl Contacts {
    /// The contacts file beside `msgstore` (the backup file), opened with `key` into a copy in `dir`: `wa.db.crypt15`,
    /// else another `.crypt15` file there that opens as contacts (an app's picker may give files neutral names).
    pub fn beside(msgstore: &Path, key: &Key, dir: &Path) -> Result<Self, NoContacts> {
        let named = msgstore.with_file_name("wa.db.crypt15");
        let first = Self::open(&named, key, dir);
        if !matches!(first, Err(NoContacts::Missing)) {
            return first;
        }
        let Some(folder) = msgstore.parent() else { return first };
        let mut others: Vec<_> =
            fs::read_dir(folder).map(|entries| entries.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        others.retain(|p| p != msgstore && p.extension().is_some_and(|e| e == "crypt15") && p.is_file());
        others.sort();
        // A few at most: a Databases folder holds dated message backups too, and those aren't contacts.
        others.iter().take(8).find_map(|p| Self::open(p, key, dir).ok()).ok_or(NoContacts::Missing)
    }

    fn open(path: &Path, key: &Key, dir: &Path) -> Result<Self, NoContacts> {
        let file = fs::read(path).map_err(|_| NoContacts::Missing)?;
        let mut plain = tempfile::NamedTempFile::new_in(dir).map_err(|_| NoContacts::Missing)?;
        {
            let mut out = std::io::BufWriter::new(plain.as_file_mut());
            crypt15::decrypt_to(file, key, &mut out).map_err(|_| NoContacts::WrongKey)?;
            out.flush().map_err(|_| NoContacts::WrongKey)?;
        }
        let db = Connection::open_with_flags(plain.path(), OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| NoContacts::UnknownShape)?;
        let shaped = crate::schema::Schema::has_columns(&db, "wa_contacts", &["jid", "display_name", "wa_name"])
            .unwrap_or(false);
        if !shaped {
            return Err(NoContacts::UnknownShape);
        }
        Ok(Self { db, _plain: plain })
    }

    /// The name of `jid`: the address book's (`display_name`), else the one they set (`wa_name`), shown with a `~`
    /// as WhatsApp does. Nothing else of the row is read.
    pub fn name(&self, jid: &str) -> crate::Result<Option<String>> {
        let row: Option<(Option<String>, Option<String>)> = self
            .db
            .prepare_cached(
                "SELECT display_name, wa_name FROM wa_contacts WHERE jid = ?1
                 ORDER BY display_name IS NULL, _rowid_ LIMIT 1",
            )?
            .query_row([jid], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let nonempty = |s: Option<String>| s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        Ok(row.and_then(|(display, own)| nonempty(display).or_else(|| nonempty(own).map(|n| format!("~{n}")))))
    }
}
