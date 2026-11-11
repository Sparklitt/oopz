use oopz_core::Filter;
use oopz_model::{Conversation, ConversationKind, Loaded, Media, MediaKind, Message, Person, Reaction, SourceInfo};

use super::*;

/// A 4×3 PNG in oopz green.
const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 4, 0, 0, 0, 3, 8, 2, 0, 0, 0, 59, 150, 57,
    145, 0, 0, 0, 16, 73, 68, 65, 84, 120, 156, 99, 208, 143, 245, 135, 35, 6, 156, 28, 0, 198, 213, 10, 69, 91, 68,
    35, 170, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

/// An image XObject (`/Image` not followed by a letter: `/ImageB` is a ProcSet name).
fn images(pdf: &[u8]) -> usize {
    pdf.windows(7).filter(|x| &x[..6] == b"/Image" && !x[6].is_ascii_alphabetic()).count()
}

const WINDOWS_FONTS: &str = "/mnt/c/Windows/Fonts";

/// Fonts for the test: Windows' when this is WSL (emoji, Arabic, Hebrew, CJK all present), else the system's.
fn fonts() -> Vec<PathBuf> {
    if Path::new(WINDOWS_FONTS).join("seguiemj.ttf").is_file() {
        vec![PathBuf::from(WINDOWS_FONTS)]
    } else {
        Vec::new()
    }
}

const HEBREW: &str = "שלום עולם, מה שלומך היום?";
const ARABIC: &str = "مرحبا بالعالم، كيف حالك اليوم؟";
const EMOJI: &str = "👍🏽 ❤️ 👨‍👩‍👧 🇪🇸";
const CJK: &str = "東京で会いましょう。明日は晴れるでしょう。";

fn setup() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().unwrap();
    let media_root = dir.path().join("media");
    fs::create_dir_all(&media_root).unwrap();
    fs::write(media_root.join("IMG-1.png"), PNG).unwrap();
    fs::write(media_root.join("broken.jpg"), b"\xFF\xD8not a jpeg").unwrap();
    let msg = |conv: &str, id: &str, me: bool, t: i64, parts: Vec<Part>| Message {
        id: id.into(),
        conversation: conv.into(),
        sender: Some(if me { "me" } else { "ana" }.into()),
        from_me: me,
        time_ms: Some(1_704_067_200_000 + t * 60_000),
        parts,
        ..Default::default()
    };
    let photo =
        |file: &str| Part::Media(Media { kind: MediaKind::Photo, file: Some(file.into()), ..Default::default() });
    let mut messages = vec![
        msg("a", "1", false, 0, vec![Part::text("Hello, plain text first.")]),
        msg("a", "2", true, 1, vec![Part::text(HEBREW)]),
        msg("a", "3", false, 2, vec![Part::text(format!("{ARABIC} and English 123 inside"))]),
        Message {
            reactions: vec![Reaction { by: None, from_me: true, emoji: "😂".into() }],
            ..msg("a", "4", false, 3, vec![Part::text(EMOJI)])
        },
        Message { reply_to: Some("1".into()), ..msg("a", "5", true, 4, vec![Part::text(CJK)]) },
        msg("a", "6", false, 5, vec![photo("IMG-1.png"), photo("broken.jpg"), photo("../escape.png")]),
        msg("a", "7", false, 6, vec![Part::text("x".repeat(9_000))]),
        Message { deleted: true, ..msg("a", "8", false, 7, vec![]) },
    ];
    for i in 0..120 {
        messages.push(msg(
            "b",
            &format!("b{i}"),
            i % 3 == 0,
            10 + i,
            vec![Part::text(format!("Group line {i}: ñandú café"))],
        ));
    }
    let mut source = Loaded {
        info: SourceInfo {
            app: App::WhatsApp,
            adapter: "test".into(),
            format: None,
            owner: None,
            media_root: Some(media_root),
            wall_clock: false,
            notes: vec![],
        },
        people: vec![
            Person { id: "me".into(), me: true, ..Default::default() },
            Person { id: "ana".into(), name: Some("Ana ❤️".into()), ..Default::default() },
        ],
        conversations: vec![
            Conversation {
                id: "a".into(),
                kind: ConversationKind::Direct,
                name: Some("Ana".into()),
                members: vec![],
                created_ms: None,
            },
            Conversation {
                id: "b".into(),
                kind: ConversationKind::Group,
                name: Some("مجموعة Family".into()),
                members: vec![],
                created_ms: None,
            },
        ],
        messages,
    };
    let mut lib = Library::open(&dir.path().join("lib")).unwrap();
    lib.import(&mut source, "x", "fp", &mut |_, _| true).unwrap();
    (dir, lib)
}

fn all() -> Selection {
    Selection { filter: Filter { apps: vec![App::WhatsApp], ..Default::default() }, ..Default::default() }
}

fn opts() -> PdfOptions {
    PdfOptions {
        base: oopz_export::Options {
            time_zone: Some("UTC".into()),
            title: Some("Ana — chats".into()),
            ..Default::default()
        },
        fonts: fonts(),
        ..Default::default()
    }
}

#[test]
fn a_pdf_with_every_script_cover_contents_and_a_thumbnail() {
    let (dir, lib) = setup();
    let w = export_pdf(&lib, &all(), &opts(), dir.path(), &mut |_, _| true).unwrap();
    assert_eq!(w.path.file_name().unwrap(), "oopz-pdf-1.pdf");
    assert_eq!(w.messages, 128);
    let bytes = fs::read(&w.path).unwrap();
    assert!(bytes.starts_with(b"%PDF-") && bytes.windows(5).any(|x| x == b"%%EOF"));
    assert!(w.pages >= 5, "cover, contents, two chapters, the long text: {}", w.pages);
    assert!(images(&bytes) > 0, "the photo is drawn");
    if !fonts().is_empty() {
        assert_eq!(w.missing_glyphs, 0, "Windows' fonts have every character used here");
    }
    // A second one never replaces the first.
    let again = export_pdf(&lib, &all(), &opts(), dir.path(), &mut |_, _| true).unwrap();
    assert_eq!(again.path.file_name().unwrap(), "oopz-pdf-2.pdf");
}

#[test]
fn merged_letter_without_media_or_cover() {
    let (dir, lib) = setup();
    let o = PdfOptions {
        page: PageSize::Letter,
        media: MediaMode::None,
        cover: false,
        contents: false,
        base: oopz_export::Options {
            order: Order::Time,
            words: oopz_export::Words { lang: "es".into(), ..Default::default() },
            ..opts().base
        },
        ..opts()
    };
    let w = export_pdf(&lib, &all(), &o, dir.path(), &mut |_, _| true).unwrap();
    let bytes = fs::read(&w.path).unwrap();
    // A colour emoji font may draw its glyphs as pictures (Linux's Noto Color Emoji does): count against the same
    // export with the photos.
    let with_photos = PdfOptions { media: opts().media, ..o.clone() };
    let w2 = export_pdf(&lib, &all(), &with_photos, dir.path(), &mut |_, _| true).unwrap();
    assert!(images(&bytes) < images(&fs::read(&w2.path).unwrap()), "no pictures asked for");
    assert!(w.pages >= 2);
}

#[test]
fn right_to_left_lines_and_breaking() {
    let mut s = Shaper { fonts: Fonts::find(&fonts()) };
    if s.fonts.faces.is_empty() {
        return;
    }
    let lines = s.lines(HEBREW, BODY, 1000.0, false);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].rtl, "a Hebrew paragraph runs right to left");
    let lines = s.lines("abc ".repeat(200).as_str(), BODY, 200.0, false);
    assert!(lines.len() > 5 && lines.iter().all(|l| l.width <= 200.0 + 0.5), "wrapped within the width");
    let lines = s.lines(&"x".repeat(2000), BODY, 200.0, false);
    assert!(lines.len() > 5 && lines.iter().all(|l| l.width <= 200.0 + 8.0), "a word too long is cut");
    let lines = s.lines(&CJK.repeat(10), BODY, 150.0, false);
    assert!(lines.len() > 3, "CJK breaks between characters");
    assert_eq!(s.lines("", BODY, 100.0, false).len(), 1);
    if !fonts().is_empty() {
        let line = &s.lines("ok 👨‍👩‍👧 ok", BODY, 1000.0, false)[0];
        let emoji = line.runs.iter().find(|r| r.text.contains('\u{200D}')).expect("the family stays one run");
        assert!(s.fonts.faces[emoji.face].emoji, "drawn with the emoji font ({})", s.fonts.faces[emoji.face].name);
        assert!(emoji.text.contains("👨\u{200D}👩\u{200D}👧"), "the joined family isn't split across fonts");
    }
}

#[test]
fn a_stop_leaves_nothing() {
    let (dir, lib) = setup();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let e = export_pdf(&lib, &all(), &opts(), &out, &mut |d, _| d < 50).unwrap_err();
    assert_eq!(e.code, "cancelled");
    assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
    assert_eq!(export_pdf(&lib, &Selection::default(), &opts(), &out, &mut |_, _| true).unwrap_err().code, "not_found");
}

/// msgkinds' registry.md, "What readers test on their side", 2 and 3: every kind, every event and an unknown code
/// lay out into the PDF, each as lines of its own, with no duration for a kind without one.
#[test]
fn every_kind_and_event_lays_out() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(&dir.path().join("lib")).unwrap();
    lib.import(&mut oopz_gen::every_kind::every_kind(), "every-kind", "fp-every", &mut |_, _| true).unwrap();
    let ids = lib.selected(&all(), Order::Conversation).unwrap();
    let total = Kind::ALL.len() + oopz_model::Event::ALL.len();
    assert_eq!(ids.len(), total);
    let words = oopz_export::Words::default();
    for row in lib.messages_by_id(&ids).unwrap() {
        let lines = oopz_export::row_lines(&row, &words);
        assert!(lines.iter().any(|l| !l.trim().is_empty()), "{:?} has no lines", row.kind);
        assert!(!lines.iter().any(|l| l.contains("· 0:00")), "{:?}: {lines:?}", row.kind);
        if row.kind == Kind::Unknown {
            assert_eq!(lines, ["[WhatsApp: 9999, not shown yet]"]);
        }
    }
    let w = export_pdf(&lib, &all(), &opts(), dir.path(), &mut |_, _| true).unwrap();
    assert_eq!(w.messages, total as u64);
    let bytes = fs::read(&w.path).unwrap();
    assert!(bytes.starts_with(b"%PDF-") && bytes.len() > 1000);
}
