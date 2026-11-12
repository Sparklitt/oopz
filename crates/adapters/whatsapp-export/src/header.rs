//! A message's first line: `31/12/2023, 21:41 - ` (Android), `12/31/23, 9:41 PM - ` (US), `[31.12.23, 21:41:05] `
//! (iPhone), with any of `/ . -` between the date's numbers, a 2- or 4-digit year, and a 12-hour clock written `PM`,
//! `p. m.`, `a.m.` after a space, a no-break space or a narrow no-break space.

/// Direction and byte-order marks WhatsApp puts around names, notices and attachments (msgkinds' wording `[shape]`).
pub use msgkinds::media_type::sources::whatsapp_export::is_mark;

#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// The date's three numbers as written.
    pub a: u32,
    pub b: u32,
    pub c: u32,
    /// `2023-12-31`: year, month, day.
    pub year_first: bool,
    pub two_digit_year: bool,
    /// On a 24-hour clock; 99 when a 12-hour time doesn't exist (`13:00 PM`), so no time is made from it.
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

struct Cur<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Cur<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i..)?.chars().next()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn space(&mut self) {
        while let Some(c) = self.peek().filter(|c| matches!(c, ' ' | '\u{A0}' | '\u{202F}' | '\u{2009}')) {
            self.i += c.len_utf8();
        }
    }

    /// 1 to `max` ASCII digits: the number and how many digits.
    fn number(&mut self, max: usize) -> Option<(u32, usize)> {
        let rest = self.s.get(self.i..)?;
        let len = rest.bytes().take_while(u8::is_ascii_digit).count();
        if len == 0 || len > max {
            return None;
        }
        let n = rest[..len].parse().ok()?;
        self.i += len;
        Some((n, len))
    }

    /// `AM`, `pm`, `a. m.`, `p.m.`: true for the afternoon.
    fn am_pm(&mut self) -> Option<bool> {
        let start = self.i;
        let pm = match self.peek()?.to_ascii_lowercase() {
            'a' => false,
            'p' => true,
            _ => return None,
        };
        self.i += 1;
        while self.eat('.') || self.eat(' ') || self.eat('\u{A0}') || self.eat('\u{202F}') {}
        if matches!(self.peek(), Some('m' | 'M')) {
            self.i += 1;
            self.eat('.');
            Some(pm)
        } else {
            self.i = start;
            None
        }
    }

    fn rest(&self) -> &'a str {
        self.s.get(self.i..).unwrap_or("")
    }
}

impl Header {
    /// A message's first line: its date and time, and the rest of the line.
    pub fn parse(line: &str) -> Option<(Header, &str)> {
        let mut c = Cur { s: line.trim_start_matches(is_mark), i: 0 };
        let ios = c.eat('[');
        let (a, la) = c.number(4)?;
        let sep = c.peek().filter(|s| matches!(s, '/' | '.' | '-'))?;
        c.eat(sep);
        let (b, _) = c.number(2)?;
        if !c.eat(sep) {
            return None;
        }
        let (cc, lc) = c.number(4)?;
        let year_first = la == 4;
        if !year_first && lc != 2 && lc != 4 {
            return None;
        }
        c.eat('.');
        c.eat(',');
        c.space();
        let (hour, _) = c.number(2)?;
        if !c.eat(':') {
            return None;
        }
        let (minute, lm) = c.number(2)?;
        if lm != 2 {
            return None;
        }
        let mut second = 0;
        if c.eat(':') {
            second = c.number(2)?.0;
        }
        let at = c.i;
        c.space();
        let hour = match c.am_pm() {
            Some(pm) => match hour {
                1..=11 if pm => hour + 12,
                12 if !pm => 0,
                1..=12 => hour,
                _ => 99,
            },
            None => {
                c.i = at;
                hour
            }
        };
        c.space();
        if ios {
            if !c.eat(']') {
                return None;
            }
            c.eat(' ');
        } else {
            if !(c.eat('-') || c.eat('–')) {
                return None;
            }
            if !c.eat(' ') && !c.rest().is_empty() {
                return None;
            }
        }
        let header = Header { a, b, c: cc, year_first, two_digit_year: !year_first && lc == 2, hour, minute, second };
        Some((header, c.rest()))
    }
}

/// The day/month order of a whole file (0015 point 3).
pub struct DateOrder {
    month_first: bool,
    note: Option<&'static str>,
}

impl DateOrder {
    pub fn note(&self) -> Option<&'static str> {
        self.note
    }

    /// Year, month, day of a header's date.
    pub fn date(&self, h: &Header) -> Option<(i64, u32, u32)> {
        if h.year_first {
            return Some((h.a as i64, h.b, h.c));
        }
        let year = if h.two_digit_year { 2000 + h.c as i64 } else { h.c as i64 };
        let (d, m) = if self.month_first { (h.b, h.a) } else { (h.a, h.b) };
        Some((year, m, d))
    }
}

/// A first number over 12 means day first; a second over 12 means month first; when nothing decides, day first.
pub fn decide_order<'a>(headers: impl Iterator<Item = &'a Header>) -> DateOrder {
    let (mut day, mut month) = (false, false);
    for h in headers.filter(|h| !h.year_first) {
        day |= h.a > 12;
        month |= h.b > 12;
    }
    match (day, month) {
        (false, true) => DateOrder { month_first: true, note: None },
        (true, false) => DateOrder { month_first: false, note: None },
        (false, false) => {
            DateOrder { month_first: false, note: Some("no date decides the day/month order: read as day first") }
        }
        (true, true) => {
            DateOrder { month_first: false, note: Some("dates disagree on the day/month order: read as day first") }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Option<(u32, u32, u32, u32, u32, u32, &str)> {
        Header::parse(line).map(|(h, rest)| (h.a, h.b, h.c, h.hour, h.minute, h.second, rest))
    }

    #[test]
    fn every_known_layout() {
        assert_eq!(parse("31/12/2023, 21:41 - Ana: hi"), Some((31, 12, 2023, 21, 41, 0, "Ana: hi")));
        assert_eq!(parse("12/31/23, 9:41\u{202F}PM - Ana: hi"), Some((12, 31, 23, 21, 41, 0, "Ana: hi")));
        assert_eq!(parse("31/12/23, 9:41\u{A0}p.\u{A0}m. - Ana: hi"), Some((31, 12, 23, 21, 41, 0, "Ana: hi")));
        assert_eq!(parse("1/2/24, 12:05 a. m. - x"), Some((1, 2, 24, 0, 5, 0, "x")));
        assert_eq!(parse("\u{200E}[31.12.23, 21:41:05] Ana: hi"), Some((31, 12, 23, 21, 41, 5, "Ana: hi")));
        assert_eq!(parse("[31/12/2023 21:41:05] Ana: hi"), Some((31, 12, 2023, 21, 41, 5, "Ana: hi")));
        assert_eq!(parse("2023-12-31, 21:41 - x"), Some((2023, 12, 31, 21, 41, 0, "x")));
        assert_eq!(parse("31-12-23 21:41 - x"), Some((31, 12, 23, 21, 41, 0, "x")));
        assert_eq!(parse("31.12.23, 21:41 - "), Some((31, 12, 23, 21, 41, 0, "")));
        assert_eq!(parse("13/1/24, 13:00 PM - x").map(|p| p.3), Some(99));
    }

    #[test]
    fn not_a_first_line() {
        for line in
            ["", "hello", "31/12", "31/12/2023", "31/12/2023, 21:41", "31/12/2023, 21:41 x", "1/2/345, 1:00 - x"]
        {
            assert!(Header::parse(line).is_none(), "{line}");
        }
    }
}
