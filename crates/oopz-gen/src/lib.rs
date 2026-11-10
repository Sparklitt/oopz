//! oopz-gen: synthetic WhatsApp backups for testing oopz, at any size. Invented people (numbers `9999…`), invented
//! chats and messages of every kind WhatsApp stores, written to a phone-shaped folder:
//!
//! ```text
//! out/Databases/msgstore.db.crypt15   the messages, encrypted with out/key.txt
//! out/Databases/wa.db.crypt15         the contacts' names
//! out/Media/…                         placeholder media files (when asked)
//! out/key.txt                         the 64-digit key
//! out/manifest.txt                    what's inside: counts by kind, for tests to compare
//! ```
//!
//! Parts: [`spec`] (what to make), [`world`] (people and chats), [`msgstore`] (the message rows), [`wadb`]
//! (contacts), [`media`] (file names and placeholder files). Chats are written interleaved in time order, as a phone
//! does, each from its own seeded stream, so memory stays flat whatever the size, and one seed gives one backup.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use oopz::crypt15::{self, Key};
use rusqlite::Connection;

pub mod corpus;
pub mod media;
pub mod msgstore;
pub mod rng;
pub mod spec;
pub mod wadb;
pub mod world;

use spec::{Kind, MediaMode, Spec};

pub struct Report {
    pub msgstore: PathBuf,
    pub contacts: Option<PathBuf>,
    pub manifest: PathBuf,
    pub chats: usize,
    pub people: usize,
    pub messages: u64,
    pub media_files: u64,
    pub seconds: f64,
}

/// A key for `seed`: 64 hex digits, never a real backup's (it's printed in `key.txt`).
pub fn key_for(seed: u64) -> String {
    (0..4).map(|i| format!("{:016x}", rng::mix(seed.wrapping_add(0x006B_6579) ^ i))).collect()
}

/// Writes the backup into `out` (created if needed; refused if it already holds one), calling `progress(done, total)`
/// in messages.
pub fn generate(spec: &Spec, out: &Path, key_hex: &str, progress: &mut dyn FnMut(u64, u64)) -> Result<Report> {
    let started = Instant::now();
    spec.check().map_err(anyhow::Error::msg)?;
    let key = Key::from_hex(key_hex).context("the key must be 64 hex digits")?;
    let databases = out.join("Databases");
    let msgstore = databases.join("msgstore.db.crypt15");
    if msgstore.exists() {
        bail!("{} already exists: choose an empty folder", msgstore.display());
    }
    fs::create_dir_all(&databases).with_context(|| format!("creating {}", databases.display()))?;

    let world = world::World::new(spec);
    let total: u64 = world.chats.iter().map(|c| c.messages).sum();
    let plain = databases.join("msgstore.db");
    let partial = databases.join("msgstore.db.partial");
    let _ = fs::remove_file(&partial);
    let db = Connection::open(&partial).context("creating the database")?;
    // A throwaway file until it's encrypted: no journal, no syncing.
    db.execute_batch(
        "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA locking_mode = EXCLUSIVE;
         PRAGMA cache_size = -65536; PRAGMA temp_store = MEMORY; PRAGMA user_version = 1;
         BEGIN;",
    )?;
    let media_root = (spec.media == MediaMode::Files).then_some(out);
    let (mut writer, mut chats) = msgstore::Writer::new(&db, spec, &world, media::Media::new(media_root))?;

    // Each chat's next message time, earliest first.
    let mut left: Vec<u64> = world.chats.iter().map(|c| c.messages).collect();
    let mut queue = BinaryHeap::new();
    for (i, chat) in world.chats.iter().enumerate() {
        if chat.messages == 0 {
            continue;
        }
        writer.open_chat(&mut chats[i], i)?;
        if spec.edge_cases {
            // One of every kind first, so even a short chat shows them all.
            for kind in Kind::ALL {
                let ts = chats[i].last_ts + 30_000;
                writer.message(&mut chats[i], ts, Some(kind))?;
                chats[i].last_ts = ts;
            }
            if i == 0 {
                writer.orphans(&mut chats[i])?;
            }
            let written = chats[i].written;
            left[i] = left[i].saturating_sub(written);
        }
        queue.push(Reverse((chats[i].last_ts, i)));
    }

    let mut done = 0u64;
    let mut last_report = 0u64;
    while let Some(Reverse((ts, i))) = queue.pop() {
        if left[i] == 0 {
            continue;
        }
        let before = chats[i].written;
        writer.message(&mut chats[i], ts, None)?;
        let wrote = chats[i].written - before;
        left[i] = left[i].saturating_sub(wrote);
        done += wrote;
        if done - last_report >= 20_000 {
            progress(done.min(total), total);
            last_report = done;
        }
        let state = &mut chats[i];
        state.last_ts = ts;
        if left[i] > 0 {
            // Conversations come in bursts: mostly seconds to minutes apart, now and then hours or days. The mean gap
            // is the time left over the messages left, so every chat ends by the end date.
            let g = ((spec.end_ms - ts) as f64).max(left[i] as f64) / left[i] as f64;
            let wait = if state.rng.chance(0.9) { state.rng.exp(g * 0.1) } else { state.rng.exp(g * 9.1) };
            let mut next = ts + 1 + wait as i64;
            // Fewer messages at night (UTC 1–6 h): most are moved to the morning.
            let hour = next.rem_euclid(86_400_000) / 3_600_000;
            if (1..7).contains(&hour) && state.rng.chance(0.8) {
                next += (7 - hour) * 3_600_000 + state.rng.below(3_600_000) as i64;
            }
            // A minute early: an album's photos follow it by seconds.
            queue.push(Reverse((next.min(spec.end_ms - 60_000).max(ts), i)));
        }
    }
    writer.finish(&chats)?;
    let media_files = writer.media.files;
    let counts = std::mem::take(&mut writer.counts);
    drop(writer);
    db.execute_batch("COMMIT;")?;
    db.close().map_err(|(_, e)| e).context("closing the database")?;
    progress(total, total);

    seal(&partial, &msgstore, &key, spec.seed, if spec.keep_plain { Some(&plain) } else { None })?;

    let contacts = if spec.contacts_db {
        let partial = databases.join("wa.db.partial");
        let _ = fs::remove_file(&partial);
        let db = Connection::open(&partial)?;
        db.execute_batch("PRAGMA journal_mode = OFF; BEGIN;")?;
        wadb::write(&db, &world, spec.edge_cases)?;
        db.execute_batch("COMMIT;")?;
        db.close().map_err(|(_, e)| e)?;
        let path = databases.join("wa.db.crypt15");
        let keep = databases.join("wa.db");
        seal(&partial, &path, &key, spec.seed ^ 1, if spec.keep_plain { Some(&keep) } else { None })?;
        Some(path)
    } else {
        None
    };

    fs::write(out.join("key.txt"), format!("{key_hex}\n")).context("writing key.txt")?;
    let manifest = out.join("manifest.txt");
    let mut text = String::new();
    let _ = writeln!(text, "oopz-gen synthetic WhatsApp backup: invented data only");
    let _ = writeln!(text, "seed {}", spec.seed);
    let _ = writeln!(text, "people {}", world.people.len());
    let _ = writeln!(text, "chats {}", world.chats.len());
    let _ = writeln!(text, "groups {}", world.chats.iter().filter(|c| c.is_group()).count());
    let _ = writeln!(text, "messages {}", counts.messages);
    let _ = writeln!(text, "main_chat_messages {}", chats.first().map_or(0, |c| c.written));
    for (name, n) in [
        ("system", counts.system),
        ("reactions", counts.reactions),
        ("replies", counts.replies),
        ("edits", counts.edits),
        ("forwarded", counts.forwarded),
        ("starred", counts.starred),
        ("poll_votes", counts.poll_votes),
        ("odd_rows", counts.orphans),
        ("media_files", media_files),
    ] {
        let _ = writeln!(text, "{name} {n}");
    }
    for (kind, n) in &counts.by_kind {
        let _ = writeln!(text, "kind.{kind} {n}");
    }
    fs::write(&manifest, text).context("writing manifest.txt")?;

    Ok(Report {
        msgstore,
        contacts,
        manifest,
        chats: world.chats.len(),
        people: world.people.len(),
        messages: counts.messages,
        media_files,
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// Encrypts the database at `partial` into `to` as WhatsApp would, then removes it (or keeps it as `keep`).
fn seal(partial: &Path, to: &Path, key: &Key, seed: u64, keep: Option<&Path>) -> Result<()> {
    let sqlite = fs::read(partial).with_context(|| format!("reading {}", partial.display()))?;
    let mut iv = [0u8; 16];
    for (i, b) in iv.iter_mut().enumerate() {
        *b = rng::mix(seed ^ (i as u64 + 0x1F)) as u8;
    }
    let sealed = crypt15::encrypt(&sqlite, key, iv, true);
    drop(sqlite);
    let tmp = to.with_extension("crypt15.partial");
    fs::write(&tmp, sealed).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, to).with_context(|| format!("writing {}", to.display()))?;
    match keep {
        Some(keep) => fs::rename(partial, keep)?,
        None => fs::remove_file(partial)?,
    }
    Ok(())
}
