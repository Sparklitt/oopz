//! `data/messages.toml` → `$OUT_DIR/messages.rs` (`oopz_app::messages`), with msgkinds' generator and its checks (msgkinds'
//! docs/messages.md).

use std::{env, fs, path::Path};

fn main() {
    let path = "data/messages.toml";
    println!("cargo::rerun-if-changed={path}");
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let rust =
        msgkinds::build_support::messages::generate(&text).unwrap_or_else(|(line, e)| panic!("{path}:{line}: {e}"));
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("messages.rs"), rust).unwrap();
}
