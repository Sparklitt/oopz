//! Receiving a backup from a phone over Wi-Fi (decision 0009). The app opens a short-lived web page on this computer;
//! the phone scans its QR code, opens the page in its browser, and picks the files. The page's address carries a
//! one-time random token; only devices on the local network are answered; files go into a new folder and never over
//! an existing file; the server stops when the phone says it's done, when the app stops it, or after a quiet spell.
//!
//! Plain HTTP on the LAN: a crypt15 backup is already encrypted with its owner's key, but a `Media/` file is not, so
//! the app says to use a trusted home network (or USB) for media. Phones' browsers can't hash a file over plain HTTP,
//! so the page checks that the computer received exactly the file's size, the computer records each file's SHA-256,
//! and the backup's own authentication tag is checked when it's opened.

use std::fs;
use std::io::Write;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::{AppError, Result};

/// Wrong-token requests after which the server stops: the token can't be guessed, so many means someone is trying.
const MAX_REFUSED: u64 = 50;

/// What the desktop shows while it receives.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReceiveStatus {
    pub running: bool,
    pub files: Vec<Received>,
    /// The file arriving now.
    pub current: Option<Progress>,
    /// Requests refused: a wrong address or a device outside the local network.
    pub refused: u64,
    /// Why it stopped: `done` (the phone said so), `stopped` (the app), `idle`, `refused` (too many wrong requests).
    pub stopped: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Received {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Progress {
    pub name: String,
    pub done: u64,
    pub total: u64,
}

struct Shared {
    stop: AtomicBool,
    status: Mutex<ReceiveStatus>,
}

impl Shared {
    fn status(&self) -> std::sync::MutexGuard<'_, ReceiveStatus> {
        self.status.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The page stops by itself after this long without a request.
pub const IDLE: Duration = Duration::from_secs(10 * 60);

/// What the Wi-Fi screen (or the terminal) shows: the QR code, the address (the person may type it) and where files
/// go.
#[derive(Debug, Serialize)]
pub struct ReceiveInfo {
    pub url: String,
    pub qr_svg: String,
    pub dest: PathBuf,
    /// Bytes free where the files go: a Media zip can be tens of GB.
    pub free: u64,
}

/// Starts the page into a new `oopz-received-N` folder in `parent`, on this computer's LAN address (or on `ip`, for
/// tests), stopping after [`IDLE`] without a request.
pub fn start_in(parent: &Path, ip: Option<IpAddr>) -> Result<(Receiver, ReceiveInfo)> {
    let ip = ip.or_else(lan_ip).ok_or_else(|| {
        AppError::new("network", "this computer isn't on a local network; connect it to the same Wi-Fi as the phone")
    })?;
    let dest = (1..1000)
        .map(|n| parent.join(format!("oopz-received-{n}")))
        .find(|p| !p.exists())
        .ok_or_else(|| AppError::new("exists", "too many oopz-received folders here; choose another folder"))?;
    let rx = Receiver::start(&dest, ip, IDLE)?;
    let free = crate::session::free_space(&dest)?;
    let info = ReceiveInfo { url: rx.url().to_owned(), qr_svg: rx.qr_svg()?, dest, free };
    Ok((rx, info))
}

/// A running receive page. Stops when dropped.
pub struct Receiver {
    url: String,
    dest: PathBuf,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Receiver {
    /// Creates `dest` (it must not exist) and serves the page on `ip` at a free port, until the phone is done, the
    /// app stops it, or nothing arrives for `idle`.
    pub fn start(dest: &Path, ip: IpAddr, idle: Duration) -> Result<Self> {
        fs::create_dir(dest).map_err(|e| AppError::io(format!("creating {}", dest.display()), e))?;
        let server = Server::http(SocketAddr::new(ip, 0))
            .map_err(|e| AppError::new("network", format!("opening the receive page: {e}")))?;
        let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or_default();
        let token = random_token()?;
        let url = format!("http://{}/{token}/", SocketAddr::new(ip, port));
        let shared = Arc::new(Shared { stop: AtomicBool::new(false), status: Mutex::new(ReceiveStatus::default()) });
        shared.status().running = true;
        let thread = {
            let (shared, dest) = (shared.clone(), dest.to_path_buf());
            std::thread::spawn(move || serve(server, &token, &dest, idle, &shared))
        };
        Ok(Self { url, dest: dest.to_path_buf(), shared, thread: Some(thread) })
    }

    /// The page's address, token included: for the QR code only, never logged.
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn dest(&self) -> &Path {
        &self.dest
    }

    /// The address as a QR code, an SVG in oopz's ink on white.
    pub fn qr_svg(&self) -> Result<String> {
        let code = qrcode::QrCode::new(self.url.as_bytes())
            .map_err(|e| AppError::new("network", format!("making the QR code: {e}")))?;
        Ok(code
            .render::<qrcode::render::svg::Color<'_>>()
            .min_dimensions(240, 240)
            .dark_color(qrcode::render::svg::Color("#23244F"))
            .light_color(qrcode::render::svg::Color("#FFFFFF"))
            .build())
    }

    /// The address as a QR code drawn with block characters, for a terminal.
    pub fn qr_text(&self) -> Result<String> {
        let code = qrcode::QrCode::new(self.url.as_bytes())
            .map_err(|e| AppError::new("network", format!("making the QR code: {e}")))?;
        Ok(code
            .render::<qrcode::render::unicode::Dense1x2>()
            .dark_color(qrcode::render::unicode::Dense1x2::Light)
            .light_color(qrcode::render::unicode::Dense1x2::Dark)
            .build())
    }

    pub fn status(&self) -> ReceiveStatus {
        self.shared.status().clone()
    }

    /// Stops the page and waits for it; a file half received is removed.
    pub fn stop(mut self) -> ReceiveStatus {
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

impl Drop for Receiver {
    fn drop(&mut self) {
        self.halt();
    }
}

/// This computer's address on the local network: the one the system would use to reach a private address. Nothing is
/// sent (connecting a UDP socket only picks a route).
pub fn lan_ip() -> Option<IpAddr> {
    ["192.168.0.1:9", "10.0.0.1:9", "172.16.0.1:9"].iter().find_map(|probe| {
        let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect(probe).ok()?;
        let ip = socket.local_addr().ok()?.ip();
        (is_lan(ip) && !ip.is_loopback()).then_some(ip)
    })
}

/// A device on the local network (or this computer): private, link-local or loopback addresses.
pub fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

fn random_token() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| AppError::new("network", "this computer gave no randomness"))?;
    Ok(hex::encode(bytes))
}

fn serve(server: Server, token: &str, dest: &Path, idle: Duration, shared: &Shared) {
    let mut last = Instant::now();
    let reason = loop {
        if shared.stop.load(Ordering::SeqCst) {
            break "stopped";
        }
        match server.recv_timeout(Duration::from_millis(100)) {
            Ok(Some(request)) => {
                last = Instant::now();
                if handle(request, token, dest, shared) {
                    break "done";
                }
                if shared.status().refused >= MAX_REFUSED {
                    break "refused";
                }
            }
            Ok(None) if last.elapsed() >= idle => break "idle",
            Ok(None) => {}
            Err(_) => break "stopped",
        }
    };
    let mut status = shared.status();
    status.running = false;
    status.current = None;
    status.stopped = Some(reason);
}

/// Answers one request; true when the phone says it's done.
fn handle(mut request: Request, token: &str, dest: &Path, shared: &Shared) -> bool {
    let from_lan = request.remote_addr().is_some_and(|a| is_lan(a.ip()));
    let url = request.url().to_owned();
    let rest = url.strip_prefix('/').and_then(|u| u.split_once('/')).filter(|(t, _)| same(t, token));
    let (Some((_, rest)), true) = (rest, from_lan) else {
        shared.status().refused += 1;
        let _ = request.respond(Response::from_string("not found").with_status_code(404));
        return false;
    };
    match (request.method(), rest) {
        (Method::Get, "") => {
            let html = Header::from_bytes("Content-Type", "text/html; charset=utf-8").expect("a valid header");
            let _ = request.respond(Response::from_string(PAGE).with_header(html));
            false
        }
        (Method::Post, "done") => {
            let _ = request.respond(Response::from_string("ok"));
            true
        }
        (Method::Put, name) if name.starts_with("files/") => {
            let (code, body) = match receive_file(&mut request, &name["files/".len()..], dest, shared) {
                Ok(r) => (200, format!("{} {}", r.bytes, r.sha256)),
                Err((code, why)) => (code, why.to_owned()),
            };
            let _ = request.respond(Response::from_string(body).with_status_code(code));
            false
        }
        _ => {
            let _ = request.respond(Response::from_string("not found").with_status_code(404));
            false
        }
    }
}

/// The token compared without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Streams one upload into `dest` (`.name.part`, then renamed), hashing as it goes; refuses unsafe or existing names
/// and a body shorter than announced.
fn receive_file(
    request: &mut Request,
    encoded: &str,
    dest: &Path,
    shared: &Shared,
) -> std::result::Result<Received, (u16, &'static str)> {
    let name = safe_name(encoded).ok_or((400, "not an allowed file name"))?;
    let total = request.body_length().ok_or((411, "the size of the file is needed"))? as u64;
    let target = dest.join(&name);
    if target.exists() {
        return Err((409, "a file with this name was already received"));
    }
    let part = dest.join(format!(".{name}.part"));
    let mut out = fs::OpenOptions::new().write(true).create_new(true).open(&part).map_err(|_| (409, "busy"))?;
    shared.status().current = Some(Progress { name: name.clone(), done: 0, total });
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut buf = vec![0u8; 1 << 20];
    let reader = request.as_reader();
    let result = loop {
        if shared.stop.load(Ordering::SeqCst) {
            break Err((503, "the computer stopped receiving"));
        }
        match reader.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                if out.write_all(&buf[..n]).is_err() {
                    break Err((507, "the computer couldn't write the file (is its disk full?)"));
                }
                hasher.update(&buf[..n]);
                done += n as u64;
                if let Some(p) = shared.status().current.as_mut() {
                    p.done = done;
                }
            }
            Err(_) => break Err((400, "the transfer was cut off")),
        }
    };
    shared.status().current = None;
    let result = result
        .and(if done == total { Ok(()) } else { Err((400, "the transfer was cut off")) })
        .and_then(|()| out.sync_all().map_err(|_| (507, "the computer couldn't write the file")))
        .and_then(|()| if target.exists() { Err((409, "already received")) } else { Ok(()) })
        .and_then(|()| fs::rename(&part, &target).map_err(|_| (507, "the computer couldn't write the file")));
    if let Err(e) = result {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    let received = Received { name, bytes: done, sha256: hex::encode(hasher.finalize()) };
    shared.status().files.push(received.clone());
    Ok(received)
}

/// A percent-encoded file name, decoded, if it's a plain name: no folders, not hidden, no control characters.
fn safe_name(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = encoded.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let name = String::from_utf8(out).ok()?;
    let ok = !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'));
    ok.then_some(name)
}

/// The phone's page: English or Spanish by the phone's language; no outside resources.
const PAGE: &str = include_str!("receive_page.html");

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read};
    use std::net::{Ipv4Addr, TcpStream};

    use super::*;

    fn addr_of(url: &str) -> String {
        url.trim_start_matches("http://").split('/').next().unwrap().to_owned()
    }

    /// A stopped page answers no more. Its port may already be another test's page, or one closing (tests run in
    /// parallel and the OS reuses a freed port), so one connection may open and even answer, but never as this page:
    /// nothing comes back, or its token is refused.
    fn assert_gone(url: &str) {
        let Ok(mut stream) = TcpStream::connect(addr_of(url)) else { return };
        let ask = format!("GET {} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n", path_of(url));
        let mut line = String::new();
        if stream.write_all(ask.as_bytes()).is_err() || BufReader::new(stream).read_line(&mut line).is_err() {
            return;
        }
        assert!(line.is_empty() || line.contains(" 404 "), "still answering after it stopped: {line}");
    }

    fn path_of(url: &str) -> String {
        format!("/{}", url.trim_start_matches("http://").split_once('/').unwrap().1)
    }

    /// One HTTP request, `Connection: close`: (status, body).
    fn http(url: &str, method: &str, path: &str, body: &[u8]) -> (u16, String) {
        let addr = addr_of(url);
        let mut stream = TcpStream::connect(&addr).unwrap();
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let status = line.split(' ').nth(1).unwrap().parse().unwrap();
        let mut rest = String::new();
        reader.read_to_string(&mut rest).unwrap();
        (status, rest.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default())
    }

    fn wait_until_stopped(rx: &Receiver) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while rx.status().running && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_phone_sends_files_with_the_token_and_they_arrive_whole() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("received");
        let rx = Receiver::start(&dest, IpAddr::V4(Ipv4Addr::LOCALHOST), Duration::from_secs(30)).unwrap();
        let url = rx.url().to_owned();
        let base = path_of(&url);
        assert!(rx.qr_svg().unwrap().contains("<svg"));

        let (code, page) = http(&url, "GET", &base, b"");
        assert_eq!(code, 200);
        assert!(page.contains("oopz") && page.contains("<input"));

        let backup = b"synthetic backup bytes, not a real one".repeat(1000);
        let name = "msgstore-2026-10-30.1.db.crypt15";
        let (code, body) = http(&url, "PUT", &format!("{base}files/{name}"), &backup);
        assert_eq!(code, 200, "{body}");
        let sha = hex::encode(Sha256::digest(&backup));
        assert_eq!(body, format!("{} {sha}", backup.len()));
        assert_eq!(fs::read(dest.join(name)).unwrap(), backup);
        // Spaces and accents in a name arrive percent-encoded.
        assert_eq!(http(&url, "PUT", &format!("{base}files/Media%20m%C3%B3vil.zip"), b"zip").0, 200);
        assert!(dest.join("Media móvil.zip").exists());

        // Never over a received file, never outside the folder, never without the token.
        assert_eq!(http(&url, "PUT", &format!("{base}files/{name}"), b"x").0, 409);
        for bad in ["..%2Fescape", "%2Fetc%2Fpasswd", ".hidden", "a%5Cb", ""] {
            assert_eq!(http(&url, "PUT", &format!("{base}files/{bad}"), b"x").0, 400, "{bad}");
        }
        assert_eq!(http(&url, "GET", "/", b"").0, 404);
        assert_eq!(http(&url, "GET", &format!("/{}/", "0".repeat(32)), b"").0, 404);

        let status = rx.status();
        assert!(status.running);
        assert_eq!(status.files.len(), 2);
        assert_eq!(status.files[0], Received { name: name.into(), bytes: backup.len() as u64, sha256: sha });
        assert_eq!(status.refused, 2);
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 2, "no part files left");

        // The phone says it's done: the page stops, and its address answers no more.
        assert_eq!(http(&url, "POST", &format!("{base}done"), b"").0, 200);
        wait_until_stopped(&rx);
        assert_eq!(rx.status().stopped, Some("done"));
        drop(rx);
        assert_gone(&url);
    }

    #[test]
    fn a_cut_off_upload_leaves_nothing_and_the_page_stops_when_idle_or_asked() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("received");
        let rx = Receiver::start(&dest, IpAddr::V4(Ipv4Addr::LOCALHOST), Duration::from_millis(300)).unwrap();
        let url = rx.url().to_owned();
        {
            // Announces 1000 bytes, sends 10, hangs up.
            let mut stream = TcpStream::connect(addr_of(&url)).unwrap();
            let head =
                format!("PUT {}files/cut.crypt15 HTTP/1.1\r\nHost: x\r\nContent-Length: 1000\r\n\r\n", path_of(&url));
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(b"0123456789").unwrap();
        }
        wait_until_stopped(&rx);
        let status = rx.status();
        assert_eq!(status.stopped, Some("idle"));
        assert!(status.files.is_empty());
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0, "a cut-off file is removed");

        // An existing folder is never reused; the app's stop is immediate.
        assert!(Receiver::start(&dest, IpAddr::V4(Ipv4Addr::LOCALHOST), Duration::from_secs(1)).is_err());
        let again = dir.path().join("again");
        let rx = Receiver::start(&again, IpAddr::V4(Ipv4Addr::LOCALHOST), Duration::from_secs(60)).unwrap();
        assert_eq!(rx.stop().stopped, Some("stopped"));
    }

    #[test]
    fn only_the_local_network_is_answered() {
        for lan in ["192.168.1.20", "10.1.2.3", "172.16.5.5", "169.254.1.1", "127.0.0.1", "fe80::1", "fd00::5", "::1"] {
            assert!(is_lan(lan.parse().unwrap()), "{lan}");
        }
        for wan in ["8.8.8.8", "172.32.0.1", "100.64.0.1", "2001:db8::1"] {
            assert!(!is_lan(wan.parse().unwrap()), "{wan}");
        }
        assert!(lan_ip().is_none_or(|ip| is_lan(ip) && !ip.is_loopback()));
        assert_eq!(safe_name("msgstore.db.crypt15").as_deref(), Some("msgstore.db.crypt15"));
        assert_eq!(safe_name("%zz"), None);
        assert!(same("abc", "abc") && !same("abc", "abd") && !same("abc", "ab"));
    }
}
