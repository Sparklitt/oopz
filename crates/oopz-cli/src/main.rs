//! `oopz`: a person's own WhatsApp backups from a terminal (decision 0007). Prints counts and masked ids only, never
//! message text, phone numbers or keys; message text goes only into a file the person names (`oopz export`).
//!
//! Every command runs the same `oopz-app` code as the desktop app's matching screen (docs/parity.md), so the two give
//! the same results and refuse the same things.

use std::io::{self, BufRead, IsTerminal, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use msgkinds::Note;
use oopz_app::export::Labels;
use oopz_app::session::{self, MergeRequest, Session, Summary};
use zeroize::Zeroizing;

mod library;
mod pull;
mod say;

use say::{Message, say};

#[derive(Parser)]
#[command(
    name = "oopz",
    version,
    about = "Your own chat history: WhatsApp backups (inspect, restore a lost chat) and every app's exports (oopz lib)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check every backup in a folder (or one file) with one key: does it open, is its database sound, how many
    /// chats and messages, the newest message's date. Older formats are listed, not opened. Fails if any backup that
    /// can be opened doesn't pass.
    Backups {
        /// A backup file, WhatsApp's `Databases/` folder, or the folder holding it; only read.
        #[arg(long)]
        dir: PathBuf,
    },
    /// What's inside one backup: soundness, counts, dates and the original file's fingerprint (SHA-256).
    Check {
        /// The msgstore.db.crypt15 file; only read.
        #[arg(long)]
        backup: PathBuf,
    },
    /// List the chats in a WhatsApp backup (asks for the backup's 64-digit key).
    Chats {
        #[arg(long)]
        backup: PathBuf,
        /// Also print each chat's name (a group's subject, a contact's name from wa.db.crypt15 beside the backup).
        #[arg(long)]
        names: bool,
    },
    /// Numbers for the whole backup or one chat: messages by month, hour and type, voice minutes, top senders
    /// (masked).
    Stats {
        #[arg(long)]
        backup: PathBuf,
        /// The chat number shown by `oopz chats`; the whole backup without it.
        #[arg(long)]
        chat: Option<i64>,
    },
    /// Search messages (accents and case don't matter). Prints where each match is (chat, date, sent or received,
    /// its position in the chat), never the text: `oopz export` writes a chat's text to a file.
    Search {
        #[arg(long)]
        backup: PathBuf,
        /// Words to find; without them, the newest messages that match the other options.
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long)]
        chat: Option<i64>,
        /// Message type codes, comma-separated (0 text, 1 photo, 2 audio, 3 video, 9 document, …).
        #[arg(long, value_delimiter = ',')]
        kind: Vec<i64>,
        /// `me`, `them`, or the last 3 characters of a sender's id (as `oopz stats` shows them).
        #[arg(long)]
        from: Option<String>,
        /// From this day on (YYYY-MM-DD, UTC).
        #[arg(long)]
        after: Option<jiff::civil::Date>,
        /// Before this day (YYYY-MM-DD, UTC).
        #[arg(long)]
        before: Option<jiff::civil::Date>,
    },
    /// Save a photo message's thumbnail (a small JPEG) from the phone's Media folder to a file.
    Thumbnail {
        #[arg(long)]
        backup: PathBuf,
        /// The message's row number (as `oopz search` positions lead to).
        #[arg(long)]
        message: i64,
        /// The phone's `Media/` folder, or the `WhatsApp` folder holding it; only read.
        #[arg(long)]
        media: PathBuf,
        /// The longest side, in pixels.
        #[arg(long, default_value_t = 256)]
        size: u32,
        /// The JPEG file to write; it must not exist.
        #[arg(long)]
        out: PathBuf,
    },
    /// The position in a chat of its first message on or after a day (YYYY-MM-DD, UTC): "jump to a date".
    Jump {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        chat: i64,
        #[arg(long)]
        date: jiff::civil::Date,
    },
    /// Save the stats (and, with --chat, that chat's messages) as a web page in a new file in a folder. The file holds
    /// message text and isn't encrypted.
    Export {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        chat: Option<i64>,
        /// A folder; a new `oopz-export-N.html` is made in it.
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// What's missing between two backups' copies of the same chat, matched by WhatsApp's message ids. Asks for the
    /// first backup's key, then the second's.
    Missing {
        #[arg(long)]
        a: PathBuf,
        #[arg(long)]
        chat_a: i64,
        #[arg(long)]
        b: PathBuf,
        #[arg(long)]
        chat_b: i64,
    },
    /// Restore a lost chat: add the messages of a one-to-one chat in one backup (the source) that the same chat in
    /// another person's backup (the target) lacks, mirrored, and write a new, checked backup for the target's owner
    /// (decision 0006) in a new `oopz-result-N` folder. Asks for the source's key, then the target's. Both
    /// originals are only read.
    Restore {
        #[arg(long)]
        source: PathBuf,
        /// The chat number in the source, shown by `oopz chats`.
        #[arg(long)]
        source_chat: i64,
        #[arg(long)]
        target: PathBuf,
        /// The same chat's number in the target.
        #[arg(long)]
        target_chat: i64,
        /// A folder; the result goes in a new `oopz-result-N` folder in it, laid out like WhatsApp's.
        #[arg(long)]
        out_dir: PathBuf,
        /// The source phone's `Media/` folder; only read. The files the result needs are copied into its `Media/`.
        #[arg(long)]
        media: Option<PathBuf>,
    },
    /// Receive a backup from the phone over Wi-Fi: shows a QR code and an address for the phone's browser, and
    /// saves the files into a new `oopz-received-N` folder. Stops when the phone is done, after 10 quiet minutes, or
    /// on Ctrl-C.
    Receive {
        /// A folder; the received files go in a new folder in it.
        #[arg(long)]
        out_dir: PathBuf,
        /// The address to put in the QR code, when this computer has several (they're listed); the likeliest
        /// without it.
        #[arg(long)]
        address: Option<IpAddr>,
    },
    /// Copy the WhatsApp backup from a phone with adb (USB or wireless debugging) into a new `oopz-pulled-N` folder:
    /// its Databases folder, and Media with `--media`. Only reads the phone. adb comes from Android's platform-tools.
    Pull(Box<pull::Pull>),
    /// For each WhatsApp message type in a chat: counts and which tables its rows link to. No content.
    Types {
        #[arg(long)]
        backup: PathBuf,
        /// The chat number shown by `oopz chats`.
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
        /// The chat number shown by `oopz chats`.
        #[arg(long)]
        chat: i64,
    },
    /// The library: every app's exports and backups together, to search, link contacts, select and export
    /// (`oopz lib --help`).
    Lib(Box<library::Lib>),
    /// Prints a completion script for a shell (bash, zsh, fish, elvish, powershell), for example
    /// `oopz completions bash > ~/.local/share/bash-completion/completions/oopz`.
    Completions { shell: clap_complete::Shell },
    /// Prints the manual page (roff); the Linux packages install it as `man oopz`.
    #[command(hide = true)]
    Man,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", say!(ErrorLine, format!("{e:#}")));
            ExitCode::FAILURE
        }
    }
}

/// A session whose decrypted copies live in a private temporary folder, removed when it's dropped (the session
/// first: Windows can't delete open files).
struct Open {
    session: Session,
    _tmp: tempfile::TempDir,
    tmp: PathBuf,
}

impl Open {
    fn new() -> Result<Self> {
        let tmp = tempfile::Builder::new().prefix("oopz-").tempdir().with_context(|| say!(TempFolder))?;
        Ok(Self { session: Session::default(), tmp: tmp.path().to_path_buf(), _tmp: tmp })
    }

    /// Checks `path` with the key asked for as `which`, as the app's Check screen does.
    fn check(&mut self, path: &Path, which: Message) -> Result<Summary> {
        let key = read_key_text(which)?;
        self.check_with(path, &key)
    }

    fn check_with(&mut self, path: &Path, key: &str) -> Result<Summary> {
        Ok(self.session.check(path, key, &self.tmp)?)
    }

    /// [`Open::check`] and the search index, for stats, search and export.
    fn indexed(&mut self, path: &Path) -> Result<u32> {
        let id = self.check(path, Message::KeyBackup)?.id;
        self.session.build_index(id, &self.tmp)?;
        Ok(id)
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.session = Session::default();
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Lib(lib) => return library::run(*lib),
        Command::Completions { shell } => {
            // Into memory first: clap_complete panics on a closed pipe (`| head`); this fails like the other commands.
            let mut script = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "oopz", &mut script);
            return Ok(io::stdout().write_all(&script)?);
        }
        Command::Man => return Ok(clap_mangen::Man::new(Cli::command()).render(&mut io::stdout())?),
        Command::Pull(args) => return pull::run(*args),
        _ => {}
    }
    let mut open = Open::new()?;
    match cli.command {
        Command::Lib(_) | Command::Completions { .. } | Command::Man | Command::Pull(_) => {
            unreachable!("handled above")
        }
        Command::Backups { dir } => {
            let files = session::find_backups(&dir)?;
            let openable = files.iter().filter(|f| f.kind == "crypt15").count();
            let key = if openable > 0 { Some(read_key_text(Message::KeyBackup)?) } else { None };
            let (mut bad, mut checked) = (0, 0);
            for file in &files {
                let mb = file.bytes / 1_000_000;
                let name = &file.name;
                match (file.kind, &key) {
                    ("crypt15", Some(key)) => {
                        checked += 1;
                        match open.check_with(&file.path, key) {
                            Ok(s) if s.sound => {
                                println!("{}", say!(BackupOk, name, mb, s.chats, s.messages, date(s.last_ms)));
                                open.session.forget(s.id);
                            }
                            Ok(s) => {
                                bad += 1;
                                println!("{}", say!(BackupDamaged, name, mb, s.problems));
                                open.session.forget(s.id);
                            }
                            Err(e) => {
                                bad += 1;
                                println!("{}", say!(BackupFailed, name, mb, e));
                            }
                        }
                    }
                    ("not_crypt15", _) => {
                        checked += 1;
                        bad += 1;
                        println!("{}", say!(BackupNotCrypt15, name, mb));
                    }
                    (older, _) => println!("{}", say!(BackupOlder, name, mb, older)),
                }
            }
            anyhow::ensure!(checked > 0, say!(NoCrypt15Backups, dir.display()));
            anyhow::ensure!(bad == 0, say!(BackupsFailed, bad, checked));
        }
        Command::Check { backup } => {
            let s = open.check(&backup, Message::KeyBackup)?;
            println!("{}", say!(BackupBytes, s.name, s.bytes));
            if s.sound {
                println!("{}", say!(DatabaseSound));
            } else {
                println!("{}", say!(DatabaseProblems, s.problems));
            }
            println!("{}", say!(ChatCounts, s.chats, s.one_to_one, s.groups));
            println!("{}", say!(MessagesCount, s.messages));
            println!("{}", say!(Dates, date(s.first_ms), date(s.last_ms)));
            println!("sha256: {}", s.sha256);
        }
        Command::Chats { backup, names } => {
            let s = open.check(&backup, Message::KeyBackup)?;
            let head = [say!(HeadChat), say!(HeadWho), say!(HeadMessages), say!(HeadLastMessage)];
            println!("{:>5}  {:<20} {:>9}  {:<12}", head[0], head[1], head[2], head[3]);
            for c in &s.chat_list {
                print!("{:>5}  {:<20} {:>9}  {:<12}", c.number, who(c.kind, &c.ending), c.messages, date(c.last_ms));
                match (&c.name, names) {
                    (Some(name), true) => println!("  {name}"),
                    _ => println!(),
                }
            }
        }
        Command::Stats { backup, chat } => {
            let id = open.indexed(&backup)?;
            let s = open.session.stats(id, chat)?;
            println!("{}", say!(MessagesSentReceived, s.messages, s.from_me, s.from_others));
            println!("{}", say!(Dates, date(s.first_ms), date(s.last_ms)));
            println!("{}", say!(VoiceMinutes, s.audio_seconds / 60));
            println!("{}", say!(ByType));
            for (code, n, bytes) in &s.by_kind {
                println!("    {}", say!(TypeMessagesMedia, code, n, bytes / 1_000_000));
            }
            println!("{}", say!(ByMonth));
            for (month, n) in &s.by_month {
                println!("    {month}: {n}");
            }
            println!("{}", say!(ByHourHere, s.by_hour.iter().map(u64::to_string).collect::<Vec<_>>().join(" ")));
            println!("{}", say!(ByDayHourHere));
            for (day, hours) in days().iter().zip(&s.by_day_hour) {
                println!("    {day}: {}", hours.iter().map(u64::to_string).collect::<Vec<_>>().join(" "));
            }
            if !s.senders.is_empty() {
                println!("{}", say!(WhoWritesMost));
                for (ending, n) in &s.senders {
                    println!("    ••••{ending}: {n}");
                }
            }
        }
        Command::Search { backup, query, chat, kind, from, after, before } => {
            let id = open.indexed(&backup)?;
            let (from_me, sender) = match from.as_deref() {
                None => (None, None),
                Some("me") => (Some(true), None),
                Some("them") => (Some(false), None),
                Some(ending) => (Some(false), Some(ending.to_owned())),
            };
            let filter = oopz_app::Filter {
                chat,
                kinds: kind,
                from_me,
                sender,
                after_ms: after.map(day_ms).transpose()?,
                before_ms: before.map(day_ms).transpose()?,
            };
            let hits = open.session.search(id, &query, &filter, session::SEARCH_LIMIT)?;
            let head = [say!(HeadChat), say!(HeadDate), say!(HeadSide), say!(HeadPosition)];
            println!("{:>5}  {:<10}  {:<8}  {:>9}", head[0], head[1], head[2], head[3]);
            for h in &hits {
                let position =
                    open.session.position(id, h.id)?.map_or_else(|| "-".into(), |(_, n)| (n + 1).to_string());
                let side = if h.from_me { say!(Sent) } else { say!(Received) };
                println!("{:>5}  {:<10}  {:<8}  {:>9}", h.chat, date(Some(h.ts)), side, position);
            }
            if hits.len() as u32 == session::SEARCH_LIMIT {
                println!("{}", say!(ResultsFirstOnes, hits.len()));
            } else {
                println!("{}", say!(Results, hits.len()));
            }
        }
        Command::Thumbnail { backup, message, media, size, out } => {
            let id = open.check(&backup, Message::KeyBackup)?.id;
            match open.session.thumbnail(id, message, &media, size.clamp(16, 1024))? {
                Some(jpeg) => {
                    std::fs::OpenOptions::new().write(true).create_new(true).open(&out)?.write_all(&jpeg)?;
                    println!("{}", say!(SavedBytes, out.display(), jpeg.len()));
                }
                None => anyhow::bail!(say!(NoPicture, message, media.display())),
            }
        }
        Command::Jump { backup, chat, date } => {
            let id = open.indexed(&backup)?;
            match open.session.position_at(id, chat, day_ms(date)?)? {
                Some(at) => println!("{}", say!(PositionOf, at + 1, open.session.chat_len(id, chat)?)),
                None => println!("{}", say!(ChatEmpty, chat)),
            }
        }
        Command::Export { backup, chat, out_dir } => {
            let id = open.indexed(&backup)?;
            let heading = match chat {
                Some(n) => say!(HeadingChat, n),
                None => say!(HeadingWholeBackup),
            };
            let page = open.session.export(id, chat, &Labels::in_language(say::language(), &heading), &out_dir)?;
            println!("{}", say!(Saved, page.display()));
            println!("{}", say!(KeepTextPrivate));
        }
        Command::Missing { a, chat_a, b, chat_b } => {
            let a = open.check(&a, Message::KeyFirstBackup)?.id;
            let b = open.check(&b, Message::KeySecondBackup)?.id;
            let m = open.session.missing(a, chat_a, b, chat_b)?;
            println!("{}", say!(InBoth, m.both));
            println!("{}", say!(OnlyInFirst, m.only_a, date(m.only_a_first_ms), date(m.only_a_last_ms)));
            println!("{}", say!(OnlyInSecond, m.only_b, date(m.only_b_first_ms), date(m.only_b_last_ms)));
        }
        Command::Restore { source, source_chat, target, target_chat, out_dir, media } => {
            anyhow::ensure!(out_dir.is_dir(), say!(NotAFolder, out_dir.display()));
            let source = open.check(&source, Message::KeySource)?.id;
            let target_key = read_key_text(Message::KeyTarget)?;
            let target = open.check_with(&target, &target_key)?.id;
            let done = open.session.merge(&MergeRequest {
                source,
                source_chat,
                target,
                target_chat,
                target_key: &target_key,
                out_parent: &out_dir,
                media: media.as_deref(),
            })?;
            println!("{}", say!(RestoreCounts, done.target_before, done.already_there, done.added));
            for check in &done.checks {
                println!("{}", say!(CheckLine, check));
            }
            if let (Some(copied), Some(missing)) = (done.media_copied, done.media_missing) {
                println!("{}", say!(MediaCopiedMissing, copied, missing));
            }
            println!("{}", if done.original_untouched { say!(OriginalUnchanged) } else { say!(OriginalChanged) });
            println!("{}", say!(Written, done.backup.display(), done.sha256));
            println!("{}", say!(NextChatFacts, done.backup.display(), target_chat));
        }
        Command::Receive { out_dir, address } => {
            let (rx, info) = oopz_app::receive::start_in(&out_dir, address)?;
            println!("{}", rx.qr_text()?);
            println!("{}", say!(ScanOrType));
            println!("{}", info.url);
            if info.addresses.len() > 1 {
                println!("{}", say!(SeveralAddresses));
                for a in &info.addresses {
                    let mark = if a.ip == info.ip { "*" } else { " " };
                    let card = if a.card.is_empty() { String::new() } else { format!("  {}", a.card) };
                    let virtual_ = if a.likely_virtual { format!("  {}", say!(Virtual)) } else { String::new() };
                    println!("  {mark} {}{card}{virtual_}", a.ip);
                }
            }
            println!("{}", say!(HomeWifi));
            println!("{}", say!(FilesGoTo, info.dest.display(), info.free / 1_000_000_000));
            let started = std::time::Instant::now();
            let (mut shown, mut said_opened, mut said_tips) = (0, false, false);
            loop {
                let status = rx.status();
                if status.opened && !said_opened {
                    said_opened = true;
                    println!("{}", say!(PhoneOpened));
                }
                if !status.opened && !said_tips && started.elapsed() >= oopz_app::receive::TIPS_AFTER {
                    said_tips = true;
                    println!("{}", say!(WifiTips));
                }
                for f in &status.files[shown..] {
                    println!("{}", say!(ReceivedFile, f.name, f.bytes, f.sha256));
                }
                shown = status.files.len();
                if let Some(p) = &status.current {
                    let speed = format!("{:.1}", p.bytes_per_sec as f64 / 1e6);
                    let line = say!(Receiving, p.name, p.done / 1_000_000, p.total / 1_000_000, speed);
                    eprint!("\r{line}{}   ", time_left(p.secs_left));
                    let _ = io::stderr().flush();
                }
                if let Some(why) = status.stopped {
                    println!("{}", say!(ReceiveStopped, why, status.files.len(), info.dest.display()));
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        Command::Types { backup, chat } => {
            let id = open.check(&backup, Message::KeyBackup)?.id;
            for p in open.session.types(id, chat)? {
                println!("{}", say!(TypeMessagesText, p.code, p.messages, p.with_text));
                for (table, n) in &p.linked {
                    println!("    {}", say!(LinkedTable, table, n));
                }
                for (mime, n) in p.mimes.iter().take(3) {
                    println!("    {}", say!(MediaMime, mime, n));
                }
            }
        }
        Command::ChatFacts { backup, chat } => {
            let id = open.check(&backup, Message::KeyBackup)?.id;
            for f in open.session.chat_facts(id, chat)? {
                println!("{}: {}", f.label, f.value);
            }
        }
        Command::Tables { backup } => {
            let id = open.check(&backup, Message::KeyBackup)?.id;
            for t in open.session.tables(id)? {
                println!("{} ({}): {}", t.name, t.rows, t.columns.join(", "));
            }
        }
    }
    Ok(())
}

/// A chat's kind (oopz-app's session names them) and the last 3 characters of its id, never the whole number.
fn who(kind: &str, ending: &str) -> String {
    match kind {
        "contact" => say!(WhoContact, ending),
        "contact_lid" => say!(WhoContactLid, ending),
        "group" => say!(WhoGroup, ending),
        "broadcast" => say!(WhoBroadcast, ending),
        _ => say!(WhoOther, ending),
    }
}

/// The days of the week, Sunday first: a by-day table's rows.
fn days() -> [String; 7] {
    [say!(DaySun), say!(DayMon), say!(DayTue), say!(DayWed), say!(DayThu), say!(DayFri), say!(DaySat)]
}

/// `, 3:05 left` for a progress line, or nothing when it isn't known yet.
fn time_left(secs: Option<u64>) -> String {
    secs.map_or_else(String::new, |s| format!(", {}", say!(TimeLeft, format!("{}:{:02}", s / 60, s % 60))))
}

/// From a hidden prompt, or one line of stdin when it isn't a terminal (`oopz … < keyfile`). Never from an argument
/// or the environment: both leak into shell history and process lists. Piped, one line per key, in the order asked.
fn read_key_text(which: Message) -> Result<Zeroizing<String>> {
    read_secret(&Note::of(which, []).text(say::language()))
}

/// [`read_key_text`] for any secret: `what` names it in the prompt.
fn read_secret(what: &str) -> Result<Zeroizing<String>> {
    if io::stdin().is_terminal() {
        Ok(Zeroizing::new(rpassword::prompt_password(format!("{}: ", say!(NotShown, what)))?))
    } else {
        let mut line = Zeroizing::new(String::new());
        io::stdin().lock().read_line(&mut line)?;
        Ok(line)
    }
}

/// Midnight UTC of `day`, in milliseconds.
fn day_ms(day: jiff::civil::Date) -> Result<i64> {
    Ok(day.to_zoned(jiff::tz::TimeZone::UTC)?.timestamp().as_millisecond())
}

fn date(ms: Option<i64>) -> String {
    ms.and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok())
        .map_or_else(|| "-".into(), |t| t.to_zoned(jiff::tz::TimeZone::UTC).date().to_string())
}
