//! What every test runs the binary with: the language pinned, so a developer's environment can't change what a test
//! reads (docs/messages.md, the command's language, row 12).

use std::process::Command;

/// The variables that pick the language, besides `LC_ALL`: each cleared.
pub const LANGUAGE_VARIABLES: [&str; 4] = ["OOPZ_LANG", "LANGUAGE", "LC_MESSAGES", "LANG"];

/// The `oopz` binary in `locale` (`C`: English), every other language source cleared.
pub fn oopz_in(locale: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oopz"));
    command.env("LC_ALL", locale);
    for variable in LANGUAGE_VARIABLES {
        command.env_remove(variable);
    }
    command
}
