//! The Tauri shell: each command wraps one call of `oopz-app` (decision 0009). Keys arrive as `Zeroizing` strings,
//! are used for that one call and wiped; nothing is logged, and nothing but the person's own results is written.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use oopz_app::index::{BuildInfo, Hit, Line, Stats};
use oopz_app::receive::{self, Receiver};
use oopz_app::session::{
    self, BackupFile, Fact, MergeOutcome, MergeRequest, Missing, Session, Summary, TableInfo, TypeInfo,
};
use oopz_app::{AppError, ReceiveInfo, ReceiveStatus};
use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{Manager, State};
use zeroize::Zeroizing;

/// What the app holds while it runs. The decrypted copies live in `tmp`, removed when the app closes.
pub struct AppState {
    session: Mutex<Session>,
    receiver: Mutex<Option<Receiver>>,
    tmp: tempfile::TempDir,
    /// Folders this run of the app made (merge results, the folders pages were saved in): the only ones it opens.
    made: Mutex<Vec<PathBuf>>,
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
        std::fs::create_dir_all(&dir).map_err(|e| AppError::new("io", format!("creating the app's cache: {e}")))?;
        let tmp = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(&dir)
            .map_err(|e| AppError::new("io", format!("creating the app's cache: {e}")))?;
        Ok(Self { session: Mutex::default(), receiver: Mutex::default(), tmp, made: Mutex::default() })
    }

    pub fn check(&self, path: &Path, key: &str, progress: &mut dyn FnMut(u64, u64)) -> Result<Summary, AppError> {
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
        let _ = std::fs::remove_dir_all(self.tmp.path());
    }

    /// Only a folder this run made, so a page can't ask the app to open just anything.
    pub fn made(&self, folder: &Path) -> Result<(), AppError> {
        if lock(&self.made).iter().any(|m| m == folder) {
            Ok(())
        } else {
            Err(AppError::new("io", "not a folder oopz made".to_owned()))
        }
    }

    /// Starts the Wi-Fi page ([`receive::start_in`]). One page at a time: a running one is stopped first.
    pub fn receive_start(&self, parent: &Path, ip: Option<std::net::IpAddr>) -> Result<ReceiveInfo, AppError> {
        let mut slot = lock(&self.receiver);
        if let Some(old) = slot.take() {
            old.stop();
        }
        let (rx, info) = receive::start_in(parent, ip)?;
        *slot = Some(rx);
        Ok(info)
    }

    /// Builds a backup's search index without holding the session, so the other commands answer meanwhile.
    pub fn build_index(&self, id: u32, progress: &mut dyn FnMut(u64, u64)) -> Result<BuildInfo, AppError> {
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
}

/// Progress for the UI's bar, in thousandths, sent only when it moves (a channel message per thousandth at most).
fn per_mille(channel: &Channel<u16>) -> impl FnMut(u64, u64) + '_ {
    let mut last = None;
    move |done, total| {
        let now = (done.min(total) * 1000 / total.max(1)) as u16;
        if last != Some(now) {
            last = Some(now);
            let _ = channel.send(now);
        }
    }
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
    progress: Channel<u16>,
) -> Result<Summary, AppError> {
    state.check(&path, &key, &mut per_mille(&progress))
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
    open::that_detached(&folder).map_err(|e| AppError::new("io", format!("opening the folder: {e}")))
}

#[tauri::command(async)]
fn missing(state: State<'_, AppState>, a: u32, chat_a: i64, b: u32, chat_b: i64) -> Result<Missing, AppError> {
    lock(&state.session).missing(a, chat_a, b, chat_b)
}

#[tauri::command(async)]
fn receive_start(state: State<'_, AppState>, parent: PathBuf) -> Result<ReceiveInfo, AppError> {
    state.receive_start(&parent, None)
}

#[tauri::command]
fn receive_status(state: State<'_, AppState>) -> ReceiveStatus {
    state.receive_status()
}

#[tauri::command(async)]
fn receive_stop(state: State<'_, AppState>) -> ReceiveStatus {
    state.receive_stop()
}

#[tauri::command(async)]
fn build_index(state: State<'_, AppState>, id: u32, progress: Channel<u16>) -> Result<BuildInfo, AppError> {
    state.build_index(id, &mut per_mille(&progress))
}

#[tauri::command(async)]
fn search(state: State<'_, AppState>, id: u32, query: String, chat: Option<i64>) -> Result<Vec<Hit>, AppError> {
    lock(&state.session).search(id, &query, chat, session::SEARCH_LIMIT)
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

    use super::{AppState, lock, per_mille};

    /// The update the last check found, until the person installs it.
    #[derive(Default)]
    pub struct Pending(Mutex<Option<Update>>);

    #[derive(Serialize)]
    pub struct Found {
        pub version: String,
    }

    fn failed(e: tauri_plugin_updater::Error) -> AppError {
        AppError::new("update", e.to_string())
    }

    #[tauri::command]
    pub async fn update_check(app: AppHandle, pending: State<'_, Pending>) -> Result<Option<Found>, AppError> {
        // On Windows the installer is started and the app quits at once, without its exit event: wipe first.
        let handle = app.clone();
        let updater = app
            .updater_builder()
            .on_before_exit(move || {
                handle.state::<AppState>().close();
                handle.cleanup_before_exit();
            })
            .build()
            .map_err(failed)?;
        let update = updater.check().await.map_err(failed)?;
        let found = update.as_ref().map(|u| Found { version: u.version.clone() });
        *lock(&pending.0) = update;
        Ok(found)
    }

    /// Downloads, checks the signature, installs and restarts (the restart runs the exit event, which wipes).
    #[tauri::command]
    pub async fn update_install(
        app: AppHandle,
        pending: State<'_, Pending>,
        progress: Channel<u16>,
    ) -> Result<(), AppError> {
        let update = lock(&pending.0).take().ok_or_else(|| AppError::new("update", "no update to install"))?;
        let mut report = per_mille(&progress);
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

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Remembers the window's size and place between runs.
        .plugin(tauri_plugin_window_state::Builder::default().build());
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build()).manage(update::Pending::default());
    builder
        .setup(|app| {
            let cache = app.path().app_cache_dir()?;
            app.manage(AppState::new(&cache)?);
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
            build_index,
            search,
            stats,
            chat_len,
            chat_lines,
            message_position,
            #[cfg(desktop)]
            update::update_check,
            #[cfg(desktop)]
            update::update_install,
        ])
        .build(tauri::generate_context!())
        .expect("oopz couldn't start its window")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<AppState>().close();
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
        let source = state.check(&two.source, KEY_HEX, &mut |_, _| {}).unwrap();
        let target = state.check(&two.target, TARGET_KEY_HEX, &mut |_, _| {}).unwrap();
        assert_eq!(state.summaries().len(), 2);
        assert_eq!(state.check(&two.target, KEY_HEX, &mut |_, _| {}).unwrap_err().code, "wrong_key");

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
        let info = state.build_index(target.id, &mut |_, _| assert_eq!(state.summaries().len(), 2)).unwrap();
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
}
