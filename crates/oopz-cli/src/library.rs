//! `oopz lib …`: the library of every app's history (decision 0016), from a terminal. The same `oopz-core` and
//! `oopz-export` calls as the app's library screens (docs/parity.md). Like the rest of the CLI it prints counts, ids,
//! dates and positions; names only with `--names`; never message text, numbers, emails or keys. Text goes only into a
//! file the person names (`oopz lib export`).
//!
//! The library is a folder the person names with `--library`: nothing is kept anywhere else.

use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand, ValueEnum};
use oopz_core::{Filter, Library, Order, Selection};
use oopz_model::{App, Kind, Secret};

use crate::{date, day_ms, read_secret};

#[derive(Args)]
pub struct Lib {
    /// The library's folder (created when missing). It holds your messages unencrypted: keep it private, delete it
    /// when done.
    #[arg(long, global = true)]
    library: Option<PathBuf>,
    #[command(subcommand)]
    command: LibCommand,
}

/// What a filter narrows to (every flag optional; lists take commas or the flag again).
#[derive(Args, Default)]
struct FilterArgs {
    /// Apps by code: whatsapp, telegram, sms, line, viber…
    #[arg(long, value_delimiter = ',')]
    app: Vec<String>,
    /// Source ids (`oopz lib sources`).
    #[arg(long, value_delimiter = ',')]
    source: Vec<i64>,
    /// Conversation ids (`oopz lib convs`).
    #[arg(long, value_delimiter = ',')]
    conv: Vec<i64>,
    /// Contact ids (`oopz lib contacts`): their messages in groups and their one-to-one conversations.
    #[arg(long, value_delimiter = ',')]
    contact: Vec<i64>,
    /// Person ids (`oopz lib people`), the same way.
    #[arg(long, value_delimiter = ',')]
    person: Vec<i64>,
    /// Message kinds: text, link, photo, video, video_note, gif, voice, audio, sticker, document, location, contact,
    /// poll, call, album, sticker_pack, event, payment, game, business, story, system, deleted, unknown.
    #[arg(long, value_delimiter = ',')]
    kind: Vec<String>,
    /// `me` (sent) or `them` (received).
    #[arg(long)]
    from: Option<String>,
    /// From this day on (YYYY-MM-DD, UTC).
    #[arg(long)]
    after: Option<jiff::civil::Date>,
    /// Before this day (YYYY-MM-DD, UTC).
    #[arg(long)]
    before: Option<jiff::civil::Date>,
    /// Words to find (accents and case ignored).
    #[arg(long, default_value = "")]
    words: String,
}

impl FilterArgs {
    fn filter(&self) -> Result<Filter> {
        let apps = self
            .app
            .iter()
            .map(|a| App::from_code(a).with_context(|| format!("unknown app {a:?} (`oopz lib adapters` lists them)")))
            .collect::<Result<_>>()?;
        let kinds = self
            .kind
            .iter()
            .map(|k| {
                Kind::from_code(k).with_context(|| {
                    let all = Kind::ALL.map(Kind::code).join(", ");
                    format!("unknown kind {k:?} (one of: {all})")
                })
            })
            .collect::<Result<_>>()?;
        let from_me = match self.from.as_deref() {
            None => None,
            Some("me") => Some(true),
            Some("them") => Some(false),
            Some(other) => bail!("--from is `me` or `them`, not {other:?}"),
        };
        Ok(Filter {
            apps,
            sources: self.source.clone(),
            conversations: self.conv.clone(),
            contacts: self.contact.clone(),
            people: self.person.clone(),
            kinds,
            from_me,
            after_ms: self.after.map(day_ms).transpose()?,
            before_ms: self.before.map(day_ms).transpose()?,
            words: self.words.clone(),
        })
    }
}

/// A saved selection, or a filter given with flags.
#[derive(Args)]
struct Pick {
    /// A saved selection's id (`oopz lib selections`) instead of filter flags.
    #[arg(long, conflicts_with_all = ["app", "source", "conv", "contact", "person", "kind", "from", "after", "before", "words"])]
    selection: Option<i64>,
    /// Everything in the library (a filter with no flags selects nothing, so an export is never everything by
    /// accident).
    #[arg(long, conflicts_with = "selection")]
    all: bool,
    #[command(flatten)]
    filter: FilterArgs,
}

impl Pick {
    fn selection(&self, lib: &Library) -> Result<Selection> {
        match self.selection {
            Some(id) => Ok(lib
                .selections()?
                .into_iter()
                .find(|s| s.id == id)
                .with_context(|| format!("no saved selection {id}"))?
                .selection),
            None => {
                let mut filter = self.filter.filter()?;
                if self.all {
                    let mut apps: Vec<App> = lib.sources()?.into_iter().map(|s| s.app).collect();
                    apps.sort();
                    apps.dedup();
                    ensure!(!apps.is_empty(), "the library is empty");
                    filter.apps = apps;
                } else {
                    ensure!(
                        !filter.is_empty(),
                        "name what to take: --all, --selection, or filter flags (--app, --source, --conv…)"
                    );
                }
                Ok(Selection { filter, ..Default::default() })
            }
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum OrderArg {
    /// Conversation by conversation.
    Conversation,
    /// Every conversation together by time.
    Time,
}

#[derive(Clone, Copy, ValueEnum)]
enum PageArg {
    A4,
    Letter,
}

#[derive(Subcommand)]
enum LibCommand {
    /// The apps and formats oopz can read, and whether each asks for a key or password.
    Adapters,
    /// Add an export or a backup to the library (the app is detected; a key or password is asked for when needed).
    /// The same file twice is refused; a stop or a failure leaves the library as it was.
    Import {
        /// The export's file or folder, or the backup; only read.
        path: PathBuf,
        /// An adapter id from `oopz lib adapters`, when detection picks the wrong one.
        #[arg(long)]
        adapter: Option<String>,
    },
    /// The sources in the library: app, counts, dates and notes on what the source couldn't hold.
    Sources,
    /// Remove a source, its messages and its files from the library.
    Remove {
        #[arg(long)]
        source: i64,
    },
    /// Say which person is you in a source whose owner isn't known (text exports): their messages become yours.
    SetMe {
        /// The person's id from `oopz lib people`.
        #[arg(long)]
        person: i64,
    },
    /// Conversations across every source: id, app, kind, messages, last message date.
    Convs {
        #[command(flatten)]
        filter: FilterArgs,
        /// Also print each conversation's name.
        #[arg(long)]
        names: bool,
    },
    /// Where a date falls in a conversation (its position).
    Jump {
        #[arg(long)]
        conv: i64,
        /// YYYY-MM-DD (UTC).
        #[arg(long)]
        date: jiff::civil::Date,
    },
    /// Search every app at once. Prints where each match is (conversation, date, app, side, position), never the
    /// text.
    Search {
        /// The words to find.
        query: String,
        #[command(flatten)]
        filter: FilterArgs,
        #[arg(long, default_value_t = 200)]
        limit: u32,
    },
    /// Every app's messages merged by time: date, app, conversation, side and kind of each, never the text.
    Timeline {
        #[command(flatten)]
        filter: FilterArgs,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
    /// How many messages a filter or a saved selection holds.
    Count {
        #[command(flatten)]
        pick: Pick,
    },
    /// Numbers across apps: by month, hour, kind, app and person.
    Stats {
        #[command(flatten)]
        filter: FilterArgs,
        /// Also print the names of who writes most.
        #[arg(long)]
        names: bool,
    },
    /// The people in each source: id, app, messages, linked contact (names with `--names`; never numbers).
    People {
        #[arg(long)]
        source: Option<i64>,
        #[arg(long)]
        names: bool,
    },
    /// Contacts: people linked across apps, and contacts from vCards.
    Contacts {
        #[arg(long)]
        names: bool,
    },
    /// People in different apps who share a number or email (the number itself isn't printed).
    Suggest {
        #[arg(long)]
        names: bool,
    },
    /// Accept a suggestion from `oopz lib suggest` by its number: its people become one contact.
    Accept { number: usize },
    /// Link people (from any apps) into one contact.
    Link {
        #[arg(long, value_delimiter = ',', required = true)]
        person: Vec<i64>,
        /// The contact's name; the people's own otherwise.
        #[arg(long)]
        name: Option<String>,
    },
    /// Take a person out of their contact.
    Unlink {
        #[arg(long)]
        person: i64,
    },
    /// Rename a contact.
    Rename {
        #[arg(long)]
        contact: i64,
        #[arg(long)]
        name: String,
    },
    /// Delete a contact (its people stay, unlinked).
    Forget {
        #[arg(long)]
        contact: i64,
    },
    /// Add contacts from a .vcf file: people with the same number or email are linked to them.
    VcardImport { file: PathBuf },
    /// Write contacts to a new .vcf file in a folder.
    VcardExport {
        #[arg(long, value_delimiter = ',', required = true)]
        contact: Vec<i64>,
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Save a filter (plus messages added or taken out by id) as a named selection.
    Select {
        #[arg(long)]
        name: String,
        #[command(flatten)]
        filter: FilterArgs,
        /// Message ids to add.
        #[arg(long, value_delimiter = ',')]
        add: Vec<i64>,
        /// Message ids to leave out.
        #[arg(long, value_delimiter = ',')]
        remove: Vec<i64>,
    },
    /// The saved selections and how many messages each holds.
    Selections,
    /// Delete a saved selection.
    Unselect {
        #[arg(long)]
        selection: i64,
    },
    /// Write a filter's or a selection's messages to a new file: html, json, csv, txt, vcard, media-zip, pdf.
    Export {
        #[arg(long)]
        format: String,
        /// An existing folder; the file never replaces another.
        #[arg(long)]
        out_dir: PathBuf,
        #[command(flatten)]
        pick: Pick,
        #[arg(long, value_enum, default_value = "conversation")]
        order: OrderArg,
        /// An IANA time zone (Europe/Madrid); this computer's otherwise.
        #[arg(long)]
        tz: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// Leave out system lines (joined, changed the subject…).
        #[arg(long)]
        skip_system: bool,
        /// PDF: the page size.
        #[arg(long, value_enum, default_value = "a4")]
        page: PageArg,
        /// PDF: no photo thumbnails.
        #[arg(long)]
        no_media: bool,
        /// PDF: no cover page.
        #[arg(long)]
        no_cover: bool,
        /// PDF: no contents page.
        #[arg(long)]
        no_contents: bool,
        /// PDF: a font file or folder to use before the system's (again for more).
        #[arg(long)]
        font: Vec<PathBuf>,
    },
}

fn side(from_me: bool) -> &'static str {
    if from_me { "sent" } else { "received" }
}

/// A name for `--names`. A source that has no name for someone names them by their number or address: that is masked
/// to its last 3 characters, as the rest of the CLI does.
fn name_or_dash(name: Option<&str>, show: bool) -> String {
    if !show {
        return String::new();
    }
    let Some(name) = name else { return "-".into() };
    let digits = name.chars().filter(char::is_ascii_digit).count();
    let number_like = digits >= 5 && name.chars().all(|c| c.is_ascii_digit() || " +-().".contains(c));
    if number_like || name.contains('@') {
        let tail: String = name.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
        format!("••••{tail}")
    } else {
        name.to_owned()
    }
}

pub fn run(lib_args: Lib) -> Result<()> {
    let dir = lib_args.library.context("--library DIR names the library's folder")?;
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut lib = Library::open(&dir)?;
    match lib_args.command {
        LibCommand::Adapters => {
            for a in oopz_app::adapters() {
                let secret = match a.secret() {
                    Secret::None => "",
                    Secret::Key => "  asks for the 64-digit key",
                    Secret::Password => "  asks for the backup password",
                };
                println!("{:<18} {:<10} {}{secret}", a.id(), a.app().code(), a.app().name());
            }
        }
        LibCommand::Import { path, adapter } => {
            let adapters = oopz_app::adapters();
            let picked = oopz_core::pick_adapter(&adapters, &path, adapter.as_deref())?;
            let secret = match picked.secret_for(&path) {
                Secret::None => None,
                Secret::Key => Some(read_secret("64-digit backup key")?),
                Secret::Password => Some(read_secret("backup password")?),
            };
            eprintln!("reading {} as {}", path.display(), picked.id());
            let row = lib.import_path(
                &adapters,
                &path,
                Some(picked.id()),
                secret.as_deref().map(|s| s.trim()),
                &mut |done, total| {
                    eprint!("\r{done} of {total} conversations   ");
                    true
                },
            )?;
            eprintln!();
            println!(
                "source {}: {} conversations, {} people, {} messages, {} to {}",
                row.id,
                row.conversations,
                row.people,
                row.messages,
                date(row.first_ms),
                date(row.last_ms)
            );
            for note in &row.notes {
                println!("note: {note}");
            }
            if !row.owner_known {
                println!("next: oopz lib people --source {} --names, then oopz lib set-me --person ID", row.id);
            }
        }
        LibCommand::Sources => {
            println!(
                "{:>4}  {:<10} {:<18} {:>6} {:>7} {:>9}  {:<10}  {:<10}",
                "id", "app", "adapter", "convs", "people", "messages", "first", "last"
            );
            for s in lib.sources()? {
                println!(
                    "{:>4}  {:<10} {:<18} {:>6} {:>7} {:>9}  {:<10}  {:<10}{}{}",
                    s.id,
                    s.app.code(),
                    s.adapter,
                    s.conversations,
                    s.people,
                    s.messages,
                    date(s.first_ms),
                    date(s.last_ms),
                    if s.wall_clock { "  times as the phone showed them" } else { "" },
                    if s.owner_known { "" } else { "  owner unknown (set-me)" },
                );
                if let Some(format) = &s.format {
                    println!("      format: {format}");
                }
                for note in &s.notes {
                    println!("      note: {note}");
                }
            }
        }
        LibCommand::Remove { source } => {
            lib.remove_source(source)?;
            println!("removed source {source}");
        }
        LibCommand::SetMe { person } => {
            lib.set_me(person)?;
            println!("person {person} is you in their source");
        }
        LibCommand::Convs { filter, names } => {
            println!("{:>6}  {:>4}  {:<10} {:<8} {:>9}  {:<10}", "conv", "src", "app", "kind", "messages", "last");
            for c in lib.conversations(&filter.filter()?)? {
                let name = c.name.clone().or_else(|| (!c.members.is_empty()).then(|| c.members.join(", ")));
                println!(
                    "{:>6}  {:>4}  {:<10} {:<8} {:>9}  {:<10}  {}",
                    c.id,
                    c.source,
                    c.app.code(),
                    c.kind.code(),
                    c.messages,
                    date(c.last_ms),
                    name_or_dash(name.as_deref(), names)
                );
            }
        }
        LibCommand::Jump { conv, date: day } => match lib.position_at(conv, day_ms(day)?)? {
            Some(at) => println!("position {} of {}", at + 1, lib.conversation_len(conv)?),
            None => println!("conversation {conv} has no messages"),
        },
        LibCommand::Search { query, filter, limit } => {
            let mut f = filter.filter()?;
            ensure!(!query.trim().is_empty(), "give words to find");
            f.words = query;
            let hits = lib.search(&f, limit.clamp(1, 10_000))?;
            println!("{:>6}  {:<10}  {:<10} {:<8}  {:>9}", "conv", "date", "app", "side", "position");
            for h in &hits {
                let position = lib.position(h.id)?.map_or_else(|| "-".into(), |(_, n)| (n + 1).to_string());
                println!(
                    "{:>6}  {:<10}  {:<10} {:<8}  {:>9}",
                    h.conversation,
                    date(h.time_ms),
                    h.app.code(),
                    side(h.from_me),
                    position
                );
            }
            let more = if hits.len() as u32 == limit { " (the first ones; narrow the search)" } else { "" };
            println!("{} results, newest first{more}", hits.len());
        }
        LibCommand::Timeline { filter, offset, limit } => {
            let f = filter.filter()?;
            let rows = lib.find(&f, Order::Time, offset, limit.clamp(1, 10_000))?;
            println!("{:<10}  {:<10} {:>6}  {:<8}  {:<9}", "date", "app", "conv", "side", "kind");
            for r in &rows {
                println!(
                    "{:<10}  {:<10} {:>6}  {:<8}  {:<9}",
                    date(r.time_ms),
                    r.app.code(),
                    r.conversation,
                    side(r.from_me),
                    r.kind.code()
                );
            }
            println!("{} to {} of {}", offset + 1, offset + rows.len() as u64, lib.count(&f)?);
        }
        LibCommand::Count { pick } => {
            let selection = pick.selection(&lib)?;
            let n = if selection.add.is_empty() && selection.remove.is_empty() {
                lib.count(&selection.filter)?
            } else {
                lib.selected(&selection, Order::Conversation)?.len() as u64
            };
            println!("{n} messages");
        }
        LibCommand::Stats { filter, names } => {
            let s = lib.stats(&filter.filter()?)?;
            println!("messages: {} ({} sent, {} received)", s.messages, s.from_me, s.messages - s.from_me);
            println!("dates: {} to {}", date(s.first_ms), date(s.last_ms));
            println!("voice notes and audio: {} minutes", s.voice_secs / 60);
            println!("media: {} MB", s.media_bytes / 1_000_000);
            println!("by app:");
            for (app, n) in &s.by_app {
                println!("    {}: {n}", app.code());
            }
            println!("by kind:");
            for (kind, n, bytes) in &s.by_kind {
                println!("    {}: {n} messages, {} MB of media", kind.code(), bytes / 1_000_000);
            }
            println!("by month:");
            for (month, n) in &s.by_month {
                println!("    {month}: {n}");
            }
            println!("by hour: {}", s.by_hour.iter().map(u64::to_string).collect::<Vec<_>>().join(" "));
            println!("by day and hour (rows Sunday to Saturday, columns 0 to 23 h):");
            for (day, hours) in ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].iter().zip(&s.by_day_hour) {
                println!("    {day}: {}", hours.iter().map(u64::to_string).collect::<Vec<_>>().join(" "));
            }
            println!("who writes most:");
            for (person, name, n) in &s.by_person {
                let who = person.map_or_else(|| "-".into(), |p| format!("person {p}"));
                println!("    {who}: {n}  {}", name_or_dash(name.as_deref(), names));
            }
        }
        LibCommand::People { source, names } => {
            println!("{:>6}  {:>4}  {:<10} {:>9}  {:>7}", "person", "src", "app", "messages", "contact");
            for p in lib.people(source)? {
                println!(
                    "{:>6}  {:>4}  {:<10} {:>9}  {:>7}{}  {}",
                    p.id,
                    p.source,
                    p.app.code(),
                    p.messages,
                    p.contact.map_or_else(|| "-".into(), |c| c.to_string()),
                    if p.me { "  (you)" } else { "" },
                    name_or_dash(p.name.as_deref(), names)
                );
            }
        }
        LibCommand::Contacts { names } => {
            println!("{:>7}  {:>6}  {:>6}  {:>6}  {:<20}", "contact", "people", "phones", "emails", "apps");
            for c in lib.contacts()? {
                let mut apps: Vec<&str> = c.people.iter().map(|(_, a)| a.code()).collect();
                apps.dedup();
                println!(
                    "{:>7}  {:>6}  {:>6}  {:>6}  {:<20}  {}",
                    c.id,
                    c.people.len(),
                    c.phones.len(),
                    c.emails.len(),
                    apps.join(","),
                    name_or_dash(Some(&c.name), names)
                );
            }
        }
        LibCommand::Suggest { names } => {
            let list = lib.suggestions()?;
            for (i, s) in list.iter().enumerate() {
                let people: Vec<String> = s.people.iter().map(i64::to_string).collect();
                let into = s.contact.map_or_else(|| "a new contact".into(), |c| format!("contact {c}"));
                println!(
                    "{:>3}  same {}: people {} → {into}  {}",
                    i + 1,
                    s.reason,
                    people.join(", "),
                    name_or_dash(s.name.as_deref(), names)
                );
            }
            println!("{} suggestions; accept one with: oopz lib accept NUMBER", list.len());
        }
        LibCommand::Accept { number } => {
            let list = lib.suggestions()?;
            let s = number
                .checked_sub(1)
                .and_then(|i| list.get(i))
                .with_context(|| format!("no suggestion {number} (there are {})", list.len()))?;
            let id = lib.accept(s)?;
            println!("linked into contact {id}");
        }
        LibCommand::Link { person, name } => {
            let id = lib.link(&person, name.as_deref())?;
            println!("linked {} people into contact {id}", person.len());
        }
        LibCommand::Unlink { person } => {
            lib.unlink(person)?;
            println!("person {person} unlinked");
        }
        LibCommand::Rename { contact, name } => {
            lib.rename_contact(contact, &name)?;
            println!("contact {contact} renamed");
        }
        LibCommand::Forget { contact } => {
            lib.delete_contact(contact)?;
            println!("contact {contact} deleted");
        }
        LibCommand::VcardImport { file } => {
            let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let n = lib.import_vcards(&String::from_utf8_lossy(&bytes))?;
            println!("{n} contacts added or updated");
        }
        LibCommand::VcardExport { contact, out_dir } => {
            let w = oopz_export::export_contacts(&lib, &contact, &out_dir)?;
            println!("saved: {} ({} contacts)", w.path.display(), w.messages);
            println!("this file holds names and numbers: keep it private");
        }
        LibCommand::Select { name, filter, add, remove } => {
            let selection = Selection { filter: filter.filter()?, add, remove };
            let n = lib.selected(&selection, Order::Conversation)?.len();
            let id = lib.save_selection(&name, &selection)?;
            println!("selection {id} saved: {n} messages");
        }
        LibCommand::Selections => {
            for s in lib.selections()? {
                let n = lib.selected(&s.selection, Order::Conversation)?.len();
                println!("{:>4}  {:<10}  {n} messages  {}", s.id, date(Some(s.saved_ms)), s.name);
            }
        }
        LibCommand::Unselect { selection } => {
            lib.delete_selection(selection)?;
            println!("selection {selection} deleted");
        }
        LibCommand::Export {
            format,
            out_dir,
            pick,
            order,
            tz,
            title,
            skip_system,
            page,
            no_media,
            no_cover,
            no_contents,
            font,
        } => {
            let plain = match format.as_str() {
                "pdf" => None,
                code => Some(oopz_export::Format::from_code(code).with_context(|| {
                    format!("--format is one of html, json, csv, txt, vcard, media-zip, pdf; not {format:?}")
                })?),
            };
            let selection = pick.selection(&lib)?;
            let options = oopz_export::Options {
                order: match order {
                    OrderArg::Conversation => Order::Conversation,
                    OrderArg::Time => Order::Time,
                },
                time_zone: tz,
                // The app passes its own language's words; the CLI writes English.
                words: oopz_export::Words::default(),
                title,
                skip_system,
            };
            let mut progress = |done: u64, total: u64| {
                eprint!("\r{done} of {total}   ");
                true
            };
            let w = if let Some(f) = plain {
                let w = oopz_export::export(&lib, &selection, f, &options, &out_dir, &mut progress)?;
                if w.missing_files > 0 {
                    println!(
                        "{} files the messages name aren't in the source's folder (listed in the zip)",
                        w.missing_files
                    );
                }
                (w.path, w.messages, w.bytes)
            } else {
                export_pdf(&lib, &selection, options, page, no_media, no_cover, no_contents, font, &out_dir)?
            };
            eprintln!();
            println!("saved: {} ({} messages, {} bytes)", w.0.display(), w.1, w.2);
            println!("this file holds message text and isn't encrypted: keep it private, delete it when done");
        }
    }
    Ok(())
}

#[cfg(feature = "pdf")]
#[allow(clippy::too_many_arguments)]
fn export_pdf(
    lib: &Library,
    selection: &Selection,
    base: oopz_export::Options,
    page: PageArg,
    no_media: bool,
    no_cover: bool,
    no_contents: bool,
    fonts: Vec<PathBuf>,
    out_dir: &std::path::Path,
) -> Result<(PathBuf, u64, u64)> {
    use oopz_export_pdf::{MediaMode, PageSize, PdfOptions};
    let options = PdfOptions {
        base,
        page: match page {
            PageArg::A4 => PageSize::A4,
            PageArg::Letter => PageSize::Letter,
        },
        media: if no_media { MediaMode::None } else { MediaMode::Thumbnails },
        cover: !no_cover,
        contents: !no_contents,
        fonts,
    };
    let w = oopz_export_pdf::export_pdf(lib, selection, &options, out_dir, &mut |done, total| {
        eprint!("\r{done} of {total}   ");
        true
    })?;
    if w.missing_glyphs > 0 {
        println!("{} characters no font on this computer has (drawn as boxes); --font adds one", w.missing_glyphs);
    }
    Ok((w.path, w.messages, w.bytes))
}

#[cfg(not(feature = "pdf"))]
#[allow(clippy::too_many_arguments)]
fn export_pdf(
    _: &Library,
    _: &Selection,
    _: oopz_export::Options,
    _: PageArg,
    _: bool,
    _: bool,
    _: bool,
    _: Vec<PathBuf>,
    _: &std::path::Path,
) -> Result<(PathBuf, u64, u64)> {
    bail!("this oopz was built without PDF export (the `pdf` feature)")
}
