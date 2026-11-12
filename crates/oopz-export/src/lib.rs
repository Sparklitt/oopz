//! Exports of a selection from the library (decision 0016, point 8): HTML (one file, offline, printable), JSON (the
//! model, for other tools), CSV (one row per message), TXT, vCard (the people in it) and a zip of its media files.
//!
//! Every export is written to a temporary file in the folder the person picked and renamed to a new name only when
//! complete, so a stop or a failure leaves nothing behind and nothing is ever overwritten. The free space is checked
//! first. View-once content is never written. The files hold the person's messages, unencrypted: the app says so.

pub mod bindings;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};

use msgkinds::Note;
use oopz_core::{ContactRow, Error, Library, MessageRow, Order, Result, Selection};
use oopz_model::messages::{LANGUAGES, Message as Msg};
use oopz_model::{self as m, App, Kind, Part};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Html,
    Json,
    Csv,
    Txt,
    Vcard,
    MediaZip,
}

impl Format {
    pub const ALL: [Format; 6] =
        [Format::Html, Format::Json, Format::Csv, Format::Txt, Format::Vcard, Format::MediaZip];

    pub fn code(self) -> &'static str {
        match self {
            // The CLI names a file's format by its extension; contacts and the media zip by what they hold.
            Format::Html | Format::Json | Format::Csv | Format::Txt => self.extension(),
            Format::Vcard => "vcard",
            Format::MediaZip => "media-zip",
        }
    }

    pub fn from_code(code: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|f| f.code() == code)
    }

    /// What its file is (msgkinds' format).
    fn mime(self) -> &'static str {
        use msgkinds::media_type::formats::{CSV, HTML, JSON, PLAIN_TEXT, VCARD, ZIP};
        match self {
            Format::Html => HTML,
            Format::Json => JSON,
            Format::Csv => CSV,
            Format::Txt => PLAIN_TEXT,
            Format::Vcard => VCARD,
            Format::MediaZip => ZIP,
        }
    }

    fn extension(self) -> &'static str {
        msgkinds::media_type::by_mime(self.mime()).and_then(|f| f.extension()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Conversation by conversation, or one merged timeline.
    pub order: Order,
    /// An IANA zone (`Europe/Madrid`); `None` for this computer's.
    pub time_zone: Option<String>,
    /// The words the export is written in.
    pub words: Words,
    /// The document's title.
    pub title: Option<String>,
    /// Leave the apps' own notices out.
    pub skip_system: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            order: Order::Conversation,
            time_zone: None,
            words: Words::default(),
            title: None,
            skip_system: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Written {
    pub path: PathBuf,
    pub messages: u64,
    pub bytes: u64,
    /// Media files the selection names but the source's folder doesn't have (media zip only).
    pub missing_files: u64,
}

/// The export's own words, from the caller (the app's language files; the CLI uses English, the default).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Words {
    /// A BCP 47 code, for `<html lang>` and the PDF's language.
    pub lang: String,
    pub me: String,
    pub note: String,
    pub exported: String,
    pub deleted: String,
    pub edited: String,
    pub forwarded: String,
    pub view_once: String,
    pub omitted: String,
    pub reply: String,
    pub votes: String,
    pub call: String,
    pub video_call: String,
    pub location: String,
    pub live: String,
    pub unknown: String,
    /// After an app and its own code, for a type oopz doesn't know: "WhatsApp: 9999, not shown yet".
    pub not_shown: String,
    pub wall_clock: String,
    pub photo: String,
    pub video: String,
    pub video_note: String,
    pub gif: String,
    pub sticker: String,
    pub voice: String,
    pub audio: String,
    pub document: String,
    /// The PDF's contents page.
    pub contents: String,
    pub messages: String,
    /// A call's outcome by its code (`missed`), and an app notice with no text of its own by its event's code.
    pub outcomes: BTreeMap<String, String>,
    pub events: BTreeMap<String, String>,
    /// An album header, a sticker pack, a calendar event… by its card kind's code (`sticker_pack`).
    pub cards: BTreeMap<String, String>,
}

impl Default for Words {
    fn default() -> Self {
        Words::in_language(LANGUAGES[0])
    }
}

impl Words {
    /// The export's words in `lang`, from oopz's messages (`oopz_model::messages`): a language they aren't worded in
    /// gets the default language's words, and `lang` says which.
    pub fn in_language(lang: &str) -> Words {
        let lang = if LANGUAGES.contains(&lang) { lang } else { LANGUAGES[0] };
        let w = |m: Msg| Note::of(m, []).text(lang);
        // A call outcome's stored code, as `code_word` looks it up.
        let code = |o: m::CallOutcome| serde_json::to_value(o).ok().and_then(|v| v.as_str().map(str::to_owned));
        Words {
            lang: lang.into(),
            me: w(Msg::ExportMe),
            note: w(Msg::ExportNote),
            exported: w(Msg::ExportExported),
            deleted: w(Msg::ExportDeleted),
            edited: w(Msg::ExportEdited),
            forwarded: w(Msg::ExportForwarded),
            view_once: w(Msg::ExportViewOnce),
            omitted: w(Msg::ExportOmitted),
            reply: w(Msg::ExportReply),
            votes: w(Msg::ExportVotes),
            call: w(Msg::ExportCall),
            video_call: w(Msg::ExportVideoCall),
            location: w(Msg::ExportLocation),
            live: w(Msg::ExportLive),
            unknown: w(Msg::ExportUnknown),
            not_shown: w(Msg::ExportNotShown),
            wall_clock: w(Msg::ExportWallClock),
            photo: w(Msg::ExportPhoto),
            video: w(Msg::ExportVideo),
            video_note: w(Msg::ExportVideoNote),
            gif: w(Msg::ExportGif),
            sticker: w(Msg::ExportSticker),
            voice: w(Msg::ExportVoice),
            audio: w(Msg::ExportAudio),
            document: w(Msg::ExportDocument),
            contents: w(Msg::ExportContents),
            messages: w(Msg::ExportMessages),
            outcomes: m::CallOutcome::ALL.into_iter().filter_map(|o| Some((code(o)?, w(outcome_message(o))))).collect(),
            events: m::Event::ALL.into_iter().map(|e| (e.code().to_owned(), w(event_message(e)))).collect(),
            cards: m::CardKind::ALL
                .into_iter()
                .map(|c| (Kind::from(c).code().to_owned(), w(card_message(c))))
                .collect(),
        }
    }
}

fn outcome_message(o: m::CallOutcome) -> Msg {
    use m::CallOutcome as O;
    match o {
        O::Answered => Msg::OutcomeAnswered,
        O::Missed => Msg::OutcomeMissed,
        O::Declined => Msg::OutcomeDeclined,
        O::Cancelled => Msg::OutcomeCancelled,
        O::Busy => Msg::OutcomeBusy,
        O::Failed => Msg::OutcomeFailed,
        O::Unknown => Msg::OutcomeUnknown,
    }
}

/// An event with no text of its own.
fn event_message(e: m::Event) -> Msg {
    use m::Event as E;
    match e {
        E::Encryption => Msg::EventEncryption,
        E::SecurityCodeChanged => Msg::EventSecurityCodeChanged,
        E::NumberChanged => Msg::EventNumberChanged,
        E::DeviceChanged => Msg::EventDeviceChanged,
        E::AccountDeleted => Msg::EventAccountDeleted,
        E::ContactJoined => Msg::EventContactJoined,
        E::BusinessNotice => Msg::EventBusinessNotice,
        E::GroupCreated => Msg::EventGroupCreated,
        E::TopicCreated => Msg::EventTopicCreated,
        E::Renamed => Msg::EventRenamed,
        E::DescriptionChanged => Msg::EventDescriptionChanged,
        E::PictureChanged => Msg::EventPictureChanged,
        E::PictureRemoved => Msg::EventPictureRemoved,
        E::SettingsChanged => Msg::EventSettingsChanged,
        E::MemberAdded => Msg::EventMemberAdded,
        E::MemberJoined => Msg::EventMemberJoined,
        E::MemberLeft => Msg::EventMemberLeft,
        E::MemberRemoved => Msg::EventMemberRemoved,
        E::JoinRequested => Msg::EventJoinRequested,
        E::AdminAdded => Msg::EventAdminAdded,
        E::AdminRemoved => Msg::EventAdminRemoved,
        E::AdminChanged => Msg::EventAdminChanged,
        E::OwnerChanged => Msg::EventOwnerChanged,
        E::CommunityChanged => Msg::EventCommunityChanged,
        E::InviteLinkReset => Msg::EventInviteLinkReset,
        E::Disappearing => Msg::EventDisappearing,
        E::KeptInChat => Msg::EventKeptInChat,
        E::ChatLocked => Msg::EventChatLocked,
        E::Screenshot => Msg::EventScreenshot,
        E::Pinned => Msg::EventPinned,
        E::Unpinned => Msg::EventUnpinned,
        E::Blocked => Msg::EventBlocked,
        E::Unblocked => Msg::EventUnblocked,
        E::MessageDeleted => Msg::EventMessageDeleted,
        E::HistoryCleared => Msg::EventHistoryCleared,
        E::GroupCall => Msg::EventGroupCall,
        E::PollClosed => Msg::EventPollClosed,
        E::PaymentStatus => Msg::EventPaymentStatus,
        E::MissedMessages => Msg::EventMissedMessages,
        E::ThemeChanged => Msg::EventThemeChanged,
        E::ProfileChanged => Msg::EventProfileChanged,
        E::Other => Msg::EventOther,
    }
}

fn card_message(c: m::CardKind) -> Msg {
    use m::CardKind as C;
    match c {
        C::Album => Msg::CardAlbum,
        C::StickerPack => Msg::CardStickerPack,
        C::Event => Msg::CardEvent,
        C::Payment => Msg::CardPayment,
        C::Game => Msg::CardGame,
        C::Business => Msg::CardBusiness,
        C::Story => Msg::CardStory,
        C::Reaction => Msg::CardReaction,
    }
}

impl Words {
    fn kind(&self, kind: m::MediaKind) -> &str {
        use m::MediaKind as K;
        match kind {
            K::Photo => &self.photo,
            K::Video => &self.video,
            K::VideoNote => &self.video_note,
            K::Gif => &self.gif,
            K::Sticker => &self.sticker,
            K::Voice => &self.voice,
            K::Audio => &self.audio,
            K::Document => &self.document,
        }
    }

    /// The word for `value` (an enum's code) in `map`, else its code.
    fn code_word(map: &BTreeMap<String, String>, value: impl Serialize) -> String {
        let code = serde_json::to_value(value).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        map.get(&code).cloned().unwrap_or(code)
    }
}

/// Times in the person's zone; a wall-clock source's as written.
pub struct Clock {
    zone: jiff::tz::TimeZone,
}

impl Clock {
    pub fn new(name: Option<&str>) -> Result<Self> {
        let zone = match name {
            Some(n) => jiff::tz::TimeZone::get(n).map_err(|_| Error::of("library", Msg::NoTimeZone, [n.to_owned()]))?,
            None => jiff::tz::TimeZone::system(),
        };
        Ok(Clock { zone })
    }

    fn zoned(&self, ms: i64, wall_clock: bool) -> Option<jiff::Zoned> {
        let t = jiff::Timestamp::from_millisecond(ms).ok()?;
        Some(t.to_zoned(if wall_clock { jiff::tz::TimeZone::UTC } else { self.zone.clone() }))
    }

    /// `2024-01-31 21:05`
    pub fn time(&self, ms: Option<i64>, wall_clock: bool) -> String {
        ms.and_then(|ms| self.zoned(ms, wall_clock))
            .map_or_else(String::new, |z| z.strftime("%Y-%m-%d %H:%M").to_string())
    }

    /// ISO 8601 with the offset (`2024-01-31T21:05:00+01:00`), or without one for a wall-clock time.
    fn iso(&self, ms: Option<i64>, wall_clock: bool) -> String {
        let Some(z) = ms.and_then(|ms| self.zoned(ms, wall_clock)) else { return String::new() };
        if wall_clock {
            z.strftime("%Y-%m-%dT%H:%M:%S").to_string()
        } else {
            z.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()
        }
    }
}

/// A temporary file in `dir` that becomes `oopz-<what>-N.<ext>` when complete.
struct Out {
    file: tempfile::NamedTempFile,
    dir: PathBuf,
    stem: String,
    ext: &'static str,
}

impl Out {
    fn new(dir: &Path, stem: &str, ext: &'static str, need: u64) -> Result<Out> {
        let shown = dir.display().to_string();
        let io = |doing: Msg, e: std::io::Error| Error::io(Note::of(doing, [shown.clone()]), e);
        if !dir.is_dir() {
            return Err(Error::of("not_found", Msg::NoFolder, [shown.clone()]));
        }
        let free = fs4::available_space(dir).map_err(|e| io(Msg::DoingFreeSpaceIn, e))?;
        // Room for the file and a margin, so the disk isn't filled to the last byte.
        if free < need.saturating_add(16 << 20) {
            return Err(Error::of("no_space", Msg::NeedsFreeSpace, [shown.clone(), (need >> 20).to_string()]));
        }
        let file = tempfile::Builder::new()
            .prefix(".oopz-export-")
            .tempfile_in(dir)
            .map_err(|e| io(Msg::DoingWritingIn, e))?;
        Ok(Out { file, dir: dir.to_path_buf(), stem: stem.to_owned(), ext })
    }

    fn writer(&mut self) -> BufWriter<&mut fs::File> {
        BufWriter::with_capacity(1 << 16, self.file.as_file_mut())
    }

    /// Gives the file its name: the first `oopz-<stem>-N.<ext>` that doesn't exist (never replacing one).
    fn finish(self) -> Result<(PathBuf, u64)> {
        self.file.as_file().sync_all().map_err(write_err)?;
        let bytes = self.file.as_file().metadata().map(|m| m.len()).unwrap_or(0);
        let mut file = self.file;
        for n in 1..10_000 {
            let path = self.dir.join(format!("oopz-{}-{n}.{}", self.stem, self.ext));
            match file.persist_noclobber(&path) {
                Ok(_) => return Ok((path, bytes)),
                Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => file = e.file,
                Err(e) => return Err(Error::io(Note::of(Msg::DoingNaming, [path.display().to_string()]), e.error)),
            }
        }
        Err(Error::of("exists", Msg::TooManyExports, []))
    }
}

/// Writes `bytes` to a new `oopz-<stem>-N.<ext>` in `dir` (free space checked, never replacing a file, nothing left
/// behind on failure), for writers that build their file in memory (the PDF).
pub fn write_new_file(dir: &Path, stem: &str, ext: &'static str, bytes: &[u8]) -> Result<(PathBuf, u64)> {
    let mut out = Out::new(dir, stem, ext, bytes.len() as u64)?;
    {
        let mut f = out.writer();
        f.write_all(bytes).map_err(write_err)?;
        f.flush().map_err(write_err)?;
    }
    out.finish()
}

fn write_err(e: std::io::Error) -> Error {
    Error::io(Note::of(Msg::DoingWritingExport, []), e)
}

/// What the rows need besides themselves: conversation names, and the text a reply answers.
struct Context {
    conversations: HashMap<i64, (App, String)>,
    replies: HashMap<i64, String>,
}

fn context(lib: &Library, rows: &[MessageRow]) -> Result<Context> {
    let ids: BTreeSet<i64> = rows.iter().map(|r| r.source).collect();
    let mut conversations = HashMap::new();
    for c in lib.conversations(&oopz_core::Filter { sources: ids.into_iter().collect(), ..Default::default() })? {
        let name = c
            .name
            .clone()
            .or_else(|| (!c.members.is_empty()).then(|| c.members.join(", ")))
            .unwrap_or_else(|| format!("#{}", c.id));
        conversations.insert(c.id, (c.app, name));
    }
    let wanted: Vec<i64> = rows.iter().filter_map(|r| r.reply).collect::<BTreeSet<_>>().into_iter().collect();
    let replies =
        lib.messages_by_id(&wanted)?.into_iter().map(|r| (r.id, excerpt(&visible_text(&r.message), 90))).collect();
    Ok(Context { conversations, replies })
}

fn excerpt(s: &str, n: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= n { flat } else { format!("{}…", flat.chars().take(n).collect::<String>()) }
}

/// The message's text, without view-once captions (never kept).
fn visible_text(msg: &m::Message) -> String {
    if msg.parts.iter().any(|p| matches!(p, Part::Media(media) if media.view_once)) {
        String::new()
    } else {
        msg.text()
    }
}

fn size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{} kB", b >> 10),
        b => format!("{b} B"),
    }
}

/// One line per part, as plain text (TXT, CSV's `text` column for non-text parts).
fn part_lines(part: &Part, app: App, w: &Words) -> Vec<String> {
    match part {
        Part::Text { text } => vec![text.clone()],
        Part::Media(media) if media.view_once => vec![format!("[{}]", w.view_once)],
        Part::Media(media) => {
            let mut facts = vec![w.kind(media.kind).to_owned()];
            if let Some(n) = &media.name {
                facts.push(n.clone());
            }
            if let Some(s) = media.size {
                facts.push(size(s));
            }
            // A sticker, a photo or a document has no duration, whatever the source wrote (WhatsApp writes 0).
            if let Some(s) = media.secs.filter(|_| Kind::from(media.kind).has_duration()) {
                facts.push(format!("{}:{:02}", s / 60, s % 60));
            }
            if media.omitted {
                facts.push(w.omitted.to_owned());
            }
            let mut lines = vec![format!("[{}]", facts.join(" · "))];
            lines.extend(media.caption.clone());
            lines
        }
        Part::Location { lat, lng, name, live } => {
            let mut s = format!("[{}", if *live { &w.live } else { &w.location });
            if let (Some(a), Some(b)) = (lat, lng) {
                let _ = write!(s, " {a:.6}, {b:.6}");
            }
            if let Some(n) = name {
                let _ = write!(s, " · {n}");
            }
            vec![s + "]"]
        }
        Part::Contact { name, phones, .. } => {
            let mut v = vec![name.clone().unwrap_or_default()];
            v.extend(phones.iter().cloned());
            vec![format!("[vCard: {}]", v.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "))]
        }
        Part::Poll { question, options, .. } => {
            let mut lines = vec![format!("[{question}]")];
            lines.extend(options.iter().map(|o| format!("  ○ {} ({} {})", o.text, o.votes, w.votes)));
            lines
        }
        Part::Call { video, outcome, secs } => {
            let mut s = format!("[{}", if *video { &w.video_call } else { &w.call });
            let _ = write!(s, " · {}", Words::code_word(&w.outcomes, outcome));
            if let Some(secs) = secs {
                let _ = write!(s, " · {}:{:02}", secs / 60, secs % 60);
            }
            vec![s + "]"]
        }
        // A link's address is already in the text it came with, or stands alone.
        Part::Link { url, title } => vec![title.as_ref().map_or_else(|| url.clone(), |t| format!("{t} <{url}>"))],
        Part::System { text, event } => {
            vec![format!("— {} —", text.clone().unwrap_or_else(|| Words::code_word(&w.events, event)))]
        }
        Part::Card { kind, title, text } => {
            let mut head = Words::code_word(&w.cards, kind);
            if let Some(t) = title {
                let _ = write!(head, ": {t}");
            }
            let mut lines = vec![format!("[{head}]")];
            lines.extend(text.clone());
            lines
        }
        Part::Unknown { code, text } => {
            let label = format!("{}: {code}, {}", app.name(), w.not_shown);
            vec![format!("[{label}]{}", text.as_ref().map_or(String::new(), |t| format!(" {t}")))]
        }
    }
}

fn message_lines(row: &MessageRow, w: &Words) -> Vec<String> {
    let msg = &row.message;
    if msg.deleted {
        return vec![format!("[{}]", w.deleted)];
    }
    let mut lines: Vec<String> = Vec::new();
    for part in &msg.parts {
        if let Part::Link { url, .. } = part
            && msg.parts.iter().any(|p| matches!(p, Part::Text { text } if text.contains(url.as_str())))
        {
            continue;
        }
        lines.extend(part_lines(part, row.app, w));
    }
    lines
}

/// A message as lines of plain text in `w`, for other writers (the PDF).
pub fn row_lines(row: &MessageRow, w: &Words) -> Vec<String> {
    message_lines(row, w)
}

/// "You", or the sender's name.
pub fn sender_label(row: &MessageRow, w: &Words) -> String {
    sender(row, w)
}

/// The warning every export carries, and "Exported with oopz", in `w`.
pub fn note(w: &Words) -> (&str, &str) {
    (&w.note, &w.exported)
}

/// Conversation (app, name) by id.
pub type Names = HashMap<i64, (App, String)>;
/// Reply excerpts by the answered message's id.
pub type Replies = HashMap<i64, String>;

/// Conversation names by id for these rows, and the excerpts their replies quote.
pub fn names_and_replies(lib: &Library, rows: &[MessageRow]) -> Result<(Names, Replies)> {
    let c = context(lib, rows)?;
    Ok((c.conversations, c.replies))
}

fn sender(row: &MessageRow, w: &Words) -> String {
    if row.from_me { w.me.to_owned() } else { row.sender_name.clone().unwrap_or_default() }
}

/// Writes `format` of `selection` into a new file in `out_dir`. `progress(done, total)` counts messages; false stops
/// (nothing is left behind).
pub fn export(
    lib: &Library,
    selection: &Selection,
    format: Format,
    options: &Options,
    out_dir: &Path,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<Written> {
    let w = &options.words;
    let clock = Clock::new(options.time_zone.as_deref())?;
    let ids = lib.selected(selection, options.order)?;
    if ids.is_empty() {
        return Err(Error::of("not_found", Msg::SelectionEmpty, []));
    }
    match format {
        Format::Vcard => return vcards(lib, &ids, out_dir),
        Format::MediaZip => return media_zip(lib, &ids, out_dir, progress),
        _ => {}
    }
    let total = ids.len() as u64;
    // About 400 bytes a message in the larger formats.
    let mut out = Out::new(out_dir, format.code(), format.extension(), total * 400)?;
    let mut done = 0u64;
    {
        let mut f = out.writer();
        let title = options.title.clone().unwrap_or_else(|| "oopz".into());
        match format {
            Format::Html => html_head(&mut f, &title, w).map_err(write_err)?,
            Format::Json => json_head(&mut f, lib, &ids)?,
            Format::Csv => {
                writeln!(f, "app,conversation,time,from_me,sender,kind,text,reply_to,edited,deleted,forwarded")
                    .map_err(write_err)?
            }
            Format::Txt => writeln!(f, "{title}\n{}\n", w.note).map_err(write_err)?,
            Format::Vcard | Format::MediaZip => unreachable!(),
        }
        let mut last_conv = None;
        let mut first_json = true;
        for chunk in ids.chunks(1000) {
            let rows = lib.messages_by_id(chunk)?;
            let ctx = context(lib, &rows)?;
            for row in &rows {
                if options.skip_system && row.kind == Kind::System {
                    continue;
                }
                let merged = options.order != Order::Conversation;
                match format {
                    Format::Html => {
                        html_row(&mut f, row, &ctx, &clock, w, merged, &mut last_conv).map_err(write_err)?
                    }
                    Format::Json => {
                        f.write_all(if first_json { b"\n" } else { b",\n" }).map_err(write_err)?;
                        first_json = false;
                        json_row(&mut f, row)?;
                    }
                    Format::Csv => csv_row(&mut f, row, &ctx, &clock, w)?,
                    Format::Txt => txt_row(&mut f, row, &ctx, &clock, w, merged, &mut last_conv).map_err(write_err)?,
                    Format::Vcard | Format::MediaZip => unreachable!(),
                }
                done += 1;
            }
            if !progress(done, total) {
                return Err(m::Error::Cancelled.into());
            }
        }
        match format {
            Format::Html => f.write_all(b"</main></body></html>\n").map_err(write_err)?,
            Format::Json => f.write_all(b"\n]}\n").map_err(write_err)?,
            _ => {}
        }
        f.flush().map_err(write_err)?;
    }
    let (path, bytes) = out.finish()?;
    Ok(Written { path, messages: done, bytes, missing_files: 0 })
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            // Bidirectional overrides would reorder the page around them; zero-width ones are kept (they shape
            // emoji and joined scripts).
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => {}
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

const CSS: &str = "
:root{color-scheme:light dark;--bg:#f6f4f0;--fg:#1b1d1c;--muted:#5f6662;--me:#dcebe3;--them:#fff;--line:#d9d5ce;--accent:#2f5d4f}
@media (prefers-color-scheme:dark){:root{--bg:#1b1d1c;--fg:#ecebe8;--muted:#a3aaa6;--me:#28463c;--them:#262928;--line:#3a3e3c;--accent:#7fb8a3}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.45 system-ui,-apple-system,'Segoe UI',Roboto,'Noto Sans','Noto Color Emoji',sans-serif}
main{max-width:860px;margin:0 auto;padding:24px 16px 64px}header h1{font-size:22px;margin:0 0 4px}.note{color:var(--muted);font-size:13px;border:1px solid var(--line);border-radius:8px;padding:8px 12px}
h2{font-size:17px;margin:32px 0 8px;padding-bottom:6px;border-bottom:1px solid var(--line)}.app{display:inline-block;font-size:11px;font-weight:600;letter-spacing:.02em;color:var(--accent);border:1px solid currentColor;border-radius:999px;padding:0 7px;margin-inline-end:6px;vertical-align:1px}
.day{text-align:center;color:var(--muted);font-size:12px;margin:18px 0 6px}.m{display:flex;margin:3px 0}.m.me{justify-content:flex-end}
.b{max-width:78%;background:var(--them);border:1px solid var(--line);border-radius:12px;padding:6px 10px 4px;overflow-wrap:anywhere;white-space:pre-wrap}.me .b{background:var(--me)}
.who{font-size:12px;font-weight:600;color:var(--accent)}.t{font-size:11px;color:var(--muted);text-align:end;margin-top:2px}.q{border-inline-start:3px solid var(--accent);padding:2px 8px;margin:2px 0 4px;font-size:13px;color:var(--muted)}
.x{color:var(--muted);font-style:italic}.sys{text-align:center;color:var(--muted);font-size:12.5px;margin:8px 0}.r{font-size:12px;margin-top:2px}.cv{font-size:12px;color:var(--muted)}
@media print{:root{--bg:#fff;--fg:#000;--me:#eef5f1;--them:#fff}main{max-width:none;padding:0}.m{break-inside:avoid}h2{break-after:avoid}}
";

fn html_head(f: &mut impl Write, title: &str, w: &Words) -> std::io::Result<()> {
    write!(
        f,
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:\">\
         <meta name=\"generator\" content=\"oopz\"><title>{}</title><style>{CSS}</style></head><body><main>\
         <header><h1 dir=\"auto\">{}</h1><p class=\"note\">{} · {}</p></header>",
        w.lang,
        esc(title),
        esc(title),
        w.exported,
        w.note
    )
}

fn html_row(
    f: &mut impl Write,
    row: &MessageRow,
    ctx: &Context,
    clock: &Clock,
    w: &Words,
    merged: bool,
    last: &mut Option<(i64, String)>,
) -> std::io::Result<()> {
    let (app, conv) = ctx.conversations.get(&row.conversation).cloned().unwrap_or((row.app, String::new()));
    let day = clock.time(row.time_ms, row.wall_clock).get(..10).unwrap_or_default().to_owned();
    if !merged && last.as_ref().map(|l| l.0) != Some(row.conversation) {
        write!(f, "<h2 dir=\"auto\"><span class=\"app\">{}</span>{}</h2>", app.name(), esc(&conv))?;
        if row.wall_clock {
            write!(f, "<p class=\"cv\">{}</p>", w.wall_clock)?;
        }
        *last = None;
    }
    if last.as_ref().map(|l| l.1.as_str()) != Some(day.as_str()) && !day.is_empty() {
        write!(f, "<p class=\"day\">{day}</p>")?;
    }
    *last = Some((row.conversation, day));
    let time = clock.time(row.time_ms, row.wall_clock);
    if row.kind == Kind::System {
        let text = message_lines(row, w).join(" ");
        return write!(
            f,
            "<p class=\"sys\" dir=\"auto\">{} <span class=\"t\">{}</span></p>",
            esc(text.trim_matches(['—', ' '])),
            esc(&time)
        );
    }
    write!(f, "<div class=\"m{}\"><div class=\"b\" dir=\"auto\">", if row.from_me { " me" } else { "" })?;
    if merged {
        write!(f, "<div class=\"cv\"><span class=\"app\">{}</span>{}</div>", app.name(), esc(&conv))?;
    }
    if !row.from_me {
        write!(f, "<div class=\"who\">{}</div>", esc(&sender(row, w)))?;
    }
    if row.message.forwarded {
        write!(f, "<div class=\"x\">{}</div>", w.forwarded)?;
    }
    if let Some(quoted) = row.reply.and_then(|r| ctx.replies.get(&r)) {
        write!(f, "<div class=\"q\">{} · {}</div>", w.reply, esc(quoted))?;
    }
    let lines = message_lines(row, w);
    let deleted = row.message.deleted;
    for line in &lines {
        if deleted || line.starts_with('[') {
            write!(f, "<div class=\"x\">{}</div>", esc(line))?;
        } else {
            write!(f, "<div>{}</div>", esc(line))?;
        }
    }
    if !row.message.reactions.is_empty() {
        let r: String = row.message.reactions.iter().map(|r| r.emoji.as_str()).collect::<Vec<_>>().join(" ");
        write!(f, "<div class=\"r\">{}</div>", esc(&r))?;
    }
    let edited = if row.message.edited { format!(" · {}", w.edited) } else { String::new() };
    write!(f, "<div class=\"t\">{}{edited}</div></div></div>", esc(&time))
}

fn txt_row(
    f: &mut impl Write,
    row: &MessageRow,
    ctx: &Context,
    clock: &Clock,
    w: &Words,
    merged: bool,
    last: &mut Option<(i64, String)>,
) -> std::io::Result<()> {
    let (app, conv) = ctx.conversations.get(&row.conversation).cloned().unwrap_or((row.app, String::new()));
    if !merged && last.as_ref().map(|l| l.0) != Some(row.conversation) {
        writeln!(f, "\n== {} · {} ==", app.name(), conv)?;
        *last = Some((row.conversation, String::new()));
    }
    let prefix = if merged { format!("{} · {} · ", app.name(), conv) } else { String::new() };
    let mut lines = message_lines(row, w);
    if row.message.edited
        && let Some(last) = lines.last_mut()
    {
        last.push_str(&format!(" ({})", w.edited));
    }
    if let Some(q) = row.reply.and_then(|r| ctx.replies.get(&r)) {
        // After the first line, so the sender's line starts with what they wrote.
        lines.insert(1.min(lines.len()), format!("> {}: {q}", w.reply));
    }
    let head = format!("[{}] {prefix}{}: ", clock.time(row.time_ms, row.wall_clock), sender(row, w));
    let mut first = true;
    for line in lines.iter().flat_map(|l| l.split('\n')) {
        if first {
            writeln!(f, "{head}{line}")?;
            first = false;
        } else {
            writeln!(f, "    {line}")?;
        }
    }
    if first {
        writeln!(f, "{head}")?;
    }
    Ok(())
}

/// A spreadsheet runs a cell that starts with `=`, `+`, `-` or `@` as a formula: such text is written with a `'`
/// first, so opening the CSV never runs what someone sent.
fn cell(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_owned() }
}

fn csv_row(f: &mut impl Write, row: &MessageRow, ctx: &Context, clock: &Clock, w: &Words) -> Result<()> {
    let conv = ctx.conversations.get(&row.conversation).map(|c| c.1.clone()).unwrap_or_default();
    let text = message_lines(row, w).join("\n");
    let mut out = csv::WriterBuilder::new().has_headers(false).from_writer(Vec::new());
    out.write_record([
        row.app.code(),
        &cell(&conv),
        &clock.iso(row.time_ms, row.wall_clock),
        if row.from_me { "1" } else { "0" },
        &cell(&sender(row, w)),
        row.kind.code(),
        &cell(&text),
        &row.reply.map(|r| r.to_string()).unwrap_or_default(),
        if row.message.edited { "1" } else { "0" },
        if row.message.deleted { "1" } else { "0" },
        if row.message.forwarded { "1" } else { "0" },
    ])
    .map_err(|e| Error::doing("io", Note::of(Msg::DoingWritingExport, []), e))?;
    let bytes = out.into_inner().map_err(|e| Error::doing("io", Note::of(Msg::DoingWritingExport, []), e))?;
    f.write_all(&bytes).map_err(write_err)
}

#[derive(Serialize)]
struct JsonMessage<'a> {
    id: i64,
    conversation: i64,
    source: i64,
    app: App,
    time_ms: Option<i64>,
    wall_clock: bool,
    from_me: bool,
    sender: Option<i64>,
    sender_name: Option<&'a str>,
    kind: Kind,
    reply: Option<i64>,
    message: m::Message,
}

/// The JSON's head: only the sources, conversations, people and contacts the selection touches.
fn json_head(f: &mut impl Write, lib: &Library, ids: &[i64]) -> Result<()> {
    let (mut sources, mut convs, mut people) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for chunk in ids.chunks(1000) {
        for r in lib.messages_by_id(chunk)? {
            sources.insert(r.source);
            convs.insert(r.conversation);
            people.extend(r.sender);
        }
    }
    let people: Vec<_> = lib.people(None)?.into_iter().filter(|p| people.contains(&p.id)).collect();
    let contact_ids: BTreeSet<i64> = people.iter().filter_map(|p| p.contact).collect();
    let head = serde_json::json!({
        "oopz": { "format": "oopz-export", "version": 1 },
        "sources": lib.sources()?.into_iter().filter(|s| sources.contains(&s.id)).collect::<Vec<_>>(),
        "conversations": lib
            .conversations(&oopz_core::Filter { sources: sources.iter().copied().collect(), ..Default::default() })?
            .into_iter()
            .filter(|c| convs.contains(&c.id))
            .collect::<Vec<_>>(),
        "people": people,
        "contacts": lib.contacts()?.into_iter().filter(|c| contact_ids.contains(&c.id)).collect::<Vec<_>>(),
    });
    let text =
        serde_json::to_string(&head).map_err(|e| Error::doing("io", Note::of(Msg::DoingWritingExport, []), e))?;
    // The head's fields, then the messages array, streamed.
    let text = text.strip_suffix('}').unwrap_or(&text);
    write!(f, "{text},\"messages\":[").map_err(write_err)
}

fn json_row(f: &mut impl Write, row: &MessageRow) -> Result<()> {
    let mut message = row.message.clone();
    for part in &mut message.parts {
        if let Part::Media(media) = part
            && media.view_once
        {
            media.caption = None;
        }
    }
    let j = JsonMessage {
        id: row.id,
        conversation: row.conversation,
        source: row.source,
        app: row.app,
        time_ms: row.time_ms,
        wall_clock: row.wall_clock,
        from_me: row.from_me,
        sender: row.sender,
        sender_name: row.sender_name.as_deref(),
        kind: row.kind,
        reply: row.reply,
        message,
    };
    serde_json::to_writer(&mut *f, &j).map_err(|e| Error::doing("io", Note::of(Msg::DoingWritingExport, []), e))
}

/// The contacts of the people in the selection (and those people not linked to one), as vCards.
fn vcards(lib: &Library, ids: &[i64], out_dir: &Path) -> Result<Written> {
    let mut senders = BTreeSet::new();
    for chunk in ids.chunks(1000) {
        senders.extend(lib.messages_by_id(chunk)?.into_iter().filter(|r| !r.from_me).filter_map(|r| r.sender));
    }
    let cards = cards_for(lib, &senders)?;
    if cards.is_empty() {
        return Err(Error::of("not_found", Msg::NobodyToExport, []));
    }
    write_cards(&cards, out_dir, ids.len() as u64)
}

/// vCards for these people: their contact's, or their own when not linked.
pub fn cards_for(lib: &Library, people: &BTreeSet<i64>) -> Result<Vec<oopz_core::vcard::Card>> {
    let contacts: HashMap<i64, ContactRow> = lib.contacts()?.into_iter().map(|c| (c.id, c)).collect();
    let mut seen_contacts = BTreeSet::new();
    let mut cards = Vec::new();
    for p in lib.people(None)?.into_iter().filter(|p| people.contains(&p.id) && !p.me) {
        match p.contact.and_then(|c| contacts.get(&c)) {
            Some(c) if seen_contacts.insert(c.id) => {
                let apps: BTreeSet<&str> = c.people.iter().map(|(_, a)| a.name()).collect();
                cards.push(oopz_core::vcard::Card {
                    name: c.name.clone(),
                    phones: c.phones.clone(),
                    emails: c.emails.clone(),
                    note: Some(apps.into_iter().collect::<Vec<_>>().join(", ")),
                });
            }
            Some(_) => {}
            None if p.phone.is_some() || p.email.is_some() || p.name.is_some() => cards.push(oopz_core::vcard::Card {
                name: p.name.clone().or_else(|| p.phone.clone()).or_else(|| p.email.clone()).unwrap_or_default(),
                phones: p.phone.into_iter().collect(),
                emails: p.email.into_iter().collect(),
                note: Some(p.app.name().to_owned()),
            }),
            None => {}
        }
    }
    Ok(cards)
}

/// Writes contacts picked in the contacts screen.
pub fn export_contacts(lib: &Library, contact_ids: &[i64], out_dir: &Path) -> Result<Written> {
    let cards: Vec<_> = lib
        .contacts()?
        .into_iter()
        .filter(|c| contact_ids.contains(&c.id))
        .map(|c| {
            let apps: BTreeSet<&str> = c.people.iter().map(|(_, a)| a.name()).collect();
            oopz_core::vcard::Card {
                name: c.name,
                phones: c.phones,
                emails: c.emails,
                note: (!apps.is_empty()).then(|| apps.into_iter().collect::<Vec<_>>().join(", ")),
            }
        })
        .collect();
    if cards.is_empty() {
        return Err(Error::of("not_found", Msg::NoContactsPicked, []));
    }
    write_cards(&cards, out_dir, 0)
}

fn write_cards(cards: &[oopz_core::vcard::Card], out_dir: &Path, messages: u64) -> Result<Written> {
    let text = oopz_core::vcard::write(cards);
    let mut out = Out::new(out_dir, "contacts", Format::Vcard.extension(), text.len() as u64)?;
    {
        let mut f = out.writer();
        f.write_all(text.as_bytes()).map_err(write_err)?;
        f.flush().map_err(write_err)?;
    }
    let (path, bytes) = out.finish()?;
    Ok(Written { path, messages, bytes, missing_files: 0 })
}

/// A path inside a source's media folder: relative, no `..`, and still inside it once links are followed.
fn inside(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let path = root.join(rel);
    let real = path.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    real.starts_with(&root).then_some(real)
}

fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '_' })
        .collect::<String>()
        .trim_matches(['.', ' '])
        .chars()
        .take(60)
        .collect();
    if cleaned.is_empty() { "chat".into() } else { cleaned }
}

/// The selection's media files from the sources' folders, one folder per conversation; the files a message names
/// but the folder doesn't have are listed in `missing.txt` inside.
fn media_zip(
    lib: &Library,
    ids: &[i64],
    out_dir: &Path,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<Written> {
    let mut roots: HashMap<i64, Option<PathBuf>> = HashMap::new();
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut names: BTreeMap<String, u32> = BTreeMap::new();
    let mut need = 0u64;
    let mut convs: HashMap<i64, String> = HashMap::new();
    for chunk in ids.chunks(1000) {
        let rows = lib.messages_by_id(chunk)?;
        let ctx = context(lib, &rows)?;
        for row in rows {
            let root = match roots.get(&row.source) {
                Some(r) => r.clone(),
                None => {
                    let r = lib.media_root(row.source)?;
                    roots.insert(row.source, r.clone());
                    r
                }
            };
            for part in &row.message.parts {
                let Part::Media(media) = part else { continue };
                if media.view_once || media.omitted {
                    continue;
                }
                let Some(rel) = media.file.as_deref() else { continue };
                let folder = convs
                    .entry(row.conversation)
                    .or_insert_with(|| {
                        let (app, name) =
                            ctx.conversations.get(&row.conversation).cloned().unwrap_or((row.app, String::new()));
                        format!("{}-{}", app.code(), safe_name(&name))
                    })
                    .clone();
                match root.as_deref().and_then(|r| inside(r, rel)) {
                    Some(path) => {
                        need += fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        let base = safe_name(Path::new(rel).file_name().and_then(|n| n.to_str()).unwrap_or("file"));
                        let mut name = format!("{folder}/{base}");
                        let n = names.entry(name.clone()).or_insert(0);
                        *n += 1;
                        if *n > 1 {
                            name = format!("{folder}/{}-{base}", *n);
                        }
                        files.push((path, name));
                    }
                    None => missing.push(format!("{folder}/{rel}")),
                }
            }
        }
    }
    if files.is_empty() && missing.is_empty() {
        return Err(Error::of("not_found", Msg::SelectionNoMedia, []));
    }
    let mut out = Out::new(out_dir, "media", Format::MediaZip.extension(), need)?;
    let total = files.len() as u64;
    {
        let mut zip = zip::ZipWriter::new(BufWriter::new(out.file.as_file_mut()));
        // Photos and videos are compressed already: stored as they are.
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        let zerr = |e: zip::result::ZipError| Error::doing("io", Note::of(Msg::DoingWritingZip, []), e);
        for (i, (path, name)) in files.iter().enumerate() {
            zip.start_file(name.as_str(), opts).map_err(zerr)?;
            let mut src = fs::File::open(path)
                .map_err(|e| Error::io(Note::of(Msg::DoingReading, [path.display().to_string()]), e))?;
            std::io::copy(&mut src, &mut zip).map_err(write_err)?;
            if !progress(i as u64 + 1, total) {
                return Err(m::Error::Cancelled.into());
            }
        }
        if !missing.is_empty() {
            zip.start_file("missing.txt", opts).map_err(zerr)?;
            zip.write_all(missing.join("\n").as_bytes()).map_err(write_err)?;
        }
        let mut inner = zip.finish().map_err(zerr)?;
        inner.flush().map_err(write_err)?;
    }
    let (path, bytes) = out.finish()?;
    Ok(Written { path, messages: ids.len() as u64, bytes, missing_files: missing.len() as u64 })
}

#[cfg(test)]
mod tests;
