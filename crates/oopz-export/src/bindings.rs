//! oopz's own types for its desktop app, as TypeScript: the codes its Rust enums store (apps, kinds, media and card
//! kinds, events, export formats), written to `apps/desktop/src/lib/generated/oopz.ts`. The app's `types.ts` takes
//! them from there, and a test fails when the file is stale (`cargo test -p oopz-export bindings`; rewrite it with
//! `OOPZ_WRITE_BINDINGS=1`).

use oopz_model::{App, CardKind, Event, Kind, MediaKind};

use crate::Format;

fn union(name: &str, doc: &str, codes: impl IntoIterator<Item = String>) -> String {
    let codes: Vec<String> = codes.into_iter().map(|c| format!("{c:?}")).collect();
    format!("/** {doc} */\nexport type {name} = {};\n\n", codes.join(" | "))
}

/// The whole file.
pub fn typescript() -> String {
    let pdf = msgkinds::media_type::by_mime(msgkinds::media_type::formats::PDF).and_then(|f| f.extension());
    let mut t = String::from(
        "// Generated from oopz's Rust enums (crates/oopz-export/src/bindings.rs): change them, not this file.\n\n",
    );
    t += &union("AppCode", "The apps oopz reads (oopz-model's App).", App::ALL.map(|a| a.code().to_owned()));
    t += &union(
        "Kind",
        "What a message is: msgkinds' kinds (oopz-model's Kind).",
        Kind::ALL.map(|k| k.code().to_owned()),
    );
    let media = Kind::ALL.into_iter().filter(|k| MediaKind::from_kind(*k).is_some());
    t += &union(
        "MediaKind",
        "The kinds that come with a file (oopz-model's MediaKind).",
        media.map(|k| k.code().to_owned()),
    );
    let cards = CardKind::ALL.map(|c| Kind::from(c).code().to_owned());
    t += &union("CardKind", "The kinds a card part holds (oopz-model's CardKind).", cards);
    t += &union(
        "EventCode",
        "What an app's notice says happened (oopz-model's Event).",
        Event::ALL.map(|e| e.code().to_owned()),
    );
    let formats = Format::ALL.iter().map(|f| f.code().to_owned()).chain(pdf.map(str::to_owned));
    t += &union("ExportFormat", "A library export's format (oopz-export's Format, and the PDF).", formats);
    t
}

#[cfg(test)]
mod tests {
    /// `apps/desktop/src/lib/generated/oopz.ts` is current; `OOPZ_WRITE_BINDINGS=1` rewrites it.
    #[test]
    fn the_desktop_apps_oopz_file_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/desktop/src/lib/generated/oopz.ts");
        if std::env::var_os("OOPZ_WRITE_BINDINGS").is_some() {
            std::fs::write(path, super::typescript()).unwrap();
        }
        assert!(std::fs::read_to_string(path).unwrap_or_default() == super::typescript(), "{path} is stale");
    }
}
