//! WhatsApp crypt15 (end-to-end encrypted backup) decryption with the person's own 64-digit key. Format and crates:
//! decision 0004. Errors never contain the key.

use std::fmt;
use std::io::{Read, Write};

use aes_gcm::aead::consts::U16;
use aes_gcm::aead::{Aead, AeadInOut, Tag};
use aes_gcm::aes::Aes256;
use aes_gcm::{AesGcm, KeyInit};
use flate2::read::ZlibDecoder;
use hkdf::Hkdf;
use prost::Message as _;
use sha2::Sha256;
use zeroize::Zeroizing;

type Cipher = AesGcm<Aes256, U16>;

const TAG_LEN: usize = 16;
const CHECKSUM_LEN: usize = 16;
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the key must be 64 hexadecimal characters (0-9, a-f), as WhatsApp showed it")]
    BadKeyFormat,
    #[error("not a crypt15 backup: {0}")]
    NotCrypt15(&'static str),
    #[error("this key doesn't open this backup (a different key, or the file is damaged or incomplete)")]
    WrongKeyOrDamaged,
    #[error("the backup opened but doesn't contain a SQLite database ({0})")]
    BadContents(&'static str),
    #[error("this computer gave no randomness for a fresh IV")]
    NoRandomness,
    #[error("writing the decrypted database: {0}")]
    Write(std::io::Error),
    #[error("stopped before the end, as asked")]
    Cancelled,
}

/// The 64-digit key, wiped from memory when dropped. Its `Debug` never shows it.
pub struct Key(Zeroizing<[u8; 32]>);

impl Key {
    /// Accepts the key as WhatsApp shows it: 64 hex characters, spaces and line breaks ignored.
    pub fn from_hex(text: &str) -> Result<Self, Error> {
        let digits = Zeroizing::new(text.chars().filter(|c| !c.is_whitespace()).collect::<String>());
        let mut key = Zeroizing::new([0u8; 32]);
        if digits.len() != 64 {
            return Err(Error::BadKeyFormat);
        }
        hex::decode_to_slice(digits.as_bytes(), &mut *key).map_err(|_| Error::BadKeyFormat)?;
        Ok(Self(key))
    }

    /// HKDF-SHA256 of the root key, zero salt, info "backup encryption".
    fn cipher(&self) -> Cipher {
        let mut okm = Zeroizing::new([0u8; 32]);
        Hkdf::<Sha256>::new(Some(&[0u8; 32]), &*self.0)
            .expand(b"backup encryption", &mut *okm)
            .expect("32 bytes is a valid HKDF-SHA256 output length");
        Cipher::new_from_slice(&*okm).expect("AES-256 takes a 32-byte key")
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(..)")
    }
}

#[derive(Clone, PartialEq, prost::Message)]
struct BackupPrefix {
    #[prost(message, optional, tag = "3")]
    c15_iv: Option<C15Iv>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct C15Iv {
    #[prost(bytes = "vec", tag = "1")]
    iv: Vec<u8>,
}

/// (everything before the ciphertext, the rest, the IV)
type Parts<'a> = (&'a [u8], &'a [u8], [u8; 16]);

/// Whether `head`, a file's first bytes (512 is plenty), starts like a crypt15 backup: its header reads and holds an IV.
/// Nothing is decrypted; for telling a backup from another file before asking for its key.
pub fn looks_like(head: &[u8]) -> bool {
    split(head).is_ok()
}

/// Splits a crypt15 file into everything before the ciphertext (length byte, optional 0x01, protobuf header), the
/// rest, and the header's IV.
fn split(file: &[u8]) -> Result<Parts<'_>, Error> {
    let (&header_len, rest) = file.split_first().ok_or(Error::NotCrypt15("empty file"))?;
    // A 0x01 after the length marks a feature table. It can't start a protobuf (field 0 doesn't exist).
    let rest = rest.strip_prefix(&[1]).unwrap_or(rest);
    if rest.len() < header_len as usize {
        return Err(Error::NotCrypt15("truncated header"));
    }
    let (header, body) = rest.split_at(header_len as usize);
    let prefix = BackupPrefix::decode(header).map_err(|_| Error::NotCrypt15("unreadable header"))?;
    let iv = prefix.c15_iv.ok_or(Error::NotCrypt15("no crypt15 IV in the header (a crypt14 backup?)"))?.iv;
    let iv = iv.as_slice().try_into().map_err(|_| Error::NotCrypt15("the IV isn't 16 bytes"))?;
    Ok((&file[..file.len() - body.len()], body, iv))
}

/// The WhatsApp version that wrote the backup (`2.24.10.80`), from its header, when the header says. Read from the
/// unencrypted header only, so before any key. The header's metadata isn't documented: this takes the first nested
/// message whose field 1 is a version (digits and dots), and reads nothing else in it (the metadata also holds the
/// end of the owner's number, which stays unread).
pub fn app_version(head: &[u8]) -> Option<String> {
    let (prefix, _, _) = split(head).ok()?;
    let header = &prefix[prefix.len() - usize::from(head[0])..];
    fields(header).into_iter().filter(|(tag, _)| *tag != 3).find_map(|(_, inner)| {
        let (tag, value) = fields(inner).into_iter().next()?;
        let v = std::str::from_utf8(value).ok().filter(|_| tag == 1)?;
        let looks =
            v.split('.').count() >= 3 && v.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        looks.then(|| v.to_owned())
    })
}

/// The length-delimited fields of a protobuf message, in order (tag, bytes); other wire types are skipped. Stops at
/// anything malformed.
fn fields(mut b: &[u8]) -> Vec<(u64, &[u8])> {
    fn varint(b: &mut &[u8]) -> Option<u64> {
        let mut v = 0u64;
        for i in 0..10 {
            let (&byte, rest) = b.split_first()?;
            *b = rest;
            v |= u64::from(byte & 0x7f) << (7 * i);
            if byte & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    let mut out = Vec::new();
    while let Some(key) = varint(&mut b) {
        let (tag, wire) = (key >> 3, key & 7);
        match wire {
            0 => {
                if varint(&mut b).is_none() {
                    break;
                }
            }
            1 | 5 => {
                let n = if wire == 1 { 8 } else { 4 };
                if b.len() < n {
                    break;
                }
                b = &b[n..];
            }
            2 => {
                let Some(n) = varint(&mut b).and_then(|n| usize::try_from(n).ok()).filter(|&n| n <= b.len()) else {
                    break;
                };
                out.push((tag, &b[..n]));
                b = &b[n..];
            }
            _ => break,
        }
    }
    out
}

/// Decrypts and decompresses a crypt15 file into the bytes of its SQLite database.
pub fn decrypt(file: &[u8], key: &Key) -> Result<Vec<u8>, Error> {
    let mut sqlite = Vec::new();
    decrypt_to(file.to_vec(), key, &mut sqlite)?;
    Ok(sqlite)
}

/// Like [`decrypt`], but decrypts `file` in place and streams the database into `out`: a phone holds the backup once
/// (compressed), never its whole database as well (about three times larger).
pub fn decrypt_to(file: Vec<u8>, key: &Key, out: &mut impl Write) -> Result<(), Error> {
    decrypt_to_with(file, key, out, &mut |_, _| true)
}

/// [`decrypt_to`], calling `progress(done, total)` in compressed bytes as it decompresses (a large backup takes tens
/// of seconds on a phone: the bar moves meanwhile).
pub fn decrypt_to_with(
    mut file: Vec<u8>,
    key: &Key,
    out: &mut impl Write,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<(), Error> {
    let (_, body, iv) = split(&file)?;
    let at = file.len() - body.len();
    let nonce = &iv.into();
    let body = &mut file[at..];

    // The main database ends with a 16-byte MD5 checksum after the tag; other backup files don't. GCM's tag
    // authenticates everything we use, so try with the checksum stripped, then without. A failed try leaves the
    // buffer as it was (the tag is checked before anything is decrypted).
    let cipher = key.cipher();
    let ends = [body.len().checked_sub(CHECKSUM_LEN), Some(body.len())];
    let mut plain = None;
    for end in ends.into_iter().flatten().filter(|&end| end >= TAG_LEN) {
        let (ciphertext, rest) = body[..end].split_at_mut(end - TAG_LEN);
        let tag = Tag::<Cipher>::try_from(&*rest).expect("16 bytes");
        if cipher.decrypt_inout_detached(nonce, &[], ciphertext.into(), &tag).is_ok() {
            plain = Some(end - TAG_LEN);
            break;
        }
    }
    let compressed = &body[..plain.ok_or(Error::WrongKeyOrDamaged)?];
    let total = compressed.len() as u64;

    let mut zlib = ZlibDecoder::new(compressed);
    let not_zlib = |_| Error::BadContents("not zlib-compressed");
    let mut head = [0u8; SQLITE_MAGIC.len()];
    match zlib.read_exact(&mut head) {
        Ok(()) if head == SQLITE_MAGIC => {}
        Ok(()) => return Err(Error::BadContents("no SQLite header")),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Err(Error::BadContents("no SQLite header")),
        Err(e) => return Err(not_zlib(e)),
    }
    out.write_all(&head).map_err(Error::Write)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut shown = 0;
    loop {
        let n = zlib.read(&mut buf).map_err(not_zlib)?;
        if n == 0 {
            progress(total, total);
            return Ok(());
        }
        out.write_all(&buf[..n]).map_err(Error::Write)?;
        if zlib.total_in() >= shown + (8 << 20) {
            shown = zlib.total_in();
            if !progress(shown, total) {
                return Err(Error::Cancelled);
            }
        }
    }
}

/// Encrypts `sqlite` as a new version of `original` (decision 0006): `original`'s header bytes exactly, except its IV
/// replaced in place with a fresh random one (the same key with a reused IV would break AES-GCM); then zlib,
/// AES-256-GCM, and the MD5 of everything before it, as WhatsApp writes its main database.
pub fn reseal(original: &[u8], sqlite: &[u8], key: &Key) -> Result<Vec<u8>, Error> {
    use md5::Digest as _;
    use std::io::Write;

    let (prefix, _, old_iv) = split(original)?;
    let at = prefix.windows(16).position(|w| w == old_iv).ok_or(Error::NotCrypt15("the IV isn't in the header"))?;
    if prefix[at + 1..].windows(16).any(|w| w == old_iv) {
        return Err(Error::NotCrypt15("the IV appears twice in the header"));
    }
    let mut iv = [0u8; 16];
    getrandom::fill(&mut iv).map_err(|_| Error::NoRandomness)?;
    if iv == old_iv {
        return Err(Error::NoRandomness);
    }

    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    zlib.write_all(sqlite).expect("writing to memory");
    let compressed = zlib.finish().expect("writing to memory");
    let ciphertext = key.cipher().encrypt(&iv.into(), compressed.as_slice()).expect("encrypting in memory");

    let mut out = prefix.to_vec();
    out[at..at + 16].copy_from_slice(&iv);
    out.extend_from_slice(&ciphertext);
    let checksum = md5::Md5::digest(&out);
    out.extend_from_slice(&checksum);
    Ok(out)
}

/// Builds a crypt15 file the way WhatsApp does, for synthetic test fixtures only. The checksum is zeros; we never
/// check it (see `decrypt`).
#[cfg(any(test, feature = "fixtures"))]
pub fn encrypt(sqlite: &[u8], key: &Key, iv: [u8; 16], with_checksum: bool) -> Vec<u8> {
    encrypt_as(sqlite, key, iv, with_checksum, None)
}

/// [`encrypt`], with the WhatsApp version the header names (field 5, field 1, as wa-crypt-tools reads it), for
/// synthetic test fixtures only.
#[cfg(any(test, feature = "fixtures"))]
pub fn encrypt_as(sqlite: &[u8], key: &Key, iv: [u8; 16], with_checksum: bool, version: Option<&str>) -> Vec<u8> {
    use std::io::Write;

    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    zlib.write_all(sqlite).expect("writing to memory");
    let compressed = zlib.finish().expect("writing to memory");
    let ciphertext = key.cipher().encrypt(&iv.into(), compressed.as_slice()).expect("encrypting in memory");
    let mut header = BackupPrefix { c15_iv: Some(C15Iv { iv: iv.to_vec() }) }.encode_to_vec();
    if let Some(v) = version {
        // field 5 { field 1: the version, field 2: a number's end (invented) }
        let inner = [&[0x0a, v.len() as u8][..], v.as_bytes(), &[0x12, 2], b"99"].concat();
        header.extend_from_slice(&[0x2a, inner.len() as u8]);
        header.extend_from_slice(&inner);
    }

    let mut out = vec![u8::try_from(header.len()).expect("a short header"), 1];
    out.extend_from_slice(&header);
    out.extend_from_slice(&ciphertext);
    if with_checksum {
        out.extend_from_slice(&[0; CHECKSUM_LEN]);
    }
    out
}
