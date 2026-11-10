use oopz_model::{
    Conversation, ConversationKind, Loaded, Media, MediaKind, Message, Part, Person, Reaction, SourceInfo,
};

use super::*;

fn person(id: &str, name: &str, phone: Option<&str>, me: bool) -> Person {
    Person { id: id.into(), name: Some(name.into()), phone: phone.map(Into::into), me, ..Default::default() }
}

fn msg(conv: &str, id: &str, sender: &str, me: bool, t: i64, parts: Vec<Part>) -> Message {
    Message {
        id: id.into(),
        conversation: conv.into(),
        sender: Some(sender.into()),
        from_me: me,
        time_ms: Some(t),
        parts,
        ..Default::default()
    }
}

const DAY: i64 = 86_400_000;
const T0: i64 = 1_704_067_200_000; // 2024-01-01 00:00 UTC

/// A WhatsApp-like export: Ana one-to-one, and a group with someone not among the people.
fn whatsapp() -> Loaded {
    let c = "Ana";
    let g = "Family";
    Loaded {
        info: SourceInfo {
            app: App::WhatsApp,
            adapter: "test-whatsapp".into(),
            format: None,
            owner: None,
            media_root: None,
            wall_clock: true,
            notes: vec!["times are the phone's".into()],
        },
        people: vec![person("me", "Me", None, true), person("ana", "Ana WA", Some("+34 999 00 11 22"), false)],
        conversations: vec![
            Conversation {
                id: c.into(),
                kind: ConversationKind::Direct,
                name: None,
                members: vec!["me".into(), "ana".into()],
                created_ms: None,
            },
            Conversation {
                id: g.into(),
                kind: ConversationKind::Group,
                name: Some("Family".into()),
                members: vec![],
                created_ms: None,
            },
        ],
        messages: vec![
            msg(c, "1", "ana", false, T0, vec![Part::text("Hola, ¿vamos al café?")]),
            Message {
                reply_to: Some("1".into()),
                ..msg(c, "2", "me", true, T0 + 60_000, vec![Part::text("Sí, a las 5")])
            },
            msg(
                c,
                "3",
                "ana",
                false,
                T0 + DAY,
                vec![Part::Media(Media {
                    kind: MediaKind::Voice,
                    secs: Some(42),
                    size: Some(9_000),
                    ..Default::default()
                })],
            ),
            Message { deleted: true, ..msg(c, "4", "ana", false, T0 + 2 * DAY, vec![]) },
            Message {
                reactions: vec![Reaction { by: Some("me".into()), from_me: true, emoji: "❤️".into() }],
                ..msg(g, "1", "ghost", false, T0 + 3 * DAY, vec![Part::text("東京で会いましょう")])
            },
            Message { time_ms: None, ..msg(g, "2", "me", true, 0, vec![Part::text("no time here")]) },
        ],
    }
}

/// A Telegram-like export with the same Ana (same number, written differently) and real UTC times.
fn telegram() -> Loaded {
    let c = "user77";
    Loaded {
        info: SourceInfo {
            app: App::Telegram,
            adapter: "test-telegram".into(),
            format: Some("1".into()),
            owner: Some(person("user1", "Me TG", None, true)),
            media_root: None,
            wall_clock: false,
            notes: vec![],
        },
        people: vec![person("user77", "Ana TG", Some("0034999001122"), false)],
        conversations: vec![Conversation {
            id: c.into(),
            kind: ConversationKind::Direct,
            name: Some("Ana".into()),
            members: vec!["user1".into(), "user77".into()],
            created_ms: None,
        }],
        messages: vec![
            msg(
                c,
                "10",
                "user77",
                false,
                T0 + DAY / 2,
                vec![Part::text("Cafe mañana?"), Part::Link { url: "https://example.org".into(), title: None }],
            ),
            msg(
                c,
                "11",
                "user1",
                true,
                T0 + DAY / 2 + 1000,
                vec![Part::Poll { question: "When?".into(), options: vec![], multiple: false }],
            ),
        ],
    }
}

fn library() -> (tempfile::TempDir, Library, i64, i64) {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    let a = lib.import(&mut whatsapp(), "WhatsApp Chat with Ana.txt", "fp-a", &mut |_, _| true).unwrap().id;
    let b = lib.import(&mut telegram(), "result.json", "fp-b", &mut |_, _| true).unwrap().id;
    (dir, lib, a, b)
}

#[test]
fn imports_count_and_refuse_a_second_copy() {
    let (_dir, mut lib, a, b) = library();
    let sources = lib.sources().unwrap();
    assert_eq!(sources.len(), 2);
    let wa = &sources[0];
    assert_eq!((wa.id, wa.app, wa.messages, wa.conversations), (a, App::WhatsApp, 6, 2));
    assert!(wa.wall_clock && wa.owner_known);
    // The sender not among the people is kept, named by their id.
    assert_eq!(wa.people, 2);
    assert_eq!(sources[1].id, b);
    assert_eq!(sources[1].first_ms, Some(T0 + DAY / 2));
    let again = lib.import(&mut whatsapp(), "copy.txt", "fp-a", &mut |_, _| true).unwrap_err();
    assert_eq!(again.code, "exists");
}

#[test]
fn a_stopped_or_failed_import_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    let mut calls = 0;
    let e = lib
        .import(&mut whatsapp(), "x", "fp", &mut |_, _| {
            calls += 1;
            calls < 2
        })
        .unwrap_err();
    assert_eq!(e.code, "cancelled");
    assert!(lib.sources().unwrap().is_empty());
    assert_eq!(lib.count(&Filter::default()).unwrap(), 0);
    // And the same file can be imported afterwards.
    lib.import(&mut whatsapp(), "x", "fp", &mut |_, _| true).unwrap();
}

#[test]
fn search_ignores_accents_and_case_and_narrows() {
    let (_dir, lib, a, _) = library();
    let hits = lib.search(&Filter { words: "CAFE".into(), ..Default::default() }, 10).unwrap();
    assert_eq!(hits.len(), 2, "café and Cafe");
    assert_eq!(hits[0].app, App::Telegram, "newest first");
    assert!(hits[0].snippet.contains('\u{2}'));
    let only_wa = Filter { words: "caf".into(), apps: vec![App::WhatsApp], ..Default::default() };
    assert_eq!(lib.search(&only_wa, 10).unwrap().len(), 1);
    let voice = Filter { kinds: vec![Kind::Voice], ..Default::default() };
    assert_eq!(lib.search(&voice, 10).unwrap().len(), 1);
    assert!(lib.search(&Filter::default(), 10).unwrap().is_empty(), "an empty filter finds nothing");
    let cjk = Filter { words: "東京".into(), ..Default::default() };
    assert_eq!(lib.search(&cjk, 10).unwrap().len(), 1, "Japanese is found inside a sentence");
    let dates =
        Filter { sources: vec![a], after_ms: Some(T0 + DAY), before_ms: Some(T0 + 3 * DAY), ..Default::default() };
    assert_eq!(lib.count(&dates).unwrap(), 2);
    assert_eq!(lib.count(&Filter { from_me: Some(true), ..Default::default() }).unwrap(), 3);
    // Quotes and operators are words, not syntax.
    assert!(lib.search(&Filter { words: "\"OR NEAR(*".into(), ..Default::default() }, 10).is_ok());
}

#[test]
fn the_viewer_reads_in_source_order_with_replies_resolved() {
    let (_dir, lib, a, _) = library();
    let convs = lib.conversations(&Filter { sources: vec![a], ..Default::default() }).unwrap();
    let ana = convs.iter().find(|c| c.kind == ConversationKind::Direct).unwrap();
    assert_eq!(ana.members, ["Ana WA"]);
    assert_eq!(lib.conversation_len(ana.id).unwrap(), 4);
    let rows = lib.messages(ana.id, 0, 10).unwrap();
    assert_eq!(rows.iter().map(|r| r.message.id.as_str()).collect::<Vec<_>>(), ["1", "2", "3", "4"]);
    assert_eq!(rows[1].reply, Some(rows[0].id));
    assert_eq!(rows[2].kind, Kind::Voice);
    assert_eq!(rows[3].kind, Kind::Deleted);
    assert_eq!(rows[0].sender_name.as_deref(), Some("Ana WA"));
    assert_eq!(lib.position_at(ana.id, T0 + DAY).unwrap(), Some(2));
    assert_eq!(lib.position_at(ana.id, T0 + 99 * DAY).unwrap(), Some(3));
    assert_eq!(lib.position(rows[2].id).unwrap(), Some((ana.id, 2)));
    let group = convs.iter().find(|c| c.kind == ConversationKind::Group).unwrap();
    let rows = lib.messages(group.id, 0, 10).unwrap();
    assert_eq!(rows[0].sender_name.as_deref(), Some("ghost"));
    assert_eq!(rows[0].message.reactions.len(), 1);
    assert_eq!(rows[1].time_ms, None);
}

#[test]
fn contacts_link_across_apps_and_the_timeline_merges_them() {
    let (_dir, mut lib, _, _) = library();
    let suggestions = lib.suggestions().unwrap();
    assert_eq!(suggestions.len(), 1, "{suggestions:?}");
    assert_eq!(suggestions[0].reason, "phone");
    assert_eq!(suggestions[0].value, "+34999001122");
    let contact = lib.accept(&suggestions[0]).unwrap();
    assert!(lib.suggestions().unwrap().is_empty(), "linked people aren't suggested again");
    let c = &lib.contacts().unwrap()[0];
    assert_eq!(c.id, contact);
    assert_eq!(c.people.len(), 2);
    // Both one-to-one conversations whole, from both apps, in time order.
    let filter = Filter { contacts: vec![contact], ..Default::default() };
    let timeline = lib.find(&filter, Order::Time, 0, 100).unwrap();
    let apps: Vec<App> = timeline.iter().map(|r| r.app).collect();
    assert_eq!(apps, [App::WhatsApp, App::WhatsApp, App::Telegram, App::Telegram, App::WhatsApp, App::WhatsApp]);
    // Sender names come from the contact now.
    assert!(timeline.iter().filter(|r| !r.from_me).all(|r| r.sender_name.as_deref() == Some(c.name.as_str())));
    lib.rename_contact(contact, "Ana García").unwrap();
    assert_eq!(lib.contacts().unwrap()[0].name, "Ana García");
    let ana_wa = lib.people(None).unwrap().into_iter().find(|p| p.name.as_deref() == Some("Ana WA")).unwrap();
    lib.unlink(ana_wa.id).unwrap();
    assert_eq!(lib.contacts().unwrap()[0].people.len(), 1);
    assert_eq!(lib.suggestions().unwrap().len(), 1, "unlinked: suggested again");
}

#[test]
fn a_vcard_contact_absorbs_the_people_it_matches() {
    let (_dir, mut lib, _, _) = library();
    let added = lib
        .import_vcards(
            "BEGIN:VCARD\nVERSION:3.0\nFN:Ana García\nTEL:+34 999 001 122\nEMAIL:ana@example.org\nEND:VCARD\n",
        )
        .unwrap();
    assert_eq!(added, 1);
    let s = lib.suggestions().unwrap();
    assert_eq!(s[0].name.as_deref(), Some("Ana García"));
    let id = lib.accept(&s[0]).unwrap();
    let contacts = lib.contacts().unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!((contacts[0].id, contacts[0].people.len()), (id, 2));
    assert_eq!(contacts[0].emails, ["ana@example.org"]);
}

#[test]
fn set_me_turns_someones_messages_into_mine() {
    let (_dir, mut lib, a, _) = library();
    let ana = lib.people(Some(a)).unwrap().into_iter().find(|p| p.key == "ana").unwrap();
    lib.set_me(ana.id).unwrap();
    let mine = lib.count(&Filter { sources: vec![a], from_me: Some(true), ..Default::default() }).unwrap();
    assert_eq!(mine, 3, "Ana's three messages");
    assert!(lib.people(Some(a)).unwrap().iter().filter(|p| p.me).all(|p| p.id == ana.id));
}

#[test]
fn stats_count_by_kind_app_person_and_wall_clock_hours() {
    let (_dir, lib, _, _) = library();
    let s = lib.stats(&Filter::default()).unwrap();
    assert_eq!(s.messages, 8);
    assert_eq!(s.voice_secs, 42);
    assert_eq!(s.media_bytes, 9_000);
    assert_eq!(s.by_app, [(App::WhatsApp, 6), (App::Telegram, 2)]);
    assert!(s.by_kind.contains(&(Kind::Poll, 1, 0)));
    assert!(s.by_kind.contains(&(Kind::Link, 1, 0)));
    // The WhatsApp export's times are counted as written: midnight stays midnight.
    let wa = lib.stats(&Filter { apps: vec![App::WhatsApp], ..Default::default() }).unwrap();
    assert_eq!(wa.by_hour[0], 5);
    assert_eq!(wa.by_month, [("2024-01".to_owned(), 5)], "the message with no time isn't in a month");
    assert_eq!(wa.by_day_hour[1][0], 2, "Monday 1 January 2024");
}

#[test]
fn selections_add_remove_and_save() {
    let (_dir, mut lib, _, b) = library();
    let tg = Filter { sources: vec![b], ..Default::default() };
    let tg_ids = lib.selected(&Selection { filter: tg.clone(), ..Default::default() }, Order::Time).unwrap();
    assert_eq!(tg_ids.len(), 2);
    let extra = lib.find(&Filter { words: "東京".into(), ..Default::default() }, Order::Time, 0, 1).unwrap()[0].id;
    let sel = Selection { filter: tg, add: vec![extra], remove: vec![tg_ids[0]] };
    let ids = lib.selected(&sel, Order::Time).unwrap();
    assert_eq!(ids, [tg_ids[1], extra]);
    assert_eq!(lib.selected(&Selection { add: vec![extra], ..Default::default() }, Order::Time).unwrap(), [extra]);
    assert!(lib.selected(&Selection::default(), Order::Time).unwrap().is_empty());
    let rows = lib.messages_by_id(&ids).unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), ids);
    let id = lib.save_selection(" Trip ", &sel).unwrap();
    assert_eq!(lib.save_selection("Trip", &sel).unwrap(), id, "same name: replaced");
    let saved = lib.selections().unwrap();
    assert_eq!((saved.len(), saved[0].name.as_str()), (1, "Trip"));
    assert_eq!(saved[0].selection, sel);
    assert_eq!(lib.save_selection("  ", &sel).unwrap_err().code, "library");
    lib.delete_selection(id).unwrap();
    assert!(lib.selections().unwrap().is_empty());
}

#[test]
fn removing_a_source_takes_its_rows_and_search_entries() {
    let (dir, mut lib, a, b) = library();
    let suggestion = lib.suggestions().unwrap().remove(0);
    lib.accept(&suggestion).unwrap();
    std::fs::create_dir_all(dir.path().join("media").join(a.to_string())).unwrap();
    lib.remove_source(a).unwrap();
    assert!(!dir.path().join("media").join(a.to_string()).exists());
    assert_eq!(lib.sources().unwrap().len(), 1);
    assert!(lib.search(&Filter { words: "vamos".into(), ..Default::default() }, 10).unwrap().is_empty());
    assert_eq!(lib.search(&Filter { words: "cafe".into(), ..Default::default() }, 10).unwrap().len(), 1);
    assert_eq!(lib.contacts().unwrap()[0].people.len(), 1, "the contact keeps the Telegram side");
    assert_eq!(lib.remove_source(a).unwrap_err().code, "not_found");
    lib.remove_source(b).unwrap();
    assert!(lib.contacts().unwrap().is_empty(), "a contact with nobody and no card is gone");
}

#[test]
fn the_library_reopens_and_refuses_a_newer_one() {
    let (dir, lib, _, _) = library();
    drop(lib);
    let lib = Library::open(dir.path()).unwrap();
    assert_eq!(lib.count(&Filter::default()).unwrap(), 8);
    lib.db.pragma_update(None, "user_version", SCHEMA + 1).unwrap();
    drop(lib);
    assert_eq!(Library::open(dir.path()).err().unwrap().code, "unsupported");
}

#[test]
fn adapters_are_picked_by_name_or_detection() {
    struct Txt;
    impl Adapter for Txt {
        fn id(&self) -> &'static str {
            "txt"
        }
        fn app(&self) -> App {
            App::Line
        }
        fn secret(&self) -> m::Secret {
            m::Secret::None
        }
        fn detect(&self, path: &Path) -> bool {
            path.extension().is_some_and(|e| e == "txt")
        }
        fn open(&self, _: &Path, _: Option<&str>) -> m::Result<Box<dyn Source>> {
            Err(m::Error::NotThisFormat("txt"))
        }
    }
    let adapters: [&dyn Adapter; 1] = [&Txt];
    assert_eq!(pick_adapter(&adapters, Path::new("a.txt"), None).unwrap().id(), "txt");
    assert_eq!(pick_adapter(&adapters, Path::new("a.pdf"), None).err().unwrap().code, "unsupported_file");
    assert_eq!(pick_adapter(&adapters, Path::new("a.pdf"), Some("txt")).unwrap().id(), "txt");
    assert_eq!(pick_adapter(&adapters, Path::new("a.txt"), Some("nope")).err().unwrap().code, "unsupported_file");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "x").unwrap();
    let mut lib = Library::open(&dir.path().join("lib")).unwrap();
    let e = lib.import_path(&adapters, &file, None, None, &mut |_, _| true).unwrap_err();
    assert_eq!(e.code, "unsupported_file");
    assert_eq!(fingerprint(&file).unwrap().len(), 64);
    assert_eq!(fingerprint(dir.path()).unwrap().len(), 64);
}

/// An SMS backup's MMS parts carry their bytes: the library keeps them in its own folder, never in the database, and a
/// stopped import leaves no file behind.
#[test]
fn inline_files_are_kept_in_the_librarys_media_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    let inline = |name: Option<&str>, view_once: bool| {
        Part::Media(Media {
            kind: MediaKind::Photo,
            name: name.map(Into::into),
            data: Some(b"\x89PNG bytes".to_vec()),
            view_once,
            ..Default::default()
        })
    };
    let source = || {
        let mut s = whatsapp();
        s.messages[0].parts = vec![
            inline(Some("../../etc/pic 1.png"), false),
            inline(None, false),
            inline(Some("secret.png"), true),
        ];
        s
    };
    let mut stop = 0;
    let e = lib
        .import(&mut source(), "mms.xml", "fp-m", &mut |_, _| {
            stop += 1;
            stop < 2
        })
        .unwrap_err();
    assert_eq!(e.code, "cancelled");
    let media = dir.path().join("media");
    assert!(!media.exists() || fs::read_dir(&media).unwrap().count() == 0, "nothing left after a stop");

    let id = lib.import(&mut source(), "mms.xml", "fp-m", &mut |_, _| true).unwrap().id;
    let root = lib.media_root(id).unwrap().expect("a media folder");
    assert_eq!(root, media.join(id.to_string()));
    let mut files: Vec<String> =
        fs::read_dir(&root).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    assert_eq!(files, ["000001-pic_1.png", "000002-photo"], "the view-once part is never kept");
    assert_eq!(fs::read(root.join("000001-pic_1.png")).unwrap(), b"\x89PNG bytes");
    let first = lib
        .conversations(&Filter::default())
        .unwrap()
        .into_iter()
        .filter(|c| c.source == id)
        .map(|c| lib.messages(c.id, 0, 1).unwrap().remove(0))
        .find(|m| m.message.id == "1" && m.message.conversation == "Ana")
        .unwrap();
    let Part::Media(m) = &first.message.parts[0] else { panic!("a photo: {:?}", first.message) };
    assert_eq!((m.file.as_deref(), m.data.as_ref()), (Some("000001-pic_1.png"), None));
    lib.remove_source(id).unwrap();
    assert!(!root.exists(), "removing the source removes its files");
}
