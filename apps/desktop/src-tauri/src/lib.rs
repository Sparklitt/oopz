//! The Tauri shell: each command wraps one call of `oopz-app` (decision 0009). Keys arrive as `Zeroizing` strings,
//! are used for that one call and wiped; nothing is logged, and nothing but the person's own results is written.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use msgkinds::Note;
use oopz_app::adb::{self, Adb, Device, PullInfo, PullStatus, Puller, Wanted};
use oopz_app::index::{BuildInfo, Hit, Line, Stats};
use oopz_app::messages::Message as Msg;
use oopz_app::receive::{self, Receiver};
use oopz_app::session::{
    self, BackupFile, Fact, MergeOutcome, MergeRequest, Missing, Session, Summary, TableInfo, TypeInfo,
};
use oopz_app::{AppError, Filter, ReceiveInfo, ReceiveStatus};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{Manager, State};
use zeroize::Zeroizing;

mod cli;
mod library;
#[cfg(desktop)]
mod platform_tools;

/// What the app holds while it runs. The decrypted copies live in `tmp`, removed when the app closes.
pub struct AppState {
    session: Mutex<Session>,
    receiver: Mutex<Option<Receiver>>,
    /// The copy over adb running now, if any.
    puller: Mutex<Option<Puller>>,
    tmp: tempfile::TempDir,
    /// Folders this run of the app made (merge results, the folders pages were saved in): the only ones it opens.
    made: Mutex<Vec<PathBuf>>,
    /// Set by `cancel_work`: the check or index running now stops at its next progress report.
    cancel: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The merge form, as the UI sends it.
#[derive(Deserialize)]
pub struct MergeForm {
    pub source: u32,
    pub source_chat: i64,
    pub target: u32,
    pub target_chat: i64,
    pub target_key: Zeroizing<String>,
    pub out_parent: PathBuf,
    pub media: Option<PathBuf>,
}

impl AppState {
    /// Decrypted copies go in a fresh private folder inside `cache`; any left by a crash are removed first.
    pub fn new(cache: &Path) -> Result<Self, AppError> {
        let dir = cache.join("decrypted");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| AppError::io(Note::of(Msg::DoingAppCache, []), e))?;
        let tmp = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(&dir)
            .map_err(|e| AppError::io(Note::of(Msg::DoingAppCache, []), e))?;
        Ok(Self {
            session: Mutex::default(),
            receiver: Mutex::default(),
            puller: Mutex::default(),
            tmp,
            made: Mutex::default(),
            cancel: AtomicBool::new(false),
        })
    }

    pub fn check(
        &self,
        path: &Path,
        key: &str,
        progress: &mut dyn FnMut(u64, u64) -> bool,
    ) -> Result<Summary, AppError> {
        self.cancel.store(false, Ordering::SeqCst);
        lock(&self.session).check_with(path, key, self.tmp.path(), progress)
    }

    pub fn forget(&self, id: u32) {
        lock(&self.session).forget(id);
    }

    pub fn summaries(&self) -> Vec<Summary> {
        lock(&self.session).summaries()
    }

    pub fn merge(&self, form: &MergeForm) -> Result<MergeOutcome, AppError> {
        let done = lock(&self.session).merge(&MergeRequest {
            source: form.source,
            source_chat: form.source_chat,
            target: form.target,
            target_chat: form.target_chat,
            target_key: &form.target_key,
            out_parent: &form.out_parent,
            media: form.media.as_deref(),
        })?;
        lock(&self.made).push(done.folder.clone());
        Ok(done)
    }

    pub fn export(
        &self,
        id: u32,
        chat: Option<i64>,
        labels: &oopz_app::export::Labels,
        out_dir: &Path,
    ) -> Result<PathBuf, AppError> {
        let page = lock(&self.session).export(id, chat, labels, out_dir)?;
        lock(&self.made).push(out_dir.to_path_buf());
        Ok(page)
    }

    /// Wipes what this run decrypted: Tauri exits without dropping its managed state, so this runs on its exit event.
    /// Dropping the session closes and removes each copy and index (closed first: Windows can't delete open files).
    pub fn close(&self) {
        *lock(&self.session) = Session::default();
        drop(lock(&self.receiver).take());
        drop(lock(&self.puller).take());
        let _ = std::fs::remove_dir_all(self.tmp.path());
    }

    /// "Wipe now": stops what's running, closes every backup and removes every decrypted copy and index, keeping the
    /// private folder for the next check. Returns how many files were left in it (0 when the wipe is complete).
    pub fn wipe(&self) -> usize {
        self.cancel.store(true, Ordering::SeqCst);
        *lock(&self.session) = Session::default();
        drop(lock(&self.receiver).take());
        drop(lock(&self.puller).take());
        if let Ok(entries) = std::fs::read_dir(self.tmp.path()) {
            for entry in entries.flatten() {
                let path = entry.path();
                let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
            }
        }
        std::fs::read_dir(self.tmp.path()).map_or(0, Iterator::count)
    }

    /// Only a folder this run made, so a page can't ask the app to open just anything.
    pub fn made(&self, folder: &Path) -> Result<(), AppError> {
        if lock(&self.made).iter().any(|m| m == folder) {
            Ok(())
        } else {
            Err(AppError::of("io", Msg::NotOursFolder, []))
        }
    }

    /// Starts the Wi-Fi page ([`receive::start_in`]) on `ip`, or the likeliest address. One page at a time: a running
    /// one is stopped first, and its folder removed if nothing arrived (so picking another address keeps the name).
    pub fn receive_start(&self, parent: &Path, ip: Option<std::net::IpAddr>) -> Result<ReceiveInfo, AppError> {
        let mut slot = lock(&self.receiver);
        if let Some(old) = slot.take() {
            let dest = old.dest().to_path_buf();
            old.stop();
            let _ = std::fs::remove_dir(dest);
        }
        let (rx, info) = receive::start_in(parent, ip)?;
        *slot = Some(rx);
        Ok(info)
    }

    /// Builds a backup's search index without holding the session, so the other commands answer meanwhile.
    pub fn build_index(&self, id: u32, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<BuildInfo, AppError> {
        self.cancel.store(false, Ordering::SeqCst);
        let job = {
            let session = lock(&self.session);
            if let Some(info) = session.index_built(id)? {
                return Ok(info);
            }
            session.index_job(id)?
        };
        let index = job.run(self.tmp.path(), progress)?;
        lock(&self.session).keep_index(id, index)
    }

    pub fn receive_status(&self) -> ReceiveStatus {
        lock(&self.receiver).as_ref().map(Receiver::status).unwrap_or_default()
    }

    pub fn receive_stop(&self) -> ReceiveStatus {
        lock(&self.receiver).take().map(Receiver::stop).unwrap_or_default()
    }

    /// Copies WhatsApp's folder off the phone `serial` with adb ([`adb::start_in`]). One copy at a time.
    pub fn pull_start(&self, adb: &Adb, parent: &Path, serial: &str, want: &Wanted<'_>) -> Result<PullInfo, AppError> {
        let mut slot = lock(&self.puller);
        if slot.as_ref().is_some_and(|p| p.status().running) {
            return Err(AppError::of("exists", Msg::CopyRunning, []));
        }
        let devices = adb.devices()?;
        let device = adb::choose(&devices, Some(serial))?;
        let phone = adb.phone(&device.serial);
        let plan = adb::plan(&phone, want)?;
        let (puller, info) = adb::start_in(parent, phone, plan)?;
        *slot = Some(puller);
        Ok(info)
    }

    pub fn pull_status(&self) -> PullStatus {
        lock(&self.puller).as_ref().map(Puller::status).unwrap_or_default()
    }

    pub fn pull_stop(&self) -> PullStatus {
        lock(&self.puller).take().map(Puller::stop).unwrap_or_default()
    }
}

/// What the UI's bar shows while a backup opens or is indexed.
#[derive(Clone, Serialize)]
pub struct Step {
    /// `fingerprint`, `read`, `unpack`, `prepare` ([`session::check_step`]) or `index`.
    pub step: &'static str,
    pub per_mille: u16,
    /// An estimate from the pace so far; none in the first second or the first 2%.
    pub secs_left: Option<u32>,
}

/// Progress for the UI, sent only when it moves (a channel message per thousandth at most); false, to stop, once
/// `cancel_work` was called.
fn progress<'a>(
    channel: &'a Channel<Step>,
    cancel: &'a AtomicBool,
    step: fn(u64, u64) -> &'static str,
) -> impl FnMut(u64, u64) -> bool + 'a {
    let (started, mut last) = (Instant::now(), None);
    move |done, total| {
        let per_mille = (done.min(total) * 1000 / total.max(1)) as u16;
        let name = step(done, total);
        if last != Some((name, per_mille)) {
            last = Some((name, per_mille));
            let elapsed = started.elapsed().as_secs_f64();
            let secs_left = (per_mille >= 20 && elapsed >= 1.0)
                .then(|| (elapsed * f64::from(1000 - per_mille) / f64::from(per_mille)).round() as u32);
            let _ = channel.send(Step { step: name, per_mille, secs_left });
        }
        !cancel.load(Ordering::SeqCst)
    }
}

/// Stops the check or index running now; it fails with `cancelled` and removes what it decrypted.
#[tauri::command]
fn cancel_work(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

/// A photo message's thumbnail (JPEG bytes) from the phone's Media folder the person chose; empty when there's none.
#[tauri::command(async)]
fn thumbnail(
    state: State<'_, AppState>,
    id: u32,
    message: i64,
    media: PathBuf,
    max: u32,
) -> Result<tauri::ipc::Response, AppError> {
    let jpeg = lock(&state.session).thumbnail(id, message, &media, max.clamp(16, 1024))?;
    Ok(tauri::ipc::Response::new(jpeg.unwrap_or_default()))
}

/// The most bytes of one media file the viewer is handed as it is (a sticker, a voice note).
const MEDIA_FILE_MAX: u64 = 16 << 20;

/// A sticker's or a voice note's file as it is, from the phone's Media folder the person chose; empty when there's
/// none, or it's larger than [`MEDIA_FILE_MAX`].
#[tauri::command(async)]
fn media_file(
    state: State<'_, AppState>,
    id: u32,
    message: i64,
    media: PathBuf,
) -> Result<tauri::ipc::Response, AppError> {
    let bytes = lock(&state.session).media_file(id, message, &media, MEDIA_FILE_MAX)?;
    Ok(tauri::ipc::Response::new(bytes.unwrap_or_default()))
}

#[tauri::command(async)]
fn find_backups(path: PathBuf) -> Result<Vec<BackupFile>, AppError> {
    session::find_backups(&path)
}

#[tauri::command(async)]
fn check_backup(
    state: State<'_, AppState>,
    path: PathBuf,
    key: Zeroizing<String>,
    progress: Channel<Step>,
) -> Result<Summary, AppError> {
    state.check(&path, &key, &mut self::progress(&progress, &state.cancel, session::check_step))
}

#[tauri::command]
fn forget_backup(state: State<'_, AppState>, id: u32) {
    state.forget(id);
}

#[tauri::command]
fn checked_backups(state: State<'_, AppState>) -> Vec<Summary> {
    state.summaries()
}

#[tauri::command(async)]
fn merge_chat(state: State<'_, AppState>, form: MergeForm) -> Result<MergeOutcome, AppError> {
    state.merge(&form)
}

#[tauri::command(async)]
fn export_page(
    state: State<'_, AppState>,
    id: u32,
    chat: Option<i64>,
    labels: oopz_app::export::Labels,
    out_dir: PathBuf,
) -> Result<PathBuf, AppError> {
    state.export(id, chat, &labels, &out_dir)
}

/// Shows a result folder in the system's file manager.
#[tauri::command]
fn show_folder(state: State<'_, AppState>, folder: PathBuf) -> Result<(), AppError> {
    state.made(&folder)?;
    // `open` tries each system's launchers in turn (on Linux xdg-open, then gio, kde-open… and wslview).
    open::that_detached(&folder).map_err(|e| AppError::io(Note::of(Msg::DoingOpeningFolder, []), e))
}

/// Copies the picked files (Android `content://` references, or paths) into one new private folder (decision 0012,
/// point 3), largest first: the message backup, then the rest (WhatsApp's contacts file, for names, is far smaller).
/// `check_backup` then opens the first; the copies go with the private folder (on close, or "Wipe now").
#[tauri::command(async)]
fn import_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    uris: Vec<String>,
) -> Result<Vec<PathBuf>, AppError> {
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
    if uris.is_empty() || uris.len() > 8 {
        return Err(AppError::of("not_found", Msg::PickBackup, []));
    }
    let dir = tempfile::Builder::new()
        .prefix("imported-")
        .tempdir_in(state.tmp.path())
        .map_err(|e| AppError::io(Note::of(Msg::DoingPrivateCopy, []), e))?;
    let mut copies = Vec::new();
    for (n, uri) in uris.iter().enumerate() {
        let picked: FilePath = uri.parse().map_err(|_| AppError::of("not_found", Msg::NotFileReference, []))?;
        let mut input = app
            .fs()
            .open(picked, OpenOptions::new().read(true).to_owned())
            .map_err(|e| AppError::doing("io", Note::of(Msg::DoingOpeningPicked, []), e))?;
        // Some providers don't know a file's size: 0, then the copy itself finds out whether it fits.
        let bytes = input.metadata().map(|m| m.len()).unwrap_or(0);
        let to = dir.path().join(format!("picked-{}.crypt15", n + 1));
        copy_in(&mut input, bytes, &to, session::free_space(state.tmp.path())?)?;
        copies.push((std::fs::metadata(&to).map(|m| m.len()).unwrap_or(0), to));
    }
    copies.sort_by_key(|c| std::cmp::Reverse(c.0));
    let mut paths: Vec<PathBuf> = copies.into_iter().map(|(_, p)| p).collect();
    // The backup gets WhatsApp's name, so it reads as one everywhere (lists, the key screen, the merge).
    let backup = dir.path().join("msgstore.db.crypt15");
    std::fs::rename(&paths[0], &backup).map_err(|e| AppError::io(Note::of(Msg::DoingPrivateCopy, []), e))?;
    paths[0] = backup;
    // Kept from here on: the private folder removes it.
    let _ = dir.keep();
    Ok(paths)
}

/// [`import_backup`]'s copy of one file to `to`: refused without room for it and for opening it (four times the
/// backup, as `Session::check_with` asks); one that fails midway (a disk filling up, a provider cut off) is removed.
fn copy_in(input: &mut impl std::io::Read, bytes: u64, to: &Path, free: u64) -> Result<(), AppError> {
    let need = bytes.saturating_mul(5);
    if free < need {
        return Err(AppError::of(
            "no_space",
            Msg::BackupNeedsSpace,
            [(free >> 20).to_string(), (need >> 20).to_string()],
        ));
    }
    let result = (|| {
        let mut out = std::fs::File::create(to).map_err(|e| AppError::io(Note::of(Msg::DoingPrivateCopy, []), e))?;
        let n = std::io::copy(input, &mut out).map_err(|e| AppError::io(Note::of(Msg::DoingCopyingPicked, []), e))?;
        if n == 0 {
            return Err(AppError::of("not_crypt15", Msg::PickedEmpty, []));
        }
        out.sync_all().map_err(|e| AppError::io(Note::of(Msg::DoingCopyingPicked, []), e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(to);
    }
    result
}

/// Writes a file oopz made (a merged backup, a saved page) to where the person picked (an Android `content://`
/// reference, or a path). Only files inside folders this run made.
#[tauri::command(async)]
fn save_result(app: tauri::AppHandle, state: State<'_, AppState>, from: PathBuf, to: String) -> Result<u64, AppError> {
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
    let from = ours(&lock(&state.made), &from)?;
    let target: FilePath = to.parse().map_err(|_| AppError::of("not_found", Msg::NotFileReference, []))?;
    let mut out = app
        .fs()
        .open(target, OpenOptions::new().write(true).create(true).truncate(true).to_owned())
        .map_err(|e| AppError::doing("io", Note::of(Msg::DoingOpeningSave, []), e))?;
    let mut input = std::fs::File::open(&from).map_err(|e| AppError::io(Note::of(Msg::DoingReadingResult, []), e))?;
    let n = std::io::copy(&mut input, &mut out).map_err(|e| AppError::io(Note::of(Msg::DoingSavingResult, []), e))?;
    out.sync_all().map_err(|e| AppError::io(Note::of(Msg::DoingSavingResult, []), e))?;
    Ok(n)
}

/// `from`, resolved (links followed, `..` gone), when it's a file inside a folder this run made.
fn ours(made: &[PathBuf], from: &Path) -> Result<PathBuf, AppError> {
    let refused = || AppError::of("io", Msg::NotOursFile, []);
    let from = from.canonicalize().map_err(|_| refused())?;
    let inside = made.iter().filter_map(|m| m.canonicalize().ok()).any(|m| from.starts_with(m));
    if inside && from.is_file() { Ok(from) } else { Err(refused()) }
}

/// "Wipe now" (see [`AppState::wipe`]), and the library with it, kept or not ([`library::Lib::wipe`]).
#[tauri::command(async)]
fn wipe_now(state: State<'_, AppState>, lib: State<'_, library::Lib>) -> usize {
    state.wipe() + lib.wipe()
}

/// Opens one of the app's own pages in the browser; the page names which, never a URL, so it can't open anything else.
#[tauri::command(async)]
fn open_link(app: tauri::AppHandle, which: String) -> Result<(), AppError> {
    let url = link_url(&which)?;
    // The opener plugin, from Rust only: it works on Android too, where `open` can't.
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::doing("io", Note::of(Msg::DoingOpeningBrowser, []), e))
}

/// The app's own pages, by name.
fn link_url(which: &str) -> Result<&'static str, AppError> {
    Ok(match which {
        "source" => "https://github.com/Sparklitt/oopz",
        "report" => "https://github.com/Sparklitt/oopz/issues/new",
        "security" => "https://github.com/Sparklitt/oopz/security/advisories/new",
        "licences" => "https://github.com/Sparklitt/oopz/blob/main/LICENSE",
        "releases" => "https://github.com/Sparklitt/oopz/releases",
        _ => return Err(AppError::of("not_found", Msg::NoLink, [which.to_owned()])),
    })
}

#[tauri::command(async)]
fn missing(state: State<'_, AppState>, a: u32, chat_a: i64, b: u32, chat_b: i64) -> Result<Missing, AppError> {
    lock(&state.session).missing(a, chat_a, b, chat_b)
}

#[tauri::command(async)]
fn receive_start(
    state: State<'_, AppState>,
    parent: PathBuf,
    ip: Option<std::net::IpAddr>,
) -> Result<ReceiveInfo, AppError> {
    state.receive_start(&parent, ip)
}

#[tauri::command]
fn receive_status(state: State<'_, AppState>) -> ReceiveStatus {
    state.receive_status()
}

#[tauri::command(async)]
fn receive_stop(state: State<'_, AppState>) -> ReceiveStatus {
    state.receive_stop()
}

/// Whether adb is on this computer (or the copy oopz downloaded): without it the adb screen offers Google's.
#[tauri::command]
fn adb_found() -> bool {
    adb::find().is_some()
}

#[tauri::command(async)]
fn adb_devices() -> Result<Vec<Device>, AppError> {
    Adb::find()?.devices()
}

/// Wireless debugging: pairs with the code on the phone's screen (used once, never kept).
#[tauri::command(async)]
fn adb_pair(host: String, code: Zeroizing<String>) -> Result<(), AppError> {
    Adb::find()?.pair(&host, &code)
}

#[tauri::command(async)]
fn adb_connect(host: String) -> Result<(), AppError> {
    Adb::find()?.connect(&host)
}

#[tauri::command(async)]
fn adb_pull_start(
    state: State<'_, AppState>,
    parent: PathBuf,
    serial: String,
    business: bool,
    media: bool,
) -> Result<PullInfo, AppError> {
    let want = Wanted { business, media, remote_dir: None };
    state.pull_start(&Adb::find()?, &parent, &serial, &want)
}

#[tauri::command]
fn adb_pull_status(state: State<'_, AppState>) -> PullStatus {
    state.pull_status()
}

#[tauri::command(async)]
fn adb_pull_stop(state: State<'_, AppState>) -> PullStatus {
    state.pull_stop()
}

#[tauri::command(async)]
fn build_index(state: State<'_, AppState>, id: u32, progress: Channel<Step>) -> Result<BuildInfo, AppError> {
    state.build_index(id, &mut self::progress(&progress, &state.cancel, |_, _| "index"))
}

#[tauri::command(async)]
fn search(state: State<'_, AppState>, id: u32, query: String, filter: Filter) -> Result<Vec<Hit>, AppError> {
    lock(&state.session).search(id, &query, &filter, session::SEARCH_LIMIT)
}

#[tauri::command(async)]
fn stats(state: State<'_, AppState>, id: u32, chat: Option<i64>) -> Result<Stats, AppError> {
    lock(&state.session).stats(id, chat)
}

#[tauri::command(async)]
fn chat_len(state: State<'_, AppState>, id: u32, chat: i64) -> Result<u64, AppError> {
    lock(&state.session).chat_len(id, chat)
}

#[tauri::command(async)]
fn chat_lines(state: State<'_, AppState>, id: u32, chat: i64, from: u64, count: u32) -> Result<Vec<Line>, AppError> {
    lock(&state.session).lines(id, chat, from, count.min(session::LINES_MAX))
}

/// The diagnostics the CLI prints (`oopz tables`, `types`, `chat-facts`): structure and counts only, for a problem
/// report.
#[tauri::command(async)]
fn backup_tables(state: State<'_, AppState>, id: u32) -> Result<Vec<TableInfo>, AppError> {
    lock(&state.session).tables(id)
}

#[tauri::command(async)]
fn chat_types(state: State<'_, AppState>, id: u32, chat: i64) -> Result<Vec<TypeInfo>, AppError> {
    lock(&state.session).types(id, chat)
}

#[tauri::command(async)]
fn chat_facts(state: State<'_, AppState>, id: u32, chat: i64) -> Result<Vec<Fact>, AppError> {
    lock(&state.session).chat_facts(id, chat)
}

#[tauri::command(async)]
fn message_position(state: State<'_, AppState>, id: u32, message: i64) -> Result<Option<(i64, u64)>, AppError> {
    lock(&state.session).position(id, message)
}

/// "Jump to a date": the position of the chat's first message at or after `ts` (ms).
#[tauri::command(async)]
fn position_at(state: State<'_, AppState>, id: u32, chat: i64, ts: i64) -> Result<Option<u64>, AppError> {
    lock(&state.session).position_at(id, chat, ts)
}

/// The opt-in update check (decision 0011): only when the person switched it on or clicked, and installed only on
/// their click. The download's signature is checked against the key in tauri.conf.json before anything runs.
#[cfg(desktop)]
mod update {
    use std::sync::Mutex;

    use oopz_app::AppError;
    use serde::Serialize;
    use tauri::ipc::Channel;
    use tauri::{AppHandle, Manager, State};
    use tauri_plugin_updater::{Update, UpdaterExt};

    use super::{AppState, Step, lock, progress};

    /// The update the last check found, until the person installs it.
    #[derive(Default)]
    pub struct Pending(Mutex<Option<Update>>);

    #[derive(Serialize)]
    pub struct Found {
        pub version: String,
        /// The release notes, as the release's latest.json carries them (Markdown).
        pub notes: Option<String>,
        pub date_ms: Option<i64>,
    }

    /// What a check found, and when it ran.
    #[derive(Serialize)]
    pub struct Checked {
        pub found: Option<Found>,
        pub checked_ms: i64,
    }

    /// The file holding when the last check succeeded: a number of milliseconds, nothing else.
    fn checked_file(app: &AppHandle) -> Option<std::path::PathBuf> {
        app.path().app_config_dir().ok().map(|d| d.join("update-checked"))
    }

    /// When the last update check succeeded, for "Last checked …".
    #[tauri::command]
    pub fn update_last_checked(app: AppHandle) -> Option<i64> {
        std::fs::read_to_string(checked_file(&app)?).ok()?.trim().parse().ok()
    }

    fn failed(e: tauri_plugin_updater::Error) -> AppError {
        AppError::new("update", msgkinds::Note::of(oopz_model::messages::Message::SystemReason, [e.to_string()]))
    }

    #[tauri::command]
    pub async fn update_check(app: AppHandle, pending: State<'_, Pending>) -> Result<Checked, AppError> {
        // On Windows the installer is started and the app quits at once, without its exit event: wipe first.
        let handle = app.clone();
        let updater = app
            .updater_builder()
            .on_before_exit(move || {
                handle.state::<AppState>().close();
                handle.state::<super::library::Lib>().close();
                handle.cleanup_before_exit();
            })
            .build()
            .map_err(failed)?;
        let update = updater.check().await.map_err(failed)?;
        let found = update.as_ref().map(|u| Found {
            version: u.version.clone(),
            notes: u.body.clone(),
            date_ms: u.date.map(|d| d.unix_timestamp() * 1000),
        });
        *lock(&pending.0) = update;
        let checked_ms =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        if let Some(file) = checked_file(&app) {
            let _ = file.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(file, checked_ms.to_string());
        }
        Ok(Checked { found, checked_ms })
    }

    /// Downloads, checks the signature, installs and restarts (the restart runs the exit event, which wipes).
    #[tauri::command]
    pub async fn update_install(
        app: AppHandle,
        pending: State<'_, Pending>,
        progress: Channel<Step>,
    ) -> Result<(), AppError> {
        let update =
            lock(&pending.0).take().ok_or_else(|| AppError::of("update", oopz_app::messages::Message::NoUpdate, []))?;
        // A download isn't cancelled: the flag stays false.
        let never = std::sync::atomic::AtomicBool::new(false);
        let mut report = self::progress(&progress, &never, |_, _| "download");
        let mut done = 0u64;
        update
            .download_and_install(
                |chunk, total| {
                    done += chunk as u64;
                    if let Some(total) = total {
                        report(done, total);
                    }
                },
                || {},
            )
            .await
            .map_err(failed)?;
        app.restart()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init());
    // Desktop only: the window's size and place remembered between runs, and the update check.
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::Pending::default())
        .manage(platform_tools::Shown::default());
    builder
        .setup(|app| {
            let cache = app.path().app_cache_dir()?;
            app.manage(AppState::new(&cache)?);
            app.manage(library::Lib::new(&cache, &app.path().app_data_dir()?));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            find_backups,
            check_backup,
            forget_backup,
            checked_backups,
            merge_chat,
            missing,
            export_page,
            show_folder,
            receive_start,
            receive_status,
            receive_stop,
            adb_found,
            adb_devices,
            adb_pair,
            adb_connect,
            adb_pull_start,
            adb_pull_status,
            adb_pull_stop,
            build_index,
            search,
            stats,
            chat_len,
            chat_lines,
            message_position,
            position_at,
            cancel_work,
            wipe_now,
            import_backup,
            save_result,
            open_link,
            thumbnail,
            media_file,
            backup_tables,
            chat_types,
            chat_facts,
            library::library_info,
            library::library_set_keep,
            library::lib_adapters,
            library::lib_import,
            library::lib_sources,
            library::lib_remove_source,
            library::lib_set_me,
            library::lib_conversations,
            library::lib_conversation_len,
            library::lib_messages,
            library::lib_position_at,
            library::lib_position,
            library::lib_search,
            library::lib_find,
            library::lib_count,
            library::lib_selected,
            library::lib_messages_by_id,
            library::lib_media,
            library::lib_stats,
            library::lib_people,
            library::lib_contacts,
            library::lib_link,
            library::lib_unlink,
            library::lib_rename_contact,
            library::lib_delete_contact,
            library::lib_suggestions,
            library::lib_accept,
            library::lib_import_vcards,
            library::lib_save_selection,
            library::lib_selections,
            library::lib_delete_selection,
            library::lib_export,
            library::lib_export_contacts,
            cli::cli_status,
            cli::cli_install,
            cli::cli_remove,
            #[cfg(desktop)]
            platform_tools::tools_installed,
            #[cfg(desktop)]
            platform_tools::tools_offer,
            #[cfg(desktop)]
            platform_tools::tools_download,
            #[cfg(desktop)]
            platform_tools::tools_remove,
            #[cfg(desktop)]
            update::update_check,
            #[cfg(desktop)]
            update::update_last_checked,
            #[cfg(desktop)]
            update::update_install,
        ])
        .build(tauri::generate_context!())
        .expect("oopz couldn't start its window")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<AppState>().close();
                app.state::<library::Lib>().close();
                #[cfg(desktop)]
                platform_tools::at_exit();
            }
        });
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr, TcpStream};

    use oopz::fixture::{self, KEY_HEX, SOURCE_CHAT, TARGET_CHAT, TARGET_KEY_HEX};

    use super::*;

    fn state() -> (tempfile::TempDir, AppState) {
        let cache = tempfile::tempdir().unwrap();
        let state = AppState::new(cache.path()).unwrap();
        (cache, state)
    }

    #[test]
    fn check_merge_and_forget_through_the_commands() {
        let (_cache, state) = state();
        let two = fixture::two_phones();
        let out = tempfile::tempdir().unwrap();
        let source = state.check(&two.source, KEY_HEX, &mut |_, _| true).unwrap();
        let target = state.check(&two.target, TARGET_KEY_HEX, &mut |_, _| true).unwrap();
        assert_eq!(state.summaries().len(), 2);
        assert_eq!(state.check(&two.target, KEY_HEX, &mut |_, _| true).unwrap_err().code, "wrong_key");

        // The form as the UI sends it, JSON with the key in it: the key goes no further than the merge.
        let json = format!(
            r#"{{"source":{},"source_chat":{SOURCE_CHAT},"target":{},"target_chat":{TARGET_CHAT},
               "target_key":"{TARGET_KEY_HEX}","out_parent":{:?},"media":null}}"#,
            source.id,
            target.id,
            out.path()
        );
        let form: MergeForm = serde_json_from(&json);
        let done = state.merge(&form).unwrap();
        assert!(done.original_untouched && done.added > 0);
        assert!(done.backup.starts_with(out.path()));
        // The app opens the folders it made, and nothing else.
        assert!(state.made(&done.folder).is_ok());
        assert!(state.made(out.path()).is_err());

        // Explore: the index is built on demand, then search and stats answer.
        // While the index builds, the session answers other commands.
        let info = state
            .build_index(target.id, &mut |_, _| {
                assert_eq!(state.summaries().len(), 2);
                true
            })
            .unwrap();
        assert_eq!(info.messages, target.messages);
        assert!(lock(&state.session).stats(target.id, None).unwrap().messages > 0);

        // The export's words as the UI sends them (type codes as JSON keys): a new page in the folder picked.
        let labels: oopz_app::export::Labels = serde_json_from(
            r#"{"lang":"es","heading":"chat","messages":"mensajes","sent":"enviados","received":"recibidos",
               "dates":"fechas","by_month":"por mes","by_hour":"por hora","by_kind":"por tipo",
               "audio_minutes":"minutos","senders":"quién","you":"Tú","note":"sin cifrar",
               "kinds":{"0":"texto","1":"foto"}}"#,
        );
        let page = state.export(target.id, Some(TARGET_CHAT), &labels, out.path()).unwrap();
        assert!(std::fs::read_to_string(&page).unwrap().contains("lang=\"es\""));
        assert!(state.made(out.path()).is_ok());

        state.forget(source.id);
        assert_eq!(state.summaries().len(), 1);

        // Closing the app leaves nothing it decrypted behind.
        state.close();
        assert!(!state.tmp.path().exists());
        assert!(state.summaries().is_empty());
        assert_eq!(state.merge(&form).unwrap_err().code, "not_found");
    }

    #[test]
    fn the_wifi_page_starts_in_a_new_folder_and_stops() {
        let (_cache, state) = state();
        let parent = tempfile::tempdir().unwrap();
        std::fs::create_dir(parent.path().join("oopz-received-1")).unwrap();
        let info = state.receive_start(parent.path(), Some(IpAddr::V4(Ipv4Addr::LOCALHOST))).unwrap();
        assert_eq!(info.dest, parent.path().join("oopz-received-2"));
        // Another address picked before anything arrived: a new page, the same folder.
        let info = state.receive_start(parent.path(), Some(IpAddr::V4(Ipv4Addr::LOCALHOST))).unwrap();
        assert_eq!(info.dest, parent.path().join("oopz-received-2"));
        assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 2);
        assert!(info.qr_svg.contains("<svg"));
        assert!(state.receive_status().running);

        let addr = info.url.trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let path = format!("/{}", info.url.trim_start_matches("http://").split_once('/').unwrap().1);
        let mut s = TcpStream::connect(&addr).unwrap();
        write!(s, "PUT {path}files/msgstore.db.crypt15 HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nConnection: close\r\n\r\nbytes")
            .unwrap();
        let mut reply = String::new();
        s.read_to_string(&mut reply).unwrap();
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        assert_eq!(state.receive_status().files.len(), 1);

        let stopped = state.receive_stop();
        assert_eq!(stopped.stopped, Some("stopped"));
        assert!(!state.receive_status().running);
        // The freed port may already be another listener's (the OS reuses it), so it may answer, but never as this
        // page: nothing comes back, or its path is refused (as in oopz-app's receive tests).
        if let Ok(mut s) = TcpStream::connect(&addr) {
            let mut reply = String::new();
            if write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").is_ok()
                && s.read_to_string(&mut reply).is_ok()
            {
                assert!(
                    reply.is_empty() || reply.starts_with("HTTP/1.1 404"),
                    "still answering after it stopped: {reply}"
                );
            }
        }
    }

    /// Tauri's deserializer for command arguments is serde_json.
    fn serde_json_from<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_check_stops_when_cancelled_and_wipe_now_empties_the_private_folder() {
        let (_cache, state) = state();
        let two = fixture::two_phones();
        // `cancel_work` while it runs: the next progress report stops it, and nothing it decrypted is left.
        let cancelled = state.check(&two.source, KEY_HEX, &mut |_, _| {
            state.cancel.store(true, Ordering::SeqCst);
            false
        });
        assert_eq!(cancelled.unwrap_err().code, "cancelled");
        assert_eq!(std::fs::read_dir(state.tmp.path()).unwrap().count(), 0);
        // A new check resets the flag.
        let s = state.check(&two.source, KEY_HEX, &mut |_, _| true).unwrap();
        state.build_index(s.id, &mut |_, _| true).unwrap();
        assert!(std::fs::read_dir(state.tmp.path()).unwrap().count() > 0);
        assert_eq!(state.wipe(), 0);
        assert!(state.summaries().is_empty());
        // The folder is still there for the next check.
        assert!(state.check(&two.source, KEY_HEX, &mut |_, _| true).is_ok());
    }

    #[test]
    fn only_the_apps_own_links_open() {
        assert_eq!(link_url("https://example.com").unwrap_err().code, "not_found");
        assert_eq!(link_url("").unwrap_err().code, "not_found");
        assert!(link_url("source").unwrap().starts_with("https://github.com/Sparklitt/oopz"));
    }

    #[test]
    fn a_check_reports_its_steps_in_order() {
        let mut steps = Vec::new();
        let total = 400;
        for done in (0..=total).step_by(50) {
            let step = session::check_step(done, total);
            if steps.last() != Some(&step) {
                steps.push(step);
            }
        }
        assert_eq!(steps, ["fingerprint", "read", "unpack", "prepare"]);
    }

    #[test]
    fn a_picked_backup_is_copied_in_only_when_it_fits_and_whole() {
        let tmp = tempfile::tempdir().unwrap();
        let backup = vec![1u8; 4096];
        let to = tmp.path().join("picked-1.crypt15");
        copy_in(&mut &backup[..], 4096, &to, 5 * 4096).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), backup);
        let other = tmp.path().join("picked-2.crypt15");
        assert_eq!(copy_in(&mut &backup[..], 4096, &other, 5 * 4096 - 1).unwrap_err().code, "no_space");
        assert_eq!(copy_in(&mut &b""[..], 0, &other, u64::MAX).unwrap_err().code, "not_crypt15");
        // A provider that fails midway: nothing is left.
        struct CutOff(u8);
        impl std::io::Read for CutOff {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.0 += 1;
                if self.0 > 2 {
                    Err(std::io::Error::other("cut off"))
                } else {
                    buf[0] = 1;
                    Ok(1)
                }
            }
        }
        assert_eq!(copy_in(&mut CutOff(0), 0, &other, u64::MAX).unwrap_err().code, "io");
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1, "only the first, whole copy");
    }

    #[test]
    fn only_files_inside_folders_oopz_made_are_saved_elsewhere() {
        let root = tempfile::tempdir().unwrap();
        let made = root.path().join("oopz-result-1");
        std::fs::create_dir_all(made.join("Databases")).unwrap();
        let result = made.join("Databases/msgstore.db.crypt15");
        std::fs::write(&result, b"x").unwrap();
        let outside = root.path().join("secret.txt");
        std::fs::write(&outside, b"x").unwrap();
        let list = vec![made.clone()];
        assert!(ours(&list, &result).is_ok());
        assert!(ours(&list, &made).is_err(), "a folder, not a file");
        assert!(ours(&list, &outside).is_err());
        assert!(ours(&list, &made.join("../secret.txt")).is_err(), "no way out with ..");
        assert!(ours(&list, &made.join("missing")).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, made.join("link")).unwrap();
            assert!(ours(&list, &made.join("link")).is_err(), "no way out through a link");
        }
    }
}
