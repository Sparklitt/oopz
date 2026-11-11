//! The adapter against oopz-gen's synthetic Viber CSV export and odd input.

use std::collections::BTreeMap;

use oopz_gen::exports::{Manifest, viber_csv};
use oopz_model::{Adapter, ConversationKind, Error, Kind, Loaded};
use oopz_viber_export::{ViberExportAdapter, read};

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
fn the_zip_reads_as_its_manifest_says() {
    let export = viber_csv::generate(3);
    let zip = oopz_gen::exports::zip(&export.files);
    let loaded = read("Viber messages.zip", &zip).expect("read");
    assert_eq!(tally(&loaded), export.manifest);
    assert!(loaded.info.wall_clock);
    assert_eq!(loaded.info.owner.as_ref().map(|p| p.id.as_str()), Some("me"));
    assert!(loaded.messages.iter().all(|m| m.time_ms.is_some()));
    let kinds: Vec<_> = loaded.conversations.iter().map(|c| (c.name.as_deref(), c.kind)).collect();
    assert_eq!(
        kinds,
        [(Some("Ben Ejemplo"), ConversationKind::Direct), (Some("Fútbol los jueves"), ConversationKind::Group)]
    );
    let texts: Vec<String> = loaded.messages.iter().map(|m| m.text()).collect();
    assert!(texts.iter().any(|t| t == "hola, ¿qué tal? \"bien\""));
    assert!(texts.iter().any(|t| t == "line one\nline two"));
    assert!(loaded.people.iter().any(|p| p.phone.as_deref() == Some("+99990000002")));
    // Both files decide their own day/month order: the first messages of each are on the same day.
    let first: Vec<i64> = loaded
        .conversations
        .iter()
        .filter_map(|c| loaded.messages.iter().find(|m| m.conversation == c.id).and_then(|m| m.time_ms))
        .collect();
    assert_eq!(first[0] / 86_400_000, first[1] / 86_400_000);
}

#[test]
fn one_csv_is_detected_and_opened() {
    let export = viber_csv::generate(4);
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join(&export.files[1].0);
    std::fs::write(&path, &export.files[1].1).expect("written");
    std::fs::write(dir.path().join("prices.csv"), "item,price\napple,1\n").expect("written");
    let adapter = ViberExportAdapter;
    assert!(adapter.detect(&path));
    assert!(!adapter.detect(&dir.path().join("prices.csv")));
    let source = adapter.open(&path, None).expect("opened");
    assert_eq!(source.conversations()[0].kind, ConversationKind::Group);
}

#[test]
fn odd_rows() {
    let csv = "DATE;TEXT;NAME;TIME\n\
        01/02/1969;before 1970;Ana;10:00\n\
        01/02/2099;future;Ana;10:00\n\
        31/02/2024;no such day;Me;10:00\n\
        oops\n\
        01/02/2024;\"open quote;Ana;10:00\n";
    let l = read("x.csv", csv.as_bytes()).expect("read");
    assert!(l.messages[0].time_ms.is_some_and(|t| t < 0));
    assert!(l.messages[1].time_ms.is_some_and(|t| t > 4_000_000_000_000));
    assert_eq!(l.messages[2].time_ms, None);
    assert!(l.messages[2].from_me);
    assert_eq!(l.messages[3].kind(), Kind::Unknown);
    assert_eq!(l.messages.len(), 5);
    let notes = l.info.notes.join("\n");
    assert!(notes.contains("too short") && notes.contains("can't read"), "{notes}");
    assert!(!notes.contains("future"), "notes never hold message text");

    let headerless = "01/13/24,9:41 PM,Ben,,a, b, c\n";
    let l = read("y.csv", headerless.as_bytes()).expect("read");
    assert_eq!(l.messages[0].text(), "a, b, c");
    assert_eq!(l.messages[0].time_ms, oopz_model::civil_to_ms(2024, 1, 13, 21, 41, 0, 0));
}

#[test]
fn other_files_are_refused() {
    for bytes in [&b""[..], b"hello,world\n", b"item,price\napple,1\n", b"PK\x03\x04junk"] {
        assert!(matches!(read("x.csv", bytes), Err(Error::NotThisFormat(_))), "{bytes:?}");
    }
    let not_viber = oopz_gen::exports::zip(&[("../a.csv".into(), b"01/02/2024,10:00,Ana,,hi\n".to_vec())]);
    assert!(matches!(read("x.zip", &not_viber), Err(Error::NotThisFormat(_))));
}

#[test]
fn nothing_panics() {
    let export = viber_csv::generate(9);
    let bytes = &export.files[0].1;
    for cut in (0..bytes.len()).step_by(29) {
        let _ = read("x.csv", &bytes[..cut]);
    }
    let zip = oopz_gen::exports::zip(&export.files);
    for cut in (0..zip.len()).step_by(101) {
        let _ = read("x.zip", &zip[..cut]);
    }
    let mut seed = 5u64;
    for _ in 0..200 {
        let mut doc = b"01/02/2024,10:00,Ana,,".to_vec();
        for _ in 0..300 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            doc.push(match (seed >> 60) as u8 {
                0 => b'"',
                1 => b',',
                2 => b'\n',
                _ => (seed >> 52) as u8,
            });
        }
        let _ = read("x.csv", &doc);
    }
    let long = format!("01/02/2024,10:00,Ana,,{}\n", "مرحبا Z̤͔ͧ̑̓ ".repeat(8000));
    assert!(read("x.csv", long.as_bytes()).expect("read").messages[0].text().len() > 64 * 1024);
}

/// What happens when Viber changes its CSV (msgkinds' registry.md, "What readers test on their side", 4): columns it
/// adds or reorders are found by their header, a column it drops leaves that detail out, and a file with neither a
/// date nor a message column is refused.
mod format_changes {
    use super::*;

    /// The one-to-one chat's file (it has a header) with `f` applied to every row, the header included.
    fn changed(f: impl Fn(Vec<&str>) -> Vec<String>) -> (Loaded, Loaded) {
        let export = viber_csv::generate(3);
        let (name, bytes) = &export.files[0];
        let text = String::from_utf8(bytes.clone()).unwrap();
        let plain = read(name, bytes).unwrap();
        let mut rdr = csv::ReaderBuilder::new().has_headers(false).flexible(true).from_reader(text.as_bytes());
        let mut out = csv::Writer::from_writer(Vec::new());
        for r in rdr.records() {
            let r = r.unwrap();
            out.write_record(f(r.iter().collect())).unwrap();
        }
        let bytes = out.into_inner().unwrap();
        (plain, read(name, &bytes).unwrap_or_else(|e| panic!("{e}")))
    }

    #[test]
    fn columns_added_or_reordered_are_found_by_their_header() {
        let (plain, loaded) = changed(|row| {
            // Message, Sender, Date, Time, Phone, then a new column.
            let extra = if row[0] == "Date" { "Reactions" } else { "👍" };
            [row[4], row[2], row[0], row[1], row[3], extra].map(str::to_string).to_vec()
        });
        assert_eq!(tally(&loaded), tally(&plain));
        assert_eq!(
            loaded.messages.iter().map(|m| (m.time_ms, m.text())).collect::<Vec<_>>(),
            plain.messages.iter().map(|m| (m.time_ms, m.text())).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_column_dropped_leaves_that_detail_out() {
        let (plain, loaded) = changed(|row| [row[0], row[1], row[2], row[4]].map(str::to_string).to_vec());
        assert_eq!(loaded.messages.len(), plain.messages.len());
        assert!(loaded.people.iter().all(|p| p.phone.is_none() || p.me));
    }

    #[test]
    fn a_file_without_dates_or_messages_is_refused() {
        let export = viber_csv::generate(3);
        let (name, _) = &export.files[0];
        assert!(matches!(read(name, b"Sender,Phone\nAna,+99990000001\n"), Err(Error::NotThisFormat(_))));
    }
}
