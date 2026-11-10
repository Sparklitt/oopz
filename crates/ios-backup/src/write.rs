//! Writes a synthetic iPhone backup in the layout Finder and iTunes use, encrypted with a password or not: for tests
//! and `oopz-gen` only (invented data). The keybag uses small PBKDF2 counts unless asked otherwise, so tests stay fast;
//! the reader takes the counts from the keybag either way.

use std::fs;
use std::path::Path;

use aes::cipher::{BlockModeEncrypt, KeyInit, KeyIvInit, block_padding::Pkcs7};
use plist::{Dictionary, Uid, Value};
use rusqlite::{Connection, params};
use sha1::{Digest, Sha1};

/// One file to put in the backup.
pub struct File<'a> {
    pub domain: &'a str,
    pub relative_path: &'a str,
    pub bytes: &'a [u8],
}

/// How to encrypt.
pub struct Encryption<'a> {
    pub password: &'a str,
    /// PBKDF2-SHA256 rounds (`DPIC`; real backups use 10,000,000).
    pub dpic: u32,
    /// PBKDF2-SHA1 rounds (`ITER`).
    pub iter: u32,
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("randomness");
    b
}

fn wrap(kek: &[u8], key: &[u8]) -> Vec<u8> {
    let kw = aes_kw::KwAes256::new_from_slice(kek).expect("a 32-byte key");
    let mut buf = vec![0u8; key.len() + 8];
    kw.wrap_key(key, &mut buf).expect("wrapping").to_vec()
}

fn encrypt(key: &[u8], plain: &[u8]) -> Vec<u8> {
    let mut buf = plain.to_vec();
    buf.resize(plain.len() + 16, 0);
    let enc = cbc::Encryptor::<aes::Aes256>::new_from_slices(key, &[0u8; 16]).expect("key and iv");
    enc.encrypt_padded::<Pkcs7>(&mut buf, plain.len()).expect("room for the padding").to_vec()
}

fn record(out: &mut Vec<u8>, tag: &[u8; 4], value: &[u8]) {
    out.extend_from_slice(tag);
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
}

/// The protection class every file uses here (class C, "until first unlock", the usual one).
const CLASS: u32 = 3;

/// An `MBFile` archive as Finder writes it (NSKeyedArchiver).
fn mbfile(relative_path: &str, size: u64, wrapped: Option<Vec<u8>>) -> Vec<u8> {
    let mut root = Dictionary::new();
    root.insert("$class".into(), Value::Uid(Uid::new(3)));
    root.insert("RelativePath".into(), Value::Uid(Uid::new(2)));
    root.insert("Size".into(), Value::Integer(size.into()));
    root.insert("ProtectionClass".into(), Value::Integer(CLASS.into()));
    root.insert("Flags".into(), Value::Integer(0.into()));
    let mut objects = vec![
        Value::String("$null".into()),
        Value::Dictionary(root),
        Value::String(relative_path.into()),
        Value::Dictionary(Dictionary::from_iter([
            ("$classname".to_owned(), Value::String("MBFile".into())),
            (
                "$classes".to_owned(),
                Value::Array(vec![Value::String("MBFile".into()), Value::String("NSObject".into())]),
            ),
        ])),
    ];
    if let Some(w) = wrapped {
        let mut key = CLASS.to_le_bytes().to_vec();
        key.extend(w);
        objects.push(Value::Dictionary(Dictionary::from_iter([
            ("NS.data".to_owned(), Value::Data(key)),
            ("$class".to_owned(), Value::Uid(Uid::new(5))),
        ])));
        objects.push(Value::Dictionary(Dictionary::from_iter([(
            "$classname".to_owned(),
            Value::String("NSMutableData".into()),
        )])));
        if let Some(Value::Dictionary(r)) = objects.get_mut(1) {
            r.insert("EncryptionKey".into(), Value::Uid(Uid::new(4)));
        }
    }
    let archive = Value::Dictionary(Dictionary::from_iter([
        ("$archiver".to_owned(), Value::String("NSKeyedArchiver".into())),
        ("$version".to_owned(), Value::Integer(100000.into())),
        ("$objects".to_owned(), Value::Array(objects)),
        ("$top".to_owned(), Value::Dictionary(Dictionary::from_iter([("root".to_owned(), Value::Uid(Uid::new(1)))]))),
    ]));
    let mut out = Vec::new();
    archive.to_writer_binary(&mut out).expect("a plist");
    out
}

/// Writes a backup of `files` into the empty folder `dir`.
pub fn write(dir: &Path, device: &str, files: &[File<'_>], encryption: Option<&Encryption<'_>>) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    // Keys: a password key unwraps the class key, which wraps the manifest's and each file's.
    let class_key: [u8; 32] = random();
    let mut keybag = Vec::new();
    if let Some(e) = encryption {
        let salt: [u8; 20] = random();
        let dpsl: [u8; 20] = random();
        let mut first = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(e.password.as_bytes(), &dpsl, e.dpic, &mut first);
        let mut pass_key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha1::Sha1>(&first, &salt, e.iter, &mut pass_key);
        record(&mut keybag, b"VERS", &4u32.to_be_bytes());
        record(&mut keybag, b"TYPE", &1u32.to_be_bytes());
        record(&mut keybag, b"UUID", &random::<16>());
        record(&mut keybag, b"HMCK", &random::<40>());
        record(&mut keybag, b"WRAP", &0u32.to_be_bytes());
        record(&mut keybag, b"SALT", &salt);
        record(&mut keybag, b"ITER", &e.iter.to_be_bytes());
        record(&mut keybag, b"DPWT", &1u32.to_be_bytes());
        record(&mut keybag, b"DPIC", &e.dpic.to_be_bytes());
        record(&mut keybag, b"DPSL", &dpsl);
        // Class C, and class A that a backup also carries.
        for class in [CLASS, 1] {
            let key = if class == CLASS { class_key } else { random() };
            record(&mut keybag, b"UUID", &random::<16>());
            record(&mut keybag, b"CLAS", &class.to_be_bytes());
            record(&mut keybag, b"WRAP", &2u32.to_be_bytes());
            record(&mut keybag, b"KTYP", &0u32.to_be_bytes());
            record(&mut keybag, b"WPKY", &wrap(&pass_key, &key));
        }
    }
    let manifest_path = dir.join("Manifest.db");
    let plain_manifest = dir.join("Manifest.db.plain");
    {
        let db = Connection::open(&plain_manifest).map_err(std::io::Error::other)?;
        db.execute_batch(
            "CREATE TABLE Files (fileID TEXT PRIMARY KEY, domain TEXT, relativePath TEXT, flags INTEGER, file BLOB);
             CREATE INDEX FilesDomainIdx ON Files(domain);
             CREATE INDEX FilesRelativePathIdx ON Files(relativePath);
             CREATE INDEX FilesFlagsIdx ON Files(flags);
             CREATE TABLE Properties (key TEXT PRIMARY KEY, value BLOB);",
        )
        .map_err(std::io::Error::other)?;
        let mut folders = std::collections::BTreeSet::new();
        for f in files {
            let id = hex(&Sha1::digest(format!("{}-{}", f.domain, f.relative_path).as_bytes()));
            let stored = dir.join(&id[..2]);
            fs::create_dir_all(&stored)?;
            let wrapped = match encryption {
                Some(_) => {
                    let key: [u8; 32] = random();
                    fs::write(stored.join(&id), encrypt(&key, f.bytes))?;
                    Some(wrap(&class_key, &key))
                }
                None => {
                    fs::write(stored.join(&id), f.bytes)?;
                    None
                }
            };
            db.execute(
                "INSERT INTO Files VALUES (?1, ?2, ?3, 1, ?4)",
                params![id, f.domain, f.relative_path, mbfile(f.relative_path, f.bytes.len() as u64, wrapped)],
            )
            .map_err(std::io::Error::other)?;
            let mut parent = Path::new(f.relative_path).parent();
            while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
                folders.insert((f.domain.to_owned(), p.to_string_lossy().into_owned()));
                parent = p.parent();
            }
        }
        for (domain, path) in folders {
            let id = hex(&Sha1::digest(format!("{domain}-{path}").as_bytes()));
            db.execute("INSERT OR IGNORE INTO Files VALUES (?1, ?2, ?3, 2, NULL)", params![id, domain, path])
                .map_err(std::io::Error::other)?;
        }
    }
    let manifest_key: [u8; 32] = random();
    let plain = fs::read(&plain_manifest)?;
    fs::remove_file(&plain_manifest)?;
    match encryption {
        Some(_) => fs::write(&manifest_path, encrypt(&manifest_key, &plain))?,
        None => fs::write(&manifest_path, plain)?,
    }
    let mut lockdown = Dictionary::new();
    lockdown.insert("DeviceName".into(), Value::String(device.into()));
    lockdown.insert("ProductVersion".into(), Value::String("17.5".into()));
    let mut m = Dictionary::new();
    m.insert("IsEncrypted".into(), Value::Boolean(encryption.is_some()));
    m.insert("Version".into(), Value::String("10.0".into()));
    m.insert(
        "Date".into(),
        Value::Date(
            std::time::SystemTime::UNIX_EPOCH
                .checked_add(std::time::Duration::from_secs(1_704_067_200))
                .expect("a date")
                .into(),
        ),
    );
    m.insert("Lockdown".into(), Value::Dictionary(lockdown));
    if encryption.is_some() {
        m.insert("BackupKeyBag".into(), Value::Data(keybag));
        let mut key = CLASS.to_le_bytes().to_vec();
        key.extend(wrap(&class_key, &manifest_key));
        m.insert("ManifestKey".into(), Value::Data(key));
    }
    Value::Dictionary(m)
        .to_writer_binary(fs::File::create(dir.join("Manifest.plist"))?)
        .map_err(std::io::Error::other)?;
    let mut info = Dictionary::new();
    info.insert("Device Name".into(), Value::String(device.into()));
    Value::Dictionary(info).to_writer_xml(fs::File::create(dir.join("Info.plist"))?).map_err(std::io::Error::other)?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
