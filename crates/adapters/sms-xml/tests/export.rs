//! The adapter against oopz-gen's synthetic SMS and call XML, and odd input.

use std::collections::{BTreeMap, BTreeSet};

use oopz_gen::exports::{Manifest, sms_xml};
use oopz_model::{Adapter, CallOutcome, ConversationKind, Error, Kind, Loaded, Part};
use oopz_sms_xml::{SmsXmlAdapter, read};

fn tally(sources: &[Loaded]) -> Manifest {
    let mut kinds = BTreeMap::new();
    let mut conversations = BTreeSet::new();
    let mut m = Manifest::default();
    for l in sources {
        conversations.extend(l.conversations.iter().map(|c| c.id.clone()));
        for msg in &l.messages {
            *kinds.entry(msg.kind()).or_default() += 1;
            m.messages += 1;
            m.from_me += msg.from_me as usize;
        }
    }
    m.conversations = conversations.len();
    m.kinds = kinds;
    m
}

#[test]
fn messages_and_calls_read_as_the_manifest_says() {
    let export = sms_xml::generate(3);
    let sms = read(&export.files[0].1).expect("sms");
    let calls = read(&export.files[1].1).expect("calls");
    assert_eq!(tally(&[sms.clone(), calls.clone()]), export.manifest);
    assert!(!sms.info.wall_clock);
    assert_eq!(sms.info.format.as_deref(), Some("SMS Backup & Restore v10.20.002"));
    assert_eq!(sms.info.owner.as_ref().and_then(|o| o.phone.as_deref()), Some("+99990000001"));

    // One conversation for Ben however his number was written.
    let ben: Vec<_> = sms.messages.iter().filter(|m| m.conversation == "+99990000002").collect();
    assert!(ben.iter().any(|m| m.text().starts_with("emoji 😊 y 👍🏽 & <b>no</b>\nsegunda")));
    let group = sms.conversations.iter().find(|c| c.kind == ConversationKind::Group).expect("a group");
    assert_eq!(group.id, "group:+99990000002,+99990000003");
    let photo = sms.messages.iter().find(|m| m.kind() == Kind::Photo).expect("a photo");
    assert_eq!(photo.text(), "la playa 🏖️");
    assert!(matches!(&photo.parts[0], Part::Media(m) if m.size == Some(30) && !m.omitted));
    let contact = sms.messages.iter().find_map(|m| match &m.parts[..] {
        [Part::Contact { name, phones, .. }] => Some((name.clone(), phones.clone())),
        _ => None,
    });
    assert_eq!(contact, Some((Some("Carla Inventado".into()), vec!["+99990000003".into()])));
    let notes = sms.info.notes.join("\n");
    assert!(notes.contains("1 records with a type"), "{notes}");
    // The registry's report, at the version the file's comment names.
    assert!(notes.contains("in sms_xml_sms 10.20.002: 1 messages"), "{notes}");

    let outcomes: Vec<CallOutcome> = calls
        .messages
        .iter()
        .filter_map(|m| match m.parts[..] {
            [Part::Call { outcome, .. }] => Some(outcome),
            _ => None,
        })
        .collect();
    use CallOutcome::*;
    assert_eq!(outcomes, [Answered, Answered, Cancelled, Missed, Missed, Declined, Declined, Unknown]);
}

#[test]
fn detected_and_opened_from_a_file() {
    let export = sms_xml::generate(4);
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join(&export.files[0].0);
    std::fs::write(&path, &export.files[0].1).expect("written");
    std::fs::write(dir.path().join("other.xml"), "<rss></rss>").expect("written");
    let adapter = SmsXmlAdapter;
    assert!(adapter.detect(&path));
    assert!(!adapter.detect(&dir.path().join("other.xml")));
    assert!(adapter.open(&path, None).is_ok());
}

#[test]
fn a_file_cut_short_keeps_what_came_before() {
    let export = sms_xml::generate(5);
    let bytes = &export.files[0].1;
    let half = read(&bytes[..bytes.len() / 2]).expect("half a file");
    assert!(!half.messages.is_empty());
    assert!(half.info.notes.iter().any(|n| n.contains("says it holds")), "{:?}", half.info.notes);
    for cut in (0..bytes.len()).step_by(41) {
        let _ = read(&bytes[..cut]);
    }
}

#[test]
fn odd_records() {
    let xml = r#"<smses count="5">
        <sms address="" date="-1000" type="1" body="before 1970" />
        <sms address="+999 9000 0005" date="4102444800000" type="2" body="future &#55357;" />
        <sms address="+99990000005" type="1" body="no date" />
        <mms date="1" msg_box="1" address="+99990000005"></mms>
        <mms date="2" msg_box="1" address="+99990000005"><parts><part ct="image/png" data="!!!" /></parts></mms>
        <unknownthing a="1" />
    </smses>"#;
    let l = read(xml.as_bytes()).expect("read");
    assert_eq!(l.messages.len(), 5);
    assert_eq!(l.messages[0].conversation, "unknown");
    assert_eq!(l.messages[0].time_ms, Some(-1000));
    assert_eq!(l.messages[1].text(), "future \u{FFFD}");
    assert_eq!(l.messages[2].time_ms, None);
    assert_eq!(l.messages[3].kind(), Kind::Unknown);
    assert!(matches!(&l.messages[4].parts[..], [Part::Media(m)] if m.omitted));
    let notes = l.info.notes.join("\n");
    for want in ["without a date", "unknownthing", "base64", "no part"] {
        assert!(notes.contains(want), "{want}: {notes}");
    }
    assert!(!notes.contains("future"), "notes never hold message text");
}

#[test]
fn other_files_are_refused() {
    for bytes in [&b""[..], b"hello", b"<rss><item/></rss>", b"{\"a\": 1}", b"\x00\xff\xfe"] {
        assert!(matches!(read(bytes), Err(Error::NotThisFormat(_))), "{bytes:?}");
    }
}

#[test]
fn nothing_panics_on_noise() {
    let mut seed = 7u64;
    for _ in 0..300 {
        let mut doc = b"<smses><sms body=\"".to_vec();
        for _ in 0..256 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            doc.push((seed >> 56) as u8);
        }
        let _ = read(&doc);
    }
    let long =
        format!("<smses><sms address=\"1\" type=\"1\" date=\"1\" body=\"{}\" /></smses>", "مرحبا Z̤͔ͧ̑̓ ".repeat(8000));
    assert!(read(long.as_bytes()).expect("read").messages[0].text().len() > 64 * 1024);
}
