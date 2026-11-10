//! The app and the CLI stay in step (docs/parity.md): every desktop command and every CLI command is in the table.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).expect("the repo root")
}

/// The commands the desktop registers in `generate_handler![…]`, without their module paths.
fn app_commands() -> BTreeSet<String> {
    let lib = std::fs::read_to_string(root().join("apps/desktop/src-tauri/src/lib.rs")).expect("the desktop shell");
    let list = lib.split_once("generate_handler![").expect("a handler list").1;
    let list = list.split_once("])").expect("the list's end").0;
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("#["))
        .map(|l| l.trim_end_matches(',').rsplit("::").next().unwrap_or(l).to_owned())
        .collect()
}

/// The CLI's subcommands, from its own `--help`.
fn cli_commands() -> BTreeSet<String> {
    let help = Command::new(env!("CARGO_BIN_EXE_oopz")).arg("--help").output().expect("oopz --help");
    let help = String::from_utf8(help.stdout).expect("utf-8 help");
    let commands = help.split_once("Commands:").expect("a commands list").1;
    let commands = commands.split_once("Options:").map_or(commands, |(c, _)| c);
    commands
        .lines()
        .filter(|l| l.starts_with("  ") && !l.starts_with("   "))
        .filter_map(|l| l.split_whitespace().next())
        .filter(|c| *c != "help")
        .map(str::to_owned)
        .collect()
}

/// (app commands, CLI commands) named in the table's rows.
fn table() -> (BTreeSet<String>, BTreeSet<String>) {
    let doc = std::fs::read_to_string(root().join("docs/parity.md")).expect("docs/parity.md");
    let (mut app, mut cli) = (BTreeSet::new(), BTreeSet::new());
    for row in doc.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = row.split('|').collect();
        for (i, cell) in cells.iter().enumerate().take(3).skip(1) {
            for code in cell.split('`').skip(1).step_by(2) {
                match (i, code.strip_prefix("oopz ")) {
                    (1, None) => drop(app.insert(code.to_owned())),
                    (2, Some(cmd)) => drop(cli.insert(cmd.split_whitespace().next().unwrap_or(cmd).to_owned())),
                    _ => {}
                }
            }
        }
    }
    (app, cli)
}

#[test]
fn every_command_is_in_the_parity_table() {
    let (app_in_table, cli_in_table) = table();
    let app = app_commands();
    let cli = cli_commands();
    assert!(app.len() > 10 && cli.len() > 10, "read the lists: {app:?} {cli:?}");
    let missing: Vec<_> = app.difference(&app_in_table).collect();
    assert!(missing.is_empty(), "desktop commands not in docs/parity.md: {missing:?}");
    let missing: Vec<_> = cli.difference(&cli_in_table).collect();
    assert!(missing.is_empty(), "CLI commands not in docs/parity.md: {missing:?}");
    let stale: Vec<_> = app_in_table.difference(&app).chain(cli_in_table.difference(&cli)).collect();
    assert!(stale.is_empty(), "docs/parity.md names commands that no longer exist: {stale:?}");
}
