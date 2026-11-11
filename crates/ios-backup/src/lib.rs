//! An iPhone's own backup, made by Finder or iTunes on the owner's computer (decision 0018). A backup is a folder:
//! `Manifest.plist` (the device, whether it's encrypted, the keybag), `Manifest.db` (every file's domain, path and,
//! when encrypted, its wrapped key) and the files themselves under `<first two hex digits>/<file id>`.
//!
//! An encrypted backup opens only with the backup password its owner chose: the keybag's class keys are unwrapped
//! with a key derived from it (PBKDF2-SHA256 then PBKDF2-SHA1, AES key wrap), and each file is AES-256-CBC with its own
//! key, wrapped by its class key. Nothing here reads a device, logs in anywhere or guesses a password.
//!
//! Only reading: the backup's folder is never written to. The decrypted manifest goes to a private temporary folder
//! removed when the [`Backup`] is dropped; files are decrypted only into what the caller hands [`Backup::copy`].

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use aes::cipher::{BlockModeDecrypt, KeyInit, KeyIvInit};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use zeroize::Zeroizing;

#[cfg(feature = "write")]
pub mod write;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not an iPhone backup folder (it needs Manifest.plist and Manifest.db)")]
    NotABackup,
    #[error("this backup is from iOS 9 or older (Manifest.mbdb), which oopz doesn't read")]
    TooOld,
    #[error("this backup is encrypted: it opens with the backup password chosen on the computer that made it")]
    NeedsPassword,
    #[error("that isn't this backup's password")]
    WrongPassword,
    #[error("the backup is damaged: {0}")]
    Damaged(String),
    #[error("{0}: {1}")]
    Io(String, io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

fn damaged(what: impl Into<String>) -> Error {
    Error::Damaged(what.into())
}

/// What `Manifest.plist` says about the backup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    pub encrypted: bool,
    pub device_name: Option<String>,
    pub product_version: Option<String>,
    /// When the backup was made (Unix ms).
    pub date_ms: Option<i64>,
    /// The apps backed up, by bundle id (`net.whatsapp.WhatsApp`), with the version each was at
    /// (`Applications` → `CFBundleVersion`, else `CFBundleShortVersionString`).
    pub apps: std::collections::BTreeMap<String, String>,
}

/// One file in the backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub file_id: String,
    pub domain: String,
    pub relative_path: String,
    /// The plain file's size, when the manifest gives it.
    pub size: Option<u64>,
    protection: Option<u32>,
    /// The file's key, wrapped by its class key (encrypted backups).
    wrapped_key: Option<Vec<u8>>,
}

/// An opened backup.
pub struct Backup {
    dir: PathBuf,
    pub info: Info,
    db: Connection,
    /// Class keys by protection class (encrypted backups).
    class_keys: HashMap<u32, Zeroizing<Vec<u8>>>,
    /// Holds the decrypted manifest; declared after `db`, so dropped after it (Windows can't remove an open file).
    _tmp: Option<tempfile::TempDir>,
}

/// The backup folder `path` names: the folder itself, or the folder of its `Manifest.plist` / `Manifest.db`.
pub fn backup_dir(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_dir() { path.to_path_buf() } else { path.parent()?.to_path_buf() };
    let is = |d: &Path| {
        d.join("Manifest.plist").is_file() && (d.join("Manifest.db").is_file() || d.join("Manifest.mbdb").is_file())
    };
    is(&dir).then_some(dir)
}

fn read_plist(path: &Path) -> Result<plist::Value> {
    plist::Value::from_file(path)
        .map_err(|e| damaged(format!("{}: {e}", path.file_name().unwrap_or_default().to_string_lossy())))
}

/// `Manifest.plist`'s facts, without a password.
pub fn info(dir: &Path) -> Result<Info> {
    let dir = backup_dir(dir).ok_or(Error::NotABackup)?;
    let manifest = read_plist(&dir.join("Manifest.plist"))?;
    let m = manifest.as_dictionary().ok_or_else(|| damaged("Manifest.plist isn't a dictionary"))?;
    let lockdown = m.get("Lockdown").and_then(|l| l.as_dictionary());
    let text = |key: &str| lockdown.and_then(|l| l.get(key)).and_then(|v| v.as_string()).map(str::to_owned);
    let date_ms = m.get("Date").and_then(|d| d.as_date()).map(|d| {
        let t: std::time::SystemTime = d.into();
        match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(after) => after.as_millis() as i64,
            Err(before) => -(before.duration().as_millis() as i64),
        }
    });
    Ok(Info {
        encrypted: m.get("IsEncrypted").and_then(|v| v.as_boolean()).unwrap_or(false),
        device_name: text("DeviceName"),
        product_version: text("ProductVersion"),
        date_ms,
        apps: m
            .get("Applications")
            .and_then(|a| a.as_dictionary())
            .into_iter()
            .flatten()
            .filter_map(|(id, app)| {
                let app = app.as_dictionary()?;
                let v = ["CFBundleVersion", "CFBundleShortVersionString"]
                    .into_iter()
                    .find_map(|k| app.get(k).and_then(|v| v.as_string()))?;
                Some((id.clone(), v.to_owned()))
            })
            .collect(),
    })
}

/// The keybag's tag-length-value records.
fn tlv(data: &[u8]) -> Result<Vec<([u8; 4], &[u8])>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let head = data.get(at..at + 8).ok_or_else(|| damaged("the keybag ends inside a record"))?;
        let tag: [u8; 4] = head[..4].try_into().expect("4 bytes");
        let len = u32::from_be_bytes(head[4..8].try_into().expect("4 bytes")) as usize;
        let value = data.get(at + 8..at + 8 + len).ok_or_else(|| damaged("a keybag record runs past its end"))?;
        out.push((tag, value));
        at += 8 + len;
    }
    Ok(out)
}

fn be_u32(v: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(v.try_into().ok()?))
}

struct WrappedClass {
    class: u32,
    wrap: u32,
    key: Vec<u8>,
}

/// The class keys of `keybag`, unwrapped with `password`.
fn unlock(keybag: &[u8], password: &str) -> Result<HashMap<u32, Zeroizing<Vec<u8>>>> {
    let mut attrs: HashMap<[u8; 4], &[u8]> = HashMap::new();
    let mut classes: Vec<WrappedClass> = Vec::new();
    let mut seen_uuid = false;
    for (tag, value) in tlv(keybag)? {
        match &tag {
            b"UUID" if !seen_uuid => seen_uuid = true,
            b"UUID" => classes.push(WrappedClass { class: 0, wrap: 0, key: Vec::new() }),
            b"CLAS" | b"WRAP" | b"WPKY" | b"KTYP" | b"PBKY" if !classes.is_empty() => {
                let c = classes.last_mut().expect("a class");
                match &tag {
                    b"CLAS" => c.class = be_u32(value).ok_or_else(|| damaged("a class number"))?,
                    b"WRAP" => c.wrap = be_u32(value).ok_or_else(|| damaged("a wrap type"))?,
                    b"WPKY" => c.key = value.to_vec(),
                    _ => {}
                }
            }
            _ => {
                attrs.insert(tag, value);
            }
        }
    }
    let salt = attrs.get(b"SALT").ok_or_else(|| damaged("the keybag has no salt"))?;
    let iterations = attrs.get(b"ITER").and_then(|v| be_u32(v)).ok_or_else(|| damaged("the keybag has no count"))?;
    let mut key = Zeroizing::new([0u8; 32]);
    match (attrs.get(b"DPSL"), attrs.get(b"DPIC").and_then(|v| be_u32(v))) {
        // iOS 10.2 and later: a slow SHA-256 round first.
        (Some(dpsl), Some(dpic)) => {
            let mut first = Zeroizing::new([0u8; 32]);
            pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), dpsl, dpic, first.as_mut());
            pbkdf2::pbkdf2_hmac::<sha1::Sha1>(first.as_ref(), salt, iterations, key.as_mut());
        }
        _ => pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password.as_bytes(), salt, iterations, key.as_mut()),
    }
    let kw = aes_kw::KwAes256::new_from_slice(key.as_ref()).expect("32 bytes");
    let mut out = HashMap::new();
    // Bit 2 of WRAP: wrapped with the password's key (bit 1, the device's key, never appears in a backup's keybag).
    for c in classes.iter().filter(|c| c.wrap & 2 != 0 && !c.key.is_empty()) {
        let mut buf = Zeroizing::new(vec![0u8; c.key.len()]);
        let plain = kw.unwrap_key(&c.key, &mut buf).map_err(|_| Error::WrongPassword)?;
        out.insert(c.class, Zeroizing::new(plain.to_vec()));
    }
    if out.is_empty() {
        return Err(damaged("the keybag holds no keys the password opens"));
    }
    Ok(out)
}

fn unwrap_with(
    class_keys: &HashMap<u32, Zeroizing<Vec<u8>>>,
    class: u32,
    wrapped: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let kek = class_keys.get(&class).ok_or_else(|| damaged(format!("no key for protection class {class}")))?;
    let kw = aes_kw::KwAes256::new_from_slice(kek).map_err(|_| damaged("a class key isn't 32 bytes"))?;
    let mut buf = Zeroizing::new(vec![0u8; wrapped.len()]);
    let plain = kw.unwrap_key(wrapped, &mut buf).map_err(|_| damaged("a file key doesn't unwrap"))?;
    Ok(Zeroizing::new(plain.to_vec()))
}

type Cbc = cbc::Decryptor<aes::Aes256>;

/// Decrypts AES-256-CBC (zero IV) from `input` into `out`: `size` bytes when known, else the PKCS#7 padding removed.
fn decrypt_stream(key: &[u8], input: &mut dyn Read, size: Option<u64>, out: &mut dyn Write) -> Result<u64> {
    let mut dec = Cbc::new_from_slices(key, &[0u8; 16]).map_err(|_| damaged("a file key isn't 32 bytes"))?;
    let mut buf = vec![0u8; 1 << 16];
    let mut held: Option<[u8; 16]> = None;
    let mut written = 0u64;
    let limit = size.unwrap_or(u64::MAX);
    let io = |e| Error::Io("reading the backup".into(), e);
    loop {
        let mut filled = 0;
        while filled < buf.len() {
            let n = input.read(&mut buf[filled..]).map_err(io)?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled % 16 != 0 {
            return Err(damaged("an encrypted file's length isn't whole blocks"));
        }
        if filled == 0 {
            break;
        }
        let (blocks, _) = buf[..filled].as_chunks_mut::<16>();
        for chunk in blocks {
            let block: &mut aes::Block = chunk.as_mut_slice().try_into().expect("16 bytes");
            dec.decrypt_block(block);
            if let Some(prev) = held.replace(block.as_slice().try_into().expect("16 bytes")) {
                let take = (limit - written).min(16) as usize;
                out.write_all(&prev[..take]).map_err(|e| Error::Io("writing".into(), e))?;
                written += take as u64;
            }
        }
    }
    if let Some(last) = held {
        let keep = match size {
            Some(_) => (limit - written).min(16) as usize,
            None => {
                let pad = last[15] as usize;
                if pad == 0 || pad > 16 || last[16 - pad..].iter().any(|&b| b as usize != pad) {
                    return Err(damaged("an encrypted file's padding is wrong"));
                }
                16 - pad
            }
        };
        out.write_all(&last[..keep]).map_err(|e| Error::Io("writing".into(), e))?;
        written += keep as u64;
    }
    Ok(written)
}

/// A file record's size, protection class and wrapped key.
type FileFacts = (Option<u64>, Option<u32>, Option<Vec<u8>>);

/// The facts an `MBFile` archive (NSKeyedArchiver) in `Manifest.db`'s `file` column holds: size, protection class,
/// wrapped key.
fn mbfile(blob: &[u8]) -> Result<FileFacts> {
    let v = plist::Value::from_reader(io::Cursor::new(blob)).map_err(|e| damaged(format!("a file record: {e}")))?;
    let objects = v
        .as_dictionary()
        .and_then(|d| d.get("$objects"))
        .and_then(|o| o.as_array())
        .ok_or_else(|| damaged("a file record without objects"))?;
    let top = v
        .as_dictionary()
        .and_then(|d| d.get("$top"))
        .and_then(|t| t.as_dictionary())
        .and_then(|t| t.get("root"))
        .and_then(|r| r.as_uid())
        .map_or(1, |u| u.get() as usize);
    let root =
        objects.get(top).and_then(|o| o.as_dictionary()).ok_or_else(|| damaged("a file record without a root"))?;
    let size = root.get("Size").and_then(|s| s.as_unsigned_integer());
    let class = root.get("ProtectionClass").and_then(|c| c.as_unsigned_integer()).map(|c| c as u32);
    let key = root.get("EncryptionKey").and_then(|k| match k {
        plist::Value::Uid(u) => objects.get(u.get() as usize),
        other => Some(other),
    });
    let key = key.and_then(|k| match k {
        plist::Value::Data(d) => Some(d.clone()),
        plist::Value::Dictionary(d) => d.get("NS.data").and_then(|x| x.as_data()).map(<[u8]>::to_vec),
        _ => None,
    });
    // The first four bytes repeat the class (little-endian); the wrapped key follows.
    let key = key.map(|k| if k.len() == 44 { k[4..].to_vec() } else { k });
    Ok((size, class, key))
}

impl Backup {
    /// Opens the backup at `path` (its folder, or a file in it). An encrypted one needs its password; an unencrypted
    /// one ignores it.
    pub fn open(path: &Path, password: Option<&str>) -> Result<Backup> {
        let dir = backup_dir(path).ok_or(Error::NotABackup)?;
        if !dir.join("Manifest.db").is_file() {
            return Err(Error::TooOld);
        }
        let info = info(&dir)?;
        let (db_path, class_keys, tmp) = if info.encrypted {
            let password = password.filter(|p| !p.is_empty()).ok_or(Error::NeedsPassword)?;
            let manifest = read_plist(&dir.join("Manifest.plist"))?;
            let m = manifest.as_dictionary().ok_or_else(|| damaged("Manifest.plist isn't a dictionary"))?;
            let keybag = m.get("BackupKeyBag").and_then(|k| k.as_data()).ok_or_else(|| damaged("no keybag"))?;
            let class_keys = unlock(keybag, password)?;
            let wrapped = m.get("ManifestKey").and_then(|k| k.as_data()).ok_or_else(|| damaged("no manifest key"))?;
            if wrapped.len() < 8 {
                return Err(damaged("the manifest key is too short"));
            }
            let class = u32::from_le_bytes(wrapped[..4].try_into().expect("4 bytes"));
            let key = unwrap_with(&class_keys, class, &wrapped[4..])?;
            let tmp = tempfile::Builder::new()
                .prefix("oopz-ios-")
                .tempdir()
                .map_err(|e| Error::Io("a private temporary folder".into(), e))?;
            let plain = tmp.path().join("Manifest.db");
            let mut input = File::open(dir.join("Manifest.db")).map_err(|e| Error::Io("Manifest.db".into(), e))?;
            let mut out = File::create(&plain).map_err(|e| Error::Io("a decrypted copy".into(), e))?;
            decrypt_stream(&key, &mut input, None, &mut out)?;
            (plain, class_keys, Some(tmp))
        } else {
            (dir.join("Manifest.db"), HashMap::new(), None)
        };
        let db =
            Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
                .map_err(|e| damaged(format!("Manifest.db: {e}")))?;
        db.query_row("SELECT count(*) FROM Files", [], |r| r.get::<_, i64>(0))
            .map_err(|e| damaged(format!("Manifest.db: {e}")))?;
        Ok(Backup { dir, info, db, class_keys, _tmp: tmp })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn entry(&self, r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
        let blob: Option<Vec<u8>> = r.get(3)?;
        let (size, protection, wrapped_key) = match blob.as_deref().map(mbfile) {
            Some(Ok(facts)) => facts,
            _ => (None, None, None),
        };
        Ok(Entry { file_id: r.get(0)?, domain: r.get(1)?, relative_path: r.get(2)?, size, protection, wrapped_key })
    }

    /// The file at `relative_path` in `domain`.
    pub fn file(&self, domain: &str, relative_path: &str) -> Result<Option<Entry>> {
        self.db
            .query_row(
                "SELECT fileID, domain, relativePath, file FROM Files WHERE domain = ?1 AND relativePath = ?2 AND flags = 1",
                [domain, relative_path],
                |r| self.entry(r),
            )
            .optional()
            .map_err(|e| damaged(format!("Manifest.db: {e}")))
    }

    /// Every file in `domain` whose path starts with `prefix`.
    pub fn files(&self, domain: &str, prefix: &str) -> Result<Vec<Entry>> {
        let like = format!("{}%", prefix.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = self
            .db
            .prepare(
                "SELECT fileID, domain, relativePath, file FROM Files
                 WHERE domain = ?1 AND relativePath LIKE ?2 ESCAPE '\\' AND flags = 1 ORDER BY relativePath",
            )
            .map_err(|e| damaged(format!("Manifest.db: {e}")))?;
        let rows = stmt.query_map([domain, like.as_str()], |r| self.entry(r)).map_err(|e| damaged(e.to_string()))?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| damaged(e.to_string()))
    }

    /// The domains whose name starts with `prefix` (an app's group container is named by its group id).
    pub fn domains(&self, prefix: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .db
            .prepare("SELECT DISTINCT domain FROM Files WHERE substr(domain, 1, length(?1)) = ?1 ORDER BY domain")
            .map_err(|e| damaged(e.to_string()))?;
        let rows = stmt.query_map([prefix], |r| r.get(0)).map_err(|e| damaged(e.to_string()))?;
        rows.collect::<rusqlite::Result<_>>().map_err(|e| damaged(e.to_string()))
    }

    /// Where the file's (encrypted or plain) bytes are in the backup folder.
    fn stored(&self, entry: &Entry) -> PathBuf {
        let prefix = entry.file_id.get(..2).unwrap_or("");
        let nested = self.dir.join(prefix).join(&entry.file_id);
        // Very old layouts keep every file at the top.
        if nested.is_file() { nested } else { self.dir.join(&entry.file_id) }
    }

    /// Whether the file's bytes are in the backup (a backup stopped halfway can lack some).
    pub fn has(&self, entry: &Entry) -> bool {
        self.stored(entry).is_file()
    }

    /// Writes the file's plain bytes to `out`; returns how many.
    pub fn copy(&self, entry: &Entry, out: &mut dyn Write) -> Result<u64> {
        let path = self.stored(entry);
        let mut input =
            File::open(&path).map_err(|e| Error::Io(format!("{} in the backup", entry.relative_path), e))?;
        if !self.info.encrypted {
            return io::copy(&mut input, out).map_err(|e| Error::Io("copying".into(), e));
        }
        let (Some(class), Some(wrapped)) = (entry.protection, entry.wrapped_key.as_deref()) else {
            return Err(damaged(format!("{} has no key", entry.relative_path)));
        };
        let key = unwrap_with(&self.class_keys, class, wrapped)?;
        decrypt_stream(&key, &mut input, entry.size, out)
    }

    /// The file's plain bytes, refused past `max` bytes.
    pub fn read(&self, entry: &Entry, max: u64) -> Result<Vec<u8>> {
        if entry.size.is_some_and(|s| s > max) {
            return Err(damaged(format!("{} is larger than {max} bytes", entry.relative_path)));
        }
        let mut out = Vec::new();
        self.copy(entry, &mut out)?;
        Ok(out)
    }

    /// Decrypts the file into `dir` under its own name (a database oopz then opens read-only).
    pub fn extract(&self, entry: &Entry, dir: &Path) -> Result<PathBuf> {
        let name =
            Path::new(&entry.relative_path).file_name().map_or_else(|| entry.file_id.clone().into(), |n| n.to_owned());
        let path = dir.join(name);
        let mut out = File::create(&path).map_err(|e| Error::Io("a decrypted copy".into(), e))?;
        self.copy(entry, &mut out)?;
        out.sync_all().map_err(|e| Error::Io("a decrypted copy".into(), e))?;
        Ok(path)
    }
}

#[cfg(all(test, feature = "write"))]
mod tests;
