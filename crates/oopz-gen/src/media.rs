//! Media rows and files, named the way WhatsApp names them (`Media/WhatsApp Images/Sent/IMG-20231114-WA0003.jpg`).
//! Files are small placeholders: their bytes aren't a real picture or sound, and `file_size` in the database is a
//! realistic size, not the placeholder's.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::rng::Rng;
use crate::spec::Kind;

pub struct MediaRow {
    pub file_path: Option<String>,
    pub mime: &'static str,
    pub size: i64,
    pub hash: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Seconds.
    pub duration: Option<i64>,
    pub name: Option<String>,
}

/// What happens to a media row's file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum File {
    New,
    /// The same bytes as the previous file (a forwarded photo, the same sticker): the same `file_hash`.
    Same,
    /// Not in `Media/`: deleted from the phone, or never downloaded.
    Missing,
}

pub struct Media {
    /// The folder that holds `Media/`, when files are written.
    root: Option<PathBuf>,
    /// Per day and file prefix, the last `WA` number used.
    counters: HashMap<(i64, &'static str), u32>,
    /// The last written file's bytes, for a duplicate.
    last: Option<Vec<u8>>,
    pub files: u64,
    pub missing: u64,
}

/// A voice note's MIME: an Ogg with its codec, as WhatsApp gives it.
static VOICE_MIME: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    use msgkinds::media_type::formats::{CODECS_PARAMETER, OGG_AUDIO};
    format!("{OGG_AUDIO}; {CODECS_PARAMETER}={}", msgkinds::media_type::signatures::CODEC_OPUS)
});

/// (folder, file prefix, extension, mime), as WhatsApp names its files (msgkinds' `whatsapp` folders and markers)
/// and the format each kind is shared as.
fn naming(kind: Kind) -> (&'static str, &'static str, &'static str, &'static str) {
    use msgkinds::Kind as K;
    use msgkinds::media_type::sources::whatsapp::{FOLDERS, Marker, NAME_MARKERS};
    let shared = match kind {
        Kind::Image | Kind::Album | Kind::ViewOnce => K::Photo,
        Kind::Video => K::Video,
        Kind::VideoNote => K::VideoNote,
        Kind::Gif => K::Gif,
        Kind::Voice => K::Voice,
        Kind::Audio => K::Audio,
        Kind::Sticker => K::Sticker,
        _ => K::Document,
    };
    let folder = FOLDERS.iter().find(|(k, _)| *k == shared).map_or("", |(_, f)| f);
    // Android names a video note's and a GIF's file as a video's.
    let named = match shared {
        K::VideoNote | K::Gif => K::Video,
        k => k,
    };
    let android = |m: &str| m.ends_with('-') && !m.starts_with('-');
    let prefix =
        NAME_MARKERS.iter().find(|(m, k, _)| *k == named && android(m)).map_or("", |(m, ..)| m.trim_end_matches('-'));
    let format = msgkinds::media_type::share_out::default_for(shared);
    match (shared, format) {
        // WhatsApp writes its voice notes as `.opus` (the extension its markers name for one).
        (K::Voice, Some(_)) => {
            let ext = NAME_MARKERS.iter().find_map(|(_, _, rule)| match rule {
                Marker::VoiceIfExtension(e) => Some(*e),
                _ => None,
            });
            (folder, prefix, ext.unwrap_or_default(), VOICE_MIME.as_str())
        }
        (_, Some(f)) => (folder, prefix, f.extension().unwrap_or_default(), f.mime),
        (_, None) => (folder, prefix, "bin", msgkinds::media_type::OCTET_STREAM),
    }
}

#[cfg(test)]
mod naming_tests {
    use super::*;

    /// Every kind has WhatsApp's folder, an Android prefix and an extension.
    #[test]
    fn every_kind_is_named() {
        for kind in Kind::ALL {
            let (folder, prefix, ext, mime) = naming(kind);
            assert!(!folder.is_empty() && !prefix.is_empty() && !ext.is_empty() && !mime.is_empty(), "{kind:?}");
        }
    }
}

impl Media {
    pub fn new(root: Option<&Path>) -> Self {
        Self { root: root.map(Path::to_path_buf), counters: HashMap::new(), last: None, files: 0, missing: 0 }
    }

    /// The row for a media message of `kind` sent at `ts_ms`, and its file.
    pub fn row(
        &mut self,
        rng: &mut Rng,
        kind: Kind,
        ts_ms: i64,
        from_me: bool,
        document: Option<(&str, &'static str)>,
        file: File,
    ) -> Result<MediaRow> {
        let (folder, prefix, ext, mime) = naming(kind);
        let day = ts_ms.div_euclid(86_400_000);
        let (y, m, d) = civil(day);
        let n = self.counters.entry((day, prefix)).or_insert(0);
        *n += 1;
        let sent = if from_me && kind != Kind::Document { "Sent/" } else { "" };
        let (path, mime, name) = match document {
            Some((name, mime)) => (format!("Media/{folder}/{sent}{name}"), mime, Some(name.to_owned())),
            None if kind == Kind::Voice => {
                // Voice notes are filed by year and week.
                let week = (civil_day_of_year(day) / 7) + 1;
                (format!("Media/{folder}/{y:04}{week:02}/{sent}{prefix}-{y:04}{m:02}{d:02}-WA{n:04}.{ext}"), mime, None)
            }
            None => (format!("Media/{folder}/{sent}{prefix}-{y:04}{m:02}{d:02}-WA{n:04}.{ext}"), mime, None),
        };
        let bytes = match (&self.last, file) {
            (Some(last), File::Same) => last.clone(),
            _ => format!("oopz-gen synthetic media: {path}").into_bytes(),
        };
        let hash = base64(&Sha256::digest(&bytes));
        if file == File::Missing {
            self.missing += 1;
        } else if let Some(root) = &self.root {
            let file = root.join(&path);
            fs::create_dir_all(file.parent().expect("a media path has a folder")).context("creating a media folder")?;
            fs::write(&file, &bytes).with_context(|| format!("writing {}", file.display()))?;
            self.files += 1;
        }
        self.last = Some(bytes);

        let (w, h) = *rng.pick(&[(1600, 1200), (1200, 1600), (1280, 720), (720, 1280), (4000, 3000), (512, 512)]);
        let visual = matches!(kind, Kind::Image | Kind::Album | Kind::ViewOnce | Kind::Video | Kind::Gif);
        let (size, duration) = match kind {
            Kind::Image | Kind::Album | Kind::ViewOnce => (rng.range(40_000, 4_000_000), None),
            Kind::Video => {
                let s = rng.range(2, 300);
                (s * rng.range(150_000, 900_000), Some(s))
            }
            Kind::VideoNote => {
                let s = rng.range(1, 60);
                (s * 120_000, Some(s))
            }
            Kind::Gif => (rng.range(100_000, 2_000_000), Some(rng.range(1, 8))),
            Kind::Voice => {
                // Most voice notes are short; a few go on for many minutes.
                let s = if rng.chance(0.95) { rng.range(1, 90) } else { rng.range(90, 1_800) };
                (s * 2_000, Some(s))
            }
            Kind::Audio => {
                let s = rng.range(60, 420);
                (s * 16_000, Some(s))
            }
            Kind::Sticker => (rng.range(8_000, 500_000), None),
            _ => (rng.range(1_000, 30_000_000), None),
        };
        let (width, height) = if visual {
            (Some(w), Some(h))
        } else if kind == Kind::Sticker {
            (Some(512), Some(512))
        } else {
            (None, None)
        };
        Ok(MediaRow {
            file_path: Some(path),
            mime,
            size: size as i64,
            hash,
            width,
            height,
            duration: duration.map(|d| d as i64),
            name,
        })
    }
}

/// The (year, month, day) of a day count since 1970-01-01 (Howard Hinnant's `civil_from_days`).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Days since 1 January of that day's year (0 on 1 January).
fn civil_day_of_year(days: i64) -> i64 {
    // Days from 1970-01-01 to 1 January of year `y` (Hinnant's `days_from_civil` with month 1, day 1).
    let y = civil(days).0 - 1;
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let jan1 = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + 306 - 719_468;
    days - jan1
}

/// Standard base64 with padding, as WhatsApp stores `file_hash`.
pub fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_base64() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(19_675), (2023, 11, 14));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(civil_day_of_year(19_675), 317);
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"hash1"), "aGFzaDE=");
    }
}
