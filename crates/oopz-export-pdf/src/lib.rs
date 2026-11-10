//! PDF exports of a selection (decision 0017): a cover, contents, one chapter per conversation or one merged
//! timeline, photos as thumbnails or not at all, A4 or Letter, in the words the caller passes (the app's language)
//! and the person's time zone. Text is shaped (joined scripts, combining marks), laid out right to left where it is
//! (the Unicode bidirectional algorithm), and every character goes to a font that has it, colour emoji included.
//! The fonts are the system's: none ships with oopz.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use krilla::color::rgb;
use krilla::geom::{Point, Size};
use krilla::num::NormalizedF32;
use krilla::paint::Fill;
use krilla::text::{GlyphId, KrillaGlyph};
use oopz_core::{Error, Library, MessageRow, Order, Result, Selection};
use oopz_model::{App, Kind, Part};
use serde::{Deserialize, Serialize};
use unicode_bidi::BidiInfo;

mod fonts;

use fonts::Fonts;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSize {
    A4,
    Letter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaMode {
    /// Photos from the source's folder, small.
    Thumbnails,
    /// A line naming each file instead.
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PdfOptions {
    #[serde(flatten)]
    pub base: oopz_export::Options,
    pub page: PageSize,
    pub media: MediaMode,
    pub cover: bool,
    pub contents: bool,
    /// Font files or folders to use before the system's.
    pub fonts: Vec<PathBuf>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions {
            base: oopz_export::Options::default(),
            page: PageSize::A4,
            media: MediaMode::Thumbnails,
            cover: true,
            contents: true,
            fonts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PdfWritten {
    pub path: PathBuf,
    pub messages: u64,
    pub bytes: u64,
    pub pages: u64,
    /// Characters no font on this system has (drawn as an empty box).
    pub missing_glyphs: u64,
}

const MARGIN: f32 = 48.0;
const BODY: f32 = 10.5;
const SMALL: f32 = 8.0;
const TITLE: f32 = 22.0;
const HEADING: f32 = 15.0;
const THUMB: f32 = 160.0;
/// The largest photo read for a thumbnail.
const THUMB_MAX_FILE: u64 = 24 << 20;

type Color = (u8, u8, u8);
const INK: Color = (27, 29, 28);
const MUTED: Color = (95, 102, 98);
const ACCENT: Color = (47, 93, 79);
const ME_BAR: Color = (126, 178, 156);

/// A shaped piece of one line, in one font and direction, positioned from the line's left.
#[derive(Clone)]
struct Run {
    face: usize,
    text: String,
    glyphs: Vec<KrillaGlyph>,
    size: f32,
    width: f32,
}

#[derive(Clone)]
struct Line {
    runs: Vec<Run>,
    width: f32,
    rtl: bool,
}

/// What's drawn on a page.
enum Op {
    Text { x: f32, y: f32, run: Run, color: Color },
    Bar { x: f32, y: f32, w: f32, h: f32, color: Color },
    Image { x: f32, y: f32, w: f32, h: f32, image: krilla::image::Image },
}

struct Shaper {
    fonts: Fonts,
}

impl Shaper {
    /// `text` split where the font changes: (byte range, face).
    fn font_runs(&mut self, text: &str, bold: bool) -> Vec<(std::ops::Range<usize>, usize)> {
        let mut out: Vec<(std::ops::Range<usize>, usize)> = Vec::new();
        for (i, c) in text.char_indices() {
            let end = i + c.len_utf8();
            if let Some(last) = out.last_mut()
                && (fonts::joins_previous(c) || c == ' ')
            {
                last.0.end = end;
                continue;
            }
            let face = match self.fonts.face_for(c) {
                Some(f) => f,
                None => {
                    self.fonts.missing += u64::from(!c.is_whitespace() && !c.is_control());
                    self.fonts.text_face().unwrap_or(0)
                }
            };
            let face = match (bold, self.fonts.bold) {
                (true, Some(b)) if Some(face) == self.fonts.text_face() && self.fonts.faces[b].has(c) => b,
                _ => face,
            };
            match out.last_mut() {
                Some(last) if last.1 == face && last.0.end == i => last.0.end = end,
                _ => out.push((i..end, face)),
            }
        }
        out
    }

    fn shape(&self, text: &str, face: usize, rtl: bool, size: f32) -> Run {
        let f = &self.fonts.faces[face];
        let mut glyphs = Vec::new();
        let mut width = 0.0;
        if let Some(rb) = rustybuzz::Face::from_slice(&f.data, f.index) {
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(text);
            buffer.guess_segment_properties();
            buffer.set_direction(if rtl {
                rustybuzz::Direction::RightToLeft
            } else {
                rustybuzz::Direction::LeftToRight
            });
            let out = rustybuzz::shape(&rb, &[], buffer);
            let infos = out.glyph_infos();
            let positions = out.glyph_positions();
            let upem = f.units_per_em;
            for (i, (info, pos)) in infos.iter().zip(positions).enumerate() {
                let start = info.cluster as usize;
                // The cluster's end: the next different cluster in the text's own order.
                let end = if rtl {
                    infos[..i].iter().rev().map(|g| g.cluster as usize).find(|&c| c > start).unwrap_or(text.len())
                } else {
                    infos[i + 1..].iter().map(|g| g.cluster as usize).find(|&c| c > start).unwrap_or(text.len())
                };
                let advance = pos.x_advance as f32 / upem;
                width += advance * size;
                glyphs.push(KrillaGlyph::new(
                    GlyphId::new(info.glyph_id),
                    advance,
                    pos.x_offset as f32 / upem,
                    pos.y_offset as f32 / upem,
                    pos.y_advance as f32 / upem,
                    start..end.max(start),
                    None,
                ));
            }
        }
        Run { face, text: text.to_owned(), glyphs, size, width }
    }

    fn width(&mut self, text: &str, size: f32, bold: bool) -> f32 {
        self.font_runs(text, bold).into_iter().map(|(r, f)| self.shape(&text[r], f, false, size).width).sum()
    }

    /// One paragraph (no `\n`) as lines at most `max` wide, each in visual order.
    fn lines(&mut self, text: &str, size: f32, max: f32, bold: bool) -> Vec<Line> {
        let text: String = text.chars().map(|c| if c == '\t' { ' ' } else { c }).filter(|c| !c.is_control()).collect();
        if text.trim().is_empty() {
            return vec![Line { runs: Vec::new(), width: 0.0, rtl: false }];
        }
        // Where a line may end: after a space, and around CJK characters (written without spaces).
        let mut pieces: Vec<std::ops::Range<usize>> = Vec::new();
        let mut start = 0;
        let cjk = |c: char| matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF);
        let chars: Vec<(usize, char)> = text.char_indices().collect();
        for (n, &(i, c)) in chars.iter().enumerate() {
            let end = i + c.len_utf8();
            let next = chars.get(n + 1).map(|x| x.1);
            let breaks =
                (c == ' ' && next != Some(' ')) || cjk(c) || next.is_some_and(|d| cjk(d) && !fonts::joins_previous(d));
            if breaks || next.is_none() {
                pieces.push(start..end);
                start = end;
            }
        }
        // Lines as byte ranges, filled greedily; a piece wider than a line is cut between characters.
        let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
        let mut line: Option<std::ops::Range<usize>> = None;
        let mut used = 0.0;
        for piece in pieces {
            let w = self.width(&text[piece.clone()], size, bold);
            let trimmed = self.width(text[piece.clone()].trim_end(), size, bold);
            if line.is_some() && used + trimmed > max {
                ranges.extend(line.take());
                used = 0.0;
            }
            if trimmed > max {
                // Cut the long piece by characters (keeping joined sequences together).
                let mut cut = piece.start;
                let mut acc = 0.0;
                let mut last = piece.start;
                for (i, c) in text[piece.clone()].char_indices() {
                    let at = piece.start + i;
                    if fonts::joins_previous(c) {
                        continue;
                    }
                    if at > last {
                        acc += self.width(&text[last..at], size, bold);
                        last = at;
                    }
                    if acc > max && at > cut {
                        ranges.push(cut..at);
                        cut = at;
                        acc = 0.0;
                    }
                }
                line = Some(cut..piece.end);
                used = self.width(&text[cut..piece.end], size, bold);
                continue;
            }
            line = Some(match line {
                Some(l) => l.start..piece.end,
                None => piece.clone(),
            });
            used += w;
        }
        ranges.extend(line);
        // Each line in visual order: the bidirectional algorithm's runs, each split by font.
        let bidi = BidiInfo::new(&text, None);
        let mut out = Vec::new();
        for range in ranges {
            let Some(para) = bidi.paragraphs.iter().find(|p| p.range.start <= range.start && range.start < p.range.end)
            else {
                continue;
            };
            let rtl_para = para.level.is_rtl();
            let mut range = range.start..range.end.min(para.range.end);
            while range.end > range.start && text[..range.end].ends_with(' ') {
                range.end -= 1;
            }
            let (levels, runs) = bidi.visual_runs(para, range.clone());
            let mut line = Line { runs: Vec::new(), width: 0.0, rtl: rtl_para };
            for run in runs {
                let rtl = levels[run.start].is_rtl();
                let mut pieces: Vec<Run> = self
                    .font_runs(&text[run.clone()], bold)
                    .into_iter()
                    .map(|(r, f)| self.shape(&text[run.start + r.start..run.start + r.end], f, rtl, size))
                    .collect();
                if rtl {
                    pieces.reverse();
                }
                for p in pieces {
                    line.width += p.width;
                    line.runs.push(p);
                }
            }
            out.push(line);
        }
        out
    }
}

/// Lays out pages and draws each one into the document as soon as it's full; a dry run only counts pages.
struct Writer<'a> {
    doc: Option<&'a mut krilla::Document>,
    shaper: &'a mut Shaper,
    size: (f32, f32),
    ops: Vec<Op>,
    y: f32,
    page: usize,
    footer: String,
    total_pages: usize,
    /// Pages where each chapter starts, with its title.
    chapters: Vec<(String, usize, f32)>,
}

impl Writer<'_> {
    fn bottom(&self) -> f32 {
        self.size.1 - MARGIN
    }

    fn new_page(&mut self) {
        self.flush();
        self.page += 1;
        self.y = MARGIN;
    }

    fn flush(&mut self) {
        let ops = std::mem::take(&mut self.ops);
        if self.page == 0 {
            return;
        }
        let Some(doc) = self.doc.as_deref_mut() else { return };
        let Some(settings) = krilla::page::PageSettings::from_wh(self.size.0, self.size.1) else { return };
        let mut page = doc.start_page_with(settings);
        let mut surface = page.surface();
        let footer = format!("{} · {} / {}", self.footer, self.page, self.total_pages.max(self.page));
        let mut all = ops;
        let lines = self.shaper.lines(&footer, SMALL, self.size.0 - 2.0 * MARGIN, false);
        if let Some(l) = lines.first() {
            let mut x = (self.size.0 - l.width) / 2.0;
            for run in &l.runs {
                all.push(Op::Text { x, y: self.size.1 - MARGIN / 2.0, run: run.clone(), color: MUTED });
                x += run.width;
            }
        }
        for op in all {
            match op {
                Op::Text { x, y, run, color } => {
                    surface.set_fill(Some(fill(color)));
                    let font = self.shaper.fonts.faces[run.face].font.clone();
                    surface.draw_glyphs(Point::from_xy(x, y), &run.glyphs, font, &run.text, run.size, false);
                }
                Op::Bar { x, y, w, h, color } => {
                    let mut path = krilla::geom::PathBuilder::new();
                    if let Some(rect) = krilla::geom::Rect::from_xywh(x, y, w, h) {
                        path.push_rect(rect);
                    }
                    if let Some(path) = path.finish() {
                        surface.set_fill(Some(fill(color)));
                        surface.draw_path(&path);
                    }
                }
                Op::Image { x, y, w, h, image } => {
                    if let Some(size) = Size::from_wh(w, h) {
                        surface.push_transform(&krilla::geom::Transform::from_translate(x, y));
                        surface.draw_image(image, size);
                        surface.pop();
                    }
                }
            }
        }
        surface.finish();
        page.finish();
    }

    /// Room for `h` more points on this page, or a new page.
    fn need(&mut self, h: f32) {
        if self.page == 0 || self.y + h > self.bottom() {
            self.new_page();
        }
    }

    /// A paragraph of `text` at `indent`, aligned by its direction; returns its height.
    fn text(&mut self, text: &str, size: f32, color: Color, bold: bool, indent: f32) {
        let max = self.size.0 - 2.0 * MARGIN - indent;
        for para in text.split('\n') {
            for line in self.shaper.lines(para, size, max, bold) {
                let h = size * 1.4;
                self.need(h);
                let ascent = line
                    .runs
                    .iter()
                    .map(|r| self.shaper.fonts.faces[r.face].ascender * r.size)
                    .fold(size * 0.9, f32::max);
                let baseline = self.y + ascent.min(size * 1.15);
                let mut x = if line.rtl { self.size.0 - MARGIN - line.width } else { MARGIN + indent };
                for run in line.runs {
                    let w = run.width;
                    self.ops.push(Op::Text { x, y: baseline, run, color });
                    x += w;
                }
                self.y += h;
            }
        }
    }

    fn gap(&mut self, h: f32) {
        self.y += h;
    }
}

fn fill(c: Color) -> Fill {
    Fill { paint: rgb::Color::new(c.0, c.1, c.2).into(), opacity: NormalizedF32::ONE, rule: Default::default() }
}

/// A photo's thumbnail, when the source's folder has it and it's a JPEG, PNG or WebP of sane size.
fn thumbnail(root: Option<&Path>, rel: &str) -> Option<(krilla::image::Image, f32, f32)> {
    let rel_path = Path::new(rel);
    if rel_path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }
    let root = root?.canonicalize().ok()?;
    let path = root.join(rel_path).canonicalize().ok()?;
    if !path.starts_with(&root) || fs::metadata(&path).ok()?.len() > THUMB_MAX_FILE {
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    let data = krilla::Data::from(bytes.clone());
    let image = match bytes.get(..4)? {
        [0xFF, 0xD8, ..] => krilla::image::Image::from_jpeg(data, true).ok()?,
        [0x89, b'P', b'N', b'G'] => krilla::image::Image::from_png(data, true).ok()?,
        [b'R', b'I', b'F', b'F'] => krilla::image::Image::from_webp(data, true).ok()?,
        _ => return None,
    };
    let (w, h) = image.size();
    if w == 0 || h == 0 || w > 16_384 || h > 16_384 {
        return None;
    }
    let scale = (THUMB / w as f32).min(THUMB / h as f32).min(1.0);
    Some((image, w as f32 * scale, h as f32 * scale))
}

/// Lays out the whole document into `w` (a dry run when it has no document).
fn layout(
    w: &mut Writer<'_>,
    lib: &Library,
    ids: &[i64],
    opts: &PdfOptions,
    contents_pages: usize,
    chapter_pages: &[(String, usize, f32)],
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<u64> {
    let words = &opts.base.words;
    let clock = oopz_export::Clock::new(opts.base.time_zone.as_deref())?;
    let (note, exported) = oopz_export::note(words);
    let title = opts.base.title.clone().unwrap_or_else(|| "oopz".into());
    w.footer = title.clone();
    let mut roots: HashMap<i64, Option<PathBuf>> = HashMap::new();
    if opts.cover {
        w.new_page();
        w.gap(w.size.1 / 4.0);
        w.text(&title, TITLE, INK, true, 0.0);
        w.gap(10.0);
        let rows = lib.messages_by_id(&[ids[0], ids[ids.len() - 1]])?;
        let span = format!(
            "{} — {} · {} {}",
            clock.time(rows.first().and_then(|r| r.time_ms), rows.first().is_some_and(|r| r.wall_clock)),
            clock.time(rows.last().and_then(|r| r.time_ms), rows.last().is_some_and(|r| r.wall_clock)),
            ids.len(),
            words.messages
        );
        w.text(&span, BODY, MUTED, false, 0.0);
        w.gap(24.0);
        w.text(&format!("{exported}. {note}"), SMALL + 1.0, MUTED, false, 0.0);
    }
    if opts.contents && opts.base.order == Order::Conversation {
        w.new_page();
        w.text(&words.contents, HEADING, INK, true, 0.0);
        w.gap(8.0);
        for (name, page, _) in chapter_pages {
            w.text(&format!("{name} · {}", page + 1), BODY, INK, false, 0.0);
        }
        // The dry run has no page numbers yet: reserve the same room.
        while w.page < usize::from(opts.cover) + contents_pages {
            w.new_page();
        }
    }
    let merged = opts.base.order != Order::Conversation;
    let mut last_conv = None;
    let mut last_day = String::new();
    let total = ids.len() as u64;
    let mut done = 0u64;
    w.new_page();
    for chunk in ids.chunks(500) {
        let rows = lib.messages_by_id(chunk)?;
        let (names, replies) = oopz_export::names_and_replies(lib, &rows)?;
        for row in &rows {
            if opts.base.skip_system && row.kind == Kind::System {
                continue;
            }
            let (app, conv) = names.get(&row.conversation).cloned().unwrap_or((row.app, String::new()));
            if !merged && last_conv != Some(row.conversation) {
                if last_conv.is_some() {
                    w.new_page();
                }
                w.chapters.push((format!("{} · {conv}", app.name()), w.page - 1, w.y));
                w.text(&format!("{} · {conv}", app.name()), HEADING, ACCENT, true, 0.0);
                w.gap(6.0);
                last_conv = Some(row.conversation);
                last_day.clear();
            }
            message(w, lib, row, &clock, words, merged, (app, &conv), &replies, &mut last_day, opts, &mut roots)?;
            done += 1;
        }
        if !progress(done, total) {
            return Err(Error::new("cancelled", "stopped"));
        }
    }
    w.flush();
    Ok(done)
}

#[allow(clippy::too_many_arguments)]
fn message(
    w: &mut Writer<'_>,
    lib: &Library,
    row: &MessageRow,
    clock: &oopz_export::Clock,
    words: &oopz_export::Words,
    merged: bool,
    (app, conv): (App, &str),
    replies: &HashMap<i64, String>,
    last_day: &mut String,
    opts: &PdfOptions,
    roots: &mut HashMap<i64, Option<PathBuf>>,
) -> Result<()> {
    let time = clock.time(row.time_ms, row.wall_clock);
    let day = time.get(..10).unwrap_or_default().to_owned();
    if !day.is_empty() && day != *last_day {
        w.gap(6.0);
        w.text(&day, SMALL, MUTED, true, 0.0);
        *last_day = day;
    }
    let mut head = format!("{} · {}", oopz_export::sender_label(row, words), time.get(11..).unwrap_or_default());
    if merged {
        head = format!("{} · {conv} · {head}", app.name());
    }
    if row.message.edited {
        head.push_str(&format!(" · {}", words.edited));
    }
    w.need(SMALL * 1.4 + BODY * 1.4);
    let top = w.y;
    let indent = 8.0;
    w.text(&head, SMALL, if row.from_me { ACCENT } else { MUTED }, true, indent);
    if let Some(q) = row.reply.and_then(|r| replies.get(&r)) {
        w.text(&format!("↩ {q}"), SMALL, MUTED, false, indent + 6.0);
    }
    let lines = oopz_export::row_lines(row, words);
    let mut shown_photo = false;
    if opts.media == MediaMode::Thumbnails {
        for part in &row.message.parts {
            let Part::Media(media) = part else { continue };
            if media.view_once || media.kind != oopz_model::MediaKind::Photo {
                continue;
            }
            let Some(rel) = media.file.as_deref() else { continue };
            let root = match roots.get(&row.source) {
                Some(r) => r.clone(),
                None => {
                    let r = lib.media_root(row.source)?;
                    roots.insert(row.source, r.clone());
                    r
                }
            };
            if let Some((image, iw, ih)) = thumbnail(root.as_deref(), rel) {
                w.need(ih + 4.0);
                let x = if row.from_me { w.size.0 - MARGIN - iw } else { MARGIN + indent };
                let y = w.y + 2.0;
                w.ops.push(Op::Image { x, y, w: iw, h: ih, image });
                w.y += ih + 4.0;
                shown_photo = true;
            }
        }
    }
    for line in &lines {
        // A photo shown as a picture doesn't need its "[Photo · …]" line too.
        if shown_photo && line.starts_with('[') && !line.contains('\n') {
            shown_photo = false;
            continue;
        }
        let color = if row.message.deleted || line.starts_with('[') || line.starts_with('—') { MUTED } else { INK };
        w.text(line, BODY, color, false, indent);
    }
    if !row.message.reactions.is_empty() {
        let r = row.message.reactions.iter().map(|r| r.emoji.as_str()).collect::<Vec<_>>().join(" ");
        w.text(&r, SMALL + 1.0, INK, false, indent);
    }
    // A bar beside what the owner wrote, on the page where the message starts.
    if row.from_me && w.y > top {
        w.ops.push(Op::Bar { x: MARGIN, y: top + 2.0, w: 2.5, h: (w.y - top - 4.0).max(2.0), color: ME_BAR });
    }
    w.gap(4.0);
    Ok(())
}

/// Writes the PDF of `selection` into a new `oopz-pdf-N.pdf` in `out_dir`.
pub fn export_pdf(
    lib: &Library,
    selection: &Selection,
    opts: &PdfOptions,
    out_dir: &Path,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<PdfWritten> {
    let ids = lib.selected(selection, opts.base.order)?;
    if ids.is_empty() {
        return Err(Error::new("not_found", "the selection is empty"));
    }
    if !out_dir.is_dir() {
        return Err(Error::new("not_found", format!("no folder {}", out_dir.display())));
    }
    let mut shaper = Shaper { fonts: Fonts::find(&opts.fonts) };
    if shaper.fonts.faces.is_empty() {
        return Err(Error::new("unsupported", "no fonts found on this system; add a font folder"));
    }
    let size = match opts.page {
        PageSize::A4 => (595.28, 841.89),
        PageSize::Letter => (612.0, 792.0),
    };
    // A dry run for the page numbers the contents need: the first half of the progress.
    let total = ids.len() as u64 * 2;
    let (chapters, pages) = {
        let mut dry = Writer {
            doc: None,
            shaper: &mut shaper,
            size,
            ops: Vec::new(),
            y: 0.0,
            page: 0,
            footer: String::new(),
            total_pages: 0,
            chapters: Vec::new(),
        };
        let mut half = |d: u64, _: u64| progress(d, total);
        layout(&mut dry, lib, &ids, opts, 0, &[], &mut half)?;
        (dry.chapters, dry.page)
    };
    // How many pages the contents take, at the contents' line height.
    let per_page = ((size.1 - 2.0 * MARGIN - HEADING * 2.0) / (BODY * 1.4)).floor().max(1.0) as usize;
    let contents_pages = if opts.contents && opts.base.order == Order::Conversation {
        chapters.len().div_ceil(per_page).max(1)
    } else {
        0
    };
    // The dry run reserved one contents page; the real one may need more.
    let shift = contents_pages.saturating_sub(usize::from(opts.contents && opts.base.order == Order::Conversation));
    let chapters: Vec<(String, usize, f32)> = chapters.into_iter().map(|(n, p, y)| (n, p + shift, y)).collect();
    let mut doc = krilla::Document::new();
    let (done, pages) = {
        let mut w = Writer {
            doc: Some(&mut doc),
            shaper: &mut shaper,
            size,
            ops: Vec::new(),
            y: 0.0,
            page: 0,
            footer: String::new(),
            total_pages: pages + shift,
            chapters: Vec::new(),
        };
        let mut second = |d: u64, _: u64| progress(ids.len() as u64 + d, total);
        let done = layout(&mut w, lib, &ids, opts, contents_pages, &chapters, &mut second)?;
        (done, w.page)
    };
    let mut outline = krilla::outline::Outline::new();
    for (name, page, y) in &chapters {
        outline.push_child(krilla::outline::OutlineNode::new(
            name.clone(),
            krilla::destination::XyzDestination::new(*page, Point::from_xy(0.0, *y)),
        ));
    }
    doc.set_outline(outline);
    let title = opts.base.title.clone().unwrap_or_else(|| "oopz".into());
    doc.set_metadata(
        krilla::metadata::Metadata::new().title(title).creator("oopz".into()).language(opts.base.words.lang.clone()),
    );
    let bytes = doc.finish().map_err(|e| Error::new("io", format!("making the PDF: {e:?}")))?;
    let (path, written) = oopz_export::write_new_file(out_dir, "pdf", "pdf", &bytes)?;
    Ok(PdfWritten {
        path,
        messages: done,
        bytes: written,
        pages: pages as u64,
        missing_glyphs: shaper.fonts.missing / 2,
    })
}

#[cfg(test)]
mod tests;
