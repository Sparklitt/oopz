//! `oops`: a person's own WhatsApp backups from a terminal (decision 0007). Prints counts and masked ids only, never
//! message text, phone numbers or keys.

use std::io::{self, BufRead, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use oops::{Backup, ChatMerge, Key};
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(name = "oops", version, about = "Your own WhatsApp backups: inspect them, restore a lost chat")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check every backup (`*.crypt15`) in a folder with one key: does it open, is its database sound, how many
    /// chats and messages, the newest message's date. Fails if any backup doesn't pass.
    Backups {
        /// A folder of backups, e.g. WhatsApp's `Databases/`; only read.
        #[arg(long)]
        dir: PathBuf,
    },
    /// List the chats in a WhatsApp backup (asks for the backup's 64-digit key).
    Chats {
        /// The msgstore.db.crypt15 file; only read.
        #[arg(long)]
        backup: PathBuf,
    },
    /// For each WhatsApp message type in a chat: counts and which tables its rows link to. No content.
    Types {
        #[arg(long)]
        backup: PathBuf,
        /// The chat number shown by `oops chats`.
        #[arg(long)]
        chat: i64,
    },
    /// Every table in a WhatsApp backup with its columns and row count. Structure only, no values.
    Tables {
        #[arg(long)]
        backup: PathBuf,
    },
    /// How WhatsApp keeps one chat: ordering ids, senders, statuses, pointers. Counts and ranges only.
    ChatFacts {
        #[arg(long)]
        backup: PathBuf,
        /// The chat number shown by `oops chats`.
        #[arg(long)]
        chat: i64,
    },
    /// Restore a lost chat: add the messages of a one-to-one chat in one backup (the source) that the same chat in
    /// another person's backup (the target) lacks, mirrored, and write a new, checked backup for the target's owner
    /// (decision 0006). Asks for the source's key, then the target's. Both originals are only read.
    Restore {
        #[arg(long)]
        source: PathBuf,
        /// The chat number in the source, shown by `oops chats`.
        #[arg(long)]
        source_chat: i64,
        #[arg(long)]
        target: PathBuf,
        /// The same chat's number in the target.
        #[arg(long)]
        target_chat: i64,
        /// The new backup; never overwritten.
        #[arg(long)]
        out: PathBuf,
        /// The source phone's `Media/` folder; only read. With `--media-out`, the files the result needs that the
        /// target lacks are copied out, laid out like the target's `Media/` folder.
        #[arg(long, requires = "media_out")]
        media: Option<PathBuf>,
        /// A new folder for those files; never overwritten.
        #[arg(long, requires = "media")]
        media_out: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Backups { dir } => {
            let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
                .with_context(|| format!("reading {}", dir.display()))?
                .map(|e| e.map(|e| e.path()))
                .collect::<io::Result<_>>()?;
            files.retain(|p| p.is_file() && p.extension().is_some_and(|e| e == "crypt15"));
            files.sort();
            anyhow::ensure!(!files.is_empty(), "no .crypt15 backups in {}", dir.display());
            let key = read_key()?;
            let mut bad = 0;
            for path in &files {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let mb = std::fs::metadata(path)?.len() / 1_000_000;
                match Backup::open(path, &key).and_then(|b| b.health()) {
                    Ok(h) if h.check == "ok" => println!(
                        "{name}  {mb} MB  ok: {} chats, {} messages, last {}",
                        h.chats,
                        h.messages,
                        date(h.last_ms)
                    ),
                    Ok(h) => {
                        bad += 1;
                        println!("{name}  {mb} MB  DAMAGED: {}", h.check);
                    }
                    Err(e) => {
                        bad += 1;
                        println!("{name}  {mb} MB  FAILED: {e}");
                    }
                }
            }
            anyhow::ensure!(bad == 0, "{bad} of {} backups didn't pass", files.len());
        }
        Command::Chats { backup } => {
            let backup = Backup::open(&backup, &read_key()?)?;
            println!("{:>5}  {:<20} {:>9}  {:<12}", "chat", "who", "messages", "last message");
            for chat in backup.chats()? {
                println!("{:>5}  {:<20} {:>9}  {:<12}", chat.row_id, chat.masked(), chat.messages, date(chat.last_ms));
            }
        }
        Command::Types { backup, chat } => {
            let backup = Backup::open(&backup, &read_key()?)?;
            for p in backup.type_profile(chat)? {
                println!("type {}: {} messages, {} with text", p.code, p.messages, p.with_text);
                for (table, n) in &p.linked {
                    println!("    linked {table}: {n}");
                }
                for (mime, n) in p.mimes.iter().take(3) {
                    println!("    media {mime}: {n}");
                }
            }
        }
        Command::ChatFacts { backup, chat } => {
            for (label, value) in Backup::open(&backup, &read_key()?)?.chat_facts(chat)? {
                println!("{label}: {value}");
            }
        }
        Command::Tables { backup } => {
            for (name, columns, rows) in Backup::open(&backup, &read_key()?)?.tables()? {
                println!("{name} ({rows}): {}", columns.join(", "));
            }
        }
        Command::Restore { source, source_chat, target, target_chat, out, media, media_out } => {
            for path in std::iter::once(&out).chain(&media_out) {
                anyhow::ensure!(!path.exists(), "{} already exists; choose a new name", path.display());
            }
            let source = Backup::open(&source, &read_key_for("source")?)?;
            let target_key = read_key_for("target")?;
            let original = std::fs::read(&target).with_context(|| format!("reading {}", target.display()))?;
            let merged =
                ChatMerge { source: &source, source_chat, target: &original, target_key: &target_key, target_chat }
                    .run()?;
            print!("{}", merged.report);
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&out)
                .and_then(|mut f| io::Write::write_all(&mut f, &merged.backup))
                .with_context(|| format!("writing {}", out.display()))?;
            println!("written: {} ({} bytes)", out.display(), merged.backup.len());
            if let (Some(media), Some(media_out)) = (media, media_out) {
                print!("{}", oops::copy_media(&merged.media, &media, &media_out)?);
                println!("media for the target's Media/ folder: {}", media_out.display());
            }
            println!("next: oops chat-facts --backup {} --chat {target_chat}", out.display());
        }
    }
    Ok(())
}

/// From a hidden prompt, or one line of stdin when it isn't a terminal (`oops … < keyfile`). Never from an argument
/// or the environment: both leak into shell history and process lists.
fn read_key() -> Result<Key> {
    read_key_for("backup")
}

/// [`read_key`] for one of several backups, named in the prompt; piped, one line each, in the order asked.
fn read_key_for(which: &str) -> Result<Key> {
    let text = if io::stdin().is_terminal() {
        Zeroizing::new(rpassword::prompt_password(format!("64-digit {which} key (not shown): "))?)
    } else {
        let mut line = Zeroizing::new(String::new());
        io::stdin().lock().read_line(&mut line)?;
        line
    };
    Ok(Key::from_hex(&text)?)
}

fn date(ms: Option<i64>) -> String {
    ms.and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok())
        .map_or_else(|| "-".into(), |t| t.to_zoned(jiff::tz::TimeZone::UTC).date().to_string())
}
