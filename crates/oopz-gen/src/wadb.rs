//! `wa.db`: the contacts file beside the messages, with the names oopz shows (`wa_contacts`).

use anyhow::Result;
use rusqlite::{Connection, params};

use crate::world::{ChatKind, World};

pub fn write(db: &Connection, world: &World, edge_cases: bool) -> Result<u64> {
    db.execute_batch(
        "CREATE TABLE wa_contacts (_id INTEGER PRIMARY KEY AUTOINCREMENT, jid TEXT NOT NULL, is_whatsapp_user INTEGER,
             status TEXT, status_timestamp INTEGER, number TEXT, raw_contact_id INTEGER, display_name TEXT,
             phone_type INTEGER, phone_label TEXT, given_name TEXT, family_name TEXT, wa_name TEXT, sort_name TEXT,
             nickname TEXT, company TEXT, title TEXT);
         CREATE INDEX wa_contacts_jid ON wa_contacts (jid);",
    )?;
    let mut insert = db.prepare(
        "INSERT INTO wa_contacts (jid, is_whatsapp_user, status, number, raw_contact_id, display_name, given_name,
             family_name, wa_name, sort_name) VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?5)",
    )?;
    let mut rows = 0;
    // The owner isn't in their own contacts.
    for (i, p) in world.people.iter().enumerate().skip(1) {
        let (given, family) = match p.display_name.as_deref().and_then(|n| n.split_once(' ')) {
            Some((g, f)) => (Some(g), Some(f)),
            None => (p.display_name.as_deref(), None),
        };
        let status = (i % 3 == 0).then_some("Disponible");
        insert.execute(params![
            p.jid,
            status,
            format!("+{}", p.number),
            i as i64,
            p.display_name,
            given,
            family,
            p.wa_name
        ])?;
        rows += 1;
        // Someone saved twice in the address book: the named row must win.
        if edge_cases && i == 2 {
            insert.execute(params![
                p.jid,
                None::<&str>,
                format!("+{}", p.number),
                0,
                None::<&str>,
                None::<&str>,
                None::<&str>,
                None::<&str>
            ])?;
            rows += 1;
        }
    }
    // Groups are listed too, under their subject.
    for chat in &world.chats {
        if let ChatKind::Group { jid, subject, .. } = &chat.kind {
            insert.execute(params![
                jid,
                None::<&str>,
                None::<&str>,
                0,
                subject,
                None::<&str>,
                None::<&str>,
                None::<&str>
            ])?;
            rows += 1;
        }
    }
    Ok(rows)
}
