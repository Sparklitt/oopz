use std::io::Read as _;

use oopz_core::Filter;
use oopz_model::{Conversation, ConversationKind, Loaded, Media, MediaKind, Message, Person, SourceInfo};

use super::*;

const T0: i64 = 1_704_067_200_000;

fn media(kind: MediaKind, file: &str) -> Part {
    Part::Media(Media { kind, file: Some(file.into()), size: Some(5), ..Default::default() })
}

/// A library with one invented source whose media folder holds some of the files its messages name.
fn setup() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().unwrap();
    let media_root = dir.path().join("media-src");
    fs::create_dir_all(media_root.join("sub")).unwrap();
    fs::write(media_root.join("IMG-1.jpg"), b"jpeg!").unwrap();
    fs::write(media_root.join("sub/IMG-1.jpg"), b"synthetic bytes").unwrap();
    fs::write(dir.path().join("secret.txt"), b"outside").unwrap();
    let c = "chat";
    let msg = |id: &str, me: bool, t: i64, parts: Vec<Part>| Message {
        id: id.into(),
        conversation: c.into(),
        sender: Some(if me { "me" } else { "ana" }.into()),
        from_me: me,
        time_ms: Some(t),
        parts,
        ..Default::default()
    };
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
            Person {
                id: "ana".into(),
                name: Some("Ana <b>".into()),
                phone: Some("+999 555 0100".into()),
                ..Default::default()
            },
        ],
        conversations: vec![Conversation {
            id: c.into(),
            kind: ConversationKind::Direct,
            name: Some("Ana & co".into()),
            members: vec!["me".into(), "ana".into()],
            created_ms: None,
        }],
        messages: vec![
            msg("1", false, T0, vec![Part::text("<script>alert(1)</script> שלום \u{202E}evil")]),
            Message {
                reply_to: Some("1".into()),
                edited: true,
                ..msg("2", true, T0 + 60_000, vec![Part::text("=HYPERLINK(\"x\")")])
            },
            msg(
                "3",
                false,
                T0 + 120_000,
                vec![media(MediaKind::Photo, "IMG-1.jpg"), media(MediaKind::Photo, "sub/IMG-1.jpg")],
            ),
            msg(
                "4",
                false,
                T0 + 180_000,
                vec![media(MediaKind::Video, "gone.mp4"), media(MediaKind::Document, "../secret.txt")],
            ),
            msg(
                "5",
                false,
                T0 + 240_000,
                vec![Part::Media(Media {
                    kind: MediaKind::Photo,
                    view_once: true,
                    caption: Some("never shown".into()),
                    file: Some("IMG-1.jpg".into()),
                    ..Default::default()
                })],
            ),
            Message { deleted: true, ..msg("6", false, T0 + 300_000, vec![]) },
            msg(
                "7",
                false,
                T0 + 360_000,
                vec![Part::System { event: m::Event::GroupCreated, text: Some("made the group".into()) }],
            ),
        ],
    };
    let mut lib = Library::open(&dir.path().join("lib")).unwrap();
    lib.import(&mut source, "chat.txt", "fp", &mut |_, _| true).unwrap();
    (dir, lib)
}

fn all() -> Selection {
    Selection { filter: Filter { apps: vec![App::WhatsApp], ..Default::default() }, ..Default::default() }
}

fn utc() -> Options {
    Options { time_zone: Some("UTC".into()), title: Some("Ana <test>".into()), ..Default::default() }
}

fn run(lib: &Library, dir: &Path, format: Format, options: &Options) -> Written {
    export(lib, &all(), format, options, dir, &mut |_, _| true).unwrap()
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap()
}

#[test]
fn html_is_escaped_offline_and_never_shows_view_once() {
    let (dir, lib) = setup();
    let w = run(&lib, dir.path(), Format::Html, &utc());
    assert_eq!(w.path.file_name().unwrap(), "oopz-html-1.html");
    assert_eq!(w.messages, 7);
    let html = read(&w.path);
    assert!(html.contains("&lt;script&gt;") && !html.contains("<script>"));
    assert!(!html.contains('\u{202E}'), "bidi overrides are dropped");
    assert!(html.contains("שלום") && html.contains("dir=\"auto\""));
    assert!(html.contains("Content-Security-Policy") && !html.contains("://"));
    assert!(!html.contains("never shown"));
    assert!(html.contains("Ana &lt;b&gt;") && html.contains("Ana &amp; co"));
    assert!(html.contains("2024-01-01 00:01"), "times in the zone asked for");
    assert!(html.contains("This message was deleted"));
    assert!(html.contains("Reply to"));
    // A second export never replaces the first.
    assert_eq!(run(&lib, dir.path(), Format::Html, &utc()).path.file_name().unwrap(), "oopz-html-2.html");
}

#[test]
fn json_parses_back_with_only_what_the_selection_touches() {
    let (dir, lib) = setup();
    let w = run(&lib, dir.path(), Format::Json, &utc());
    let v: serde_json::Value = serde_json::from_str(&read(&w.path)).unwrap();
    assert_eq!(v["messages"].as_array().unwrap().len(), 7);
    assert_eq!(v["people"].as_array().unwrap().len(), 2);
    assert_eq!(v["oopz"]["format"], "oopz-export");
    assert!(!read(&w.path).contains("never shown"));
    let first = &v["messages"][0];
    assert_eq!(first["app"], m::App::WhatsApp.code());
    assert_eq!(first["message"]["parts"][0]["type"], "text");
}

#[test]
fn csv_rows_parse_and_formulas_stay_text() {
    let (dir, lib) = setup();
    let w = run(&lib, dir.path(), Format::Csv, &utc());
    let mut r = csv::Reader::from_path(&w.path).unwrap();
    let rows: Vec<csv::StringRecord> = r.records().map(|x| x.unwrap()).collect();
    assert_eq!(rows.len(), 7);
    assert_eq!(&rows[1][6], "'=HYPERLINK(\"x\")");
    assert_eq!(&rows[0][2], "2024-01-01T00:00:00+00:00");
    assert!(!rows[1][7].is_empty() && rows[0][7].is_empty(), "the reply points at the first message");
    assert_eq!(&rows[5][9], "1", "the deleted column");
}

#[test]
fn txt_reads_like_a_chat() {
    let (dir, lib) = setup();
    let merged = Options { order: Order::Time, ..utc() };
    let w = run(&lib, dir.path(), Format::Txt, &merged);
    let txt = read(&w.path);
    assert!(txt.contains("[2024-01-01 00:01] WhatsApp · Ana & co · You: =HYPERLINK"), "{txt}");
    assert!(txt.contains("    > Reply to: <script>"), "the reply quotes what it answers");
    assert!(txt.contains("[Photo · 5 B]"));
    assert!(!txt.contains("never shown"));
    let skip = Options { skip_system: true, ..utc() };
    assert!(!read(&run(&lib, dir.path(), Format::Txt, &skip).path).contains("made the group"));
}

#[test]
fn media_zip_takes_present_files_and_lists_missing_ones_never_outside() {
    let (dir, lib) = setup();
    let w = run(&lib, dir.path(), Format::MediaZip, &utc());
    assert_eq!(w.missing_files, 2, "gone.mp4 and the path outside the folder");
    let mut zip = zip::ZipArchive::new(fs::File::open(&w.path).unwrap()).unwrap();
    let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned()).collect();
    assert_eq!(names, ["whatsapp-Ana _ co/IMG-1.jpg", "whatsapp-Ana _ co/2-IMG-1.jpg", "missing.txt"]);
    let mut s = String::new();
    zip.by_name("missing.txt").unwrap().read_to_string(&mut s).unwrap();
    assert!(s.contains("gone.mp4") && s.contains("../secret.txt"));
    let mut photo = Vec::new();
    zip.by_name("whatsapp-Ana _ co/IMG-1.jpg").unwrap().read_to_end(&mut photo).unwrap();
    assert_eq!(photo, b"jpeg!");
}

#[test]
fn vcards_of_the_people_in_it() {
    let (dir, lib) = setup();
    let w = run(&lib, dir.path(), Format::Vcard, &utc());
    let cards = oopz_core::vcard::parse(&read(&w.path));
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].name, "Ana <b>");
    assert_eq!(cards[0].phones, ["+999 555 0100"]);
    assert!(export_contacts(&lib, &[], dir.path()).is_err());
}

#[test]
fn a_stop_leaves_nothing_and_bad_requests_are_refused() {
    let (dir, lib) = setup();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let e = export(&lib, &all(), Format::Html, &utc(), &out, &mut |_, _| false).unwrap_err();
    assert_eq!(e.code, "cancelled");
    assert_eq!(fs::read_dir(&out).unwrap().count(), 0, "no partial or temporary file");
    let e = export(&lib, &Selection::default(), Format::Html, &utc(), &out, &mut |_, _| true).unwrap_err();
    assert_eq!(e.code, "not_found");
    let bad_zone = Options { time_zone: Some("Mars/Olympus".into()), ..utc() };
    assert!(export(&lib, &all(), Format::Html, &bad_zone, &out, &mut |_, _| true).is_err());
    let e = export(&lib, &all(), Format::Html, &utc(), &out.join("nope"), &mut |_, _| true).unwrap_err();
    assert_eq!(e.code, "not_found");
    for f in Format::ALL {
        assert_eq!(Format::from_code(f.code()), Some(f));
    }
}

#[test]
fn spanish_words_are_the_ones_passed_in() {
    let (dir, lib) = setup();
    let words = Words {
        lang: "es".into(),
        me: "Tú".into(),
        edited: "editado".into(),
        deleted: "Se eliminó este mensaje".into(),
        photo: "Foto".into(),
        ..Default::default()
    };
    let es = Options { words, ..utc() };
    let html = read(&run(&lib, dir.path(), Format::Html, &es).path);
    assert!(html.contains("lang=\"es\"") && html.contains("Se eliminó este mensaje") && html.contains(" · editado"));
    let txt = read(&run(&lib, dir.path(), Format::Txt, &es).path);
    assert!(txt.contains("Tú: =HYPERLINK") && txt.contains("(editado)") && txt.contains("[Foto · 5 B]"), "{txt}");
}

#[test]
fn any_language_works() {
    let (dir, lib) = setup();
    let words = Words {
        lang: "x-zorb".into(),
        me: "Mimi".into(),
        edited: "zorbed".into(),
        photo: "Pikka".into(),
        outcomes: [("missed".to_owned(), "zipped".to_owned())].into(),
        events: [(m::Event::GroupCreated.code().to_owned(), "Bloop".to_owned())].into(),
        ..Default::default()
    };
    let o = Options { words: words.clone(), ..utc() };
    let txt = read(&run(&lib, dir.path(), Format::Txt, &o).path);
    assert!(txt.contains("Mimi: =HYPERLINK") && txt.contains("(zorbed)") && txt.contains("[Pikka · 5 B]"), "{txt}");
    assert!(read(&run(&lib, dir.path(), Format::Html, &o).path).contains("lang=\"x-zorb\""));
    let call = Part::Call { video: false, outcome: m::CallOutcome::Missed, secs: None };
    assert_eq!(part_lines(&call, App::WhatsApp, &words), ["[Voice call · zipped]"]);
    let notice = Part::System { event: m::Event::GroupCreated, text: None };
    assert_eq!(part_lines(&notice, App::WhatsApp, &words), ["— Bloop —"]);
    // Words the app leaves out are English.
    let partial: Options = serde_json::from_str(r#"{"words":{"me":"Mimi"}}"#).unwrap();
    assert_eq!((partial.words.me.as_str(), partial.words.edited.as_str()), ("Mimi", "edited"));
}

/// A library holding oopz-gen's message of every kind and notice of every event.
fn every_kind() -> (tempfile::TempDir, Library) {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(&dir.path().join("lib")).unwrap();
    lib.import(&mut oopz_gen::every_kind::every_kind(), "every-kind", "fp-every", &mut |_, _| true).unwrap();
    (dir, lib)
}

/// msgkinds' registry.md, "What readers test on their side", 2: every kind and every event exports in every text
/// format, and a kind without a duration never shows one (its file says 0 seconds).
#[test]
fn every_kind_and_event_exports_without_a_duration_where_none_belongs() {
    let (dir, lib) = every_kind();
    let total = (Kind::ALL.len() + m::Event::ALL.len()) as u64;
    let w = Words::default();
    for format in [Format::Html, Format::Json, Format::Csv, Format::Txt] {
        let written = run(&lib, dir.path(), format, &utc());
        assert_eq!(written.messages, total, "{format:?}");
        let text = read(&written.path);
        // Durations follow a " · " (times of day are "00:00", so a bare "0:00" would match them).
        assert!(!text.contains("· 0:00"), "{format:?}: a duration of 0 for a kind without one");
        if format == Format::Json {
            continue;
        }
        for word in [&w.sticker, &w.photo, &w.document, &w.gif, &w.video_note, &w.deleted] {
            assert!(text.contains(word.as_str()), "{format:?}: {word}");
        }
        assert!(text.contains("· 1:15"), "{format:?}: kinds that play keep their duration");
        for card in m::CardKind::ALL {
            assert!(text.contains(&format!("{} 9999", m::Kind::from(card).code())), "{format:?}: {card:?}");
            assert!(text.contains(&Words::code_word(&w.cards, card)), "{format:?}: {card:?}'s word");
        }
        for event in m::Event::ALL {
            let word = Words::code_word(&w.events, event);
            assert_ne!(word, event.code(), "an English word for {event:?}");
            assert!(text.contains(&esc(&word)), "{format:?}: {event:?}");
        }
    }
    // Each message's kind is in the CSV's kind column.
    let csv = read(&run(&lib, dir.path(), Format::Csv, &utc()).path);
    for kind in Kind::ALL {
        assert!(csv.contains(&format!(",{},", kind.code())), "{kind:?}");
    }
}

/// 3: an unknown code is a labelled line in every format, and counted.
#[test]
fn an_unknown_code_is_labelled_counted_and_exported() {
    let (dir, lib) = every_kind();
    for format in [Format::Html, Format::Csv, Format::Txt] {
        let text = read(&run(&lib, dir.path(), format, &utc()).path);
        assert!(text.contains("[WhatsApp: 9999, not shown yet]"), "{format:?}");
    }
    let json = read(&run(&lib, dir.path(), Format::Json, &utc()).path);
    assert!(json.contains(r#""type":"unknown","code":"9999""#));
    let stats = lib.stats(&Filter::default()).unwrap();
    assert!(stats.by_kind.iter().any(|(k, n, _)| *k == Kind::Unknown && *n == 1), "{:?}", stats.by_kind);
    for kind in Kind::ALL {
        let only = Selection { filter: Filter { kinds: vec![kind], ..Default::default() }, ..Default::default() };
        let w = export(&lib, &only, Format::Txt, &utc(), dir.path(), &mut |_, _| true).unwrap();
        let want = if kind == Kind::System { 1 + m::Event::ALL.len() as u64 } else { 1 };
        assert_eq!(w.messages, want, "filtered by {kind:?}");
    }
}

/// What the app shows a sticker, photo or voice note from: the file in the source's folder, never a view-once one or
/// one outside it.
#[test]
fn a_media_file_is_found_only_inside_its_folder() {
    let (_dir, lib) = setup();
    let ids = lib.selected(&all(), Order::Conversation).unwrap();
    let rows = lib.messages_by_id(&ids).unwrap();
    let id = |key: &str| rows.iter().find(|r| r.message.id == key).unwrap().id;
    assert!(lib.media_file(id("3"), 0).unwrap().unwrap().ends_with("IMG-1.jpg"));
    assert!(lib.media_file(id("3"), 1).unwrap().unwrap().ends_with("sub/IMG-1.jpg"));
    assert_eq!(lib.media_file(id("4"), 0).unwrap(), None, "missing");
    assert_eq!(lib.media_file(id("4"), 1).unwrap(), None, "outside the folder");
    assert_eq!(lib.media_file(id("5"), 0).unwrap(), None, "view once");
    assert_eq!(lib.media_file(id("1"), 0).unwrap(), None, "not media");
    assert_eq!(lib.media_file(9_999_999, 0).unwrap(), None, "no such message");
}

/// Every language of oopz's messages words every field, outcome, event and card; another language gets the default
/// language's words and says so.
#[test]
fn words_in_every_language_and_a_fallback() {
    for lang in LANGUAGES {
        let w = Words::in_language(lang);
        assert_eq!(w.lang, lang);
        assert_eq!(
            (w.outcomes.len(), w.events.len(), w.cards.len()),
            (m::CallOutcome::ALL.len(), m::Event::ALL.len(), m::CardKind::ALL.len())
        );
        for word in w.outcomes.values().chain(w.events.values()).chain(w.cards.values()).chain([&w.me, &w.note]) {
            assert!(!word.is_empty(), "{lang}");
        }
    }
    assert_ne!(Words::in_language(LANGUAGES[0]).note, Words::in_language(LANGUAGES[1]).note);
    let other = Words::in_language("xx");
    assert_eq!((other.lang.as_str(), other.note), (LANGUAGES[0], Words::default().note));
}
