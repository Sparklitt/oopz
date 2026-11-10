//! Times the library on a large invented source: `cargo run --release -p oopz-core --example perf -- [MESSAGES]`
//! (1,000,000 by default). Prints the timings; writes only to a temporary folder.

use std::time::Instant;

use oopz_core::{Filter, Library, Order};
use oopz_model::{App, Conversation, ConversationKind, Loaded, Message, Part, Person, SourceInfo};

const WORDS: &[&str] = &[
    "hola",
    "café",
    "mañana",
    "playa",
    "cena",
    "trabajo",
    "東京",
    "שלום",
    "مرحبا",
    "photo",
    "meeting",
    "ok",
    "👍",
    "gracias",
    "noche",
    "lunes",
    "viernes",
    "familia",
    "película",
    "música",
];

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(1_000_000);
    let convs = 500;
    let people: Vec<Person> = (0..200)
        .map(|i| Person {
            id: format!("p{i}"),
            name: Some(format!("Person {i}")),
            phone: Some(format!("+9999{i:06}")),
            ..Default::default()
        })
        .chain([Person { id: "me".into(), me: true, ..Default::default() }])
        .collect();
    let conversations: Vec<Conversation> = (0..convs)
        .map(|i| Conversation {
            id: format!("c{i}"),
            kind: if i % 10 == 0 { ConversationKind::Group } else { ConversationKind::Direct },
            name: Some(format!("Chat {i}")),
            members: vec!["me".into(), format!("p{}", i % 200)],
            created_ms: None,
        })
        .collect();
    let mut seed = 0x9E3779B97F4A7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let t = Instant::now();
    let mut messages = Vec::with_capacity(n);
    for c in 0..convs {
        let per = n / convs + usize::from(c < n % convs);
        for j in 0..per {
            let words: Vec<&str> =
                (0..(3 + rnd() % 12)).map(|_| WORDS[(rnd() % WORDS.len() as u64) as usize]).collect();
            let me = rnd() % 2 == 0;
            messages.push(Message {
                id: j.to_string(),
                conversation: format!("c{c}"),
                sender: Some(if me { "me".into() } else { format!("p{}", rnd() % 200) }),
                from_me: me,
                time_ms: Some(1_500_000_000_000 + (j as i64) * 3_600_000 + (rnd() % 3_600_000) as i64),
                parts: vec![Part::text(words.join(" "))],
                ..Default::default()
            });
        }
    }
    println!("generated {} messages in {:.1?}", messages.len(), t.elapsed());
    let mut source = Loaded {
        info: SourceInfo {
            app: App::Telegram,
            adapter: "perf".into(),
            format: None,
            owner: None,
            media_root: None,
            wall_clock: false,
            notes: vec![],
        },
        people,
        conversations,
        messages,
    };
    let dir = tempfile::tempdir().unwrap();
    if let Some(keep) = std::env::var_os("OOPZ_PERF_KEEP") {
        println!("library at {}", dir.path().display());
        let _ = keep;
    }
    let mut lib = Library::open(dir.path()).unwrap();
    let t = Instant::now();
    lib.import(&mut source, "perf", "fp", &mut |_, _| true).unwrap();
    println!("import: {:.1?}", t.elapsed());
    let size = std::fs::metadata(dir.path().join(oopz_core::FILE)).unwrap().len();
    println!("library.db: {} MB", size / 1_000_000);
    let all = Filter { apps: vec![App::Telegram], ..Default::default() };
    let time = |label: &str, f: &mut dyn FnMut() -> String| {
        let t = Instant::now();
        let out = f();
        println!("{label}: {:.1?} ({out})", t.elapsed());
    };
    time("count", &mut || lib.count(&all).unwrap().to_string());
    time("conversations", &mut || lib.conversations(&all).unwrap().len().to_string());
    for q in ["playa", "mañana famil", "東京", "cafe", "zzzz"] {
        let f = Filter { words: q.into(), ..all.clone() };
        time(&format!("search {q:?}"), &mut || lib.search(&f, 200).unwrap().len().to_string());
    }
    let f = Filter { words: "playa".into(), from_me: Some(true), ..all.clone() };
    time("count words+side", &mut || lib.count(&f).unwrap().to_string());
    time("stats", &mut || lib.stats(&all).unwrap().messages.to_string());
    time("timeline page (offset 0)", &mut || lib.find(&all, Order::Time, 0, 100).unwrap().len().to_string());
    time("timeline page (offset 900k)", &mut || {
        lib.find(&all, Order::Time, (n as u64) * 9 / 10, 100).unwrap().len().to_string()
    });
    let conv = lib.conversations(&all).unwrap()[0].id;
    time("open a conversation (page)", &mut || lib.messages(conv, 1000, 100).unwrap().len().to_string());
    time("jump to a date", &mut || format!("{:?}", lib.position_at(conv, 1_505_000_000_000).unwrap()));
    if std::env::var_os("OOPZ_PERF_KEEP").is_some() {
        std::mem::forget(dir);
    }
    time("selected ids (all)", &mut || {
        lib.selected(&oopz_core::Selection { filter: all.clone(), ..Default::default() }, Order::Conversation)
            .unwrap()
            .len()
            .to_string()
    });
}
