//! oopz's Rust spells none of msgkinds' values (MIME types, extensions, magic bytes, registry codes, wording,
//! messages): it names msgkinds' constants or looks them up. `msgkinds::scan::spelled_in_reader` reads every crate and the desktop app's Rust.
//!
//! `OWN` lists oopz's own words that only happen to equal one of msgkinds' (a field of oopz's stored JSON named
//! `text`): each with the file and why it's oopz's, not a copy.

use std::path::Path;

/// (file, literal, why it's oopz's own).
const OWN: &[(&str, &str, &str)] = &[
    ("crates/oopz-model/src/lib.rs", "\"text\"", "a field of oopz's stored part JSON"),
    ("crates/oopz-model/src/lib.rs", "\"topic_created\"", "oopz's stored event name (msgkinds says topic_changed)"),
    ("crates/oopz-export/src/lib.rs", "\"vcard\"", "the CLI's name for the contacts export"),
    ("crates/oopz-export/src/tests.rs", "\"text\"", "the type of oopz's stored text part"),
    ("crates/adapters/telegram-export/src/lib.rs", "\"text\"", "Telegram's JSON field for a message's text"),
    ("crates/adapters/telegram-export/tests/export.rs", "\"text\"", "Telegram's JSON field for a message's text"),
    ("crates/adapters/sms-xml/src/lib.rs", "\"text\"", "SMS Backup & Restore's XML attribute for a part's text"),
    ("crates/oopz-gen/src/spec.rs", "\"image\"", "the generator's command-line name for a photo"),
    ("crates/oopz-gen/src/exports/telegram_json.rs", "\"text\"", "Telegram's JSON field for a message's text"),
    ("crates/adapters/whatsapp-android/src/lib.rs", "\"video_call\"", "a column of WhatsApp's msgstore.db"),
    ("crates/adapters/whatsapp-android/src/schema.rs", "\"video_call\"", "a column of WhatsApp's msgstore.db"),
    ("crates/adapters/whatsapp-android/src/schema.rs", "\"place_name\"", "a column of WhatsApp's msgstore.db"),
    ("crates/adapters/whatsapp-android/src/schema.rs", "\"vcard\"", "a column of WhatsApp's msgstore.db"),
    ("crates/oopz-app/src/index.rs", "\"video_call\"", "a column of WhatsApp's msgstore.db"),
    ("crates/oopz-app/src/receive.rs", "\"http://\"", "the scheme of oopz's own local page, in its tests"),
    ("crates/oopz-cli/src/library.rs", "\"pdf\"", "the name of oopz-cli's Cargo feature"),
    ("apps/desktop/src-tauri/src/lib.rs", "\"http://\"", "the scheme of oopz's own local page, in its tests"),
    ("apps/desktop/src-tauri/src/library.rs", "\"pdf\"", "the name of the desktop app's Cargo feature"),
];

#[test]
fn no_value_of_msgkinds_is_spelled_in_oopz() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let all = msgkinds::scan::spelled_in_reader(&root, &["crates", "apps/desktop/src-tauri/src"], &|_| false);
    let own = |line: &str, (file, lit, _): &(&str, &str, &str)| line.starts_with(&format!("{file}: {lit} is "));
    // Each entry of OWN still names a literal there: the list says only what's true.
    for entry in OWN {
        assert!(all.iter().any(|line| own(line, entry)), "OWN lists {} {}, no longer there", entry.0, entry.1);
    }
    let found: Vec<&String> = all.iter().filter(|line| !OWN.iter().any(|e| own(line, e))).collect();
    let shown: Vec<&str> = found.iter().map(|s| s.as_str()).collect();
    assert!(found.is_empty(), "{} values of msgkinds spelled in oopz:\n{}", found.len(), shown.join("\n"));
}
