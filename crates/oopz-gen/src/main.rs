//! `oopz-gen`: writes a synthetic WhatsApp backup for testing oopz. Invented data only.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use oopz_gen::spec::{Kind, MediaMode, Spec};

#[derive(Parser)]
#[command(
    name = "oopz-gen",
    version,
    about = "Write a synthetic WhatsApp backup (invented people and messages) for testing oopz"
)]
struct Cli {
    /// The folder to write into (created; refused if it already holds a backup).
    out: PathBuf,
    /// small (a few thousand messages), million (one chat of a million), stress (thousands of chats), edge (odd rows).
    #[arg(long, default_value = "small")]
    preset: String,
    /// The same seed gives the same backup.
    #[arg(long)]
    seed: Option<u64>,
    /// Messages in chat 1, the long one-to-one conversation (0 for none).
    #[arg(long)]
    messages: Option<u64>,
    /// People with a one-to-one chat.
    #[arg(long)]
    contacts: Option<u32>,
    #[arg(long)]
    groups: Option<u32>,
    /// Members per group besides the owner, as MIN-MAX.
    #[arg(long, value_parser = range)]
    group_size: Option<(u64, u64)>,
    /// Messages in every other chat, as MIN-MAX.
    #[arg(long, value_parser = range)]
    per_chat: Option<(u64, u64)>,
    /// First and last message years, as FROM-TO (UTC, 1 January of FROM to 31 December of TO).
    #[arg(long, value_parser = range)]
    years: Option<(u64, u64)>,
    /// `files` writes a placeholder file per media message; `none` writes rows only.
    #[arg(long)]
    media: Option<String>,
    /// Only these kinds, comma-separated (e.g. `text,image,voice`); see --list-kinds.
    #[arg(long)]
    only: Option<String>,
    /// Add odd but real rows and chats to any preset.
    #[arg(long)]
    edge_cases: bool,
    /// No wa.db (contacts' names).
    #[arg(long)]
    no_contacts: bool,
    /// Keep the unencrypted msgstore.db and wa.db too.
    #[arg(long)]
    keep_plain: bool,
    /// 64 hex digits; by default one derived from the seed (written to key.txt either way).
    #[arg(long)]
    key: Option<String>,
    /// Print the kinds of message and exit.
    #[arg(long)]
    list_kinds: bool,
    /// Write an app's official text export instead of a backup: whatsapp-txt, telegram-json, sms-xml, line-txt or
    /// viber-csv (with --seed; the other options don't apply).
    #[arg(long)]
    export: Option<String>,
}

/// Writes every export of `format` into `out`, each in its folder with its manifest.
fn write_export(format: &str, seed: u64, out: &std::path::Path) -> Result<(), String> {
    let exports = oopz_gen::exports::generate(format, seed)
        .ok_or_else(|| format!("no export `{format}`; there are: {}", oopz_gen::exports::FORMATS.join(", ")))?;
    for (folder, export) in exports {
        let dir = out.join(&folder);
        std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        for (name, bytes) in &export.files {
            let path = dir.join(name);
            std::fs::write(&path, bytes).map_err(|e| format!("writing {}: {e}", path.display()))?;
        }
        let path = dir.join("manifest.txt");
        std::fs::write(&path, export.manifest.to_text(format, seed))
            .map_err(|e| format!("writing {}: {e}", path.display()))?;
        println!("{}: {} messages in {} files", dir.display(), export.manifest.messages, export.files.len());
    }
    Ok(())
}

fn range(text: &str) -> Result<(u64, u64), String> {
    let (a, b) = text.split_once('-').unwrap_or((text, text));
    let n = |s: &str| s.trim().replace('_', "").parse::<u64>().map_err(|_| format!("`{text}` isn't MIN-MAX"));
    let (a, b) = (n(a)?, n(b)?);
    if a > b {
        return Err(format!("`{text}`: the minimum is above the maximum"));
    }
    Ok((a, b))
}

fn year_ms(year: u64) -> Result<i64, String> {
    if !(1970..=2100).contains(&year) {
        return Err(format!("year {year} is outside 1970–2100"));
    }
    // Days from 1970 to 1 January of `year`.
    let days: i64 =
        (1970..year as i64).map(|y| if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 }).sum();
    Ok(days * 86_400_000)
}

fn build(cli: &Cli) -> Result<Spec, String> {
    let mut spec = Spec::preset(&cli.preset)
        .ok_or_else(|| format!("no preset `{}`; there are: {}", cli.preset, Spec::PRESETS.join(", ")))?;
    if let Some(seed) = cli.seed {
        spec.seed = seed;
    }
    if let Some(n) = cli.messages {
        spec.main_chat = n;
    }
    if let Some(n) = cli.contacts {
        spec.contacts = n;
    }
    if let Some(n) = cli.groups {
        spec.groups = n;
    }
    let small = |n: u64| u32::try_from(n).map_err(|_| "a group size is too big".to_string());
    if let Some((a, b)) = cli.group_size {
        spec.group_size = (small(a)?, small(b)?);
    }
    if let Some(r) = cli.per_chat {
        spec.per_chat = r;
    }
    if let Some((from, to)) = cli.years {
        spec.start_ms = year_ms(from)?;
        spec.end_ms = year_ms(to + 1)? - 1;
    }
    match cli.media.as_deref() {
        None => {}
        Some("files") => spec.media = MediaMode::Files,
        Some("none") => spec.media = MediaMode::None,
        Some(other) => return Err(format!("--media is `files` or `none`, not `{other}`")),
    }
    if let Some(only) = &cli.only {
        let mut mix = [0; 20];
        for name in only.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let kind = Kind::from_name(name).ok_or_else(|| format!("no kind `{name}`; see --list-kinds"))?;
            let i = Kind::ALL.iter().position(|k| *k == kind).expect("a listed kind");
            mix[i] = Spec::usual_mix()[i];
        }
        spec.mix = mix;
    }
    spec.edge_cases |= cli.edge_cases;
    spec.contacts_db &= !cli.no_contacts;
    spec.keep_plain |= cli.keep_plain;
    spec.check()?;
    Ok(spec)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.list_kinds {
        for kind in Kind::ALL {
            println!("{:<14} message_type {}", kind.name(), kind.code());
        }
        return ExitCode::SUCCESS;
    }
    if let Some(format) = &cli.export {
        return match write_export(format, cli.seed.unwrap_or(1), &cli.out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("oopz-gen: {e}");
                ExitCode::from(2)
            }
        };
    }
    let spec = match build(&cli) {
        Ok(spec) => spec,
        Err(e) => {
            eprintln!("oopz-gen: {e}");
            return ExitCode::from(2);
        }
    };
    let key = cli.key.clone().unwrap_or_else(|| oopz_gen::key_for(spec.seed));
    let mut err = std::io::stderr();
    let result = oopz_gen::generate(&spec, &cli.out, &key, &mut |done, total| {
        let _ = write!(err, "\r{done} / {total} messages");
        let _ = err.flush();
    });
    eprintln!();
    match result {
        Ok(r) => {
            println!(
                "{} messages in {} chats ({} people), {} media files, in {:.1} s",
                r.messages, r.chats, r.people, r.media_files, r.seconds
            );
            println!("backup:   {}", r.msgstore.display());
            if let Some(c) = &r.contacts {
                println!("contacts: {}", c.display());
            }
            println!("key:      {}", cli.out.join("key.txt").display());
            println!("manifest: {}", r.manifest.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("oopz-gen: {e:#}");
            ExitCode::FAILURE
        }
    }
}
