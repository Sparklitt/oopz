//! The `oopz` command in a terminal. Every bundle carries the CLI beside the app (`bundle.externalBin`); the Windows
//! installers and the Linux packages put it on PATH when they install. The AppImage and macOS can't, so Settings offers
//! it: the AppImage copies the CLI to ~/.local/bin (its mount folder changes every run, so a link would break), macOS
//! links /usr/local/bin/oopz to the app's copy (asking for an administrator's password), or ~/.local/bin if refused.

use std::path::{Path, PathBuf};

use oopz_app::AppError;
use serde::Serialize;

/// Where `oopz` stands now; the screen hides the setting when the command answers null.
#[derive(Serialize, Debug, PartialEq)]
pub struct CliStatus {
    /// Where the command is, if it was put there.
    pub installed: Option<PathBuf>,
    /// An AppImage's copy is older than the app (the app updated since): installing again updates it.
    pub stale: bool,
    /// The folder isn't on this app's PATH: the line to add to the shell's startup file.
    pub path_line: Option<String>,
}

/// The CLI inside this bundle, where the setting is offered: macOS, and Linux run as an AppImage.
fn bundled() -> Option<PathBuf> {
    let offered = cfg!(target_os = "macos") || (cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_some());
    if !offered {
        return None;
    }
    let cli = std::env::current_exe().ok()?.parent()?.join("oopz");
    cli.is_file().then_some(cli)
}

fn home() -> Result<PathBuf, AppError> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| AppError::new("io", "HOME isn't set"))
}

/// ~/.local/bin/oopz: the AppImage's copy, or macOS's link when the administrator's password was refused.
fn user_bin(home: &Path) -> PathBuf {
    home.join(".local/bin/oopz")
}

/// macOS's link, in a folder every shell has on PATH (/etc/paths).
#[cfg(target_os = "macos")]
const SYSTEM_BIN: &str = "/usr/local/bin/oopz";

/// `at` is this app's command: a link to `cli` (macOS), or a copy of it (AppImage).
fn is_ours(at: &Path, cli: &Path) -> bool {
    if cfg!(target_os = "macos") { std::fs::read_link(at).is_ok_and(|to| to == cli) } else { at.is_file() }
}

#[tauri::command]
pub fn cli_status() -> Option<CliStatus> {
    let cli = bundled()?;
    Some(status(&cli, &home().ok()?, std::env::var_os("PATH").as_deref()))
}

fn status(cli: &Path, home: &Path, path: Option<&std::ffi::OsStr>) -> CliStatus {
    #[cfg(target_os = "macos")]
    let places = [PathBuf::from(SYSTEM_BIN), user_bin(home)];
    #[cfg(not(target_os = "macos"))]
    let places = [user_bin(home)];
    let Some(installed) = places.into_iter().find(|at| is_ours(at, cli)) else {
        return CliStatus { installed: None, stale: false, path_line: None };
    };
    let stale = !cfg!(target_os = "macos") && std::fs::read(&installed).ok() != std::fs::read(cli).ok();
    let dir = installed.parent().unwrap_or(home);
    // /usr/local/bin is in /etc/paths, which a terminal reads and an app started from the Finder doesn't.
    let on_path =
        dir == Path::new("/usr/local/bin") || path.is_some_and(|p| std::env::split_paths(p).any(|d| d == dir));
    let path_line = (!on_path).then(|| r#"export PATH="$HOME/.local/bin:$PATH""#.to_owned());
    CliStatus { installed: Some(installed), stale, path_line }
}

#[tauri::command(async)]
pub fn cli_install() -> Result<CliStatus, AppError> {
    let cli = bundled().ok_or_else(|| AppError::new("not_found", "this copy of oopz has no command to install"))?;
    let home = home()?;
    install(&cli, &home)?;
    Ok(status(&cli, &home, std::env::var_os("PATH").as_deref()))
}

#[tauri::command(async)]
pub fn cli_remove() -> Result<CliStatus, AppError> {
    let cli = bundled().ok_or_else(|| AppError::new("not_found", "this copy of oopz has no command to remove"))?;
    let home = home()?;
    remove(&cli, &home)?;
    Ok(status(&cli, &home, std::env::var_os("PATH").as_deref()))
}

/// Copies the AppImage's CLI to ~/.local/bin, through a temporary file so a running `oopz` is never half written.
#[cfg(all(unix, not(target_os = "macos")))]
fn install(cli: &Path, home: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    let to = user_bin(home);
    let dir = to.parent().expect("a folder");
    std::fs::create_dir_all(dir).map_err(|e| AppError::io("creating ~/.local/bin", e))?;
    let part = dir.join(".oopz.part");
    std::fs::copy(cli, &part).map_err(|e| AppError::io("copying the oopz command", e))?;
    std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755))
        .and_then(|()| std::fs::rename(&part, &to))
        .map_err(|e| {
            let _ = std::fs::remove_file(&part);
            AppError::io("copying the oopz command", e)
        })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn remove(_cli: &Path, home: &Path) -> Result<(), AppError> {
    match std::fs::remove_file(user_bin(home)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(AppError::io("removing the oopz command", e)),
        _ => Ok(()),
    }
}

/// Windows never offers it ([`bundled`] is None): its installers put the install folder on PATH.
#[cfg(windows)]
fn install(_cli: &Path, _home: &Path) -> Result<(), AppError> {
    Err(AppError::new("unsupported", "the Windows installers put oopz on PATH"))
}

#[cfg(windows)]
fn remove(cli: &Path, home: &Path) -> Result<(), AppError> {
    install(cli, home)
}

/// Links /usr/local/bin/oopz to the app's CLI: as this user if the folder lets them, else as an administrator (macOS
/// asks for the password), else in ~/.local/bin. Never replaces a file there that isn't a link.
#[cfg(target_os = "macos")]
fn install(cli: &Path, home: &Path) -> Result<(), AppError> {
    let system = Path::new(SYSTEM_BIN);
    let replaceable = std::fs::symlink_metadata(system).map_or(true, |m| m.file_type().is_symlink());
    if replaceable {
        let _ = std::fs::remove_file(system);
        if std::os::unix::fs::symlink(cli, system).is_ok() {
            return Ok(());
        }
        let link = format!("mkdir -p /usr/local/bin && ln -sfh {} {SYSTEM_BIN}", sh_quote(cli));
        if as_admin(&link) {
            return Ok(());
        }
    }
    let to = user_bin(home);
    std::fs::create_dir_all(to.parent().expect("a folder")).map_err(|e| AppError::io("creating ~/.local/bin", e))?;
    let _ = std::fs::remove_file(&to);
    std::os::unix::fs::symlink(cli, &to).map_err(|e| AppError::io("linking the oopz command", e))
}

/// Removes this app's links (only links to this app's CLI), asking for the password when /usr/local/bin needs it.
#[cfg(target_os = "macos")]
fn remove(cli: &Path, home: &Path) -> Result<(), AppError> {
    let user = user_bin(home);
    if is_ours(&user, cli) {
        std::fs::remove_file(&user).map_err(|e| AppError::io("removing the oopz command", e))?;
    }
    let system = Path::new(SYSTEM_BIN);
    if is_ours(system, cli) && std::fs::remove_file(system).is_err() && !as_admin(&format!("rm -f {SYSTEM_BIN}")) {
        return Err(AppError::new("io", format!("removing {SYSTEM_BIN} needs an administrator")));
    }
    Ok(())
}

/// Runs a shell line as an administrator through AppleScript, which shows macOS's own password dialog; false when
/// refused or failed.
#[cfg(target_os = "macos")]
fn as_admin(line: &str) -> bool {
    let escaped = line.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("do shell script \"{escaped}\" with administrator privileges");
    std::process::Command::new("/usr/bin/osascript").args(["-e", &script]).status().is_ok_and(|s| s.success())
}

/// A path as one shell word.
#[cfg(target_os = "macos")]
fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

#[cfg(all(test, unix, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn the_appimage_copy_goes_to_local_bin_and_says_when_it_isnt_on_path() {
        let home = tempfile::tempdir().unwrap();
        let cli = home.path().join("mount/usr/bin/oopz");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, "#!/bin/sh\necho oopz 1\n").unwrap();
        let bin = home.path().join(".local/bin");
        let none = CliStatus { installed: None, stale: false, path_line: None };

        assert_eq!(status(&cli, home.path(), None), none);
        install(&cli, home.path()).unwrap();
        let elsewhere = std::env::join_paths(["/usr/bin", "/bin"]).unwrap();
        let s = status(&cli, home.path(), Some(&elsewhere));
        assert_eq!(s.installed.as_deref(), Some(bin.join("oopz").as_path()));
        assert!(!s.stale);
        assert_eq!(s.path_line.as_deref(), Some(r#"export PATH="$HOME/.local/bin:$PATH""#));
        let with_bin = std::env::join_paths([bin.as_path(), Path::new("/usr/bin")]).unwrap();
        assert_eq!(status(&cli, home.path(), Some(&with_bin)).path_line, None);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(bin.join("oopz")).unwrap().permissions().mode() & 0o777, 0o755);

        // The app updated: its CLI changed, the copy didn't.
        std::fs::write(&cli, "#!/bin/sh\necho oopz 2\n").unwrap();
        assert!(status(&cli, home.path(), Some(&with_bin)).stale);
        install(&cli, home.path()).unwrap();
        assert!(!status(&cli, home.path(), Some(&with_bin)).stale);

        remove(&cli, home.path()).unwrap();
        assert_eq!(status(&cli, home.path(), Some(&with_bin)), none);
        remove(&cli, home.path()).unwrap();
        assert!(!bin.join(".oopz.part").exists());
    }
}
