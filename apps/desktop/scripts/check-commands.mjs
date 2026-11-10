// Fails unless every Rust command the app registers (generate_handler![…] in src-tauri/src/lib.rs) is called by
// name somewhere in src/lib/backend/tauri.ts, so a new command can't be left out of the UI's Backend.
import { readFileSync } from "node:fs";

const here = (path) => new URL(path, import.meta.url);
const rust = readFileSync(here("../src-tauri/src/lib.rs"), "utf8");
const ts = readFileSync(here("../src/lib/backend/tauri.ts"), "utf8");

const list = rust.match(/generate_handler!\[([\s\S]*?)\n\s*\]/); // to the closing ] on its own line, past #[cfg]
if (!list) {
  console.error("check-commands: no generate_handler![…] in src-tauri/src/lib.rs");
  process.exit(1);
}
const commands = list[1]
  .split("\n")
  .map((line) => line.replace(/\/\/.*$/, "").trim().replace(/,$/, ""))
  .filter((line) => line && !line.startsWith("#["))
  .map((path) => path.split("::").pop());

const missing = commands.filter((name) => !ts.includes(`"${name}"`));
if (missing.length) {
  console.error(`check-commands: not called from src/lib/backend/tauri.ts: ${missing.join(", ")}`);
  process.exit(1);
}
console.log(`check-commands: all ${commands.length} commands are in tauri.ts`);
