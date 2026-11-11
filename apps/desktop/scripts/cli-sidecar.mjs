// Builds the `oopz` CLI for the app to carry (bundle.externalBin in src-tauri/tauri.cli.conf.json), before
// `pnpm tauri build --config src-tauri/tauri.cli.conf.json`. Writes src-tauri/binaries/oopz-<target>[.exe] and, for
// Linux, the completions and the man page the .deb and .rpm install (binaries/share/).
//   pnpm sidecar                                  this computer's system
//   pnpm sidecar --target universal-apple-darwin  both Macs, joined with lipo (and each one, which tauri also wants)
//   pnpm sidecar --target x86_64-pc-windows-msvc --runner cargo-xwin   from Linux, as `tauri build --runner` does
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { gzipSync } from "node:zlib";

const arg = (name) => {
  const i = process.argv.indexOf(name);
  return i < 0 ? null : process.argv[i + 1];
};
const root = new URL("../../../", import.meta.url).pathname;
const out = new URL("../src-tauri/binaries/", import.meta.url).pathname;
const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (.+)$/m)[1];
const target = arg("--target") ?? host;
const runner = arg("--runner") ?? "cargo";
const exe = target.includes("windows") ? ".exe" : "";
const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: "inherit" });

/** Builds the CLI for one system; returns where cargo put it. */
function build(triple) {
  run(runner, ["build", "--locked", "--release", "-p", "oopz-cli", "--target", triple]);
  return `${root}target/${triple}/release/oopz${exe}`;
}

mkdirSync(out, { recursive: true });
let cli;
if (target === "universal-apple-darwin") {
  const both = ["aarch64-apple-darwin", "x86_64-apple-darwin"].map((t) => {
    const built = build(t);
    copyFileSync(built, `${out}oopz-${t}`);
    return built;
  });
  cli = `${out}oopz-${target}`;
  run("lipo", ["-create", "-output", cli, ...both]);
} else {
  cli = `${out}oopz-${target}${exe}`;
  copyFileSync(build(target), cli);
}
console.log(`cli-sidecar: ${cli}`);

if (target.includes("linux")) {
  const share = `${out}share/`;
  mkdirSync(share, { recursive: true });
  const say = (...args) => execFileSync(cli, args);
  writeFileSync(`${share}oopz.bash`, say("completions", "bash"));
  writeFileSync(`${share}_oopz`, say("completions", "zsh"));
  writeFileSync(`${share}oopz.fish`, say("completions", "fish"));
  writeFileSync(`${share}oopz.1.gz`, gzipSync(say("man"), { level: 9 }));
  console.log(`cli-sidecar: completions and man page in ${share}`);
}
