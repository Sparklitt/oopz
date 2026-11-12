//! The adapter against oopz-gen's synthetic `result.json` and odd input.

use std::collections::BTreeMap;

use oopz_gen::exports::{Manifest, telegram_json};
use oopz_model::{Adapter, CallOutcome, ConversationKind, Error, Event, Loaded, Message, Part};
use oopz_telegram_export::{TelegramExportAdapter, read};

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
fn the_full_export_reads_as_its_manifest_says() {
    let export = telegram_json::generate(3);
    let loaded = read(&export.files[0].1).expect("read");
    assert_eq!(tally(&loaded), export.manifest);
    assert!(!loaded.info.wall_clock);
    let owner = loaded.info.owner.as_ref().expect("an owner");
    assert_eq!(owner.id, format!("user{}", telegram_json::OWNER_ID));
    assert_eq!(owner.phone.as_deref(), Some("+99990000001"));
    let kinds: Vec<ConversationKind> = loaded.conversations.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [
            ConversationKind::Direct,
            ConversationKind::Group,
            ConversationKind::Notes,
            ConversationKind::Channel,
            ConversationKind::Channel
        ]
    );
    assert!(loaded.people.iter().any(|p| p.id == "phone:+99990000003"));
    let m = &loaded.messages;
    assert!(m.iter().any(|m| m.reply_to.as_deref() == Some("999999")));
    assert!(m.iter().any(|m| m.forwarded));
    assert!(m.iter().any(|m| m.edited && m.edited_ms == Some(1_705_744_800_000)));
    let reacted = m.iter().find(|m| !m.reactions.is_empty()).expect("reactions");
    assert_eq!(reacted.reactions.len(), 3);
    assert_eq!(reacted.reactions.iter().filter(|r| r.from_me).count(), 1);
    assert!(m.iter().any(|m| matches!(&m.parts[..], [Part::Media(x)] if x.omitted && x.file.is_none())));
    assert!(
        m.iter().any(|m| matches!(&m.parts[..], [Part::Call { outcome: CallOutcome::Answered, secs: Some(95), .. }]))
    );
    // A notice msgkinds names but oopz has no event for: its words.
    assert!(
        m.iter().any(|m| matches!(&m.parts[..], [Part::System { text: Some(t), .. }] if t.starts_with("boost apply")))
    );
    // The rest of core.telegram.org/import-export's schema.
    let text = |m: &Message| m.parts.iter().map(|p| format!("{p:?}")).collect::<String>();
    let has = |needle: &str| m.iter().any(|x| text(x).contains(needle));
    assert!(has("solo entidades"));
    assert!(has("Entrada · Concierto · 1.45 USD"));
    assert!(m.iter().any(|x| matches!(&x.parts[..], [Part::Text { text }, Part::Link { url, title: Some(t) }]
        if text == "un juego" && url == "https://example.org/juego" && t == "Tetrominó")));
    assert!(has("giveaway: 3 × 6 months, until 2024-03-01T00:00:00"));
    assert!(has("Partidos 2024") && has("⚽") && has("score in game (score 120)") && has("send payment (500 JPY)"));
    assert!(has("group call scheduled (2024-01-23 08:53 UTC)"));
    assert!(m.iter().any(|x| matches!(&x.parts[..], [Part::Call { secs: Some(600), .. }])));
    assert!(m.iter().any(|x| matches!(&x.parts[..], [Part::Contact { .. }, Part::Media(v)] if v.file.as_deref() == Some("contacts/contact_1.vcard"))));
    assert!(
        m.iter().any(
            |x| matches!(&x.parts[..], [Part::Media(a)] if a.name.as_deref() == Some("Nadie – Canción de prueba"))
        )
    );
    let owner_vote = format!("user{}", telegram_json::OWNER_ID);
    assert!(m.iter().any(|x| matches!(&x.parts[..], [Part::Poll { options, .. }] if options[0].voters == [owner_vote.clone()] && options[1].voters.is_empty())));
    let pinned = m.iter().find(|x| matches!(&x.parts[..], [Part::System { event: Event::Pinned, .. }])).expect("a pin");
    assert_eq!(pinned.reply_to.as_deref(), Some("3"));
    let notes = loaded.info.notes.join("\n");
    assert!(notes.contains("`message type something_new`"), "{notes}");
    // msgkinds' report: the export names no version, so it says so.
    assert!(
        notes.contains(
            "unknown type something_newer in telegram_json_action (version not recorded): 1 message, kept with its type"
        ),
        "{notes}"
    );
    assert!(notes.contains(msgkinds::HOW_TO_ADD), "{notes}");
}

#[test]
fn a_zip_and_a_folder_open() {
    let export = telegram_json::generate(4);
    let dir = tempfile::tempdir().expect("a temp dir");
    let inner = dir.path().join("DataExport_2026-11-10");
    std::fs::create_dir(&inner).expect("dir");
    std::fs::write(inner.join("result.json"), &export.files[0].1).expect("written");
    let zip = oopz_gen::exports::zip(&[
        ("../result.json".into(), b"{}".to_vec()),
        ("DataExport_2026-11-10/result.json".into(), export.files[0].1.clone()),
    ]);
    std::fs::write(dir.path().join("export.zip"), zip).expect("written");
    let adapter = TelegramExportAdapter;
    for path in [dir.path().to_path_buf(), inner.join("result.json"), dir.path().join("export.zip")] {
        assert!(adapter.detect(&path), "{}", path.display());
        let source = adapter.open(&path, None).expect("opened");
        assert_eq!(source.conversations().len(), export.manifest.conversations);
    }
}

#[test]
fn a_single_chat_export_with_wall_clock_times_only() {
    let json = r#"{"name": "Ben", "type": "personal_chat", "id": 42, "messages": [
        {"id": 1, "type": "message", "date": "1969-12-31T23:59:59", "from": "Ben", "from_id": "user42", "text": "old"},
        {"id": 1, "type": "message", "date": "2099-01-01T00:00:00", "from": "Ana", "from_id": "user7", "text": "same id"},
        {"id": 3, "type": "message", "from": "Zed", "from_id": "user99", "text": "no date", "reply_to_message_id": 77},
        "not an object"
    ]}"#;
    let loaded = read(json.as_bytes()).expect("read");
    assert!(loaded.info.wall_clock);
    assert_eq!(loaded.messages[0].time_ms, Some(-1000));
    assert_eq!(loaded.messages[1].id, "1#2");
    assert!(loaded.messages[1].from_me, "the one who isn't the chat's id");
    assert_eq!(loaded.messages[2].time_ms, None);
    let notes = loaded.info.notes.join("\n");
    assert!(notes.contains("inferred") && notes.contains("repeated") && notes.contains("wasn't an object"), "{notes}");
}

#[test]
fn other_shapes_are_refused_by_name() {
    assert!(matches!(read(b""), Err(Error::NotThisFormat(_))));
    assert!(matches!(read(b"[1, 2]"), Err(Error::NotThisFormat(_))));
    assert!(matches!(read(b"{\"hello\": 1}"), Err(Error::NotThisFormat(_))));
    assert!(matches!(read(b"{\"chats\": {\"list\": 5}}"), Err(Error::UnsupportedVersion(_))));
    assert!(matches!(read(b"{\"messages\": {}}"), Err(Error::UnsupportedVersion(_))));
}

#[test]
fn nothing_panics() {
    let export = telegram_json::generate(9);
    let bytes = &export.files[0].1;
    for cut in (0..bytes.len()).step_by(53) {
        let _ = read(&bytes[..cut]);
    }
    // Every field with the wrong type.
    let text = String::from_utf8_lossy(bytes)
        .replace("\"text\": \"", "\"text\": 5, \"x\": \"")
        .replace("\"date_unixtime\": \"", "\"date_unixtime\": \"x");
    let _ = read(text.as_bytes());
    let deep = "[".repeat(100_000);
    assert!(read(deep.as_bytes()).is_err());
    let long = format!(r#"{{"messages": [{{"id": 1, "type": "message", "text": "{}"}}]}}"#, "مرحبا Z̤͔ͧ̑̓".repeat(8000));
    let loaded = read(long.as_bytes()).expect("a long text");
    assert!(loaded.messages[0].text().len() > 64 * 1024);
    let huge = r#"{"messages": [{"id": 1, "type": "message", "text": "", "reactions": [{"type": "emoji", "emoji": "x", "count": 99999999999}]}]}"#;
    assert_eq!(read(huge.as_bytes()).expect("read").messages[0].reactions.len(), 1000);
}

/// What happens when Telegram Desktop changes its export (msgkinds' registry.md, "What readers test on their side",
/// 4): keys it adds are ignored, optional keys it drops leave those details out (said in the notes), and a missing
/// or changed required part is refused with what's wrong; never a panic or a lost message.
mod format_changes {
    use super::*;
    use serde_json::{Value, json};

    fn changed(f: impl Fn(&mut Value)) -> (Vec<u8>, Manifest) {
        let export = telegram_json::generate(3);
        let mut v: Value = serde_json::from_slice(&export.files[0].1).unwrap();
        f(&mut v);
        (serde_json::to_vec(&v).unwrap(), export.manifest)
    }

    fn each_message(v: &mut Value, f: &dyn Fn(&mut serde_json::Map<String, Value>)) {
        for chat in v["chats"]["list"].as_array_mut().unwrap() {
            for m in chat["messages"].as_array_mut().unwrap() {
                f(m.as_object_mut().unwrap());
            }
        }
    }

    #[test]
    fn keys_added_anywhere_are_ignored() {
        let (bytes, manifest) = changed(|v| {
            v["export_format_version"] = json!(99);
            for chat in v["chats"]["list"].as_array_mut().unwrap() {
                chat["folder_id"] = json!({"nested": [1, 2]});
            }
            each_message(v, &|m| {
                m.insert("ai_summary".into(), json!({"text": "x"}));
                m.insert("paid_stars".into(), json!(5));
            });
        });
        let loaded = read(&bytes).expect("reads");
        assert_eq!(tally(&loaded), manifest);
    }

    #[test]
    fn optional_keys_dropped_leave_details_out() {
        let (bytes, manifest) = changed(|v| {
            v.as_object_mut().unwrap().remove("personal_information");
            v.as_object_mut().unwrap().remove("contacts");
            each_message(v, &|m| {
                m.remove("date_unixtime");
                m.remove("text_entities");
                m.remove("from_id");
                m.remove("edited");
            });
        });
        let loaded = read(&bytes).expect("reads");
        assert_eq!(loaded.messages.len(), manifest.messages);
        assert_eq!(loaded.conversations.len(), manifest.conversations);
        let notes = loaded.info.notes.join("\n");
        assert!(loaded.info.wall_clock && notes.contains("no zone"), "{notes}");
        assert!(loaded.info.owner.is_none() && notes.contains("pick yourself"), "{notes}");
    }

    #[test]
    fn a_required_part_missing_or_changed_is_refused_with_what() {
        let (no_chats, _) = changed(|v| {
            v.as_object_mut().unwrap().remove("chats");
        });
        assert!(matches!(read(&no_chats), Err(Error::NotThisFormat(_))));
        let (not_a_list, _) = changed(|v| v["chats"]["list"] = json!({"0": {}}));
        let Err(Error::UnsupportedVersion(why)) = read(&not_a_list) else { panic!("refused") };
        assert!(why.contains("chats aren't a list"), "{why}");
        // A chat whose messages changed shape keeps the chat, and says so.
        let (odd_messages, _) = changed(|v| v["chats"]["list"][0]["messages"] = json!("later"));
        let loaded = read(&odd_messages).expect("reads the rest");
        assert!(!loaded.conversations.is_empty());
    }
}
