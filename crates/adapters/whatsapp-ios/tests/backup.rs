//! The adapter on oopz-gen's invented iPhone backups, plain and encrypted.

use std::collections::HashMap;

use oopz_model::{Adapter, ConversationKind, MediaKind, Message, Part, Secret};
use oopz_whatsapp_ios::{BUSINESS, WHATSAPP};

const PASSWORD: &str = "invented backup pässword";

fn read_all(source: &mut dyn oopz_model::Source) -> Vec<Message> {
    let ids: Vec<String> = source.conversations().iter().map(|c| c.id.clone()).collect();
    let mut all = Vec::new();
    for id in ids {
        source
            .messages(&id, &mut |m| {
                all.push(m);
                true
            })
            .unwrap();
    }
    all
}

fn check(dir: &std::path::Path, password: Option<&str>, manifest: &oopz_gen::ios::Manifest) {
    let mut s = WHATSAPP.open(dir, password).unwrap();
    assert_eq!(s.conversations().len(), manifest.conversations);
    let all = read_all(s.as_mut());
    assert_eq!(all.len(), manifest.messages);
    assert_eq!(all.iter().filter(|m| m.from_me).count(), manifest.from_me);
    let by_kind = |k: MediaKind| {
        all.iter()
            .flat_map(|m| &m.parts)
            .filter_map(|p| match p {
                Part::Media(m) if m.kind == k => Some(m.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let photo = &by_kind(MediaKind::Photo)[0];
    assert!(photo.data.as_ref().unwrap().starts_with(b"\x89PNG"), "the photo's bytes come out of the backup");
    assert_eq!(photo.caption.as_deref(), Some("a caption"));
    assert!(by_kind(MediaKind::Video)[0].omitted, "the video isn't in the backup");
    assert_eq!(by_kind(MediaKind::Voice).len(), 1);
    assert_eq!(by_kind(MediaKind::Document)[0].name.as_deref(), Some("Invented report.pdf"));
    assert_eq!(by_kind(MediaKind::Gif).len() + by_kind(MediaKind::Sticker).len(), 2);
    assert!(all.iter().any(|m| m.deleted));
    assert!(all.iter().any(|m| m.starred));
    assert!(all.iter().flat_map(|m| &m.parts).any(|p| matches!(p, Part::Location { lat: Some(_), .. })));
    assert!(
        all.iter()
            .flat_map(|m| &m.parts)
            .any(|p| matches!(p, Part::Contact { phones, .. } if phones == &["+99990000004"]))
    );
    assert!(all.iter().flat_map(|m| &m.parts).any(|p| matches!(p, Part::Link { .. })));
    assert_eq!(
        all.iter().flat_map(|m| &m.parts).filter(|p| matches!(p, Part::Unknown { .. })).count(),
        manifest.unknown_types
    );
    // Apple's dates: 2001 + seconds; a negative one is before 2001.
    let old =
        all.iter().find(|m| m.parts.iter().any(|p| matches!(p, Part::Text { text } if text == "from 2000"))).unwrap();
    assert_eq!(old.time_ms, Some((978_307_200 - 86_400) * 1000));
    // Group senders by member, the one who left included; names from the address book, else the push name.
    let group = s.conversations().iter().find(|c| c.kind == ConversationKind::Group).unwrap().clone();
    assert_eq!(group.name.as_deref(), Some("Familia 🌙"));
    let senders: Vec<_> =
        all.iter().filter(|m| m.conversation == group.id && !m.from_me).filter_map(|m| m.sender.clone()).collect();
    assert!(senders.contains(&"99990000009@s.whatsapp.net".to_string()));
    let names: HashMap<_, _> = s.people().iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    assert_eq!(names["99990000001@s.whatsapp.net"].as_deref(), Some("Ana Agenda"));
    assert_eq!(names["99990000002@s.whatsapp.net"].as_deref(), Some("Ben ~ push"));
    assert!(s.people().iter().all(|p| p.phone.as_deref().is_none_or(|ph| ph.starts_with("+9999"))));
    assert!(
        s.conversations().iter().any(|c| c.kind == ConversationKind::Broadcast && c.name.as_deref() == Some("Status"))
    );
    let notes = s.info().notes.join("\n");
    assert!(notes.contains("1 media files aren't in the backup") && notes.contains("message type 999"), "{notes}");
}

#[test]
fn a_plain_backup_opens_without_a_password() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("backup");
    let m = oopz_gen::ios::generate(&out, None, false).unwrap();
    assert!(WHATSAPP.detect(&out) && WHATSAPP.detect(&out.join("Manifest.plist")));
    assert_eq!(WHATSAPP.secret_for(&out), Secret::None);
    check(&out, None, &m);
}

#[test]
fn an_encrypted_backup_opens_only_with_its_password() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("backup");
    let m = oopz_gen::ios::generate(&out, Some(PASSWORD), true).unwrap();
    assert_eq!(WHATSAPP.secret_for(&out), Secret::Password);
    assert!(matches!(WHATSAPP.open(&out, None), Err(oopz_model::Error::NeedsSecret)));
    assert!(matches!(WHATSAPP.open(&out, Some("wrong")), Err(oopz_model::Error::WrongSecret)));
    check(&out, Some(PASSWORD), &m);
    let s = WHATSAPP.open(&out, Some(PASSWORD)).unwrap();
    assert!(s.info().notes.iter().any(|n| n.contains("WhatsApp Business")));
    let b = BUSINESS.open(&out, Some(PASSWORD)).unwrap();
    assert_eq!(b.info().adapter, "whatsapp-business-ios");
}

#[test]
fn not_a_backup_or_no_whatsapp_inside() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!WHATSAPP.detect(dir.path()));
    assert!(matches!(WHATSAPP.open(dir.path(), None), Err(oopz_model::Error::NotThisFormat(_))));
    let out = dir.path().join("b");
    oopz_gen::ios::generate(&out, None, false).unwrap();
    let e = BUSINESS.open(&out, None).err().unwrap();
    assert!(e.to_string().contains("no WhatsApp Business"), "{e}");
}
