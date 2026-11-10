//! Fonts for the PDF: the system's own (no font ships with oopz), found by file name in the usual folders of each
//! system, plus any the person adds. A character goes to the first font that has it: the text font, then the ones for
//! other scripts and emoji, then any other font on the system. A character no font has is counted, not hidden.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustybuzz::ttf_parser;

/// Text fonts, in order of preference (file names, lower case).
const TEXT: &[&str] = &[
    "notosans-regular.ttf",
    "roboto-regular.ttf",
    "dejavusans.ttf",
    "segoeui.ttf",
    "arial.ttf",
    "helvetica.ttc",
    "sfns.ttf",
    "liberationsans-regular.ttf",
    "ubuntu-r.ttf",
];

const BOLD: &[&str] = &[
    "notosans-bold.ttf",
    "roboto-bold.ttf",
    "dejavusans-bold.ttf",
    "segoeuib.ttf",
    "arialbd.ttf",
    "liberationsans-bold.ttf",
    "ubuntu-b.ttf",
];

/// Colour emoji first, then black-and-white ones.
const EMOJI: &[&str] = &[
    "notocoloremoji.ttf",
    "notocoloremoji-regular.ttf",
    "seguiemj.ttf",
    "apple color emoji.ttc",
    "samsungcoloremoji.ttf",
    "twemoji.ttf",
    "notoemoji-regular.ttf",
    "seguisym.ttf",
];

/// Other scripts: Arabic, Hebrew, CJK, Indic, Thai…, and broad fallbacks.
const SCRIPTS: &[&str] = &[
    "notosansarabic-regular.ttf",
    "notonaskharabic-regular.ttf",
    "notosanshebrew-regular.ttf",
    "notosanscjk-regular.ttc",
    "notosanscjksc-regular.otf",
    "notosanscjkjp-regular.otf",
    "notosansjp-regular.otf",
    "notosanssc-regular.otf",
    "notosanskr-regular.otf",
    "droidsansfallbackfull.ttf",
    "droidsansfallback.ttf",
    "msyh.ttc",
    "yugothr.ttc",
    "msgothic.ttc",
    "malgun.ttf",
    "notosansdevanagari-regular.ttf",
    "nirmala.ttf",
    "notosansthai-regular.ttf",
    "leelawui.ttf",
    "tahoma.ttf",
    "arial.ttf",
    "dejavusans.ttf",
    "arialuni.ttf",
];

/// Where systems keep fonts.
fn system_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/system/fonts",
        "/System/Library/Fonts",
        "/Library/Fonts",
        "C:\\Windows\\Fonts",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.extend([home.join(".fonts"), home.join(".local/share/fonts"), home.join("Library/Fonts")]);
    }
    if let Some(win) = std::env::var_os("WINDIR") {
        dirs.push(PathBuf::from(win).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft\\Windows\\Fonts"));
    }
    dirs
}

fn is_font(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc"))
}

/// Every font file under `dirs` (a few levels down), by lower-case file name; a file given directly counts too.
fn index(dirs: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, u32)> = dirs.iter().map(|d| (d.clone(), 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        if dir.is_file() {
            if is_font(&dir) {
                let name = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                out.push((name, dir));
            }
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() && depth < 5 {
                stack.push((p, depth + 1));
            } else if is_font(&p) {
                out.push((p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default(), p));
            }
        }
    }
    out
}

pub struct Face {
    pub data: Arc<Vec<u8>>,
    pub index: u32,
    pub font: krilla::text::Font,
    pub units_per_em: f32,
    pub ascender: f32,
    pub emoji: bool,
    #[allow(dead_code)]
    pub name: String,
}

impl Face {
    fn load(path: &Path, emoji: bool) -> Vec<Face> {
        // A font file larger than this isn't one (or isn't one a PDF should embed).
        const MAX: u64 = 64 << 20;
        if fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX) > MAX {
            return Vec::new();
        }
        let Ok(bytes) = fs::read(path) else { return Vec::new() };
        let data = Arc::new(bytes);
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1).min(4);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        (0..count)
            .filter_map(|index| {
                let face = ttf_parser::Face::parse(&data, index).ok()?;
                let units_per_em = face.units_per_em() as f32;
                let ascender = face.ascender() as f32 / units_per_em;
                let font = krilla::text::Font::new(
                    krilla::Data::from(data.clone() as Arc<dyn AsRef<[u8]> + Send + Sync>),
                    index,
                )?;
                Some(Face { data: data.clone(), index, font, units_per_em, ascender, emoji, name: name.clone() })
            })
            .collect()
    }

    pub fn has(&self, c: char) -> bool {
        ttf_parser::Face::parse(&self.data, self.index).ok().and_then(|f| f.glyph_index(c)).is_some_and(|g| g.0 != 0)
    }
}

pub struct Fonts {
    pub faces: Vec<Face>,
    /// The bold text face, when there is one (else bold is drawn with the text face).
    pub bold: Option<usize>,
    /// Font files not loaded yet, for characters nothing loaded has.
    rest: Vec<PathBuf>,
    covers: HashMap<char, Option<usize>>,
    pub missing: u64,
}

/// What a character is, for picking its font.
fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2300..=0x23FF | 0x2B00..=0x2BFF | 0x3030 | 0x303D | 0x3297 | 0x3299 | 0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139 | 0x2194..=0x21AA)
}

/// Characters that stay in the font of the one before: joiners, variation selectors, skin tones, tags, marks.
pub fn joins_previous(c: char) -> bool {
    matches!(c as u32, 0x200C | 0x200D | 0xFE00..=0xFE0F | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F | 0x20E3 | 0x0300..=0x036F | 0x0591..=0x05C7 | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06ED | 0xE0100..=0xE01EF)
}

impl Fonts {
    /// The system's fonts, after `extra` (files or folders the person gave).
    pub fn find(extra: &[PathBuf]) -> Fonts {
        let mut dirs = extra.to_vec();
        dirs.extend(system_dirs());
        let all = index(&dirs);
        let mut by_name: HashMap<&str, &PathBuf> = HashMap::new();
        for (name, path) in &all {
            by_name.entry(name.as_str()).or_insert(path);
        }
        let mut faces = Vec::new();
        let mut used: Vec<PathBuf> = Vec::new();
        let mut take = |path: &PathBuf, emoji: bool, faces: &mut Vec<Face>| {
            if !used.contains(path) {
                used.push(path.clone());
                faces.extend(Face::load(path, emoji));
            }
        };
        // Font files the person names come first, in the order given; folders they name are searched like the
        // system's, before them.
        for path in extra.iter().filter(|p| p.is_file() && is_font(p)) {
            let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            take(path, EMOJI.contains(&name.as_str()), &mut faces);
        }
        if let Some(p) = TEXT.iter().find_map(|n| by_name.get(n)) {
            take(p, false, &mut faces);
        }
        let bold = BOLD.iter().find_map(|n| by_name.get(n)).map(|p| {
            let at = faces.len();
            faces.extend(Face::load(p, false));
            at
        });
        let bold = bold.filter(|&b| b < faces.len());
        for n in EMOJI {
            if let Some(p) = by_name.get(n) {
                take(p, true, &mut faces);
            }
        }
        for n in SCRIPTS {
            if let Some(p) = by_name.get(n) {
                take(p, false, &mut faces);
            }
        }
        // Not every font on the system: enough to find a rare character without reading hundreds of megabytes.
        let rest = all.into_iter().map(|(_, p)| p).filter(|p| !used.contains(p)).take(150).collect();
        Fonts { faces, bold, rest, covers: HashMap::new(), missing: 0 }
    }

    /// The face for `c`: an emoji face first for emoji, else the first face that has it; loading more of the
    /// system's fonts when none does. `None` when no font at all has it.
    pub fn face_for(&mut self, c: char) -> Option<usize> {
        if let Some(&f) = self.covers.get(&c) {
            return f;
        }
        let emoji_first = is_emoji(c);
        let order: Vec<usize> = (0..self.faces.len())
            .filter(|&i| Some(i) != self.bold)
            .filter(|&i| self.faces[i].emoji == emoji_first)
            .chain((0..self.faces.len()).filter(|&i| Some(i) != self.bold && self.faces[i].emoji != emoji_first))
            .collect();
        let mut found = order.into_iter().find(|&i| self.faces[i].has(c));
        // Look through the rest of the system's fonts, a few at a time, keeping the ones that help.
        while found.is_none() && !self.rest.is_empty() {
            let path = self.rest.remove(0);
            let loaded = Face::load(&path, false);
            if let Some(i) = loaded.iter().position(|f| f.has(c)) {
                let at = self.faces.len();
                self.faces.extend(loaded);
                found = Some(at + i);
            }
        }
        self.covers.insert(c, found);
        found
    }

    /// The text face (the first loaded), for characters no font has (drawn as the font's empty box).
    pub fn text_face(&self) -> Option<usize> {
        (0..self.faces.len()).find(|&i| Some(i) != self.bold && !self.faces[i].emoji)
    }
}
