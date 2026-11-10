//! The adapter against oopz-gen's synthetic exports (every layout, zipped, in a folder) and odd input.

use std::collections::BTreeMap;

use oopz_gen::exports::{Manifest, whatsapp_txt};
use oopz_model::{Adapter, ConversationKind, Error, Kind, Loaded, Part};
use oopz_whatsapp_export::{WhatsAppExportAdapter, read};

fn tally(l: &Loaded) -> Manifest {
    let mut kinds = BTreeMap::new();
    for m in &l.messages {
        *kinds.entry(m.kind()).or_default() += 1;
    }
    Manifest {
        conversations: l.conversations.len(),
        messages: l.messages.len(),
        from_me: l.messages.iter().filter(|m| m.from_me).count(),
        kinds,
    }
}

fn times(l: &Loaded) -> Vec<Option<i64>> {
    l.messages.iter().map(|m| m.time_ms.map(|t| t - t % 60_000)).collect()
}

#[test]
fn every_layout_reads_as_its_manifest_says() {
    let mut all_times = Vec::new();
    for layout in whatsapp_txt::Layout::ALL {
        let export = whatsapp_txt::generate(3, layout);
        let (name, bytes) = &export.files[0];
        let loaded = read(name, bytes).unwrap_or_else(|e| panic!("{layout:?}: {e}"));
        assert_eq!(tally(&loaded), export.manifest, "{layout:?}");
        let chat = &loaded.conversations[0];
        assert_eq!(chat.kind, ConversationKind::Group, "{layout:?}");
        assert_eq!(chat.members.len(), 3, "{layout:?}: {:?}", chat.members);
        if layout != whatsapp_txt::Layout::Ios {
            assert_eq!(chat.name.as_deref(), Some(whatsapp_txt::GROUP));
        }
        assert_eq!(loaded.info.owner.as_ref().map(|p| p.id.as_str()), Some(whatsapp_txt::OWNER), "{layout:?}");
        assert!(loaded.info.wall_clock);
        assert!(loaded.messages.iter().all(|m| m.time_ms.is_some()), "{layout:?}");
        assert!(loaded.messages.iter().any(|m| m.edited), "{layout:?}");
        // The order of day and month is decided by the file, so every layout gives the same minutes.
        all_times.push(times(&loaded));
        let poll = loaded.messages.iter().find_map(|m| match &m.parts[..] {
            [Part::Poll { options, .. }] => Some(options.iter().map(|o| o.votes).collect::<Vec<_>>()),
            _ => None,
        });
        assert_eq!(poll, Some(vec![2, 1, 0]), "{layout:?}");
    }
    assert!(all_times.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn a_zip_reads_the_same_with_its_files_and_contact_card() {
    for layout in [whatsapp_txt::Layout::Android, whatsapp_txt::Layout::Ios] {
        let export = whatsapp_txt::generate(3, layout);
        let zip = oopz_gen::exports::zip(&export.files);
        let loaded = read(&format!("WhatsApp Chat - {}.zip", whatsapp_txt::GROUP), &zip).expect("a zip");
        assert_eq!(tally(&loaded), export.manifest, "{layout:?}");
        assert_eq!(loaded.conversations[0].name.as_deref(), Some(whatsapp_txt::GROUP));
        assert!(!loaded.info.notes.iter().any(|n| n.contains("aren't in the export")), "{:?}", loaded.info.notes);
        let phones = loaded.messages.iter().find_map(|m| match &m.parts[..] {
            [Part::Contact { phones, .. }] => Some(phones.clone()),
            _ => None,
        });
        assert_eq!(phones, Some(vec!["+99990000002".to_string()]));
    }
}

#[test]
fn a_folder_is_detected_and_opened() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let export = whatsapp_txt::generate(5, whatsapp_txt::Layout::Android);
    for (name, bytes) in &export.files {
        std::fs::write(dir.path().join(name), bytes).expect("written");
    }
    let adapter = WhatsAppExportAdapter;
    assert!(adapter.detect(dir.path()));
    assert!(adapter.detect(&dir.path().join(&export.files[0].0)));
    assert!(!adapter.detect(&dir.path().join("IMG-20231212-WA0001.jpg")));
    let mut source = adapter.open(dir.path(), None).expect("opened");
    assert_eq!(source.info().media_root.as_deref(), Some(dir.path()));
    let chat = source.conversations()[0].id.clone();
    let mut n = 0;
    source
        .messages(&chat, &mut |_| {
            n += 1;
            true
        })
        .expect("messages");
    assert_eq!(n, export.manifest.messages);
}

#[test]
fn unsafe_zip_entries_are_ignored() {
    let files = vec![
        ("../WhatsApp Chat with Evil.txt".to_string(), b"01/02/2024, 10:00 - Eve: evil\n".to_vec()),
        ("WhatsApp Chat with Ana.txt".to_string(), b"01/02/2024, 10:00 - Ana: hola\n".to_vec()),
    ];
    let loaded = read("x.zip", &oopz_gen::exports::zip(&files)).expect("a zip");
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].sender.as_deref(), Some("Ana"));
    assert!(loaded.info.notes.iter().any(|n| n.contains("unsafe")));
}

#[test]
fn odd_lines_are_kept_and_counted() {
    let text = "a stray line\n\
        01/02/1969, 10:00 - Ana: before 1970\n\
        01/02/2099, 10:00 - Ben: from the future\n\
        31/02/2024, 10:00 - Ana: no such day\n\
        01/01/2024, 10:00 - Ana: out of order\n\
        01/01/2024, 10:00 - Ana: out of order\n\
        continued\n\
        01/01/2024, 10:01 - Ben: <This message was edited>\n";
    let loaded = read("WhatsApp Chat with Ben.txt", text.as_bytes()).expect("read");
    let m = &loaded.messages;
    assert_eq!(m.len(), 7);
    assert_eq!(m[0].kind(), Kind::Unknown);
    assert!(m[1].time_ms.is_some_and(|t| t < 0));
    assert!(m[2].time_ms.is_some_and(|t| t > 4_000_000_000_000));
    assert_eq!(m[3].time_ms, None);
    assert_ne!(m[4].id, m[5].id);
    assert_eq!(m[5].text(), "out of order\ncontinued");
    assert!(m[6].edited);
    let notes = loaded.info.notes.join("\n");
    assert!(notes.contains("1 lines before the first message"), "{notes}");
    assert!(notes.contains("doesn't exist"), "{notes}");
    assert!(notes.contains("pick yourself"), "{notes}");
    let undecided = read("x.txt", b"01/02/2024, 10:00 - Ana: hola\n").expect("read");
    assert!(undecided.info.notes.iter().any(|n| n.contains("day first")));
    assert_eq!(undecided.messages[0].time_ms, oopz_model::civil_to_ms(2024, 2, 1, 10, 0, 0, 0));
    assert!(!notes.contains("stray") && !notes.contains("future"), "notes never hold message text");
    assert_eq!(loaded.conversations[0].kind, ConversationKind::Direct);
}

#[test]
fn other_files_are_refused() {
    for bytes in [&b""[..], b"hello\nworld\n", b"\x00\x01\x02", b"PK\x03\x04broken", b"{\"chats\": []}"] {
        assert!(matches!(read("x.txt", bytes), Err(Error::NotThisFormat(_))), "{bytes:?}");
    }
}

#[test]
fn nothing_panics() {
    let export = whatsapp_txt::generate(9, whatsapp_txt::Layout::Ios);
    let bytes = &export.files[0].1;
    for cut in (0..bytes.len()).step_by(97).chain([1, 2, 3, bytes.len() - 1]) {
        let _ = read("_chat.txt", &bytes[..cut]);
    }
    let zip = oopz_gen::exports::zip(&export.files);
    for cut in (0..zip.len()).step_by(211) {
        let _ = read("WhatsApp Chat - x.zip", &zip[..cut]);
    }
    let mut seed = 1u64;
    for _ in 0..200 {
        let noise: Vec<u8> = (0..512)
            .map(|_| {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                (seed >> 56) as u8
            })
            .collect();
        let _ = read("x.txt", &noise);
        let mut line = b"01/02/2024, 10:00 - Ana: ".to_vec();
        line.extend(&noise);
        let _ = read("x.txt", &line);
    }
    let huge = format!("01/02/2024, 10:00 - Ana: {}\n", "مرحبا Z̤͔ͧ̑̓ ".repeat(8000));
    let loaded = read("x.txt", huge.as_bytes()).expect("a long line");
    assert!(loaded.messages[0].text().len() > 64 * 1024);
}
