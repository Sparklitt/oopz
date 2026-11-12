//! "Save as a web page" (decision 0009, point 5): one self-contained HTML file with a backup's or a chat's stats and,
//! for a chat, its messages. Written only when the person asks, to a folder they pick, as a new file that never
//! replaces another. It holds their own message text and isn't encrypted: the screen says so. No scripts, no outside
//! resources (a Content-Security-Policy forbids them), every value escaped.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::index::{Index, Stats};
use crate::{AppError, Result};

/// The page's words, in the person's language, from the UI (`src/i18n.ts`), so they live in one place.
#[derive(Debug, Deserialize)]
pub struct Labels {
    /// `en` or `es`, for `<html lang>`.
    pub lang: String,
    /// What is exported, e.g. "12 · contact ••••001", or the whole backup.
    pub heading: String,
    pub messages: String,
    pub sent: String,
    pub received: String,
    pub dates: String,
    pub by_month: String,
    pub by_hour: String,
    pub by_kind: String,
    pub audio_minutes: String,
    pub senders: String,
    pub you: String,
    /// The warning at the top: the file holds message text and isn't encrypted.
    pub note: String,
    /// WhatsApp's message type codes to words; unknown ones show as `#code`.
    pub kinds: HashMap<i64, String>,
}

impl Labels {
    /// The page's words in `lang`, from oopz's messages (`oopz_model::messages`), for the CLI, which has no UI to send
    /// them (a language they aren't worded in gets the default's, and `lang` says which); the app sends its own, from
    /// its language files (`apps/desktop/src/lib/i18n/`). `heading` names what is exported.
    pub fn in_language(lang: &str, heading: &str) -> Labels {
        use oopz_model::messages::{LANGUAGES, Message as Msg};
        let lang = if LANGUAGES.contains(&lang) { lang } else { LANGUAGES[0] };
        let t = |m: Msg| msgkinds::Note::of(m, []).text(lang);
        // WhatsApp's own labels for its type codes, msgkinds' registry rows (English, as the CLI shows them).
        let kinds: Vec<(i64, String)> = msgkinds::Source::WhatsAppAndroid
            .entries()
            .iter()
            .filter_map(|e| match e.code {
                msgkinds::Code::Int(code) => Some((code, e.label.to_lowercase())),
                msgkinds::Code::Name(_) => None,
            })
            .collect();
        Labels {
            lang: lang.to_owned(),
            heading: heading.to_owned(),
            messages: t(Msg::ExportMessages),
            sent: t(Msg::StatsSent),
            received: t(Msg::StatsReceived),
            dates: t(Msg::StatsDates),
            by_month: t(Msg::StatsByMonth),
            by_hour: t(Msg::StatsByHour),
            by_kind: t(Msg::StatsByKind),
            audio_minutes: t(Msg::StatsAudioMinutes),
            senders: t(Msg::StatsSenders),
            you: t(Msg::ExportMe),
            note: t(Msg::ExportNote),
            kinds: kinds.into_iter().collect(),
        }
    }
}

/// Writes the page into a new `oopz-export-N.html` in `out_dir` and returns its path. `chat` adds that chat's
/// messages; `None` exports the whole backup's stats only.
pub fn write(index: &Index, backup_name: &str, chat: Option<i64>, labels: &Labels, out_dir: &Path) -> Result<PathBuf> {
    let html = page(index, backup_name, chat, labels)?;
    for n in 1..1000 {
        let path = out_dir.join(format!("oopz-export-{n}.html"));
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                return match f.write_all(html.as_bytes()).and_then(|()| f.sync_all()) {
                    Ok(()) => Ok(path),
                    Err(e) => {
                        drop(f);
                        let _ = fs::remove_file(&path); // Never leave half a page.
                        Err(AppError::io(format!("writing {}", path.display()), e))
                    }
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(AppError::io(format!("writing {}", path.display()), e)),
        }
    }
    Err(AppError::new("exists", "too many oopz-export files here; choose another folder"))
}

fn page(index: &Index, backup_name: &str, chat: Option<i64>, l: &Labels) -> Result<String> {
    let stats = index.stats(chat)?;
    let mut o = String::with_capacity(1 << 16);
    let _ = write!(
        o,
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{} · {}</title><style>{CSS}</style></head><body><main>\
         <h1>{}</h1><p class=\"sub\">{}</p><p class=\"note\">{}</p>",
        esc(&l.lang),
        esc(&l.heading),
        esc(backup_name),
        esc(&l.heading),
        esc(backup_name),
        esc(&l.note),
    );
    stats_html(&mut o, &stats, l);
    if let Some(chat) = chat {
        messages_html(&mut o, index, chat, l)?;
    }
    o.push_str("</main></body></html>\n");
    Ok(o)
}

fn stats_html(o: &mut String, s: &Stats, l: &Labels) {
    let n = |v: u64| group(v);
    let _ = write!(
        o,
        "<section class=\"tiles\"><div><b>{}</b>{}</div><div><b>{}</b>{}</div><div><b>{}</b>{}</div>\
         <div class=\"wide\"><b>{}</b>{}</div><div><b>{}</b>{}</div></section>",
        n(s.messages),
        esc(&l.messages),
        n(s.from_me),
        esc(&l.sent),
        n(s.from_others),
        esc(&l.received),
        span(s.first_ms, s.last_ms),
        esc(&l.dates),
        n(s.audio_seconds.div_ceil(60)),
        esc(&l.audio_minutes),
    );
    let rows: Vec<(String, u64)> = s.by_month.iter().map(|(m, v)| (m.clone(), *v)).collect();
    bars(o, &l.by_month, &rows);
    let _ = write!(o, "<h2>{}</h2><div class=\"hours\">", esc(&l.by_hour));
    let top = s.by_hour.iter().copied().max().unwrap_or(0).max(1);
    for (h, v) in s.by_hour.iter().enumerate() {
        let _ = write!(o, "<span title=\"{h}:00 · {}\"><i style=\"height:{}%\"></i>{h}</span>", n(*v), v * 100 / top);
    }
    o.push_str("</div>");
    let kinds: Vec<(String, u64)> =
        s.by_kind.iter().map(|(k, v, _)| (l.kinds.get(k).cloned().unwrap_or_else(|| format!("#{k}")), *v)).collect();
    bars(o, &l.by_kind, &kinds);
    if !s.senders.is_empty() {
        let senders: Vec<(String, u64)> = s.senders.iter().map(|(w, v)| (format!("••••{w}"), *v)).collect();
        bars(o, &l.senders, &senders);
    }
}

fn bars(o: &mut String, title: &str, rows: &[(String, u64)]) {
    if rows.is_empty() {
        return;
    }
    let top = rows.iter().map(|r| r.1).max().unwrap_or(0).max(1);
    let _ = write!(o, "<h2>{}</h2><div class=\"bars\">", esc(title));
    for (label, v) in rows {
        let _ = write!(
            o,
            "<span>{}</span><span class=\"bar\"><i style=\"width:{}%\"></i></span><span>{}</span>",
            esc(label),
            (v * 100 / top).max(1),
            group(*v)
        );
    }
    o.push_str("</div>");
}

fn messages_html(o: &mut String, index: &Index, chat: i64, l: &Labels) -> Result<()> {
    let _ = write!(o, "<h2>{}</h2><div class=\"chat\">", esc(&l.messages));
    let len = index.chat_len(chat)?;
    let mut last_day = String::new();
    let mut from = 0;
    while from < len {
        let lines = index.lines(chat, from, 500)?;
        if lines.is_empty() {
            break;
        }
        from += lines.len() as u64;
        for m in lines {
            let today = if m.ts > 0 { day(m.ts) } else { String::new() };
            if today != last_day {
                let _ = write!(o, "<p class=\"day\">{today}</p>");
                last_day = today;
            }
            let who = if m.from_me {
                esc(&l.you)
            } else {
                m.sender.as_deref().map(|w| format!("••••{}", esc(w))).unwrap_or_default()
            };
            let body = match &m.text {
                Some(text) => esc(text),
                None => format!("<em>[{}]</em>", esc(&l.kinds.get(&m.kind).cloned().unwrap_or(format!("#{}", m.kind)))),
            };
            let _ = write!(
                o,
                "<div class=\"m{}\"><small>{who} {}</small>{body}</div>",
                if m.from_me { " me" } else { "" },
                if m.ts > 0 { time(m.ts) } else { String::new() },
            );
        }
    }
    o.push_str("</div>");
    Ok(())
}

/// Text for HTML, between tags or in a double-quoted attribute.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// "first – last", or one day when they're the same.
fn span(first: Option<i64>, last: Option<i64>) -> String {
    let (first, last) = (first.map(day).unwrap_or_default(), last.map(day).unwrap_or_default());
    if first == last { first } else { format!("{first} – {last}") }
}

fn local(ms: i64) -> Option<jiff::Zoned> {
    Some(jiff::Timestamp::from_millisecond(ms).ok()?.to_zoned(jiff::tz::TimeZone::system()))
}

/// `YYYY-MM-DD` in this computer's time zone, as the app's stats.
fn day(ms: i64) -> String {
    local(ms).map(|z| z.strftime("%Y-%m-%d").to_string()).unwrap_or_default()
}

fn time(ms: i64) -> String {
    local(ms).map(|z| z.strftime("%H:%M").to_string()).unwrap_or_default()
}

/// 12345 → "12 345" (a thin space reads in both languages).
fn group(v: u64) -> String {
    let digits = v.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// The page's look: the oopz palette, light and dark, readable printed.
const CSS: &str = "
:root{--bg:#fff7ee;--card:#fff;--ink:#23244f;--soft:#6b6c8f;--line:#e4d9cc;--coral:#f2604f;--me:#ffe9d6;color-scheme:light dark}
@media (prefers-color-scheme:dark){:root{--bg:#15162e;--card:#26284f;--ink:#f6f0e7;--soft:#a9aacb;--line:#34366a;--coral:#ff7565;--me:#3a3c72}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.5 system-ui,-apple-system,'Segoe UI',sans-serif,'Apple Color Emoji','Segoe UI Emoji','Noto Color Emoji'}
main{max-width:860px;margin:0 auto;padding:32px 20px 64px}h1{margin:0;font-size:28px}h2{margin:32px 0 12px;font-size:18px}
.sub{color:var(--soft);margin:4px 0 16px}.note{background:var(--card);border-left:4px solid var(--coral);padding:10px 14px;border-radius:8px}
.tiles{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin-top:20px}
.tiles div{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:12px 14px;color:var(--soft)}
.tiles b{display:block;color:var(--ink);font-size:20px;white-space:nowrap}.tiles .wide{grid-column:span 2}
.bars{display:grid;grid-template-columns:max-content 1fr max-content;gap:6px 12px;align-items:center;font-size:13px}
.bar{height:10px;background:var(--line);border-radius:5px;overflow:hidden}.bar i{display:block;height:100%;background:var(--coral)}
.hours{display:grid;grid-template-columns:repeat(24,1fr);gap:3px;height:110px;align-items:end;font-size:10px;color:var(--soft);text-align:center}
.hours span{display:flex;flex-direction:column;justify-content:flex-end;height:100%}.hours i{display:block;background:var(--coral);border-radius:3px 3px 0 0;min-height:1px}
.chat{display:flex;flex-direction:column;gap:6px}.day{align-self:center;color:var(--soft);font-size:12px;margin:14px 0 4px}
.m{align-self:flex-start;max-width:75%;background:var(--card);border:1px solid var(--line);border-radius:12px;padding:6px 10px;white-space:pre-wrap;overflow-wrap:anywhere}
.m.me{align-self:flex-end;background:var(--me)}.m small{display:block;color:var(--soft);font-size:11px}
@media print{body{background:#fff}.m,.tiles div{break-inside:avoid}}
";

#[cfg(test)]
mod tests {
    use oopz::fixture::{self, BEN_CHAT, KEY_HEX};

    use super::*;

    fn labels() -> Labels {
        let w = |s: &str| s.to_owned();
        Labels {
            lang: w("en"),
            heading: w("Chat <b>"),
            messages: w("messages"),
            sent: w("sent"),
            received: w("received"),
            dates: w("dates"),
            by_month: w("by month"),
            by_hour: w("by hour"),
            by_kind: w("by type"),
            audio_minutes: w("minutes of audio"),
            senders: w("senders"),
            you: w("You"),
            note: w("not encrypted"),
            kinds: HashMap::from([(0, w("kind zero")), (1, w("kind one"))]),
        }
    }

    #[test]
    fn a_chat_page_is_self_contained_escaped_and_never_replaces_a_file() {
        let fx = fixture::build();
        let tmp = tempfile::tempdir().unwrap();
        let backup = oopz::Backup::open_in(&fx.backup, &oopz::Key::from_hex(KEY_HEX).unwrap(), tmp.path()).unwrap();
        let index = Index::build(backup.db(), tmp.path()).unwrap();
        let out = tempfile::tempdir().unwrap();

        let first = write(&index, "msgstore.db.crypt15", Some(BEN_CHAT), &labels(), out.path()).unwrap();
        let second = write(&index, "msgstore.db.crypt15", None, &labels(), out.path()).unwrap();
        assert_eq!(first, out.path().join("oopz-export-1.html"));
        assert_eq!(second, out.path().join("oopz-export-2.html"));

        let html = fs::read_to_string(&first).unwrap();
        assert!(html.contains("Content-Security-Policy") && html.contains("default-src 'none'"));
        assert!(!html.contains("<script") && !html.contains("://"));
        assert!(html.contains("Chat &lt;b&gt;") && !html.contains("Chat <b>"));
        assert!(html.contains("Hola"), "the chat's own text is in its page");
        assert!(!html.contains(fixture::BEN_JID), "never a whole number");
        // The backup-wide page has the stats, no messages.
        let stats_only = fs::read_to_string(&second).unwrap();
        assert!(stats_only.contains("by month") && !stats_only.contains("Hola"));
    }

    #[test]
    fn escapes_and_groups() {
        assert_eq!(esc(r#"<a href="x">'&'</a>"#), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;");
        assert_eq!(group(1_234_567), "1\u{202f}234\u{202f}567");
        assert_eq!(group(12), "12");
    }
}
