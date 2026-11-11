//! The adapter against oopz-gen's synthetic LINE exports and odd input.

use std::collections::BTreeMap;

use oopz_gen::exports::{Manifest, line_txt};
use oopz_line_export::{LineExportAdapter, read};
use oopz_model::{Adapter, CallOutcome, ConversationKind, Error, Kind, Loaded, Part};

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

#[test]
fn every_layout_reads_as_its_manifest_says() {
    let mut english_times = Vec::new();
    for layout in line_txt::Layout::ALL {
        let export = line_txt::generate(3, layout);
        let (name, bytes) = &export.files[0];
        let loaded = read(name, bytes).unwrap_or_else(|e| panic!("{layout:?}: {e}"));
        assert_eq!(tally(&loaded), export.manifest, "{layout:?}");
        assert!(loaded.info.wall_clock);
        assert!(loaded.messages.iter().all(|m| m.time_ms.is_some()), "{layout:?}");
        let chat = &loaded.conversations[0];
        let quoted = loaded.messages.iter().find(|m| m.text().starts_with("he said")).map(|m| m.text());
        assert_eq!(quoted.as_deref(), Some("he said \"hi\"\nand then\n\n\"bye\""), "{layout:?}");
        let call = loaded.messages.iter().find_map(|m| match m.parts[..] {
            [Part::Call { outcome, secs, .. }] => Some((outcome, secs)),
            _ => None,
        });
        assert_eq!(call, Some((CallOutcome::Answered, Some(15))), "{layout:?}");
        if layout == line_txt::Layout::Ja {
            assert_eq!(chat.name.as_deref(), Some("フットサル部"));
            assert_eq!(chat.kind, ConversationKind::Group);
            assert!(loaded.info.owner.is_none());
        } else {
            assert_eq!(chat.name.as_deref(), Some("Ben Ejemplo"));
            assert_eq!(chat.kind, ConversationKind::Direct);
            assert_eq!(loaded.info.owner.as_ref().map(|p| p.id.as_str()), Some("Ana Prueba"));
            english_times.push(loaded.messages.iter().map(|m| m.time_ms).collect::<Vec<_>>());
        }
    }
    assert!(english_times.windows(2).all(|w| w[0] == w[1]), "every English layout gives the same times");
}

#[test]
fn detected_and_opened_from_a_file() {
    let export = line_txt::generate(4, line_txt::Layout::En);
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join(&export.files[0].0);
    std::fs::write(&path, &export.files[0].1).expect("written");
    std::fs::write(dir.path().join("notes.txt"), "just some notes\n").expect("written");
    let adapter = LineExportAdapter;
    assert!(adapter.detect(&path));
    assert!(!adapter.detect(&dir.path().join("notes.txt")));
    assert_eq!(adapter.open(&path, None).expect("opened").conversations().len(), 1);
}

#[test]
fn odd_lines() {
    let text = "[LINE] Chat history in Club\nSaved on: 2024/02/01 10:00\n\n\
        10:00\tAna\tbefore any date\n\
        1969/12/31(Wed)\n\
        23:59\tAna\tbefore 1970\n\
        2099/01/01(Thu)\n\
        25:00\tBen\tno such hour\n\
        10:01\tBen\t\"never closed\n\
        10:02\tCarla\tstill read\n\
        a line that continues\n";
    let l = read("x.txt", text.as_bytes()).expect("read");
    let texts: Vec<String> = l.messages.iter().map(|m| m.text()).collect();
    assert_eq!(
        texts,
        ["before any date", "before 1970", "no such hour", "\"never closed", "still read\na line that continues"]
    );
    assert_eq!(l.messages[0].time_ms, None);
    assert_eq!(l.messages[1].time_ms, Some(-60_000));
    assert_eq!(l.messages[2].time_ms, None);
    assert_eq!(l.conversations[0].kind, ConversationKind::Group);
    let notes = l.info.notes.join("\n");
    for want in ["before any date line", "doesn't exist", "never closed", "pick yourself"] {
        assert!(notes.contains(want), "{want}: {notes}");
    }
    assert!(!notes.contains("1970"), "notes never hold message text");

    let headless = "2024/01/02(Tue)\n10:00\tAna\thi\n";
    assert_eq!(read("x.txt", headless.as_bytes()).expect("read").messages.len(), 1);
    let stray = "[LINE] Chat history with Ben\nsomething odd\n2024/01/02(Tue)\n10:00\tBen\thi\n";
    let l = read("x.txt", stray.as_bytes()).expect("read");
    assert_eq!(l.messages[0].kind(), Kind::Unknown);
}

#[test]
fn other_files_are_refused() {
    for bytes in [&b""[..], b"hello\nworld", b"10:00\tAna\thi\n", b"\x00\x01"] {
        assert!(matches!(read("x.txt", bytes), Err(Error::NotThisFormat(_))), "{bytes:?}");
    }
}

#[test]
fn nothing_panics() {
    let export = line_txt::generate(9, line_txt::Layout::Ja);
    let bytes = &export.files[0].1;
    for cut in (0..bytes.len()).step_by(37) {
        let _ = read("x.txt", &bytes[..cut]);
    }
    let mut seed = 3u64;
    for _ in 0..200 {
        let mut doc = b"[LINE] Chat history with X\n2024/01/01(Mon)\n10:00\tX\t".to_vec();
        for _ in 0..300 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            doc.push(match (seed >> 60) as u8 {
                0 => b'"',
                1 => b'\t',
                2 => b'\n',
                _ => (seed >> 52) as u8,
            });
        }
        let _ = read("x.txt", &doc);
    }
    let many_quotes = format!("[LINE] Chat history with X\n2024/01/01(Mon)\n{}", "10:00\tX\t\"open\n".repeat(5000));
    assert_eq!(read("x.txt", many_quotes.as_bytes()).expect("read").messages.len(), 5000);
    let long = format!("[LINE] Chat history with X\n2024/01/01(Mon)\n10:00\tX\t{}\n", "مرحبا Z̤͔ͧ̑̓ ".repeat(8000));
    assert!(read("x.txt", long.as_bytes()).expect("read").messages[0].text().len() > 64 * 1024);
}

/// What happens when LINE changes its `.txt` (msgkinds' registry.md, "What readers test on their side", 4): lines it
/// adds are kept as unknown and counted, a column it adds stays in the text, and lines missing what dates them are
/// kept without a time and said; nothing is dropped.
mod format_changes {
    use super::*;

    fn text() -> (String, String, Manifest) {
        let export = line_txt::generate(3, line_txt::Layout::ALL[0]);
        let (name, bytes) = &export.files[0];
        (name.clone(), String::from_utf8(bytes.clone()).unwrap(), export.manifest)
    }

    #[test]
    fn lines_added_before_the_chat_are_kept_and_counted() {
        let (name, t, manifest) = text();
        let changed = format!("Exported by LINE 99.0\u{a0}(beta)\n{t}");
        let loaded = read(&name, changed.as_bytes()).expect("reads");
        assert!(loaded.messages.len() >= manifest.messages);
        let notes = loaded.info.notes.join("\n");
        assert!(loaded.messages.len() == manifest.messages || notes.contains("kept as unknown"), "{notes}");
    }

    #[test]
    fn a_column_added_stays_in_the_text() {
        let (name, t, manifest) = text();
        let changed: String = t
            .lines()
            .map(|l| if l.matches('\t').count() >= 2 { format!("{l}\t✓✓\n") } else { format!("{l}\n") })
            .collect();
        let loaded = read(&name, changed.as_bytes()).expect("reads");
        assert_eq!(loaded.messages.len(), manifest.messages);
    }

    #[test]
    fn messages_without_their_date_line_are_kept_without_a_time() {
        let (name, t, manifest) = text();
        // The header, then every line but the date lines.
        let mut lines = t.lines();
        let head: Vec<&str> = lines.by_ref().take(2).collect();
        let body: Vec<&str> = lines.filter(|l| l.contains('\t') || l.is_empty() || !l.contains('/')).collect();
        let changed = [head, body].concat().join("\n");
        let loaded = read(&name, changed.as_bytes()).expect("reads");
        assert_eq!(loaded.messages.len(), manifest.messages);
        let notes = loaded.info.notes.join("\n");
        assert!(notes.contains("before any date line") || notes.contains("without a time"), "{notes}");
    }
}
