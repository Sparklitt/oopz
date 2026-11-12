//! vCards (RFC 6350, and the 2.1 and 3.0 files phones still write): read for contacts, written for exports.
//! Only names, numbers, emails and a note are kept; photos and the rest are skipped.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub name: String,
    pub phones: Vec<String>,
    pub emails: Vec<String>,
    pub note: Option<String>,
}

/// Lines with their folding undone: a line starting with a space or tab continues the one before (3.0/4.0), and a
/// quoted-printable value ending in `=` continues on the next line (2.1).
/// A vCard's first line: how msgkinds knows one by its first bytes (its text start for `text/vcard`).
fn begin() -> String {
    use msgkinds::media_type::{formats::VCARD, signatures::TEXT_STARTS};
    TEXT_STARTS
        .iter()
        .find(|(_, mime)| *mime == VCARD)
        .map(|(b, _)| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default()
}

fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut soft = false;
    for raw in text.trim_start_matches('\u{feff}').split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        match lines.last_mut() {
            Some(last) if soft => {
                last.pop();
                last.push_str(line);
            }
            Some(last) if line.starts_with([' ', '\t']) => last.push_str(&line[1..]),
            _ => lines.push(line.to_owned()),
        }
        let last = lines.last().map(String::as_str).unwrap_or_default();
        soft = last.to_ascii_uppercase().contains("QUOTED-PRINTABLE") && last.ends_with('=');
    }
    lines
}

fn quoted_printable(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'='
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `\,` `\;` `\n` `\\` as the characters they stand for.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;").replace('\n', "\\n").replace('\r', "")
}

/// Every card in `text`; a card with no name gets its first number or email as its name, and one with neither is
/// skipped.
pub fn parse(text: &str) -> Vec<Card> {
    let mut cards = Vec::new();
    let mut card: Option<(Card, Option<String>)> = None;
    for line in unfold(text) {
        let Some((head, value)) = line.split_once(':') else { continue };
        let mut params = head.split(';');
        // `item1.TEL` (Apple's grouping) names TEL.
        let name = params.next().unwrap_or_default().rsplit('.').next().unwrap_or_default().to_ascii_uppercase();
        let params: Vec<String> = params.map(str::to_ascii_uppercase).collect();
        let value = if params.iter().any(|p| p.contains("QUOTED-PRINTABLE")) {
            quoted_printable(value)
        } else {
            value.to_owned()
        };
        match name.as_str() {
            "BEGIN" if value.eq_ignore_ascii_case("VCARD") => card = Some((Card::default(), None)),
            "END" if value.eq_ignore_ascii_case("VCARD") => {
                if let Some((mut c, n)) = card.take() {
                    if c.name.trim().is_empty() {
                        c.name = n
                            .or_else(|| c.phones.first().cloned())
                            .or_else(|| c.emails.first().cloned())
                            .unwrap_or_default();
                    }
                    if !c.name.trim().is_empty() && (!c.phones.is_empty() || !c.emails.is_empty() || !c.name.is_empty())
                    {
                        cards.push(c);
                    }
                }
            }
            "FN" => {
                if let Some((c, _)) = card.as_mut() {
                    c.name = unescape(value.trim());
                }
            }
            "N" => {
                if let Some((_, n)) = card.as_mut() {
                    // Family;Given;Middle;Prefix;Suffix → "Given Middle Family".
                    let parts: Vec<String> = value.split(';').map(unescape).collect();
                    let order = [3, 1, 2, 0, 4];
                    let joined = order
                        .iter()
                        .filter_map(|&i| parts.get(i))
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !joined.is_empty() {
                        *n = Some(joined);
                    }
                }
            }
            "TEL" => {
                if let Some((c, _)) = card.as_mut() {
                    let v = unescape(value.trim().trim_start_matches("tel:"));
                    if !v.is_empty() && !c.phones.contains(&v) {
                        c.phones.push(v);
                    }
                }
            }
            "EMAIL" => {
                if let Some((c, _)) = card.as_mut() {
                    let v = unescape(value.trim());
                    if v.contains('@') && !c.emails.iter().any(|e| e.eq_ignore_ascii_case(&v)) {
                        c.emails.push(v);
                    }
                }
            }
            "NOTE" => {
                if let Some((c, _)) = card.as_mut() {
                    c.note = Some(unescape(value.trim()));
                }
            }
            _ => {}
        }
    }
    cards
}

/// A vCard 3.0 file (the version every phone and mail app reads), CRLF line ends, long lines folded at 75 bytes.
pub fn write(cards: &[Card]) -> String {
    let mut out = String::new();
    for c in cards {
        let mut lines = vec![begin(), "VERSION:3.0".to_owned(), format!("FN:{}", escape(&c.name))];
        lines.push(format!("N:;{};;;", escape(&c.name)));
        lines.extend(c.phones.iter().map(|p| format!("TEL;TYPE=CELL:{}", escape(p))));
        lines.extend(c.emails.iter().map(|e| format!("EMAIL;TYPE=INTERNET:{}", escape(e))));
        if let Some(n) = &c.note {
            lines.push(format!("NOTE:{}", escape(n)));
        }
        lines.push("END:VCARD".to_owned());
        for line in lines {
            fold(&line, &mut out);
        }
    }
    out
}

fn fold(line: &str, out: &mut String) {
    let mut width = 0;
    for ch in line.chars() {
        let n = ch.len_utf8();
        if width + n > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(ch);
        width += n;
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_phones_write() {
        let text = "\u{feff}BEGIN:VCARD\r\nVERSION:2.1\r\nN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Gonz=C3=A1lez;Mar=\r\n=C3=ADa;;;\r\n\
            TEL;CELL:+34 999 00 11 22\r\nTEL;HOME:+34 999 00 11 22\r\nEND:VCARD\r\n\
            BEGIN:VCARD\nVERSION:3.0\nFN:Ana\\, la de\n  Madrid\nitem1.TEL;type=CELL:9999001\nitem1.X-ABLabel:móvil\n\
            EMAIL;TYPE=INTERNET:ana@example.org\nEMAIL:ANA@example.org\nNOTE:line\\nnext\nEND:VCARD\n\
            BEGIN:VCARD\nVERSION:4.0\nTEL;VALUE=uri:tel:+999-555-0100\nEND:VCARD\n\
            BEGIN:VCARD\nVERSION:3.0\nEND:VCARD\nnot a card at all\n";
        let cards = parse(text);
        assert_eq!(cards.len(), 3, "{cards:?}");
        assert_eq!(cards[0].name, "María González");
        assert_eq!(cards[0].phones, ["+34 999 00 11 22"]);
        assert_eq!(cards[1].name, "Ana, la de Madrid");
        assert_eq!(cards[1].phones, ["9999001"]);
        assert_eq!(cards[1].emails, ["ana@example.org"]);
        assert_eq!(cards[1].note.as_deref(), Some("line\nnext"));
        assert_eq!(cards[2].name, "+999-555-0100");
    }

    #[test]
    fn writes_what_it_reads_back() {
        let cards = vec![Card {
            name: "Zoë; \"the\" one, ".repeat(6).trim().to_owned(),
            phones: vec!["+999 555 0100".into()],
            emails: vec!["z@example.org".into()],
            note: Some("in WhatsApp, Telegram".into()),
        }];
        let text = write(&cards);
        assert!(text.lines().all(|l| l.len() <= 76), "folded");
        assert!(text.ends_with("END:VCARD\r\n"));
        assert_eq!(parse(&text), cards);
    }
}
