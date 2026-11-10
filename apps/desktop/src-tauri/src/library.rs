//! The library (decision 0016): every source the person adds, from every app, in one SQLite file to read, search,
//! select, link and export from. Each command wraps one `oopz_core::Library` call. The library holds the person's own
//! messages, numbers and names: it lives in the app's private cache folder and goes when the app closes, unless the
//! person keeps it (then it lives in the app's private data folder). Nothing is logged.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use oopz_app::AppError;
use oopz_core::{
    ContactRow, ConversationRow, Filter, Hit, Library, MessageRow, Order, PersonRow, SavedSelection, Selection,
    SourceRow, Stats, Suggestion,
};
use oopz_model::{App, Secret};
use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;
use zeroize::Zeroizing;

use super::{AppState, Step, lock, progress};

/// The most messages one call hands the UI (a page of a conversation, of a filter, of a selection).
const PAGE_MAX: u32 = 500;
/// The WhatsApp backup adapter's id (`oopz_app::adapter::WhatsAppAdapter`).
const WHATSAPP_BACKUP: &str = "whatsapp-android";

fn fail(e: oopz_core::Error) -> AppError {
    AppError::new(e.code, e.detail)
}

/// The library, open or not yet, and where it lives.
pub struct Lib {
    open: Mutex<Option<Library>>,
    /// Held by work on its own connection (an import, an export), and by moving or removing the library, so the
    /// folder never moves under it.
    work: Mutex<()>,
    keep: AtomicBool,
    /// `<cache>/library`: removed on close and at the next start.
    cache: PathBuf,
    /// `<data>/library`: kept between runs.
    data: PathBuf,
    /// Its presence means "keep my library".
    marker: PathBuf,
}

#[derive(Serialize)]
pub struct LibraryInfo {
    pub dir: PathBuf,
    pub kept: bool,
    pub sources: u64,
    pub messages: u64,
    /// The library's files on disk (its database and the media it keeps).
    pub bytes: u64,
    /// PDF exports are in this build.
    pub pdf: bool,
}

#[derive(Serialize)]
pub struct AdapterInfo {
    pub id: &'static str,
    pub app: App,
    pub secret: Secret,
}

/// What an export wrote. `pages` and `missing_glyphs` are a PDF's; `missing_files` a media zip's.
#[derive(Serialize)]
pub struct Exported {
    pub path: PathBuf,
    pub folder: PathBuf,
    pub messages: u64,
    pub bytes: u64,
    pub missing_files: u64,
    pub pages: u64,
    pub missing_glyphs: u64,
}

impl Lib {
    /// A library left in the cache by a crash is removed first.
    pub fn new(cache_root: &Path, data_root: &Path) -> Self {
        let cache = cache_root.join("library");
        let _ = std::fs::remove_dir_all(&cache);
        let marker = data_root.join("keep-library");
        Self {
            open: Mutex::default(),
            work: Mutex::default(),
            keep: AtomicBool::new(marker.is_file()),
            cache,
            data: data_root.join("library"),
            marker,
        }
    }

    fn kept(&self) -> bool {
        self.keep.load(Ordering::SeqCst)
    }

    fn dir(&self) -> &Path {
        if self.kept() { &self.data } else { &self.cache }
    }

    /// Runs `f` on the library, opening (or making) it on first use.
    fn with<T>(&self, f: impl FnOnce(&mut Library) -> oopz_core::Result<T>) -> Result<T, AppError> {
        let mut slot = lock(&self.open);
        if slot.is_none() {
            *slot = Some(Library::open(self.dir()).map_err(fail)?);
        }
        f(slot.as_mut().expect("opened above")).map_err(fail)
    }

    /// A second connection for long work, so the screens keep reading while it runs; the guard keeps the folder
    /// where it is until the work ends.
    fn second(&self) -> Result<(MutexGuard<'_, ()>, Library), AppError> {
        let guard = lock(&self.work);
        self.with(|_| Ok(()))?;
        let lib = Library::open(self.dir()).map_err(fail)?;
        Ok((guard, lib))
    }

    pub fn info(&self) -> Result<LibraryInfo, AppError> {
        let exists = self.dir().join(oopz_core::FILE).is_file();
        let sources = if exists { self.with(|l| l.sources())? } else { Vec::new() };
        Ok(LibraryInfo {
            dir: self.dir().to_path_buf(),
            kept: self.kept(),
            sources: sources.len() as u64,
            messages: sources.iter().map(|s| s.messages).sum(),
            bytes: size_of(self.dir()),
            pdf: cfg!(feature = "pdf"),
        })
    }

    /// Keeps the library between runs, or not: closes it and moves its folder (the next call opens it there).
    pub fn set_keep(&self, keep: bool) -> Result<LibraryInfo, AppError> {
        {
            let _work = lock(&self.work);
            let mut slot = lock(&self.open);
            if keep != self.kept() {
                // Closing first: the move takes every file (SQLite folds its journal back in on close), and Windows
                // can't move open files.
                drop(slot.take());
                let (from, to) = if keep { (&self.cache, &self.data) } else { (&self.data, &self.cache) };
                if from.exists() {
                    move_dir(from, to)?;
                }
                if keep {
                    if let Some(parent) = self.marker.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| AppError::io("keeping the library", e))?;
                    }
                    std::fs::write(&self.marker, b"").map_err(|e| AppError::io("keeping the library", e))?;
                } else {
                    let _ = std::fs::remove_file(&self.marker);
                }
                self.keep.store(keep, Ordering::SeqCst);
            }
        }
        self.info()
    }

    /// The app closes: the library is closed, and removed unless it's kept.
    pub fn close(&self) {
        drop(lock(&self.open).take());
        let _ = std::fs::remove_dir_all(&self.cache);
    }

    /// "Wipe now": waits for an import or export to stop (the caller set the cancel flag), then closes and removes
    /// the library, kept or not. The keep setting stays: a new library is kept again.
    pub fn wipe(&self) -> usize {
        let _work = lock(&self.work);
        drop(lock(&self.open).take());
        let _ = std::fs::remove_dir_all(&self.cache);
        let _ = std::fs::remove_dir_all(&self.data);
        usize::from(self.cache.exists()) + usize::from(self.data.exists())
    }
}

/// Bytes of the files under `dir` (0 when it isn't there).
fn size_of(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(t) if t.is_file() => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

/// Moves a folder: a rename, or (across disks) a copy then a removal. A copy that fails leaves the original.
fn move_dir(from: &Path, to: &Path) -> Result<(), AppError> {
    let _ = std::fs::remove_dir_all(to);
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::io("moving the library", e))?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if let Err(e) = copy_dir(from, to) {
        let _ = std::fs::remove_dir_all(to);
        return Err(AppError::io("moving the library", e));
    }
    std::fs::remove_dir_all(from).map_err(|e| AppError::io("moving the library", e))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A picked file as a path Rust can open: a path as it is, or (Android) a `content://` reference copied into a new
/// private folder, named so the adapters recognise it. The folder goes with the private folder, or when the caller
/// drops it.
fn picked_path(
    app: &tauri::AppHandle,
    state: &AppState,
    picked: &str,
    adapter: Option<&str>,
) -> Result<(PathBuf, Option<tempfile::TempDir>), AppError> {
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
    if !picked.starts_with("content://") {
        return Ok((PathBuf::from(picked), None));
    }
    let uri: FilePath = picked.parse().map_err(|_| AppError::new("not_found", "not a file reference"))?;
    let mut input = app
        .fs()
        .open(uri, OpenOptions::new().read(true).to_owned())
        .map_err(|e| AppError::new("io", format!("opening the picked file: {e}")))?;
    let bytes = input.metadata().map(|m| m.len()).unwrap_or(0);
    let dir = tempfile::Builder::new()
        .prefix("picked-")
        .tempdir_in(state.tmp.path())
        .map_err(|e| AppError::io("making a private copy", e))?;
    let copy = dir.path().join("picked");
    super::copy_in(&mut input, bytes, &copy, oopz_app::session::free_space(state.tmp.path())?)
        .map_err(|e| if e.code == "not_crypt15" { AppError::new("unsupported_file", e.detail) } else { e })?;
    let mut head = Vec::new();
    if let Ok(f) = std::fs::File::open(&copy) {
        use std::io::Read;
        let _ = f.take(512).read_to_end(&mut head);
    }
    let named = dir.path().join(picked_name(picked, adapter, &head));
    std::fs::rename(&copy, &named).map_err(|e| AppError::io("making a private copy", e))?;
    Ok((named, Some(dir)))
}

/// A name for a picked file: its own when the reference carries one, else one its app's adapter recognises (the one
/// named, or a guess from its first bytes).
fn picked_name(uri: &str, adapter: Option<&str>, head: &[u8]) -> String {
    let last = percent_decode(uri.rsplit(['/', ':']).next().unwrap_or_default());
    let last = last.rsplit(['/', ':', '\\']).next().unwrap_or_default().trim();
    if last.contains('.') && !last.starts_with('.') && last.len() <= 200 {
        return last.to_owned();
    }
    let zip = head.starts_with(b"PK");
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let name = match adapter {
        Some(WHATSAPP_BACKUP) => "msgstore.db.crypt15",
        Some("whatsapp-export") => {
            if zip {
                "WhatsApp Chat.zip"
            } else {
                "WhatsApp Chat.txt"
            }
        }
        Some("telegram-json") => {
            if zip {
                "Telegram.zip"
            } else {
                "result.json"
            }
        }
        Some("sms-xml") => "sms.xml",
        Some("line-export") => "LINE.txt",
        Some("viber-csv") => {
            if zip {
                "Viber.zip"
            } else {
                "Viber.csv"
            }
        }
        _ if zip => "picked.zip",
        _ if text.starts_with('{') => "result.json",
        _ if text.starts_with('<') => "sms.xml",
        _ => "picked.txt",
    };
    name.to_owned()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Where an export goes: the folder picked, or (Android, where a picked folder is a reference Rust can't write in) a
/// new private folder the UI then saves from. Either way a folder this run made, which `show_folder` and
/// `save_result` accept.
fn export_folder(state: &AppState, folder: Option<PathBuf>) -> Result<PathBuf, AppError> {
    let folder = match folder {
        Some(f) => f,
        None => tempfile::Builder::new()
            .prefix("export-")
            .tempdir_in(state.tmp.path())
            .map_err(|e| AppError::io("making a private folder", e))?
            .keep(),
    };
    lock(&state.made).push(folder.clone());
    Ok(folder)
}

#[tauri::command(async)]
pub fn library_info(lib: State<'_, Lib>) -> Result<LibraryInfo, AppError> {
    lib.info()
}

#[tauri::command(async)]
pub fn library_set_keep(lib: State<'_, Lib>, keep: bool) -> Result<LibraryInfo, AppError> {
    lib.set_keep(keep)
}

#[tauri::command]
pub fn lib_adapters() -> Vec<AdapterInfo> {
    oopz_app::adapters().into_iter().map(|a| AdapterInfo { id: a.id(), app: a.app(), secret: a.secret() }).collect()
}

/// Adds a file or folder to the library: its app detected (or named), opened with the key or password when its
/// adapter needs one (used for this one call, then wiped). Stops on `cancel_work`, leaving the library as it was.
#[tauri::command(async)]
pub fn lib_import(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    lib: State<'_, Lib>,
    path: String,
    adapter: Option<String>,
    secret: Option<Zeroizing<String>>,
    progress: Channel<Step>,
) -> Result<SourceRow, AppError> {
    state.cancel.store(false, Ordering::SeqCst);
    let (path, _copy) = picked_path(&app, &state, &path, adapter.as_deref())?;
    let adapters = oopz_app::adapters();
    let secret = secret.filter(|s| !s.trim().is_empty());
    if secret.is_none() {
        let picked = oopz_core::pick_adapter(&adapters, &path, adapter.as_deref()).map_err(fail)?;
        if let Some(code) = secret_needed(picked.secret_for(&path)) {
            return Err(AppError::new(code, "this file needs its owner's secret"));
        }
    }
    let (_work, mut second) = lib.second()?;
    second
        .import_path(
            &adapters,
            &path,
            adapter.as_deref(),
            secret.as_ref().map(|s| s.as_str()),
            &mut self::progress(&progress, &state.cancel, |_, _| "import"),
        )
        .map_err(fail)
}

/// The error that asks the person for what this source needs: `needs_key` (WhatsApp's 64-digit key) or
/// `needs_password` (a backup password); none for a source that needs nothing (an unencrypted iPhone backup).
fn secret_needed(secret: Secret) -> Option<&'static str> {
    match secret {
        Secret::None => None,
        Secret::Key => Some("needs_key"),
        Secret::Password => Some("needs_password"),
    }
}

#[tauri::command(async)]
pub fn lib_sources(lib: State<'_, Lib>) -> Result<Vec<SourceRow>, AppError> {
    lib.with(|l| l.sources())
}

#[tauri::command(async)]
pub fn lib_remove_source(lib: State<'_, Lib>, id: i64) -> Result<(), AppError> {
    lib.with(|l| l.remove_source(id))
}

#[tauri::command(async)]
pub fn lib_set_me(lib: State<'_, Lib>, person: i64) -> Result<(), AppError> {
    lib.with(|l| l.set_me(person))
}

#[tauri::command(async)]
pub fn lib_conversations(lib: State<'_, Lib>, filter: Filter) -> Result<Vec<ConversationRow>, AppError> {
    lib.with(|l| l.conversations(&filter))
}

#[tauri::command(async)]
pub fn lib_conversation_len(lib: State<'_, Lib>, conversation: i64) -> Result<u64, AppError> {
    lib.with(|l| l.conversation_len(conversation))
}

#[tauri::command(async)]
pub fn lib_messages(
    lib: State<'_, Lib>,
    conversation: i64,
    from: u64,
    count: u32,
) -> Result<Vec<MessageRow>, AppError> {
    lib.with(|l| l.messages(conversation, from, count.min(PAGE_MAX)))
}

/// "Jump to a date": the position of the conversation's first message at or after `ts` (ms).
#[tauri::command(async)]
pub fn lib_position_at(lib: State<'_, Lib>, conversation: i64, ts: i64) -> Result<Option<u64>, AppError> {
    lib.with(|l| l.position_at(conversation, ts))
}

/// A message's conversation and position, to open it there.
#[tauri::command(async)]
pub fn lib_position(lib: State<'_, Lib>, message: i64) -> Result<Option<(i64, u64)>, AppError> {
    lib.with(|l| l.position(message))
}

#[tauri::command(async)]
pub fn lib_search(lib: State<'_, Lib>, filter: Filter, limit: u32) -> Result<Vec<Hit>, AppError> {
    lib.with(|l| l.search(&filter, limit.min(PAGE_MAX)))
}

#[tauri::command(async)]
pub fn lib_find(
    lib: State<'_, Lib>,
    filter: Filter,
    order: Order,
    offset: u64,
    limit: u32,
) -> Result<Vec<MessageRow>, AppError> {
    lib.with(|l| l.find(&filter, order, offset, limit.min(PAGE_MAX)))
}

#[tauri::command(async)]
pub fn lib_count(lib: State<'_, Lib>, filter: Filter) -> Result<u64, AppError> {
    lib.with(|l| l.count(&filter))
}

/// The ids of a selection's messages, in `order`.
#[tauri::command(async)]
pub fn lib_selected(lib: State<'_, Lib>, selection: Selection, order: Order) -> Result<Vec<i64>, AppError> {
    lib.with(|l| l.selected(&selection, order))
}

#[tauri::command(async)]
pub fn lib_messages_by_id(lib: State<'_, Lib>, ids: Vec<i64>) -> Result<Vec<MessageRow>, AppError> {
    let ids = &ids[..ids.len().min(PAGE_MAX as usize)];
    lib.with(|l| l.messages_by_id(ids))
}

#[tauri::command(async)]
pub fn lib_stats(lib: State<'_, Lib>, filter: Filter) -> Result<Stats, AppError> {
    lib.with(|l| l.stats(&filter))
}

#[tauri::command(async)]
pub fn lib_people(lib: State<'_, Lib>, source: Option<i64>) -> Result<Vec<PersonRow>, AppError> {
    lib.with(|l| l.people(source))
}

#[tauri::command(async)]
pub fn lib_contacts(lib: State<'_, Lib>) -> Result<Vec<ContactRow>, AppError> {
    lib.with(|l| l.contacts())
}

#[tauri::command(async)]
pub fn lib_link(lib: State<'_, Lib>, people: Vec<i64>, name: Option<String>) -> Result<i64, AppError> {
    lib.with(|l| l.link(&people, name.as_deref()))
}

#[tauri::command(async)]
pub fn lib_unlink(lib: State<'_, Lib>, person: i64) -> Result<(), AppError> {
    lib.with(|l| l.unlink(person))
}

#[tauri::command(async)]
pub fn lib_rename_contact(lib: State<'_, Lib>, contact: i64, name: String) -> Result<(), AppError> {
    lib.with(|l| l.rename_contact(contact, &name))
}

#[tauri::command(async)]
pub fn lib_delete_contact(lib: State<'_, Lib>, contact: i64) -> Result<(), AppError> {
    lib.with(|l| l.delete_contact(contact))
}

#[tauri::command(async)]
pub fn lib_suggestions(lib: State<'_, Lib>) -> Result<Vec<Suggestion>, AppError> {
    lib.with(|l| l.suggestions())
}

/// Accepts the suggestion that links the people sharing `value` (a phone number or email): found again here, so the
/// page can't link people no suggestion named.
#[tauri::command(async)]
pub fn lib_accept(lib: State<'_, Lib>, reason: String, value: String) -> Result<i64, AppError> {
    lib.with(|l| {
        let found = l.suggestions()?.into_iter().find(|s| s.reason == reason && s.value == value);
        let suggestion = found.ok_or_else(|| oopz_core::Error::new("not_found", "that suggestion is gone"))?;
        l.accept(&suggestion)
    })
}

/// Reads a .vcf file (a path, or Android's reference) into contacts; returns how many cards it held.
#[tauri::command(async)]
pub fn lib_import_vcards(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    lib: State<'_, Lib>,
    path: String,
) -> Result<usize, AppError> {
    let (path, _copy) = picked_path(&app, &state, &path, None)?;
    let bytes = std::fs::read(&path).map_err(|e| AppError::io("reading the contacts file", e))?;
    let text = String::from_utf8_lossy(&bytes);
    lib.with(|l| l.import_vcards(&text))
}

#[tauri::command(async)]
pub fn lib_save_selection(lib: State<'_, Lib>, name: String, selection: Selection) -> Result<i64, AppError> {
    lib.with(|l| l.save_selection(&name, &selection))
}

#[tauri::command(async)]
pub fn lib_selections(lib: State<'_, Lib>) -> Result<Vec<SavedSelection>, AppError> {
    lib.with(|l| l.selections())
}

#[tauri::command(async)]
pub fn lib_delete_selection(lib: State<'_, Lib>, id: i64) -> Result<(), AppError> {
    lib.with(|l| l.delete_selection(id))
}

/// Writes a selection as `format` (`html`, `json`, `csv`, `txt`, `vcard`, `media-zip`, or `pdf` when this build has
/// it) into a new file in `folder` (none: a private folder to save from). The options are the export's (order, time
/// zone, the words in the app's language, title, leaving out notices) and a PDF's (page, thumbnails, cover,
/// contents). Stops on `cancel_work`, leaving no file.
#[tauri::command(async)]
pub fn lib_export(
    state: State<'_, AppState>,
    lib: State<'_, Lib>,
    selection: Selection,
    format: String,
    options: serde_json::Value,
    folder: Option<PathBuf>,
    progress: Channel<Step>,
) -> Result<Exported, AppError> {
    state.cancel.store(false, Ordering::SeqCst);
    let bad = |e: serde_json::Error| AppError::new("not_found", format!("export options: {e}"));
    let (_work, second) = lib.second()?;
    let folder = export_folder(&state, folder)?;
    let mut report = self::progress(&progress, &state.cancel, |_, _| "export");
    if format == "pdf" {
        return export_pdf(&second, &selection, options, &folder, &mut report);
    }
    let format = oopz_export::Format::from_code(&format)
        .ok_or_else(|| AppError::new("unsupported", format!("no export format `{format}`")))?;
    let options: oopz_export::Options = serde_json::from_value(options).map_err(bad)?;
    let w = oopz_export::export(&second, &selection, format, &options, &folder, &mut report).map_err(fail)?;
    Ok(Exported {
        path: w.path,
        folder,
        messages: w.messages,
        bytes: w.bytes,
        missing_files: w.missing_files,
        pages: 0,
        missing_glyphs: 0,
    })
}

#[cfg(feature = "pdf")]
fn export_pdf(
    lib: &Library,
    selection: &Selection,
    options: serde_json::Value,
    folder: &Path,
    report: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<Exported, AppError> {
    let mut options: oopz_export_pdf::PdfOptions =
        serde_json::from_value(options).map_err(|e| AppError::new("not_found", format!("export options: {e}")))?;
    // The system's fonts only: the page doesn't name files for Rust to read.
    options.fonts.clear();
    let w = oopz_export_pdf::export_pdf(lib, selection, &options, folder, report).map_err(fail)?;
    Ok(Exported {
        path: w.path,
        folder: folder.to_path_buf(),
        messages: w.messages,
        bytes: w.bytes,
        missing_files: 0,
        pages: w.pages,
        missing_glyphs: w.missing_glyphs,
    })
}

#[cfg(not(feature = "pdf"))]
fn export_pdf(
    _: &Library,
    _: &Selection,
    _: serde_json::Value,
    _: &Path,
    _: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<Exported, AppError> {
    Err(AppError::new("unsupported", "PDF exports aren't in this build"))
}

/// Writes the picked contacts as one .vcf file in `folder` (none: a private folder to save from).
#[tauri::command(async)]
pub fn lib_export_contacts(
    state: State<'_, AppState>,
    lib: State<'_, Lib>,
    contacts: Vec<i64>,
    folder: Option<PathBuf>,
) -> Result<Exported, AppError> {
    let folder = export_folder(&state, folder)?;
    let w = lib.with(|l| oopz_export::export_contacts(l, &contacts, &folder))?;
    Ok(Exported { path: w.path, folder, messages: 0, bytes: w.bytes, missing_files: 0, pages: 0, missing_glyphs: 0 })
}

#[cfg(test)]
mod tests {
    use oopz::fixture::{self, KEY_HEX};

    use super::*;

    fn lib() -> (tempfile::TempDir, Lib) {
        let root = tempfile::tempdir().unwrap();
        let lib = Lib::new(&root.path().join("cache"), &root.path().join("data"));
        (root, lib)
    }

    fn import(lib: &Lib, path: &Path, secret: Option<&str>) -> oopz_core::Result<SourceRow> {
        let (_work, mut second) = lib.second().unwrap();
        second.import_path(&oopz_app::adapters(), path, None, secret, &mut |_, _| true)
    }

    #[test]
    fn a_backup_is_imported_read_kept_and_wiped() {
        let (root, lib) = lib();
        assert_eq!(lib.info().unwrap().sources, 0);
        assert!(!lib.cache.exists(), "nothing made until it's used");

        let two = fixture::two_phones();
        assert_eq!(import(&lib, &two.source, None).unwrap_err().code, "needs_secret");
        let source = import(&lib, &two.source, Some(KEY_HEX)).unwrap();
        assert_eq!(import(&lib, &two.source, Some(KEY_HEX)).unwrap_err().code, "exists");
        // The main connection sees what the second one imported.
        let info = lib.info().unwrap();
        assert_eq!((info.sources, info.messages), (1, source.messages));
        assert!(info.bytes > 0 && !info.kept && info.dir.starts_with(root.path().join("cache")));

        let convs = lib.with(|l| l.conversations(&Filter::default())).unwrap();
        let first = lib.with(|l| l.messages(convs[0].id, 0, 10)).unwrap();
        assert!(!first.is_empty());
        let stats = lib.with(|l| l.stats(&Filter::default())).unwrap();
        assert_eq!(stats.messages, source.messages);

        // Kept: moved to the data folder, still open, and found there by the next run.
        let kept = lib.set_keep(true).unwrap();
        assert!(kept.kept && kept.dir.starts_with(root.path().join("data")));
        assert_eq!(kept.messages, source.messages);
        assert!(!lib.cache.exists());
        lib.close();
        assert!(lib.data.join(oopz_core::FILE).is_file(), "a kept library survives closing");
        let again = Lib::new(&root.path().join("cache"), &root.path().join("data"));
        assert_eq!(again.info().unwrap().sources, 1);

        // Not kept any more: back in the cache, and gone when the app closes.
        let back = again.set_keep(false).unwrap();
        assert!(!back.kept && back.sources == 1 && !again.data.exists());
        again.close();
        assert!(!again.cache.exists());

        // "Wipe now" removes a kept library too.
        let third = Lib::new(&root.path().join("cache"), &root.path().join("data"));
        third.set_keep(true).unwrap();
        import(&third, &two.source, Some(KEY_HEX)).unwrap();
        assert_eq!(third.wipe(), 0);
        assert!(!third.data.exists());
        assert_eq!(third.info().unwrap().sources, 0);
    }

    #[test]
    fn an_export_writes_a_new_file() {
        let (root, lib) = lib();
        let two = fixture::two_phones();
        let source = import(&lib, &two.target, Some(fixture::TARGET_KEY_HEX)).unwrap();
        // An empty filter selects nothing: "everything" is every source.
        let all =
            Selection { filter: Filter { sources: vec![source.id], ..Filter::default() }, ..Selection::default() };
        let out = root.path().join("out");
        std::fs::create_dir(&out).unwrap();
        let (_work, second) = lib.second().unwrap();
        let w = oopz_export::export(
            &second,
            &all,
            oopz_export::Format::Html,
            &oopz_export::Options::default(),
            &out,
            &mut |_, _| true,
        )
        .unwrap();
        assert!(w.path.starts_with(&out) && w.messages > 0);
    }

    #[test]
    fn every_adapter_is_listed_with_what_it_needs() {
        let list = lib_adapters();
        assert!(list.iter().any(|a| a.id == WHATSAPP_BACKUP && a.secret == Secret::Key));
        assert!(list.iter().any(|a| a.id == "telegram-json" && a.secret == Secret::None));
        // Asked for only when this source needs it (an unencrypted iPhone backup needs nothing).
        assert_eq!(secret_needed(Secret::None), None);
        assert_eq!(secret_needed(Secret::Key), Some("needs_key"));
        assert_eq!(secret_needed(Secret::Password), Some("needs_password"));
    }

    #[test]
    fn a_picked_reference_gets_a_name_its_adapter_knows() {
        let doc = "content://com.android.externalstorage.documents/document/primary%3ADownload%2Fresult.json";
        assert_eq!(picked_name(doc, None, b""), "result.json");
        let opaque = "content://com.android.providers.downloads.documents/document/msf%3A1234";
        assert_eq!(picked_name(opaque, None, b"PK\x03\x04"), "picked.zip");
        assert_eq!(picked_name(opaque, None, b"\xef\xbb\xbf <?xml"), "sms.xml");
        assert_eq!(picked_name(opaque, Some("viber-csv"), b"a,b"), "Viber.csv");
        assert_eq!(picked_name(opaque, Some(WHATSAPP_BACKUP), b""), "msgstore.db.crypt15");
        assert_eq!(picked_name("content://x/document/..%2F..%2Fetc", None, b""), "picked.txt");
    }
}
