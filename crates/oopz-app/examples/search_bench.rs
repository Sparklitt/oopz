//! Search index benchmark on a synthetic backup database (invented words, no real data):
//! `cargo run --release -j 2 -p oopz-app --example search_bench -- 100000`. Prints the build time and, per query,
//! the median and slowest of 21 runs. The numbers are in docs/desktop-plan.md.

use std::time::Instant;

use oopz_app::index::Index;
use rusqlite::{Connection, params};

/// A small deterministic generator: the same database every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn vocabulary() -> Vec<String> {
    // Real Spanish and English words for the queries, then invented syllable words for a large vocabulary.
    let mut words: Vec<String> = [
        "hola",
        "playa",
        "canción",
        "mañana",
        "café",
        "cumpleaños",
        "corazón",
        "película",
        "vale",
        "gracias",
        "casa",
        "trabajo",
        "fiesta",
        "llamada",
        "foto",
        "perro",
        "tren",
        "viaje",
        "comida",
        "noche",
        "good",
        "morning",
        "dinner",
        "tomorrow",
        "love",
        "train",
        "photo",
        "birthday",
        "movie",
        "weekend",
    ]
    .iter()
    .map(|w| w.to_string())
    .collect();
    let syllables = ["ma", "lo", "ti", "ra", "pe", "su", "ne", "ca", "do", "vi", "ñu", "qué", "ló", "ar", "en", "is"];
    let mut rng = Lcg(7);
    while words.len() < 5000 {
        let n = 2 + rng.below(3);
        words.push((0..n).map(|_| syllables[rng.below(syllables.len() as u64) as usize]).collect());
    }
    words
}

fn synthetic(messages: u64) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE jid (_id INTEGER PRIMARY KEY, raw_string TEXT);
         CREATE TABLE chat (_id INTEGER PRIMARY KEY, jid_row_id INTEGER);
         CREATE TABLE message (_id INTEGER PRIMARY KEY, chat_row_id INTEGER, from_me INTEGER, sender_jid_row_id INTEGER,
             timestamp INTEGER, message_type INTEGER, text_data TEXT);
         CREATE TABLE message_media (message_row_id INTEGER PRIMARY KEY, file_size INTEGER, media_duration INTEGER);",
    )
    .unwrap();
    let words = vocabulary();
    let mut rng = Lcg(42);
    let tx = db.unchecked_transaction().unwrap();
    for j in 1..=300 {
        // Invented ids in the 9999 range, never a real number.
        tx.execute("INSERT INTO jid VALUES (?1, ?2)", params![j, format!("99990000{j:04}@s.whatsapp.net")]).unwrap();
    }
    for c in 1..=200 {
        tx.execute("INSERT INTO chat VALUES (?1, ?1)", [c]).unwrap();
    }
    let mut msg = tx.prepare("INSERT INTO message VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)").unwrap();
    let mut media = tx.prepare("INSERT INTO message_media VALUES (?1, ?2, ?3)").unwrap();
    let start_ms = 1_420_070_400_000i64; // 2015-01-01
    for id in 1..=messages as i64 {
        // A few chats hold most messages, as in real life.
        let chat = 1 + (rng.below(200) * rng.below(200) / 200) as i64;
        let ts = start_ms + id * 3_000 + rng.below(2_000) as i64;
        let kind = match rng.below(100) {
            0..=79 => 0,
            80..=89 => 1,
            90..=94 => 2,
            _ => 3,
        };
        let text = (kind == 0 || rng.below(3) == 0).then(|| {
            let n = 2 + rng.below(18);
            // Zipf-ish: low word numbers far more often.
            (0..n)
                .map(|_| {
                    words[(rng.below(words.len() as u64) * rng.below(words.len() as u64) / words.len() as u64) as usize]
                        .as_str()
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
        let from_me = rng.below(2) as i64;
        let sender = if from_me == 1 { 0 } else { 1 + rng.below(300) as i64 };
        msg.execute(params![id, chat, from_me, sender, ts, kind, text]).unwrap();
        if kind != 0 {
            media.execute(params![id, 50_000 + rng.below(3_000_000) as i64, rng.below(120) as i64]).unwrap();
        }
    }
    drop((msg, media));
    tx.commit().unwrap();
    db
}

fn time(label: &str, mut run: impl FnMut() -> usize) {
    let mut ms: Vec<f64> = (0..21)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(run());
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    let results = run();
    println!("{label:<44} median {:>7.2} ms   slowest {:>7.2} ms   ({results} results)", ms[10], ms[20]);
}

fn main() {
    let messages: u64 = std::env::args().nth(1).and_then(|n| n.parse().ok()).unwrap_or(100_000);
    let t = Instant::now();
    let source = synthetic(messages);
    println!("synthetic database: {messages} messages in {:.1} s", t.elapsed().as_secs_f64());
    let dir = tempfile::tempdir().unwrap();
    let index = Index::build(&source, dir.path()).unwrap();
    let size: u64 = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().metadata().unwrap().len()).sum();
    println!(
        "index: {} messages, {} with text, built in {:.2} s, {:.0} MB on disk",
        index.info.messages,
        index.info.with_text,
        index.info.millis as f64 / 1000.0,
        size as f64 / 1e6
    );
    for (label, query, chat) in [
        ("common word: hola", "hola", None),
        ("accent-free: cancion (finds canción)", "cancion", None),
        ("prefix: cumple", "cumple", None),
        ("two words: good morning", "good morning", None),
        ("rare invented word", "malotira", None),
        ("no match", "zzzzzz", None),
        ("one letter prefix: m", "m", None),
        ("in the biggest chat: playa", "playa", Some(1)),
        ("in a small chat: gracias", "gracias", Some(150)),
    ] {
        time(label, || index.search(query, &oopz_app::Filter { chat, ..Default::default() }, 50).unwrap().len());
    }
    let len = index.chat_len(1).unwrap();
    time(&format!("viewer: 60 lines at the end of chat 1 ({len})"), || {
        index.lines(1, len.saturating_sub(60), 60).unwrap().len()
    });
    time("viewer: 60 lines at the start", || index.lines(1, 0, 60).unwrap().len());
    let hit = index.search("hola", &oopz_app::Filter { chat: Some(1), ..Default::default() }, 1).unwrap()[0].id;
    time("jump to a search hit (position)", || index.position(hit).unwrap().map_or(0, |_| 1));
    time("stats: whole backup", || index.stats(None).unwrap().by_month.len());
    time("stats: biggest chat", || index.stats(Some(1)).unwrap().by_month.len());
}
