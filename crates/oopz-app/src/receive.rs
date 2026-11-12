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
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tiny_http::{Header, Method, Request, Response, Server};

mod page;

use crate::messages::Message as Msg;
use crate::{AppError, Result};
use msgkinds::Note;
use oopz_model::messages::Message as ModelMsg;
use page::Refusal;

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
    /// Seconds until the page stops by itself if nothing arrives (0 once stopped).
    pub idle_secs_left: u64,
    /// The phone reached the page (a request with the token arrived): the network works.
    pub opened: bool,
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
    /// The pace since this file started, in bytes a second (0 in its first half second).
    pub bytes_per_sec: u64,
    /// At that pace; none until there's a pace.
    pub secs_left: Option<u64>,
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

/// If the phone hasn't opened the page after this long, the screen (and the terminal) shows what usually stops it.
pub const TIPS_AFTER: Duration = Duration::from_secs(45);

/// What the Wi-Fi screen (or the terminal) shows: the QR code, the address (the person may type it) and where files
/// go.
#[derive(Debug, Serialize)]
pub struct ReceiveInfo {
    pub url: String,
    pub qr_svg: String,
    pub dest: PathBuf,
    /// Bytes free where the files go: a Media zip can be tens of GB.
    pub free: u64,
    /// The address in the QR code.
    pub ip: IpAddr,
    /// Every address the phone might reach, the likeliest first ([`addresses`]): the person may pick another.
    pub addresses: Vec<Address>,
}

/// Starts the page into a new `oopz-received-N` folder in `parent`, on `ip` (one of [`addresses`], or loopback for
/// tests) or else the likeliest address, stopping after [`IDLE`] without a request.
pub fn start_in(parent: &Path, ip: Option<IpAddr>) -> Result<(Receiver, ReceiveInfo)> {
    let addresses = addresses();
    let ip =
        ip.or_else(|| addresses.first().map(|a| a.ip)).ok_or_else(|| AppError::of("network", Msg::NoNetwork, []))?;
    let dest = new_folder(parent, "oopz-received")?;
    let rx = Receiver::start(&dest, ip, IDLE)?;
    let free = crate::session::free_space(&dest)?;
    let info = ReceiveInfo { url: rx.url().to_owned(), qr_svg: rx.qr_svg()?, dest, free, ip, addresses };
    Ok((rx, info))
}

/// The first `stem-N` in `parent` that doesn't exist yet.
pub(crate) fn new_folder(parent: &Path, stem: &str) -> Result<PathBuf> {
    (1..1000)
        .map(|n| parent.join(format!("{stem}-{n}")))
        .find(|p| !p.exists())
        .ok_or_else(|| AppError::of("exists", Msg::TooManyFolders, [stem.to_owned()]))
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
        fs::create_dir(dest).map_err(|e| AppError::io_at(ModelMsg::DoingMaking, dest, e))?;
        let server = Server::http(SocketAddr::new(ip, 0))
            .map_err(|e| AppError::doing("network", Note::of(Msg::DoingReceivePage, []), e))?;
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
            .map_err(|e| AppError::doing("network", Note::of(Msg::DoingQrCode, []), e))?;
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
            .map_err(|e| AppError::doing("network", Note::of(Msg::DoingQrCode, []), e))?;
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

/// One of this computer's addresses a phone might reach, with its network card's name (`Wi-Fi`, `eth0`,
/// `vEthernet (WSL)`).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Address {
    pub ip: IpAddr,
    pub card: String,
    /// The card looks like a virtual one (WSL, Hyper-V, Docker, a virtual machine, a VPN): a phone rarely reaches it.
    pub likely_virtual: bool,
}

/// This computer's private IPv4 addresses, the likeliest first: real cards before virtual ones, then the one the
/// system routes the local network through, then 192.168.x before 10.x before 172.16-31.x (where WSL and Docker
/// live). Loopback, link-local and public addresses are left out, and so are cards that are down.
pub fn addresses() -> Vec<Address> {
    let cards = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(if_addrs::Interface::is_oper_up)
        .filter_map(|card| match card.ip() {
            IpAddr::V4(ip) => Some((card.name, ip, card.is_p2p)),
            IpAddr::V6(_) => None,
        })
        .collect();
    rank(cards, lan_ip())
}

/// [`addresses`] from the cards found (name, address, point-to-point) and the routed address.
fn rank(cards: Vec<(String, Ipv4Addr, bool)>, routed: Option<IpAddr>) -> Vec<Address> {
    let mut found: Vec<Address> = cards
        .into_iter()
        .filter(|(_, ip, _)| ip.is_private())
        .map(|(card, ip, p2p)| Address { ip: IpAddr::V4(ip), likely_virtual: p2p || looks_virtual(&card), card })
        .collect();
    // The routed address even when listing the cards failed.
    if let Some(ip @ IpAddr::V4(v4)) = routed
        && v4.is_private()
        && !found.iter().any(|a| a.ip == ip)
    {
        found.push(Address { ip, card: String::new(), likely_virtual: false });
    }
    let range = |ip: IpAddr| match ip {
        IpAddr::V4(v4) if v4.octets()[0] == 192 => 0,
        IpAddr::V4(v4) if v4.octets()[0] == 10 => 1,
        _ => 2,
    };
    found.sort_by_key(|a| (a.likely_virtual, Some(a.ip) != routed, range(a.ip)));
    let mut seen = Vec::new();
    found.retain(|a| {
        let new = !seen.contains(&a.ip);
        seen.push(a.ip);
        new
    });
    found
}

/// A card name that's usually a virtual adapter or a tunnel, on Windows, macOS or Linux.
fn looks_virtual(card: &str) -> bool {
    const STARTS: &[&str] = &[
        "docker", "br-", "veth", "virbr", "vboxnet", "vmnet", "tun", "tap", "utun", "wg", "zt", "lxc", "lxdbr",
        "podman", "cni", "flannel", "ppp", "ipsec", "bridge",
    ];
    const HAS: &[&str] = &[
        "vethernet",
        "wsl",
        "hyper-v",
        "virtual",
        "vpn",
        "vmware",
        "tailscale",
        "zerotier",
        "wireguard",
        "docker",
        "tap-",
    ];
    let card = card.to_lowercase();
    STARTS.iter().any(|s| card.starts_with(s)) || HAS.iter().any(|s| card.contains(s))
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
    getrandom::fill(&mut bytes).map_err(|_| AppError::of("network", Msg::NoRandomness, []))?;
    Ok(hex::encode(bytes))
}

fn serve(server: Server, token: &str, dest: &Path, idle: Duration, shared: &Shared) {
    let mut last = Instant::now();
    shared.status().idle_secs_left = idle.as_secs();
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
            Ok(None) => shared.status().idle_secs_left = idle.saturating_sub(last.elapsed()).as_secs(),
            Err(_) => break "stopped",
        }
    };
    let mut status = shared.status();
    status.running = false;
    status.current = None;
    status.idle_secs_left = 0;
    status.stopped = Some(reason);
}

/// Answers one request; true when the phone says it's done.
fn handle(mut request: Request, token: &str, dest: &Path, shared: &Shared) -> bool {
    let from_lan = request.remote_addr().is_some_and(|a| is_lan(a.ip()));
    let url = request.url().to_owned();
    let rest = url.strip_prefix('/').and_then(|u| u.split_once('/')).filter(|(t, _)| same(t, token));
    let (Some((_, rest)), true) = (rest, from_lan) else {
        shared.status().refused += 1;
        refuse(request, Refusal::NotFound);
        return false;
    };
    shared.status().opened = true;
    match (request.method(), rest) {
        (Method::Get, "") => {
            let accept =
                request.headers().iter().filter(|h| h.field.equiv("Accept-Language")).map(|h| h.value.as_str());
            let page = page::page(page::language(accept));
            let html = Header::from_bytes("Content-Type", "text/html; charset=utf-8").expect("a valid header");
            let _ = request.respond(Response::from_string(page).with_header(html));
            false
        }
        (Method::Post, "done") => {
            let _ = request.respond(Response::from_string("ok"));
            true
        }
        (Method::Put, name) if name.starts_with("files/") => {
            match receive_file(&mut request, &name["files/".len()..], dest, shared) {
                Ok(r) => {
                    let _ = request.respond(Response::from_string(format!("{} {}", r.bytes, r.sha256)));
                }
                Err(why) => refuse(request, why),
            }
            false
        }
        _ => {
            refuse(request, Refusal::NotFound);
            false
        }
    }
}

/// Answers with a refusal's status and code (docs/messages.md, the receive page, row 13: nothing the phone sent).
fn refuse(request: Request, why: Refusal) {
    let _ = request.respond(Response::from_string(why.code()).with_status_code(why.status()));
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
) -> std::result::Result<Received, Refusal> {
    let name = safe_name(encoded).ok_or(Refusal::Name)?;
    let total = request.body_length().ok_or(Refusal::SizeNeeded)? as u64;
    let target = dest.join(&name);
    if target.exists() {
        return Err(Refusal::Exists);
    }
    let part = dest.join(format!(".{name}.part"));
    let mut out = fs::OpenOptions::new().write(true).create_new(true).open(&part).map_err(|_| Refusal::Busy)?;
    shared.status().current = Some(Progress { name: name.clone(), done: 0, total, bytes_per_sec: 0, secs_left: None });
    let started = Instant::now();
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut buf = vec![0u8; 1 << 20];
    let reader = request.as_reader();
    let result = loop {
        if shared.stop.load(Ordering::SeqCst) {
            break Err(Refusal::Stopped);
        }
        match reader.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                if out.write_all(&buf[..n]).is_err() {
                    break Err(Refusal::Disk);
                }
                hasher.update(&buf[..n]);
                done += n as u64;
                if let Some(p) = shared.status().current.as_mut() {
                    p.done = done;
                    let secs = started.elapsed().as_secs_f64();
                    if secs >= 0.5 {
                        p.bytes_per_sec = (done as f64 / secs) as u64;
                        p.secs_left = (p.bytes_per_sec > 0).then(|| total.saturating_sub(done) / p.bytes_per_sec);
                    }
                }
            }
            Err(_) => break Err(Refusal::CutOff),
        }
    };
    shared.status().current = None;
    let result = result
        .and(if done == total { Ok(()) } else { Err(Refusal::CutOff) })
        .and_then(|()| out.sync_all().map_err(|_| Refusal::Write))
        .and_then(|()| if target.exists() { Err(Refusal::Exists) } else { Ok(()) })
        .and_then(|()| fs::rename(&part, &target).map_err(|_| Refusal::Write));
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
        http_with(url, method, path, "", body)
    }

    /// With `headers`, each line ending in `\r\n`.
    fn http_with(url: &str, method: &str, path: &str, headers: &str, body: &[u8]) -> (u16, String) {
        let addr = addr_of(url);
        let mut stream = TcpStream::connect(&addr).unwrap();
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
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

        assert_eq!(http(&url, "GET", "/", b"").0, 404);
        assert!(!rx.status().opened, "a wrong address isn't the phone");
        let (code, page) = http(&url, "GET", &base, b"");
        assert_eq!(code, 200);
        assert!(rx.status().opened, "the phone opened the page");
        assert!(page.contains("oopz") && page.contains("<input"));
        // docs/messages.md, the receive page, rows 8, 9 and 14: in the phone's language, the token only in the URL.
        assert!(page.contains(r#"<html lang="en">"#) && page.contains(r#""receive_page_send":"Send""#));
        let token = base.trim_matches('/');
        assert!(!page.contains(token));
        let (_, spanish) = http_with(&url, "GET", &base, "Accept-Language: es-419,es;q=0.9\r\n", b"");
        assert!(spanish.contains(r#"<html lang="es">"#) && spanish.contains(r#""receive_page_send":"Enviar""#));

        let backup = b"synthetic backup bytes, not a real one".repeat(1000);
        let name = "msgstore-2026-10-30.1.db.crypt15";
        let (code, body) = http(&url, "PUT", &format!("{base}files/{name}"), &backup);
        assert_eq!(code, 200, "{body}");
        let sha = hex::encode(Sha256::digest(&backup));
        assert_eq!(body, format!("{} {sha}", backup.len()));
        assert_eq!(fs::read(dest.join(name)).unwrap(), backup);
        // Spaces and accents in a name arrive percent-encoded.
        assert_eq!(http(&url, "PUT", &format!("{base}files/Media%20m%C3%B3vil.zip"), b"synthetic bytes").0, 200);
        assert!(dest.join("Media móvil.zip").exists());

        // Never over a received file, never outside the folder, never without the token.
        assert_eq!(http(&url, "PUT", &format!("{base}files/{name}"), b"x").0, 409);
        // Row 13: an answer is a code, never the name or bytes the phone sent.
        assert_eq!(http(&url, "PUT", &format!("{base}files/{name}"), b"x").1, "receive_refused_exists");
        for bad in ["..%2Fescape", "%2Fetc%2Fpasswd", ".hidden", "a%5Cb", ""] {
            assert_eq!(http(&url, "PUT", &format!("{base}files/{bad}"), b"x"), (400, "receive_refused_name".into()));
        }
        assert_eq!(http(&url, "GET", "/", b""), (404, "receive_refused_not_found".into()));
        assert_eq!(http(&url, "GET", &format!("/{}/", "0".repeat(32)), b"").0, 404);

        let status = rx.status();
        assert!(status.running);
        assert_eq!(status.files.len(), 2);
        assert_eq!(status.files[0], Received { name: name.into(), bytes: backup.len() as u64, sha256: sha });
        assert_eq!(status.refused, 3);
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

    #[test]
    fn real_cards_come_first_and_virtual_ones_are_marked() {
        let card = |name: &str, ip: &str, p2p: bool| (name.to_owned(), ip.parse().unwrap(), p2p);
        let cards = vec![
            card("vEthernet (WSL)", "172.25.160.1", false),
            card("lo", "127.0.0.1", false),
            card("Ethernet 2", "169.254.10.4", false),
            card("docker0", "172.17.0.1", false),
            card("Ethernet", "10.0.0.12", false),
            card("Wi-Fi", "192.168.1.24", false),
            card("Tailscale", "100.101.102.103", false),
            card("ProtonVPN", "10.2.0.2", true),
            card("eth1", "8.8.8.8", false),
        ];
        let ranked = rank(cards.clone(), Some("10.0.0.12".parse().unwrap()));
        let shown: Vec<_> = ranked.iter().map(|a| (a.ip.to_string(), a.likely_virtual)).collect();
        let want = [("10.0.0.12", false), ("192.168.1.24", false), ("10.2.0.2", true), ("172.25.160.1", true)];
        let want: Vec<_> =
            want.iter().map(|(ip, v)| (ip.to_string(), *v)).chain([("172.17.0.1".into(), true)]).collect();
        assert_eq!(shown, want);
        // Without a route, 192.168.x first; with no card listed, the routed address alone; never loopback.
        assert_eq!(rank(cards, None)[0].card, "Wi-Fi");
        let alone = rank(Vec::new(), Some("192.168.0.7".parse().unwrap()));
        assert_eq!(alone, [Address { ip: "192.168.0.7".parse().unwrap(), card: String::new(), likely_virtual: false }]);
        assert!(rank(Vec::new(), Some("127.0.0.1".parse().unwrap())).is_empty());
        for name in ["vEthernet (Default Switch)", "utun3", "wg0", "br-1a2b", "VirtualBox Host-Only Network", "tun0"] {
            assert!(looks_virtual(name), "{name}");
        }
        for name in ["Wi-Fi", "Ethernet", "eth0", "en0", "wlan0", "wlp2s0", "enp3s0", "WLAN"] {
            assert!(!looks_virtual(name), "{name}");
        }
        // This computer's own list: private addresses only.
        assert!(addresses().iter().all(|a| is_lan(a.ip) && !a.ip.is_loopback()));
    }

    #[test]
    fn the_page_counts_down_to_its_idle_stop() {
        let dir = tempfile::tempdir().unwrap();
        let rx = Receiver::start(&dir.path().join("r"), IpAddr::V4(Ipv4Addr::LOCALHOST), IDLE).unwrap();
        std::thread::sleep(Duration::from_millis(250));
        let left = rx.status().idle_secs_left;
        assert!((IDLE.as_secs() - 2..=IDLE.as_secs()).contains(&left), "{left}");
        assert_eq!(rx.stop().idle_secs_left, 0);
    }

    #[test]
    fn a_slow_upload_shows_its_pace_and_time_left() {
        let dir = tempfile::tempdir().unwrap();
        let rx = Receiver::start(&dir.path().join("r"), IpAddr::V4(Ipv4Addr::LOCALHOST), IDLE).unwrap();
        let url = rx.url().to_owned();
        let mut stream = TcpStream::connect(addr_of(&url)).unwrap();
        let head =
            format!("PUT {}files/slow.crypt15 HTTP/1.1\r\nHost: x\r\nContent-Length: 4000000\r\n\r\n", path_of(&url));
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(&vec![7u8; 1 << 20]).unwrap();
        std::thread::sleep(Duration::from_millis(700));
        stream.write_all(&vec![7u8; 1 << 16]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let current = loop {
            let c = rx.status().current;
            if c.as_ref().is_some_and(|c| c.bytes_per_sec > 0) || Instant::now() > deadline {
                break c.expect("a file arriving");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(current.bytes_per_sec > 0 && current.done < current.total);
        let left = current.secs_left.expect("time left once there's a pace");
        assert!(left >= 1, "{left}");
        drop(stream);
        assert!(rx.stop().files.is_empty(), "a cut-off file isn't kept");
    }
}
