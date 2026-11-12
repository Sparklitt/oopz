//! SMS Backup & Restore's XML: `sms.xml` (`<smses>` with every SMS type, MMS with pictures, a contact card and a
//! group, emoji written as surrogate-pair entities as the app writes them) and `calls.xml` (`<calls>` with every call
//! type). The manifest counts both files together.

use msgkinds::media_type::formats;
use std::fmt::Write as _;

use oopz_model::Kind;

use super::{Export, Manifest, year_start};
use crate::corpus;
use crate::rng::Rng;

const BEN: (&str, &str) = ("+99990000002", "Ben Ejemplo");
const CARLA: (&str, &str) = ("+99990000003", "Carla Inventado");
const OWNER_NUMBER: &str = "+99990000001";

/// Text as SMS Backup & Restore escapes it: markup characters by name, a newline as `&#10;`, and every character
/// beyond the Basic Multilingual Plane as two UTF-16 surrogate entities.
pub fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            c if (c as u32) > 0xFFFF => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    let _ = write!(out, "&#{u};");
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub fn generate(seed: u64) -> Export {
    let mut rng = Rng::fork(seed, 0x5135);
    let mut manifest = Manifest::default();
    let mut ms = year_start(2024) + 9 * 3_600_000;
    let mut records = Vec::new();
    let mut tick = |rng: &mut Rng| {
        ms += 60_000 + rng.below(5 * 3_600_000) as i64;
        ms
    };

    let sms = |date: i64, address: &str, name: &str, kind: u32, body: &str| {
        format!(
            "  <sms protocol=\"0\" address=\"{}\" date=\"{date}\" type=\"{kind}\" subject=\"null\" body=\"{}\" toa=\"null\" sc_toa=\"null\" service_center=\"null\" read=\"1\" status=\"-1\" locked=\"0\" date_sent=\"0\" sub_id=\"1\" readable_date=\"\" contact_name=\"{}\" />",
            escape(address),
            escape(body),
            escape(name)
        )
    };
    // Every SMS type, each with a sentence; type 2..6 are the owner's.
    for kind in 1..=6 {
        let date = tick(&mut rng);
        let body = corpus::sentence(&mut rng, true);
        records.push(sms(date, BEN.0, BEN.1, kind, &body));
        manifest.count(Kind::Text, kind != 1);
    }
    for _ in 0..12 {
        let date = tick(&mut rng);
        let (who, kind) = if rng.chance(0.5) { (CARLA, 1) } else { (BEN, 2) };
        let body = {
            let es = rng.chance(0.5);
            corpus::sentence(&mut rng, es)
        };
        records.push(sms(date, who.0, who.1, kind, &body));
        manifest.count(Kind::Text, kind == 2);
    }
    let odd: [(&str, &str, u32, &str, Kind); 7] = [
        // The same person written another way: one conversation.
        ("+999 9000 0002", "Ben Ejemplo", 1, "emoji 😊 y 👍🏽 & <b>no</b>\nsegunda línea", Kind::Text),
        (BEN.0, BEN.1, 2, "mira https://example.org/receta", Kind::Link),
        ("22777", "(Unknown)", 1, "Tu código es 123456", Kind::Text),
        ("INFO-PRUEBA", "(Unknown)", 1, "Mensaje de un remitente con nombre", Kind::Text),
        (CARLA.0, CARLA.1, 1, "مرحبا Z̤͔ͧ̑̓ä͖̭̈̇lͮ̒ͫǫ̗̻", Kind::Text),
        (CARLA.0, CARLA.1, 1, "", Kind::Text),
        (CARLA.0, CARLA.1, 9, "un tipo que no existe", Kind::Text),
    ];
    for (address, name, kind, body, k) in odd {
        let date = tick(&mut rng);
        records.push(sms(date, address, name, kind, body));
        manifest.count(k, (2..=6).contains(&kind));
    }

    let smil = "<smil><body><par><img src=\"image.jpg\"/></par></body></smil>";
    let mms = |date: i64, address: &str, name: &str, box_: u32, parts: &str, addrs: &str| {
        format!(
            "  <mms date=\"{date}\" ct_t=\"application/vnd.wap.multipart.related\" msg_box=\"{box_}\" address=\"{}\" contact_name=\"{}\" read=\"1\" sub=\"null\" m_id=\"m{date}\" text_only=\"0\">\n    <parts>\n{parts}    </parts>\n    <addrs>\n{addrs}    </addrs>\n  </mms>",
            escape(address),
            escape(name)
        )
    };
    let part = |seq: i32, ct: &str, name: &str, text: Option<&str>, data: Option<&[u8]>| {
        format!(
            "      <part seq=\"{seq}\" ct=\"{ct}\" name=\"{name}\" chset=\"106\" cd=\"null\" fn=\"null\" cid=\"&lt;{name}&gt;\" cl=\"{name}\" ctt_s=\"null\" ctt_t=\"null\" text=\"{}\"{} />\n",
            text.map(escape).unwrap_or_else(|| "null".into()),
            data.map(|d| format!(" data=\"{}\"", b64(d))).unwrap_or_default()
        )
    };
    let addr =
        |address: &str, kind: u32| format!("      <addr address=\"{address}\" type=\"{kind}\" charset=\"106\" />\n");

    // A picture with a caption, received.
    let date = tick(&mut rng);
    let parts = part(-1, msgkinds::media_type::formats::SMIL, "smil.xml", Some(smil), None)
        + &part(0, formats::JPEG, "image.jpg", None, Some(b"oopz-gen placeholder image.jpg"))
        + &part(1, formats::PLAIN_TEXT, "text_0.txt", Some("la playa 🏖️"), None);
    records.push(mms(date, BEN.0, BEN.1, 1, &parts, &(addr(BEN.0, 137) + &addr(OWNER_NUMBER, 151))));
    manifest.count(Kind::Photo, false);
    // A contact card, sent.
    let date = tick(&mut rng);
    let vcard = b"BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Carla Inventado\r\nTEL;TYPE=CELL:+99990000003\r\nEND:VCARD\r\n";
    let parts = part(0, "text/x-vCard", "Carla.vcf", None, Some(vcard));
    records.push(mms(date, BEN.0, BEN.1, 2, &parts, &(addr("insert-address-token", 137) + &addr(BEN.0, 151))));
    manifest.count(Kind::Contact, true);
    // A voice recording, received.
    let date = tick(&mut rng);
    let parts = part(0, formats::AMR, "audio.amr", None, Some(b"#!AMR\n oopz-gen"));
    records.push(mms(date, CARLA.0, CARLA.1, 1, &parts, &(addr(CARLA.0, 137) + &addr(OWNER_NUMBER, 151))));
    manifest.count(Kind::Voice, false);
    // A group: received from Ben, then sent to both.
    let group = format!("{}~{}", BEN.0, CARLA.0);
    let date = tick(&mut rng);
    let parts = part(0, formats::PLAIN_TEXT, "text_0.txt", Some("¿quedamos el jueves?"), None);
    let addrs = addr(BEN.0, 137) + &addr(OWNER_NUMBER, 151) + &addr(CARLA.0, 151);
    records.push(mms(date, &group, "Ben Ejemplo, Carla Inventado", 1, &parts, &addrs));
    manifest.count(Kind::Text, false);
    let date = tick(&mut rng);
    let parts = part(0, formats::PLAIN_TEXT, "text_0.txt", Some("¡vale! 😊"), None);
    let addrs = addr(OWNER_NUMBER, 137) + &addr(BEN.0, 151) + &addr(CARLA.0, 151);
    records.push(mms(date, &group, "Ben Ejemplo, Carla Inventado", 2, &parts, &addrs));
    manifest.count(Kind::Text, true);

    let mut xml = String::from(
        "<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>\n<!--File Created By SMS Backup & Restore v10.20.002 on 10/11/2026 10:00:00-->\n",
    );
    let _ = writeln!(
        xml,
        "<smses count=\"{}\" backup_set=\"oopz-gen\" backup_date=\"1794304800000\" type=\"full\">",
        records.len()
    );
    for r in &records {
        xml.push_str(r);
        xml.push('\n');
    }
    xml.push_str("</smses>\n");

    let mut calls = Vec::new();
    for (number, name, kind, duration) in [
        (BEN.0, BEN.1, 1, 125),
        (BEN.0, BEN.1, 2, 30),
        (BEN.0, BEN.1, 2, 0),
        (CARLA.0, CARLA.1, 3, 0),
        (CARLA.0, CARLA.1, 4, 12),
        ("+99990000004", "Dani Ficticio", 5, 0),
        ("+99990000004", "Dani Ficticio", 6, 0),
        ("+99990000004", "Dani Ficticio", 10, 0),
    ] {
        let date = tick(&mut rng);
        calls.push(format!(
            "  <call number=\"{number}\" duration=\"{duration}\" date=\"{date}\" type=\"{kind}\" presentation=\"1\" subscription_id=\"1\" post_dial_digits=\"\" subscription_component_name=\"\" readable_date=\"\" contact_name=\"{}\" />",
            escape(name)
        ));
        manifest.count(Kind::Call, kind == 2);
    }
    let mut calls_xml = String::from("<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>\n");
    let _ = writeln!(
        calls_xml,
        "<calls count=\"{}\" backup_set=\"oopz-gen\" backup_date=\"1794304800000\" type=\"full\">",
        calls.len()
    );
    for c in &calls {
        calls_xml.push_str(c);
        calls_xml.push('\n');
    }
    calls_xml.push_str("</calls>\n");

    // Ben, Carla, the short code, the named sender, the group, and Dani (calls only).
    manifest.conversations = 6;
    Export {
        files: vec![
            ("sms-20261110.xml".into(), xml.into_bytes()),
            ("calls-20261110.xml".into(), calls_xml.into_bytes()),
        ],
        manifest,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn emoji_are_written_as_surrogate_pairs() {
        assert_eq!(super::escape("😊&\n"), "&#55357;&#56842;&amp;&#10;");
    }
}
