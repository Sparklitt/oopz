//! Copying WhatsApp's backup off a phone with adb, for people who use USB or wireless debugging. oopz doesn't bundle
//! adb: it runs the one on `PATH`, in Android's platform-tools (`$ANDROID_HOME`), or the copy the desktop app
//! downloaded from Google when asked (decision 0019, [`downloaded_dir`]). It only reads the phone: it lists
//! a folder (`find`, `stat`) and copies it (`adb pull`) into a new `oopz-pulled-N` folder here, checks every file's
//! size, and leaves no half-copied file behind. The pairing code for wireless debugging goes to adb and nowhere else.

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::{AppError, Result};

/// Where to get adb when it's missing.
pub const PLATFORM_TOOLS: &str = "https://developer.android.com/tools/releases/platform-tools";

const ADB: &str = if cfg!(windows) { "adb.exe" } else { "adb" };

/// How long pairing or connecting may take: adb keeps waiting for an address that doesn't answer.
const WIRELESS: Duration = Duration::from_secs(30);

/// adb on `PATH`, else in `$ANDROID_HOME/platform-tools` (or `$ANDROID_SDK_ROOT`'s), else the copy the desktop app
/// downloaded. A system adb comes first: two adb servers of different versions fight over the same port.
pub fn find() -> Option<PathBuf> {
    find_in(
        std::env::var_os("PATH"),
        [std::env::var_os("ANDROID_HOME"), std::env::var_os("ANDROID_SDK_ROOT")],
        downloaded_dir(),
    )
}

fn find_in(path: Option<OsString>, sdks: [Option<OsString>; 2], downloaded: Option<PathBuf>) -> Option<PathBuf> {
    let on_path = path.iter().flat_map(std::env::split_paths).collect::<Vec<_>>();
    let in_sdks = sdks.into_iter().flatten().map(|sdk| PathBuf::from(sdk).join("platform-tools"));
    on_path.into_iter().chain(in_sdks).chain(downloaded).map(|dir| dir.join(ADB)).find(|adb| adb.is_file())
}

/// Where the desktop app keeps the platform-tools it downloaded: its local data folder (Tauri's
/// `app_local_data_dir`, named after the app's identifier) plus `platform-tools`. None without a home folder.
pub fn downloaded_dir() -> Option<PathBuf> {
    const APP: &str = "com.sparklitt.oopz";
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let data = if cfg!(windows) {
        env("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else {
        env("XDG_DATA_HOME").filter(|p| p.is_absolute()).or_else(|| env("HOME").map(|h| h.join(".local/share")))?
    };
    Some(data.join(APP).join("platform-tools"))
}

/// A phone adb sees.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Device {
    pub serial: String,
    /// `device` (ready), `unauthorized` (the phone hasn't said yes), `offline`, `no_permissions` (Linux: no udev
    /// rule), or what adb said.
    pub state: String,
    /// As the phone names itself (`SM G965F`), if adb says.
    pub model: Option<String>,
    /// Over wireless debugging rather than USB.
    pub wireless: bool,
}

/// `adb devices -l`'s list.
fn parse_devices(out: &str) -> Vec<Device> {
    out.lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.starts_with("List of devices") && !line.starts_with('*'))
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.to_owned();
            let state = match parts.next()? {
                "no" => "no_permissions".to_owned(),
                state => state.to_owned(),
            };
            let model = parts.find_map(|p| p.strip_prefix("model:")).map(|m| m.replace('_', " "));
            let wireless = serial.contains(':') || serial.contains("._adb-tls-connect.");
            Some(Device { serial, state, model, wireless })
        })
        .collect()
}

/// The phone to copy from: the one named, or the only one. Refuses to guess between several, and explains a phone
/// that isn't ready.
pub fn choose<'a>(devices: &'a [Device], serial: Option<&str>) -> Result<&'a Device> {
    let device = match serial {
        Some(serial) => devices.iter().find(|d| d.serial == serial).ok_or_else(|| {
            AppError::new(
                "no_device",
                format!("adb doesn't see a phone named {serial} (it sees: {})", serials(devices)),
            )
        })?,
        None => match devices {
            [] => {
                return Err(AppError::new(
                    "no_device",
                    "adb sees no phone: connect it with a USB cable and turn on USB debugging, or pair it for wireless debugging",
                ));
            }
            [one] => one,
            several => {
                return Err(AppError::new(
                    "several_devices",
                    format!("several phones are connected ({}): choose one with --serial", serials(several)),
                ));
            }
        },
    };
    match device.state.as_str() {
        "device" => Ok(device),
        "unauthorized" => Err(AppError::new(
            "unauthorized",
            "the phone hasn't allowed this computer yet: unlock it and tap Allow when it asks about USB debugging",
        )),
        "offline" => Err(AppError::new(
            "device_offline",
            "the phone is offline for adb: unplug it and plug it in again, or turn USB debugging off and on",
        )),
        "no_permissions" => Err(AppError::new(
            "no_permissions",
            "this computer doesn't let adb use the phone (on Linux, add a udev rule for Android phones)",
        )),
        other => Err(AppError::new("adb", format!("the phone's adb state is {other}"))),
    }
}

fn serials(devices: &[Device]) -> String {
    if devices.is_empty() {
        return "none".into();
    }
    devices.iter().map(|d| d.serial.as_str()).collect::<Vec<_>>().join(", ")
}

/// adb, found.
#[derive(Debug, Clone)]
pub struct Adb {
    exe: PathBuf,
}

impl Adb {
    /// [`find`], or a `no_adb` error that says where to get it.
    pub fn find() -> Result<Self> {
        find().map(|exe| Self { exe }).ok_or_else(|| {
            AppError::new(
                "no_adb",
                format!(
                    "adb isn't on this computer: the desktop app can download it (Add a backup › With USB or wireless debugging), or get Android's platform-tools from {PLATFORM_TOOLS} and put its folder on PATH (or set ANDROID_HOME)"
                ),
            )
        })
    }

    /// The phones adb sees, over USB and wireless debugging.
    pub fn devices(&self) -> Result<Vec<Device>> {
        Ok(parse_devices(&run(&self.exe, &["devices", "-l"], None, Duration::from_secs(60))?))
    }

    /// Wireless debugging: pairs with the phone at `host` (its "Pair device with pairing code" screen) with `code`,
    /// sent to adb on its standard input, never as an argument.
    pub fn pair(&self, host: &str, code: &str) -> Result<()> {
        let host = host_port(host)?;
        let code = code.trim();
        if code.is_empty() || !code.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::new("adb", "the pairing code is the 6 digits on the phone's pairing screen"));
        }
        let out = run(&self.exe, &["pair", host], Some(code), WIRELESS)?;
        if out.contains("Successfully paired") {
            Ok(())
        } else {
            Err(AppError::new("adb", format!("pairing didn't work: {}", last_line(&out))))
        }
    }

    /// Wireless debugging: connects to the phone at `host` (its Wireless debugging screen's "IP address & port").
    pub fn connect(&self, host: &str) -> Result<()> {
        let out = run(&self.exe, &["connect", host_port(host)?], None, WIRELESS).map_err(|e| {
            AppError::new(
                e.code,
                format!("{}: check the address and port, and that the phone is on this Wi-Fi", e.detail),
            )
        })?;
        // adb says "failed to connect" or "cannot connect" and still exits with 0.
        if out.contains("connected to") && !out.contains("failed") && !out.contains("cannot") {
            Ok(())
        } else {
            Err(AppError::new("adb", format!("connecting didn't work: {}", last_line(&out))))
        }
    }

    /// The phone named `serial`, to list and copy from.
    pub fn phone(&self, serial: &str) -> Phone {
        Phone { exe: self.exe.clone(), serial: serial.to_owned() }
    }
}

/// `HOST:PORT`, checked so it can't be taken for an option.
fn host_port(host: &str) -> Result<&str> {
    let host = host.trim();
    let ok = host.contains(':') && !host.starts_with('-') && !host.chars().any(char::is_whitespace);
    if ok {
        Ok(host)
    } else {
        Err(AppError::new("adb", format!("not an address and port like 192.168.1.20:37099: {host}")))
    }
}

/// The line of adb's output that says what went wrong: the last that mentions an error, else the last.
fn last_line(out: &str) -> &str {
    let lines = || out.split(['\n', '\r']).map(str::trim).filter(|l| !l.is_empty());
    let bad = |l: &&str| l.contains("error") || l.contains("failed") || l.contains("cannot");
    lines().rfind(bad).or_else(|| lines().next_back()).unwrap_or("adb said nothing")
}

fn command(exe: &Path) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Command::new(exe);
    #[cfg(windows)]
    {
        // No console window flashing up behind the app.
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

/// Runs adb with `args` (and `input` on its standard input), for at most `limit`; what it printed, or an error with
/// what it said.
fn run(exe: &Path, args: &[&str], input: Option<&str>, limit: Duration) -> Result<String> {
    let mut child = command(exe)
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::io(format!("starting {}", exe.display()), e))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = writeln!(stdin, "{input}");
    }
    let (stdout, stderr) = (drain(child.stdout.take()), drain(child.stderr.take()));
    let deadline = Instant::now() + limit;
    let Some(status) = wait(&mut child, Duration::from_millis(50), &mut || Instant::now() < deadline)? else {
        return Err(AppError::new("adb", format!("adb didn't answer within {} s", limit.as_secs())));
    };
    let (stdout, stderr) = (stdout(), stderr());
    if status.success() { Ok(format!("{stdout}{stderr}")) } else { Err(adb_error(&format!("{stderr}{stdout}"))) }
}

/// Reads one of adb's pipes as it comes, so a full pipe never holds adb up; what it read, once adb is done.
fn drain(pipe: Option<impl Read + Send + 'static>) -> impl FnOnce() -> String {
    let reader = pipe.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    });
    move || reader.and_then(|r| r.join().ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// Waits for adb, asking `go_on` every `every`; when that says no, stops adb and returns none.
fn wait(child: &mut Child, every: Duration, go_on: &mut dyn FnMut() -> bool) -> Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = child.try_wait().map_err(|e| AppError::io("running adb", e))? {
            return Ok(Some(status));
        }
        if !go_on() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(every);
    }
}

/// What adb said when it failed, as a code the app words.
fn adb_error(said: &str) -> AppError {
    let line = last_line(said);
    let line = line.strip_prefix("adb: ").unwrap_or(line);
    let code = if said.contains("unauthorized") {
        "unauthorized"
    } else if said.contains("no devices") || said.contains("not found") && said.contains("device") {
        "no_device"
    } else if said.contains("more than one device") {
        "several_devices"
    } else if said.contains("offline") {
        "device_offline"
    } else if said.contains("No space left") {
        "no_space"
    } else {
        "adb"
    };
    AppError::new(code, format!("adb: {line}"))
}

/// What a copy needs from the phone. [`Phone`] is adb; a test stands in for it.
pub trait Remote: Send + 'static {
    /// Runs `command` in the phone's shell; what it printed.
    fn shell(&self, command: &str) -> Result<String>;
    /// Copies the phone's folder `remote` to `local` (which doesn't exist yet), calling `tick` now and then; stops
    /// with a `cancelled` error when `tick` says false.
    fn pull(&self, remote: &str, local: &Path, tick: &mut dyn FnMut() -> bool) -> Result<()>;
}

/// One phone, through adb.
#[derive(Debug, Clone)]
pub struct Phone {
    exe: PathBuf,
    serial: String,
}

impl Remote for Phone {
    fn shell(&self, command: &str) -> Result<String> {
        run(&self.exe, &["-s", &self.serial, "shell", command], None, Duration::from_secs(600))
    }

    fn pull(&self, remote: &str, local: &Path, tick: &mut dyn FnMut() -> bool) -> Result<()> {
        let mut child = command(&self.exe)
            .args(["-s", &self.serial, "pull", "-a", remote])
            .arg(local)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::io(format!("starting {}", self.exe.display()), e))?;
        // adb reports every file on its error output.
        let said = drain(child.stderr.take());
        match wait(&mut child, Duration::from_millis(250), tick)? {
            Some(status) if status.success() => Ok(()),
            Some(_) => Err(adb_error(&said())),
            None => Err(AppError::new("cancelled", "stopped")),
        }
    }
}

/// A file on the phone, by its path inside the folder copied.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RemoteFile {
    pub path: String,
    pub bytes: u64,
}

/// One folder to copy (`Databases` or `Media`) and its files.
#[derive(Debug, Clone, Serialize)]
pub struct Folder {
    pub name: &'static str,
    pub files: Vec<RemoteFile>,
}

/// What a copy will bring: from which of the phone's folders, how many files and bytes.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// The phone's WhatsApp folder (holding `Databases`).
    pub root: String,
    pub folders: Vec<Folder>,
    pub files: u64,
    pub bytes: u64,
}

/// What to copy.
#[derive(Debug, Clone, Copy, Default)]
pub struct Wanted<'a> {
    /// WhatsApp Business (`com.whatsapp.w4b`) instead of WhatsApp.
    pub business: bool,
    /// Media too (photos, videos, voice notes: often many GB).
    pub media: bool,
    /// Another folder of the phone's holding `Databases` (and `Media`), instead of WhatsApp's.
    pub remote_dir: Option<&'a str>,
}

/// WhatsApp's folder on the phone: Android 11's and later first, then the one before.
pub fn roots(business: bool) -> [&'static str; 2] {
    if business {
        ["/sdcard/Android/media/com.whatsapp.w4b/WhatsApp Business", "/sdcard/WhatsApp Business"]
    } else {
        ["/sdcard/Android/media/com.whatsapp/WhatsApp", "/sdcard/WhatsApp"]
    }
}

/// Lists what to copy: the first of the WhatsApp folders whose `Databases` has files (or `want.remote_dir`'s).
pub fn plan(remote: &dyn Remote, want: &Wanted<'_>) -> Result<Plan> {
    let roots: Vec<String> = match want.remote_dir {
        Some(dir) => vec![dir.trim_end_matches('/').to_owned()],
        None => roots(want.business).iter().map(|r| (*r).to_owned()).collect(),
    };
    let mut empty = None;
    let mut found = None;
    for root in &roots {
        match list(remote, &format!("{root}/Databases"))? {
            Some(files) if !files.is_empty() => {
                found = Some((root.clone(), files));
                break;
            }
            Some(_) => empty = empty.or_else(|| Some(root.clone())),
            None => {}
        }
    }
    let (root, databases) = match (found, empty) {
        (Some(found), _) => found,
        (None, Some(root)) => {
            return Err(AppError::new(
                "not_found",
                format!(
                    "{root}/Databases on the phone is empty: make a backup in WhatsApp first (Settings › Chats › Chat backup)"
                ),
            ));
        }
        (None, None) => {
            return Err(AppError::new(
                "not_found",
                format!("no WhatsApp Databases folder on the phone (looked in {})", roots.join(" and ")),
            ));
        }
    };
    let mut folders = vec![Folder { name: "Databases", files: databases }];
    if want.media {
        let media = list(remote, &format!("{root}/Media"))?
            .ok_or_else(|| AppError::new("not_found", format!("no Media folder on the phone in {root}")))?;
        folders.push(Folder { name: "Media", files: media });
    }
    let all = || folders.iter().flat_map(|f| &f.files);
    let (files, bytes) = (all().count() as u64, all().map(|f| f.bytes).sum());
    Ok(Plan { root, folders, files, bytes })
}

/// The files under the phone's folder `dir` with their sizes; none if there's no such folder.
fn list(remote: &dyn Remote, dir: &str) -> Result<Option<Vec<RemoteFile>>> {
    let q = quote(dir);
    let out = remote.shell(&format!(
        "if [ -d {q} ]; then cd {q} && find . -type f -exec stat -c '%s %n' {{}} + ; echo oopz:listed; else echo oopz:missing; fi"
    ))?;
    parse_listing(&out)
}

fn parse_listing(out: &str) -> Result<Option<Vec<RemoteFile>>> {
    let lines: Vec<&str> = out.lines().map(|l| l.trim_end_matches('\r')).collect();
    if lines.contains(&"oopz:missing") {
        return Ok(None);
    }
    if !lines.contains(&"oopz:listed") {
        return Err(AppError::new("adb", format!("couldn't list the phone's folder: {}", last_line(out))));
    }
    let mut files: Vec<RemoteFile> = lines
        .iter()
        .filter_map(|line| {
            let (bytes, path) = line.split_once(' ')?;
            let path = path.strip_prefix("./")?;
            Some(RemoteFile { path: path.to_owned(), bytes: bytes.parse().ok()? })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Some(files))
}

/// `s` as one word for the phone's shell.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Fails with `no_space` when `need` bytes don't fit in `free`.
fn enough_space(need: u64, free: u64, at: &Path) -> Result<()> {
    if free >= need {
        return Ok(());
    }
    let mb = |b: u64| b.div_ceil(1_000_000);
    Err(AppError::new(
        "no_space",
        format!("needs about {} MB at {}, {} MB free", mb(need), at.display(), free / 1_000_000),
    ))
}

/// A folder copied and checked.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Pulled {
    pub name: &'static str,
    pub files: u64,
    pub bytes: u64,
}

/// What the desktop (or the terminal) shows while it copies.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PullStatus {
    pub running: bool,
    /// The folder being copied now.
    pub folder: Option<&'static str>,
    /// Bytes copied so far, of `total`.
    pub done: u64,
    pub total: u64,
    /// The pace so far, in bytes a second (0 in the first second).
    pub bytes_per_sec: u64,
    /// At that pace; none until there's a pace.
    pub secs_left: Option<u64>,
    /// The folders copied whole, each file's size checked.
    pub copied: Vec<Pulled>,
    /// Why it stopped: `done`, `stopped` (asked to), `failed` (see `error`).
    pub stopped: Option<&'static str>,
    pub error: Option<AppError>,
}

/// Where the copy goes.
#[derive(Debug, Serialize)]
pub struct PullInfo {
    pub dest: PathBuf,
    /// The phone's folder copied from.
    pub root: String,
    pub files: u64,
    pub bytes: u64,
    /// Bytes free where the copy goes, before it.
    pub free: u64,
}

struct Shared {
    stop: AtomicBool,
    status: Mutex<PullStatus>,
}

impl Shared {
    fn status(&self) -> std::sync::MutexGuard<'_, PullStatus> {
        self.status.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Copies `plan` from the phone into a new `oopz-pulled-N` folder in `parent`, if it fits; in the background.
pub fn start_in(parent: &Path, remote: impl Remote, plan: Plan) -> Result<(Puller, PullInfo)> {
    let free = crate::session::free_space(parent)?;
    enough_space(plan.bytes, free, parent)?;
    let dest = crate::receive::new_folder(parent, "oopz-pulled")?;
    fs::create_dir(&dest).map_err(|e| AppError::io(format!("creating {}", dest.display()), e))?;
    let info = PullInfo { dest: dest.clone(), root: plan.root.clone(), files: plan.files, bytes: plan.bytes, free };
    let shared = Arc::new(Shared {
        stop: AtomicBool::new(false),
        status: Mutex::new(PullStatus { running: true, total: plan.bytes, ..PullStatus::default() }),
    });
    let thread = {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let result = copy_all(&remote, &plan, &dest, &shared);
            // Nothing copied: no empty folder left.
            let _ = fs::remove_dir(&dest);
            let mut status = shared.status();
            status.running = false;
            status.folder = None;
            match result {
                Ok(()) => status.stopped = Some("done"),
                Err(e) if e.code == "cancelled" => status.stopped = Some("stopped"),
                Err(e) => {
                    status.stopped = Some("failed");
                    status.error = Some(e);
                }
            }
        })
    };
    Ok((Puller { shared, thread: Some(thread) }, info))
}

/// Each folder into `.NAME.part`, its files' sizes checked, then renamed to `NAME`; a folder that fails is removed.
fn copy_all(remote: &dyn Remote, plan: &Plan, dest: &Path, shared: &Shared) -> Result<()> {
    let started = Instant::now();
    let mut before = 0;
    for folder in &plan.folders {
        let part = dest.join(format!(".{}.part", folder.name));
        shared.status().folder = Some(folder.name);
        let mut tick = || {
            let done = before + size_of(&part);
            let mut status = shared.status();
            status.done = done;
            let secs = started.elapsed().as_secs_f64();
            if secs >= 1.0 {
                status.bytes_per_sec = (done as f64 / secs) as u64;
                status.secs_left =
                    (status.bytes_per_sec > 0).then(|| status.total.saturating_sub(done) / status.bytes_per_sec);
            }
            !shared.stop.load(Ordering::SeqCst)
        };
        let result = remote
            .pull(&format!("{}/{}", plan.root, folder.name), &part, &mut tick)
            .and_then(|()| check_sizes(&part, &folder.files))
            .and_then(|()| {
                let target = dest.join(folder.name);
                fs::rename(&part, &target).map_err(|e| AppError::io(format!("finishing {}", target.display()), e))
            });
        if let Err(e) = result {
            let _ = fs::remove_dir_all(&part);
            return Err(e);
        }
        let bytes = folder.files.iter().map(|f| f.bytes).sum::<u64>();
        before += bytes;
        let mut status = shared.status();
        status.done = before;
        status.copied.push(Pulled { name: folder.name, files: folder.files.len() as u64, bytes });
    }
    Ok(())
}

/// Every listed file arrived whole: there, and exactly as long as on the phone.
fn check_sizes(local: &Path, files: &[RemoteFile]) -> Result<()> {
    for file in files {
        let rel = Path::new(&file.path);
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err(AppError::new("adb", format!("the phone listed an odd path: {}", file.path)));
        }
        let got = fs::metadata(local.join(rel)).map(|m| m.len()).unwrap_or(0);
        if got != file.bytes {
            return Err(AppError::new(
                "incomplete",
                format!("{} arrived with {got} of its {} bytes", file.path, file.bytes),
            ));
        }
    }
    Ok(())
}

/// The bytes of every file under `dir` so far.
fn size_of(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => size_of(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

/// A copy running in the background. Stops when dropped.
pub struct Puller {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Puller {
    pub fn status(&self) -> PullStatus {
        self.shared.status().clone()
    }

    /// Stops the copy and waits for it; a folder half copied is removed.
    pub fn stop(mut self) -> PullStatus {
        self.halt();
        self.status()
    }

    fn halt(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Puller {
    fn drop(&mut self) {
        self.halt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A phone made of a local folder: `shell` answers the listing command for paths under it; `pull` copies, and can
    /// be told to cut a file short or to fail.
    struct FakePhone {
        root: PathBuf,
        cut: bool,
        slow: bool,
    }

    impl FakePhone {
        fn local(&self, remote: &str) -> PathBuf {
            self.root.join(remote.trim_start_matches('/'))
        }
    }

    impl Remote for FakePhone {
        fn shell(&self, command: &str) -> Result<String> {
            // The folder is the first quoted word.
            let dir = command.split('\'').nth(1).unwrap();
            let local = self.local(dir);
            if !local.is_dir() {
                return Ok("oopz:missing\n".into());
            }
            let mut out = String::new();
            fn walk(base: &Path, dir: &Path, out: &mut String) {
                for e in fs::read_dir(dir).unwrap().flatten() {
                    if e.file_type().unwrap().is_dir() {
                        walk(base, &e.path(), out);
                    } else {
                        let rel = e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                        out.push_str(&format!("{} ./{rel}\n", e.metadata().unwrap().len()));
                    }
                }
            }
            walk(&local, &local, &mut out);
            out.push_str("oopz:listed\n");
            Ok(out)
        }

        fn pull(&self, remote: &str, local: &Path, tick: &mut dyn FnMut() -> bool) -> Result<()> {
            assert!(!local.exists(), "pulled into a new folder only");
            let from = self.local(remote);
            fn copy(from: &Path, to: &Path, phone: &FakePhone, tick: &mut dyn FnMut() -> bool) -> Result<()> {
                fs::create_dir_all(to).unwrap();
                for e in fs::read_dir(from).unwrap().flatten() {
                    let target = to.join(e.file_name());
                    if e.file_type().unwrap().is_dir() {
                        copy(&e.path(), &target, phone, tick)?;
                    } else {
                        let mut bytes = fs::read(e.path()).unwrap();
                        if phone.cut {
                            bytes.truncate(bytes.len() / 2);
                        }
                        fs::write(&target, bytes).unwrap();
                    }
                    if phone.slow {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    if !tick() {
                        return Err(AppError::new("cancelled", "stopped"));
                    }
                }
                Ok(())
            }
            copy(&from, local, self, tick)
        }
    }

    /// A phone with an old-style `/sdcard/WhatsApp` (and a test folder), invented files only.
    fn fake_phone() -> (tempfile::TempDir, FakePhone) {
        let dir = tempfile::tempdir().unwrap();
        let wa = dir.path().join("sdcard/WhatsApp");
        fs::create_dir_all(wa.join("Databases")).unwrap();
        fs::write(wa.join("Databases/msgstore.db.crypt15"), vec![1u8; 3000]).unwrap();
        fs::write(wa.join("Databases/msgstore-2026-11-01.1.db.crypt15"), vec![2u8; 2000]).unwrap();
        fs::create_dir_all(wa.join("Media/WhatsApp Images/Sent")).unwrap();
        fs::write(wa.join("Media/WhatsApp Images/Sent/IMG-1.jpg"), vec![3u8; 500]).unwrap();
        fs::create_dir_all(dir.path().join("sdcard/Download/empty/Databases")).unwrap();
        let phone = FakePhone { root: dir.path().to_path_buf(), cut: false, slow: false };
        (dir, phone)
    }

    fn wait(puller: &Puller) -> PullStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        while puller.status().running && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        puller.status()
    }

    #[test]
    fn adb_devices_are_read_and_a_phone_is_chosen_only_when_clear() {
        let out = "* daemon started successfully\nList of devices attached\n\
            224ab8793b027ece       device usb:1-1 product:star2ltexx model:SM_G965F device:star2lte transport_id:1\n\
            192.168.1.20:37099     device product:x model:Pixel_7 transport_id:3\n\
            R58N123                unauthorized usb:1-2 transport_id:4\n\
            ZY22                   no permissions (missing udev rules? user is in the plugdev group); see [http://developer.android.com/tools/device.html]\n\
            emulator-5554          offline\n\n";
        let devices = parse_devices(out);
        assert_eq!(devices.len(), 5);
        assert_eq!(
            devices[0],
            Device {
                serial: "224ab8793b027ece".into(),
                state: "device".into(),
                model: Some("SM G965F".into()),
                wireless: false
            }
        );
        assert!(devices[1].wireless && devices[1].model.as_deref() == Some("Pixel 7"));
        assert_eq!(devices[3].state, "no_permissions");

        // Several: never a guess; named: that one, if it's ready.
        assert_eq!(choose(&devices, None).unwrap_err().code, "several_devices");
        assert_eq!(choose(&devices, Some("192.168.1.20:37099")).unwrap().serial, "192.168.1.20:37099");
        assert_eq!(choose(&devices, Some("R58N123")).unwrap_err().code, "unauthorized");
        assert_eq!(choose(&devices, Some("ZY22")).unwrap_err().code, "no_permissions");
        assert_eq!(choose(&devices, Some("emulator-5554")).unwrap_err().code, "device_offline");
        assert_eq!(choose(&devices, Some("nope")).unwrap_err().code, "no_device");
        assert_eq!(choose(&[], None).unwrap_err().code, "no_device");
        assert!(parse_devices("List of devices attached\n\n").is_empty());
        let one = &devices[..1];
        assert_eq!(choose(one, None).unwrap().serial, "224ab8793b027ece");
        assert_eq!(choose(&devices[2..3], None).unwrap_err().code, "unauthorized");
    }

    #[test]
    fn adb_is_found_on_path_or_in_the_sdk_or_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let (bin, sdk, got) = (dir.path().join("bin"), dir.path().join("sdk"), dir.path().join("downloaded"));
        fs::create_dir_all(sdk.join("platform-tools")).unwrap();
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&got).unwrap();
        assert_eq!(find_in(Some(bin.clone().into()), [None, None], Some(got.clone())), None);
        fs::write(got.join(ADB), b"").unwrap();
        let path = std::env::join_paths([&bin]).unwrap();
        assert_eq!(find_in(Some(path.clone()), [None, None], Some(got.clone())), Some(got.join(ADB)));
        // A system adb comes before the downloaded one.
        fs::write(sdk.join("platform-tools").join(ADB), b"").unwrap();
        assert_eq!(
            find_in(Some(path.clone()), [Some(sdk.clone().into()), None], Some(got.clone())),
            Some(sdk.join("platform-tools").join(ADB))
        );
        fs::write(bin.join(ADB), b"").unwrap();
        assert_eq!(find_in(Some(path), [Some(sdk.into()), None], Some(got)), Some(bin.join(ADB)));
        assert!(host_port("192.168.1.20:37099").is_ok());
        for bad in ["-s", "192.168.1.20", "a b:1", ""] {
            assert!(host_port(bad).is_err(), "{bad}");
        }
        assert_eq!(quote("/sdcard/it's here"), r"'/sdcard/it'\''s here'");
    }

    #[test]
    fn errors_adb_prints_get_codes() {
        assert_eq!(adb_error("error: device unauthorized.\nThis adb server's $ADB_VENDOR_KEYS").code, "unauthorized");
        assert_eq!(adb_error("adb: error: failed to get feature set: no devices/emulators found").code, "no_device");
        assert_eq!(adb_error("error: device 'x' not found").code, "no_device");
        assert_eq!(adb_error("adb: error: more than one device/emulator").code, "several_devices");
        assert_eq!(adb_error("adb: error: cannot create file/directory 'x': No space left on device").code, "no_space");
        assert_eq!(adb_error("adb: error: remote object 'x' does not exist").code, "adb");
        // The line that says what failed, not adb's last progress report.
        let full =
            adb_error("adb: error: cannot create '/x/a.mp3': No space left on device\n[  0%] /sdcard/b.mp3: 100%\r");
        assert_eq!(full.detail, "adb: error: cannot create '/x/a.mp3': No space left on device");
        assert_eq!(parse_listing("sh: find: inaccessible\n").unwrap_err().code, "adb");
        assert_eq!(parse_listing("oopz:missing\r\n").unwrap(), None);
        let listed = parse_listing("12 ./b\r\n3 ./a b/c\r\nfind: ./x: Permission denied\r\noopz:listed\r\n").unwrap();
        let want = [RemoteFile { path: "a b/c".into(), bytes: 3 }, RemoteFile { path: "b".into(), bytes: 12 }];
        assert_eq!(listed.unwrap(), want);
    }

    #[test]
    fn the_whatsapp_folder_is_found_new_place_first_then_old() {
        let (dir, phone) = fake_phone();
        let found = plan(&phone, &Wanted::default()).unwrap();
        assert_eq!(found.root, "/sdcard/WhatsApp", "Android 10 and before");
        assert_eq!((found.files, found.bytes, found.folders.len()), (2, 5000, 1));
        // Android 11's place wins when it has a backup.
        let new = dir.path().join("sdcard/Android/media/com.whatsapp/WhatsApp/Databases");
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("msgstore.db.crypt15"), b"x").unwrap();
        assert_eq!(plan(&phone, &Wanted::default()).unwrap().root, roots(false)[0]);
        let with_media =
            plan(&phone, &Wanted { media: true, remote_dir: Some("/sdcard/WhatsApp/"), ..Wanted::default() });
        let with_media = with_media.unwrap();
        assert_eq!((with_media.root.as_str(), with_media.files, with_media.bytes), ("/sdcard/WhatsApp", 3, 5500));
        // Business has none; an empty folder and a missing one say so; Media asked for but missing.
        let business = plan(&phone, &Wanted { business: true, ..Wanted::default() }).unwrap_err();
        assert!(business.code == "not_found" && business.detail.contains("WhatsApp Business"), "{business}");
        let empty = plan(&phone, &Wanted { remote_dir: Some("/sdcard/Download/empty"), ..Wanted::default() });
        assert!(empty.unwrap_err().detail.contains("is empty"));
        let missing = plan(&phone, &Wanted { remote_dir: Some("/sdcard/nothing"), ..Wanted::default() });
        assert_eq!(missing.unwrap_err().code, "not_found");
        let no_media = plan(&phone, &Wanted { media: true, ..Wanted::default() }).unwrap_err();
        assert!(no_media.detail.contains("no Media"), "{no_media}");
    }

    #[test]
    fn a_pull_lands_whole_in_a_new_folder() {
        let (_phone_dir, phone) = fake_phone();
        let out = tempfile::tempdir().unwrap();
        fs::create_dir(out.path().join("oopz-pulled-1")).unwrap();
        let plan = plan(&phone, &Wanted { media: true, ..Wanted::default() }).unwrap();
        let (puller, info) = start_in(out.path(), phone, plan).unwrap();
        assert_eq!(info.dest, out.path().join("oopz-pulled-2"));
        let status = wait(&puller);
        assert_eq!(status.stopped, Some("done"), "{:?}", status.error);
        assert_eq!((status.done, status.total), (5500, 5500));
        let names: Vec<_> = status.copied.iter().map(|p| (p.name, p.files)).collect();
        assert_eq!(names, [("Databases", 2), ("Media", 1)]);
        assert_eq!(fs::read(info.dest.join("Databases/msgstore.db.crypt15")).unwrap(), vec![1u8; 3000]);
        assert!(info.dest.join("Media/WhatsApp Images/Sent/IMG-1.jpg").is_file());
        let left: Vec<_> = fs::read_dir(&info.dest).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 2, "no part folder left: {left:?}");
    }

    #[test]
    fn a_short_file_a_full_disk_or_a_stop_leaves_nothing() {
        let out = tempfile::tempdir().unwrap();
        let nothing_left = || fs::read_dir(out.path()).unwrap().count() == 0;

        // A file cut short: refused, and nothing is left, not even the new folder.
        let (_dir, mut phone) = fake_phone();
        let listed = plan(&phone, &Wanted::default()).unwrap();
        phone.cut = true;
        let (puller, _) = start_in(out.path(), phone, listed).unwrap();
        let status = wait(&puller);
        assert_eq!(status.stopped, Some("failed"));
        assert_eq!(status.error.map(|e| e.code), Some("incomplete"));
        assert!(nothing_left());

        // Stopped midway.
        let (_dir, mut phone) = fake_phone();
        phone.slow = true;
        let listed = plan(&phone, &Wanted { media: true, ..Wanted::default() }).unwrap();
        let (puller, _) = start_in(out.path(), phone, listed).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        let status = puller.stop();
        assert_eq!(status.stopped, Some("stopped"));
        assert!(status.copied.is_empty() && nothing_left());

        // More than the disk holds: refused before anything is written.
        let (_dir, phone) = fake_phone();
        let mut huge = plan(&phone, &Wanted::default()).unwrap();
        huge.bytes = u64::MAX;
        assert_eq!(start_in(out.path(), phone, huge).err().map(|e| e.code), Some("no_space"));
        assert!(nothing_left());
    }

    #[test]
    fn there_must_be_room_for_the_whole_copy() {
        let at = Path::new("/x");
        assert!(enough_space(10, 10, at).is_ok());
        let full = enough_space(5_000_001, 1_000_000, at).unwrap_err();
        assert_eq!(full.code, "no_space");
        assert!(full.detail.contains("6 MB") && full.detail.contains("1 MB free"), "{full}");
    }
}
