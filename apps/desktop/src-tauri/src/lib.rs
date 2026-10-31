//! The Tauri shell: each command wraps one call of `oopz-app` (decision 0009). Keys arrive as `Zeroizing` strings,
//! are used for that one call and wiped; nothing is logged, and nothing but the person's own results is written.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use oopz_app::index::{BuildInfo, Hit, Line, Stats};
use oopz_app::receive::{self, Receiver};
use oopz_app::session::{self, BackupFile, MergeOutcome, MergeRequest, Missing, Session, Summary};
use oopz_app::{AppError, ReceiveStatus};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{Manager, State};
use zeroize::Zeroizing;

/// The Wi-Fi page stops by itself after this long without a request.
const RECEIVE_IDLE: Duration = Duration::from_secs(10 * 60);

/// What the app holds while it runs. The decrypted copies live in `tmp`, removed when the app closes.
pub struct AppState {
    session: Mutex<Session>,
    receiver: Mutex<Option<Receiver>>,
    tmp: tempfile::TempDir,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the Wi-Fi screen shows: the QR code, the address (the person may type it) and where files go.
#[derive(Debug, Serialize)]
pub struct ReceiveInfo {
    pub url: String,
    pub qr_svg: String,
    pub dest: PathBuf,
    /// Bytes free where the files go: a Media zip can be tens of GB.
    pub free: u64,
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
        Ok(Self { session: Mutex::default(), receiver: Mutex::default(), tmp })
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
        lock(&self.session).merge(&MergeRequest {
            source: form.source,
            source_chat: form.source_chat,
            target: form.target,
            target_chat: form.target_chat,
            target_key: &form.target_key,
            out_parent: &form.out_parent,
            media: form.media.as_deref(),
        })
    }

    /// Starts the Wi-Fi page into a new `oopz-received-N` folder in `parent`, on this computer's LAN address (or on
    /// `ip`, for tests). One page at a time: a running one is stopped first.
    pub fn receive_start(&self, parent: &Path, ip: Option<std::net::IpAddr>) -> Result<ReceiveInfo, AppError> {
        let ip = ip.or_else(receive::lan_ip).ok_or_else(|| {
            AppError::new(
                "network",
                "this computer isn't on a local network; connect it to the same Wi-Fi as the phone",
            )
        })?;
        let mut slot = lock(&self.receiver);
        if let Some(old) = slot.take() {
            old.stop();
        }
        let dest = (1..1000)
            .map(|n| parent.join(format!("oopz-received-{n}")))
            .find(|p| !p.exists())
            .ok_or_else(|| AppError::new("exists", "too many oopz-received folders here; choose another folder"))?;
        let rx = Receiver::start(&dest, ip, RECEIVE_IDLE)?;
        let free = session::free_space(&dest)?;
        let info = ReceiveInfo { url: rx.url().to_owned(), qr_svg: rx.qr_svg()?, dest, free };
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
    lock(&state.session).export(id, chat, &labels, &out_dir)
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
    lock(&state.session).search(id, &query, chat, 200)
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
    lock(&state.session).lines(id, chat, from, count.min(500))
}

#[tauri::command(async)]
fn message_position(state: State<'_, AppState>, id: u32, message: i64) -> Result<Option<(i64, u64)>, AppError> {
    lock(&state.session).position(id, message)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            receive_start,
            receive_status,
            receive_stop,
            build_index,
            search,
            stats,
            chat_len,
            chat_lines,
            message_position,
        ])
        .run(tauri::generate_context!())
        .expect("oopz couldn't start its window");
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
        let page = lock(&state.session).export(target.id, Some(TARGET_CHAT), &labels, out.path()).unwrap();
        assert!(std::fs::read_to_string(&page).unwrap().contains("lang=\"es\""));

        state.forget(source.id);
        assert_eq!(state.summaries().len(), 1);
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
        assert!(TcpStream::connect(&addr).is_err());
    }

    /// Tauri's deserializer for command arguments is serde_json.
    fn serde_json_from<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).unwrap()
    }
}
