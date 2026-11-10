//! Synthetic official text exports (decision 0015): WhatsApp's "Export chat", Telegram Desktop's JSON, SMS Backup &
//! Restore's XML, LINE's text and Viber's CSV, with invented people (numbers `9999…`) and every odd line each format
//! allows. Each comes with a [`Manifest`] of what an adapter must read from it, for the adapters' tests to compare.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use oopz_model::Kind;

pub mod line_txt;
pub mod sms_xml;
pub mod telegram_json;
pub mod viber_csv;
pub mod whatsapp_txt;

/// The formats `oopz-gen --export` writes.
pub const FORMATS: [&str; 5] = ["whatsapp-txt", "telegram-json", "sms-xml", "line-txt", "viber-csv"];

/// One export: its files (path, bytes), the first being the one to open, and what reading them all gives.
pub struct Export {
    pub files: Vec<(String, Vec<u8>)>,
    pub manifest: Manifest,
}

/// What an adapter must read from an export: counted as the generator writes it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub conversations: usize,
    pub messages: usize,
    pub from_me: usize,
    /// Messages by [`oopz_model::Message::kind`].
    pub kinds: BTreeMap<Kind, usize>,
}

impl Manifest {
    pub fn count(&mut self, kind: Kind, from_me: bool) {
        self.messages += 1;
        self.from_me += from_me as usize;
        *self.kinds.entry(kind).or_default() += 1;
    }

    pub fn to_text(&self, format: &str, seed: u64) -> String {
        let mut text = String::new();
        let _ = writeln!(text, "oopz-gen synthetic {format} export: invented data only");
        let _ = writeln!(text, "seed {seed}");
        let _ = writeln!(text, "conversations {}", self.conversations);
        let _ = writeln!(text, "messages {}", self.messages);
        let _ = writeln!(text, "from_me {}", self.from_me);
        for (kind, n) in &self.kinds {
            let _ = writeln!(text, "kind.{} {n}", kind.code());
        }
        text
    }
}

/// Every export of `format` for `seed`, each with the folder it's written to (empty for the output folder itself).
pub fn generate(format: &str, seed: u64) -> Option<Vec<(String, Export)>> {
    Some(match format {
        "whatsapp-txt" => whatsapp_txt::Layout::ALL
            .into_iter()
            .map(|l| (l.name().to_string(), whatsapp_txt::generate(seed, l)))
            .collect(),
        "telegram-json" => vec![(String::new(), telegram_json::generate(seed))],
        "sms-xml" => vec![(String::new(), sms_xml::generate(seed))],
        "line-txt" => {
            line_txt::Layout::ALL.into_iter().map(|l| (l.name().to_string(), line_txt::generate(seed, l))).collect()
        }
        "viber-csv" => vec![(String::new(), viber_csv::generate(seed))],
        _ => return None,
    })
}

/// The files as a zip, stored uncompressed, as the apps' "share" sheets make them.
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in files {
        // Writing into memory doesn't fail.
        let _ = zip.start_file(name.as_str(), options);
        let _ = zip.write_all(bytes);
    }
    zip.finish().map(Cursor::into_inner).unwrap_or_default()
}

/// Year, month, day, hour, minute, second of a UTC time in milliseconds.
pub fn civil(ms: i64) -> (i64, u32, u32, u32, u32, u32) {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d, (rem / 3600) as u32, (rem / 60 % 60) as u32, (rem % 60) as u32)
}

/// Day of the week, 0 = Monday.
pub fn weekday(ms: i64) -> usize {
    (ms.div_euclid(86_400_000) + 3).rem_euclid(7) as usize
}

/// Milliseconds of 1 January of `year`, UTC.
pub fn year_start(year: i64) -> i64 {
    oopz_model::days_from_civil(year, 1, 1) * 86_400_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip() {
        for ms in [0, -1000, 1_709_208_000_000, 4_102_444_800_000, -2_208_988_800_000] {
            let (y, mo, d, h, mi, s) = civil(ms);
            assert_eq!(oopz_model::civil_to_ms(y, mo, d, h, mi, s, 0), Some(ms));
        }
        assert_eq!(weekday(0), 3);
    }

    #[test]
    fn every_format_generates_the_same_for_one_seed() {
        for format in FORMATS {
            let a = generate(format, 7).expect("a format");
            let b = generate(format, 7).expect("a format");
            assert_eq!(a.len(), b.len());
            for ((da, ea), (db, eb)) in a.iter().zip(&b) {
                assert_eq!(da, db);
                assert_eq!(ea.files, eb.files);
                assert_eq!(ea.manifest, eb.manifest);
                assert!(ea.manifest.messages > 20, "{format} {da}");
            }
        }
    }
}
