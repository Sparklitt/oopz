//! Android's platform-tools (adb), downloaded from Google only when the person asks (decision 0019). oopz doesn't ship
//! adb: Google's licence covers its download, so the person reads and accepts it here, as Android Studio's SDK manager
//! has them do. The archive's size and SHA-1 are the ones Google's own package index gives; it's unpacked into oopz's
//! data folder (`oopz_app::adb::downloaded_dir`), where `adb::find` looks after `PATH` and `ANDROID_HOME`, and the
//! "Remove" button, or the uninstaller on Windows, deletes it.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use oopz_app::AppError;
use oopz_app::adb;
use serde::Serialize;
use sha1::{Digest, Sha1};
use tauri::State;
use tauri::ipc::Channel;

use crate::{Step, lock};

/// Google's package index, as `sdkmanager` reads it.
const INDEX: &str = "https://dl.google.com/android/repository/repository2-3.xml";
const BASE: &str = "https://dl.google.com/android/repository/";
/// Far above the real archive (8–17 MB): a bigger answer isn't platform-tools.
const MAX_ARCHIVE: u64 = 200_000_000;
const MAX_INDEX: usize = 20_000_000;

const HOST_OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "macosx"
} else {
    "linux"
};

/// What Google offers for this system: shown with its licence before anything is downloaded.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Offer {
    /// `37.0.3`.
    pub version: String,
    pub bytes: u64,
    /// The licence's text, as Google's index gives it; accepting it is the person's click.
    pub license: String,
    #[serde(skip)]
    file: String,
    #[serde(skip)]
    sha1: String,
}

/// The platform-tools oopz downloaded, if any: its version, and whether adb is there.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Installed {
    pub version: Option<String>,
    pub folder: String,
}

fn failed(detail: impl std::fmt::Display) -> AppError {
    AppError::new("platform_tools", detail.to_string())
}

fn io(e: std::io::Error) -> AppError {
    AppError::io("Android's platform-tools", e)
}

/// The text between `open` and the next `close`.
fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let end = s[start..].find(close)? + start;
    Some(&s[start..end])
}

/// Undoes the five XML entities (the licence text uses them).
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// platform-tools for `host_os` in Google's index: its version, archive, size, SHA-1 and licence.
fn parse_offer(index: &str, host_os: &str) -> Result<Offer, AppError> {
    let package = between(index, "<remotePackage path=\"platform-tools\">", "</remotePackage>")
        .ok_or_else(|| failed("Google's index has no platform-tools"))?;
    let number = |tag: &str| between(package, &format!("<{tag}>"), &format!("</{tag}>")).map(str::trim);
    let version =
        [number("major"), number("minor"), number("micro")].into_iter().flatten().collect::<Vec<_>>().join(".");
    let archive = package
        .split("<archive>")
        .skip(1)
        .find(|a| between(a, "<host-os>", "</host-os>").map(str::trim) == Some(host_os))
        .ok_or_else(|| failed(format!("Google's index has no platform-tools for {host_os}")))?;
    let field = |tag: &str| {
        between(archive, &format!("<{tag}"), &format!("</{tag}>")).map(|v| v.rsplit('>').next().unwrap_or(v).trim())
    };
    let file = field("url").ok_or_else(|| failed("no archive name in Google's index"))?.to_owned();
    let bytes =
        field("size").and_then(|s| s.parse().ok()).ok_or_else(|| failed("no archive size in Google's index"))?;
    let sha1 = field("checksum").ok_or_else(|| failed("no checksum in Google's index"))?.to_ascii_lowercase();
    // Only a plain file name in Google's folder: never another host, never a path.
    if file.is_empty() || file.contains(['/', '\\', ':']) || !file.ends_with(".zip") {
        return Err(failed(format!("unexpected archive name in Google's index: {file}")));
    }
    if sha1.len() != 40 || !sha1.bytes().all(|b| b.is_ascii_hexdigit()) || bytes == 0 || bytes > MAX_ARCHIVE {
        return Err(failed("unexpected size or checksum in Google's index"));
    }
    let license_id =
        between(package, "<uses-license ref=\"", "\"").ok_or_else(|| failed("no licence in Google's index"))?;
    let license = between(index, &format!("<license id=\"{license_id}\""), "</license>")
        .map(|l| unescape(l.split_once('>').map_or(l, |(_, text)| text).trim()))
        .filter(|l| !l.is_empty())
        .ok_or_else(|| failed(format!("Google's index doesn't include the licence {license_id}")))?;
    Ok(Offer { version, bytes, license, file, sha1 })
}

fn client() -> Result<reqwest::Client, AppError> {
    // The updater installs the same provider; whichever runs first does it.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .user_agent(concat!("oopz/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(failed)
}

/// Asks Google's index what platform-tools it has for this system (one request, when the person clicks).
pub async fn offer() -> Result<Offer, AppError> {
    let response = client()?.get(INDEX).send().await.and_then(|r| r.error_for_status()).map_err(failed)?;
    let body = response.bytes().await.map_err(failed)?;
    if body.len() > MAX_INDEX {
        return Err(failed("Google's index is unexpectedly large"));
    }
    parse_offer(&String::from_utf8_lossy(&body), HOST_OS)
}

/// The version in an unpacked platform-tools' `source.properties` (`Pkg.Revision=37.0.3`).
fn version_in(folder: &Path) -> Option<String> {
    let text = fs::read_to_string(folder.join("source.properties")).ok()?;
    text.lines().find_map(|l| l.strip_prefix("Pkg.Revision=")).map(|v| v.trim().to_owned())
}

/// What oopz downloaded before, if anything.
pub fn installed() -> Option<Installed> {
    let folder = adb::downloaded_dir()?;
    folder.is_dir().then(|| Installed { version: version_in(&folder), folder: folder.display().to_string() })
}

/// Downloads the archive `offer` names, checks its size and SHA-1, and unpacks its `platform-tools/` folder into
/// [`adb::downloaded_dir`], replacing an older one only once the new one is complete.
pub async fn download(offer: &Offer, progress: &Channel<Step>) -> Result<Installed, AppError> {
    let dest = adb::downloaded_dir().ok_or_else(|| failed("no folder for oopz's data on this computer"))?;
    download_into(&dest, offer, progress).await
}

async fn download_into(dest: &Path, offer: &Offer, progress: &Channel<Step>) -> Result<Installed, AppError> {
    let parent = dest.parent().ok_or_else(|| failed("no folder for oopz's data on this computer"))?;
    fs::create_dir_all(parent).map_err(io)?;

    let mut response = client()?
        .get(format!("{BASE}{}", offer.file))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(failed)?;
    let mut archive = tempfile::tempfile_in(parent).map_err(io)?;
    let (mut done, mut hash) = (0u64, Sha1::new());
    let never = AtomicBool::new(false);
    let mut report = crate::progress(progress, &never, |_, _| "download");
    while let Some(chunk) = response.chunk().await.map_err(failed)? {
        done += chunk.len() as u64;
        if done > offer.bytes {
            return Err(failed("the download is bigger than Google's index says"));
        }
        hash.update(&chunk);
        archive.write_all(&chunk).map_err(io)?;
        report(done, offer.bytes);
    }
    let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if done != offer.bytes || got != offer.sha1 {
        return Err(failed("the download doesn't match Google's index (size or SHA-1): nothing was kept"));
    }

    // Unpack beside the destination, then swap: a failure leaves the old copy (or nothing), never half a folder.
    let staging = tempfile::tempdir_in(parent).map_err(io)?;
    unpack(archive, staging.path())?;
    let unpacked = staging.path().join("platform-tools");
    if !unpacked.join(if cfg!(windows) { "adb.exe" } else { "adb" }).is_file() {
        return Err(failed("the archive has no adb in it"));
    }
    if dest.exists() {
        stop_server(dest);
        fs::remove_dir_all(dest).map_err(io)?;
    }
    fs::rename(&unpacked, dest).map_err(io)?;
    Ok(Installed { version: version_in(dest), folder: dest.display().to_string() })
}

/// Unpacks a zip's files into `into`, keeping the executable bit; refuses names that leave the folder.
fn unpack(archive: fs::File, into: &Path) -> Result<(), AppError> {
    let mut zip = zip::ZipArchive::new(archive).map_err(failed)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(failed)?;
        let Some(name) = entry.enclosed_name() else {
            return Err(failed(format!("unsafe name in the archive: {}", entry.name_raw().escape_ascii())));
        };
        let path: PathBuf = into.join(name);
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(io)?;
            continue;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io)?;
        }
        let mut out = fs::File::create(&path).map_err(io)?;
        let mut buf = vec![0; 64 * 1024];
        loop {
            let n = entry.read(&mut buf).map_err(io)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(io)?;
        }
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o755)).map_err(io)?;
        }
    }
    Ok(())
}

/// Stops the adb server oopz's copy started (it holds its files open on Windows), then deletes the folder.
pub fn remove() -> Result<(), AppError> {
    let Some(dir) = adb::downloaded_dir() else { return Ok(()) };
    stop_server(&dir);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(io)?;
    }
    Ok(())
}

/// `adb kill-server` with oopz's own copy, if it's there. Only called when that copy is the one oopz uses, so a
/// server another adb started is left alone.
pub fn stop_server(dir: &Path) {
    let exe = dir.join(if cfg!(windows) { "adb.exe" } else { "adb" });
    if exe.is_file() {
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("kill-server")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let _ = cmd.status();
    }
}

/// The offer the person was shown, until they accept it: the download is exactly that archive, under that licence.
#[derive(Default)]
pub struct Shown(std::sync::Mutex<Option<Offer>>);

#[tauri::command]
pub fn tools_installed() -> Option<Installed> {
    installed()
}

#[tauri::command]
pub async fn tools_offer(shown: State<'_, Shown>) -> Result<Offer, AppError> {
    let offer = offer().await?;
    *lock(&shown.0) = Some(offer.clone());
    Ok(offer)
}

#[tauri::command]
pub async fn tools_download(shown: State<'_, Shown>, progress: Channel<Step>) -> Result<Installed, AppError> {
    let offer = lock(&shown.0).clone().ok_or_else(|| failed("look at Google's licence first"))?;
    let got = download(&offer, &progress).await?;
    *lock(&shown.0) = None;
    Ok(got)
}

#[tauri::command(async)]
pub fn tools_remove() -> Result<(), AppError> {
    remove()
}

/// At exit: when the adb oopz uses is its own downloaded copy, stop the server it started, so nothing of oopz's keeps
/// running (and the folder can be removed).
pub fn at_exit() {
    if let (Some(dir), Some(exe)) = (adb::downloaded_dir(), adb::find())
        && exe.starts_with(&dir)
    {
        stop_server(&dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX_SAMPLE: &str = r#"<sdk:sdk-repository>
<license id="android-sdk-license" type="text">Terms &amp; conditions
1. Introduction &lt;here&gt;</license>
<remotePackage path="build-tools;1"><archives><archive><complete><size>1</size><url>x.zip</url></complete><host-os>linux</host-os></archive></archives></remotePackage>
<remotePackage path="platform-tools">
 <revision><major>37</major><minor>0</minor><micro>3</micro></revision>
 <uses-license ref="android-sdk-license"/>
 <archives>
  <archive><complete><size>9571024</size><checksum type="sha1">E1579B288A7AE4E9BC28DEDEC6A154D78B3C04ED</checksum><url>platform-tools_r37.0.3-linux.zip</url></complete><host-os>linux</host-os></archive>
  <archive><complete><size>8188595</size><checksum type="sha1">b3fde5239ad89229d5a9178375b81c9887a24172</checksum><url>platform-tools_r37.0.3-win.zip</url></complete><host-os>windows</host-os></archive>
 </archives>
</remotePackage>
</sdk:sdk-repository>"#;

    #[test]
    fn reads_the_offer_for_each_system() {
        let linux = parse_offer(INDEX_SAMPLE, "linux").unwrap();
        assert_eq!((linux.version.as_str(), linux.bytes), ("37.0.3", 9_571_024));
        assert_eq!(linux.file, "platform-tools_r37.0.3-linux.zip");
        assert_eq!(linux.sha1, "e1579b288a7ae4e9bc28dedec6a154d78b3c04ed");
        assert_eq!(linux.license, "Terms & conditions\n1. Introduction <here>");
        assert_eq!(parse_offer(INDEX_SAMPLE, "windows").unwrap().file, "platform-tools_r37.0.3-win.zip");
        assert!(parse_offer(INDEX_SAMPLE, "macosx").is_err());
    }

    #[test]
    fn refuses_odd_names_and_missing_licences() {
        let elsewhere = INDEX_SAMPLE.replace("platform-tools_r37.0.3-linux.zip", "https://example.com/a.zip");
        assert!(parse_offer(&elsewhere, "linux").is_err());
        let path = INDEX_SAMPLE.replace("platform-tools_r37.0.3-linux.zip", "../a.zip");
        assert!(parse_offer(&path, "linux").is_err());
        let no_licence = INDEX_SAMPLE.replace("<license id=\"android-sdk-license\"", "<license id=\"other\"");
        assert!(parse_offer(&no_licence, "linux").is_err());
        assert!(parse_offer("<sdk/>", "linux").is_err());
    }

    #[test]
    fn unpacks_and_refuses_names_that_escape() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("a.zip");
        let mut z = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        z.start_file("platform-tools/adb", opts).unwrap();
        z.write_all(b"#!/bin/sh\n").unwrap();
        z.start_file("platform-tools/source.properties", opts).unwrap();
        z.write_all(b"Pkg.Revision=37.0.3\n").unwrap();
        z.finish().unwrap();
        let out = dir.path().join("out");
        unpack(fs::File::open(&zip_path).unwrap(), &out).unwrap();
        assert_eq!(version_in(&out.join("platform-tools")).as_deref(), Some("37.0.3"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(out.join("platform-tools/adb")).unwrap().permissions().mode() & 0o111, 0o111);
        }

        let bad_path = dir.path().join("bad.zip");
        let mut z = zip::ZipWriter::new(fs::File::create(&bad_path).unwrap());
        z.start_file("../escape", zip::write::SimpleFileOptions::default()).unwrap();
        z.finish().unwrap();
        assert!(unpack(fs::File::open(&bad_path).unwrap(), &dir.path().join("out2")).is_err());
        assert!(!dir.path().join("escape").exists());
    }

    /// Google's real index and archive, into a temporary folder: `cargo test -- --ignored real_download`.
    #[test]
    #[ignore = "downloads from dl.google.com"]
    fn real_download() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("platform-tools");
        let offer = tauri::async_runtime::block_on(offer()).unwrap();
        assert!(offer.license.len() > 1000, "the licence text is shown in full");
        let got = tauri::async_runtime::block_on(download_into(&dest, &offer, &Channel::new(|_| Ok(())))).unwrap();
        assert_eq!(got.version.as_deref(), Some(offer.version.as_str()));
        let adb = dest.join(if cfg!(windows) { "adb.exe" } else { "adb" });
        let out = std::process::Command::new(&adb).arg("version").output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("Android Debug Bridge"));
        // Tampered: a wrong checksum keeps nothing.
        let bad = Offer { sha1: "0".repeat(40), ..offer };
        let dest2 = dir.path().join("second");
        assert!(tauri::async_runtime::block_on(download_into(&dest2, &bad, &Channel::new(|_| Ok(())))).is_err());
        assert!(!dest2.exists());
    }
}
