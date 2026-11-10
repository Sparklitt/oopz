//! The invented people and chats. Every number starts `9999` and every `lid` `8888` (12 digits), so a test can grep
//! for them, and none can be a real person's.

use crate::corpus;
use crate::rng::Rng;
use crate::spec::Spec;

pub struct Person {
    /// 12 digits, `9999…`.
    pub number: String,
    /// `<number>@s.whatsapp.net`.
    pub jid: String,
    /// `8888…@lid`, for people WhatsApp files under their hidden id in groups.
    pub lid: Option<String>,
    /// The name in the owner's address book.
    pub display_name: Option<String>,
    /// The name they set themselves (shown with a `~`).
    pub wa_name: Option<String>,
    pub spanish: bool,
}

pub enum ChatKind {
    /// With person `n`.
    Direct(usize),
    Group {
        jid: String,
        subject: Option<String>,
        /// Person indexes, the owner (0) first.
        members: Vec<usize>,
    },
    /// "Message yourself".
    Own,
    /// `status@broadcast`: the owner's and contacts' status posts.
    Status,
}

pub struct Chat {
    pub kind: ChatKind,
    pub messages: u64,
    /// When the chat starts, ms.
    pub start_ms: i64,
}

impl Chat {
    /// Who may write in this chat (person indexes).
    pub fn members(&self) -> Vec<usize> {
        match &self.kind {
            ChatKind::Direct(n) => vec![0, *n],
            ChatKind::Group { members, .. } => members.clone(),
            ChatKind::Own => vec![0],
            ChatKind::Status => vec![0, 1, 2, 3],
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self.kind, ChatKind::Group { .. })
    }
}

pub struct World {
    pub people: Vec<Person>,
    pub chats: Vec<Chat>,
}

fn person(rng: &mut Rng, index: usize, in_book: bool) -> Person {
    let number = format!("9999{index:08}");
    let first = *rng.pick(corpus::FIRST_NAMES);
    let last = *rng.pick(corpus::LAST_NAMES);
    let display_name = in_book.then(|| format!("{first} {last}"));
    let wa_name = rng.chance(0.7).then(|| if rng.chance(0.5) { first.to_owned() } else { format!("{first} ✨") });
    Person {
        jid: format!("{number}@s.whatsapp.net"),
        lid: rng.chance(0.3).then(|| format!("8888{index:08}@lid")),
        number,
        display_name,
        wa_name,
        spanish: rng.chance(0.7),
    }
}

impl World {
    pub fn new(spec: &Spec) -> World {
        let mut rng = Rng::fork(spec.seed, 1);
        let span = spec.end_ms - spec.start_ms;
        let strangers = if spec.groups > 0 { spec.group_size.1 as usize } else { 0 };
        let mut people = vec![Person {
            number: "999900000000".into(),
            jid: "999900000000@s.whatsapp.net".into(),
            lid: None,
            display_name: None,
            wa_name: Some("Yo".into()),
            spanish: true,
        }];
        for i in 1..=spec.contacts as usize {
            people.push(person(&mut rng, i, true));
        }
        for i in 0..strangers {
            people.push(person(&mut rng, spec.contacts as usize + 1 + i, false));
        }

        let mut chats = Vec::new();
        for n in 1..=spec.contacts as usize {
            let messages =
                if n == 1 && spec.main_chat > 0 { spec.main_chat } else { rng.range(spec.per_chat.0, spec.per_chat.1) };
            // The long conversation spans every year; the others start somewhere in the first two thirds.
            let start_ms =
                if n == 1 { spec.start_ms } else { spec.start_ms + (rng.unit() * span as f64 * 0.66) as i64 };
            chats.push(Chat { kind: ChatKind::Direct(n), messages, start_ms });
        }
        let pool = people.len() - 1;
        for g in 0..spec.groups as usize {
            let size = (rng.range(spec.group_size.0 as u64, spec.group_size.1 as u64) as usize).min(pool);
            let mut members = vec![0];
            while members.len() < size + 1 {
                // Contacts more often than strangers.
                let n = if rng.chance(0.6) && spec.contacts > 0 {
                    rng.range(1, spec.contacts as u64) as usize
                } else {
                    rng.range(1, pool as u64) as usize
                };
                if !members.contains(&n) {
                    members.push(n);
                }
            }
            let subject = Some(format!("{} {}", rng.pick(corpus::GROUP_NAMES), g + 1));
            chats.push(Chat {
                kind: ChatKind::Group { jid: format!("120363{g:012}@g.us"), subject, members },
                messages: rng.range(spec.per_chat.0, spec.per_chat.1),
                start_ms: spec.start_ms + (rng.unit() * span as f64 * 0.66) as i64,
            });
        }

        if spec.edge_cases {
            Self::add_edge_cases(&mut rng, spec, &mut people, &mut chats);
        }
        World { people, chats }
    }

    /// Contacts and chats that have broken readers: no name at all, a name of emoji and right-to-left text, a very
    /// long name, an empty chat, the owner's own chat, status posts, a group with no subject and one whose subject is
    /// right-to-left, a group the owner alone is left in.
    fn add_edge_cases(rng: &mut Rng, spec: &Spec, people: &mut Vec<Person>, chats: &mut Vec<Chat>) {
        let mut add = |people: &mut Vec<Person>, display: Option<&str>, wa: Option<&str>| {
            let i = people.len();
            let mut p = person(rng, i, false);
            p.display_name = display.map(str::to_owned);
            p.wa_name = wa.map(str::to_owned);
            people.push(p);
            i
        };
        let nameless = add(people, None, None);
        let emoji = add(people, Some("🌙 Luna 🌙 مرحبا"), Some("🌙"));
        let long = add(people, Some(&"Nombre Larguísimo ".repeat(14)), None);
        let blank = add(people, Some("   "), Some(""));
        let only_wa = add(people, None, Some("Solo Su Nombre"));
        let mid = spec.start_ms + (spec.end_ms - spec.start_ms) / 2;
        for (n, messages) in [(nameless, 25), (emoji, 25), (long, 10), (blank, 5), (only_wa, 10)] {
            chats.push(Chat { kind: ChatKind::Direct(n), messages, start_ms: mid });
        }
        // A contact whose chat was opened and never written in.
        let silent = add(people, Some("Nunca Escribe"), None);
        chats.push(Chat { kind: ChatKind::Direct(silent), messages: 0, start_ms: mid });
        chats.push(Chat { kind: ChatKind::Own, messages: 30, start_ms: mid });
        if spec.contacts >= 3 {
            chats.push(Chat { kind: ChatKind::Status, messages: 15, start_ms: spec.end_ms - 86_400_000 });
        }
        let some: Vec<usize> = (0..people.len()).step_by(2).take(5).collect();
        for (subject, members) in [(None, some.clone()), (Some("مجموعة الاختبار 👨‍👩‍👧"), some), (Some("Solo yo"), vec![0])]
        {
            let g = chats.len();
            chats.push(Chat {
                kind: ChatKind::Group {
                    jid: format!("120363999{g:09}@g.us"),
                    subject: subject.map(Into::into),
                    members,
                },
                messages: 12,
                start_ms: mid,
            });
        }
    }
}
