// Fails unless every import note an adapter writes (a string pushed to `notes` in crates/adapters/* and
// crates/oopz-app/src/adapter.rs) is said again by every language but English (its `notes` in src/lib/i18n/<code>.ts):
// each `{…}` becomes a sample value and the sentence must come back changed.
import { readFileSync, readdirSync } from "node:fs";
// The locale files import each other with their .ts extension, so Node loads them as they are.
import { LOCALES, noteText } from "../src/lib/i18n/index.ts";

const langs = Object.keys(LOCALES).filter((l) => l !== "en");
const root = new URL("../../../crates/", import.meta.url);
const files = [new URL("oopz-app/src/adapter.rs", root)];
for (const a of readdirSync(new URL("adapters/", root))) {
  const dir = new URL(`adapters/${a}/src/`, root);
  for (const f of readdirSync(dir)) if (f.endsWith(".rs") && f !== "tests.rs") files.push(new URL(f, dir));
}
const missing = [];
let seen = 0;
for (const f of files) {
  const rust = readFileSync(f, "utf8");
  for (const m of rust.matchAll(/notes\.push\(\s*(?:format!\(\s*)?"((?:[^"\\]|\\.)*)"/g)) {
    seen++;
    const sample = m[1]
      .replace(/\\"/g, '"')
      .replace(/\{(\w*)\}/g, (_, name) => (name === "title" ? "Chat" : name === "code" || name === "name" ? "x" : "7"));
    for (const l of langs) if (noteText(sample, l) === sample) missing.push(`${l}: ${f.pathname.split("/crates/")[1]}: ${m[1]}`);
  }
}
if (missing.length) {
  console.error(`check-notes: notes a language doesn't say again (add a line to its notes in src/lib/i18n/<code>.ts):\n  ${missing.join("\n  ")}`);
  process.exit(1);
}
console.log(`check-notes: all ${seen} adapter notes are said again in ${langs.join(", ")}`);
