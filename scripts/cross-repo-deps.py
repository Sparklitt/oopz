#!/usr/bin/env -S python3 -I -B
# copy of sparklitt scripts/cross-repo-deps.py sha256 7e23b7eef5da8fdd060228323ffcba76a715f8f6b2f47595d4c335778c03cb9c
"""Crates from another of our repositories come one way: pinned, named once per workspace (decision 0047).

    python3 -I -B scripts/cross-repo-deps.py                  check every manifest (quick check)
    python3 -I -B scripts/cross-repo-deps.py --pins           also check each path source's checkout (full check)
    python3 -I -B scripts/cross-repo-deps.py --set <repo> [T] move a source to tag T, or a path source's pin to its
                                                              checkout's commit today
    python3 -I -B scripts/cross-repo-deps.py --dev on|off     build against the sibling checkouts (an uncommitted
                                                              [patch] in .cargo/config.toml), or stop
    python3 -I -B scripts/cross-repo-deps.py --self-test      check this checker against known answers (XR1-XR10)

The sources are the repository's, in `.claude/cross-repo.toml`; this checker is sparklitt's, copied whole
(`scripts/copies.py`). A source is a sibling repository (`../<repo>`) whose crates' names start with its prefix:

    [[source]]
    repo = "sparklitt"
    prefix = "sparklitt-"
    git = "ssh://git@github.com/Sparklitt/sparklitt.git"   # a git source: one URL, one release tag
    tag = '^sparklitt-0\\.1\\.0-pre\\.[1-9][0-9]*$'          # what a release tag looks like

    [[source]]
    repo = "silq"
    prefix = "silq-"
    pin = "<commit>"   # a path source (no remote yet): the sibling's commit this repository was checked against
    only = ["apps/pilot/"]   # optional, either kind: the only manifests that may name its crates

Rules. XR1: a source's crate is spelled out only in a workspace's root manifest; every other manifest takes it from
its workspace (`workspace = true`). XR2: from a git source, `{ git = <its URL>, tag = <a release tag> }`, never a
branch, rev or path. XR3: one tag per source across the repository (Cargo builds each tag as its own source: two
copies of a shared crate whose types don't match). XR4: from a path source, a path into `../<repo>`. XR5:
`.cargo/config.toml` (where --dev writes) is never tracked. XR6 (--pins): a path source's checkout is at its pin
with no uncommitted change to a tracked file. XR7: a git source's checkout with a newer release tag is said, not failed.
XR10: a source with `only` is named in no manifest outside those paths.
"""
import re
import subprocess
import sys
import tomllib
from os.path import normpath, relpath
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONF = Path(".claude/cross-repo.toml")
CARGO_CONFIG = Path(".cargo/config.toml")
DEP_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")
# Keys a dependency may add to where it comes from.
EXTRA_KEYS = {"features", "default-features", "optional", "package", "version"}
DEV_BEGIN, DEV_END = "# cross-repo-deps --dev: begin", "# cross-repo-deps --dev: end"
SHA = re.compile(r"^[0-9a-f]{40}$")


def sources(text):
    """The configuration's sources, each checked: (sources, [what is wrong])."""
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError as e:
        return [], [f"{CONF}: not TOML ({e})"]
    found, wrong = data.pop("source", []), [f"{CONF}: unknown table {k}" for k in sorted(data)]
    for i, s in enumerate(found, 1):
        keys = set(s) - {"only"}
        if keys not in ({"repo", "prefix", "git", "tag"}, {"repo", "prefix", "pin"}):
            wrong.append(f"{CONF} source {i}: needs repo and prefix, then git and tag, or pin (and may add only)")
        elif "only" in s and (not isinstance(s["only"], list) or not s["only"]
                              or not all(isinstance(o, str) and o for o in s["only"])):
            wrong.append(f"{CONF} source {i}: only is a list of paths")
        elif "pin" in s and not SHA.match(s["pin"]):
            wrong.append(f"{CONF} source {i}: pin isn't a full commit id")
        elif "tag" in s:
            try:
                re.compile(s["tag"])
            except re.error:
                wrong.append(f"{CONF} source {i}: tag isn't a regular expression")
    return found, wrong


def dependencies(manifest):
    """(table, name, spec) of every dependency of a parsed manifest, the crate's own name for a renamed one."""
    tables = [(t, manifest.get(t, {})) for t in DEP_TABLES]
    tables += [(f"target.{k}.{t}", v.get(t, {})) for k, v in manifest.get("target", {}).items() for t in DEP_TABLES]
    tables.append(("workspace.dependencies", manifest.get("workspace", {}).get("dependencies", {})))
    for table, deps in tables:
        for key, spec in deps.items():
            spec = spec if isinstance(spec, dict) else {"version": spec}
            yield table, spec.get("package", key), spec


def problems(manifests, srcs):
    """[what is wrong] for {path relative to ROOT: manifest text} against the sources; and {repo: tags used}."""
    out, tags = [], {s["repo"]: set() for s in srcs}
    for path, text in sorted(manifests.items()):
        try:
            manifest = tomllib.loads(text)
        except tomllib.TOMLDecodeError:
            out.append(f"{path}: not TOML")
            continue
        root = "workspace" in manifest
        for table, name, spec in dependencies(manifest):
            src = next((s for s in srcs if name.startswith(s["prefix"])), None)
            if not src:
                continue
            where = f"{path} [{table}] {name}"
            if "only" in src and not any(path.startswith(o) for o in src["only"]):
                out.append(f"{where}: {src['repo']}'s crates only under {', '.join(src['only'])} (XR10)")
                continue
            if spec.get("workspace") is True:
                if set(spec) - EXTRA_KEYS - {"workspace"}:
                    out.append(f"{where}: `workspace = true` adds only {', '.join(sorted(EXTRA_KEYS))}")
                continue
            if not root:
                out.append(f"{where}: spelled out outside a workspace root: `workspace = true` (XR1)")
                continue
            if "git" in src:
                if set(spec) - EXTRA_KEYS != {"git", "tag"} or spec["git"] != src["git"]:
                    out.append(f"{where}: needs {{ git = \"{src['git']}\", tag = ... }}, no branch, rev or path (XR2)")
                elif not re.search(src["tag"], spec["tag"]):
                    out.append(f"{where}: {spec['tag']} isn't a release tag of {src['repo']} (XR2)")
                else:
                    tags[src["repo"]].add(spec["tag"])
            else:
                into = Path(normpath(Path(path).parent / spec.get("path", "\0")))
                if set(spec) - EXTRA_KEYS != {"path"} or not into.is_relative_to(Path("..") / src["repo"]):
                    out.append(f"{where}: needs {{ path = \"<into ../{src['repo']}>\" }} (XR4)")
    out += [f"{repo}: crates from {len(t)} tags ({', '.join(sorted(t))}): use one (XR3)"
            for repo, t in sorted(tags.items()) if len(t) > 1]
    return out, tags


def manifests(root):
    """{path relative to root: text} of every Cargo.toml, past target folders and other worktrees."""
    found = [p.relative_to(root) for p in root.glob("**/Cargo.toml")]
    return {str(r): (root / r).read_text(encoding="utf-8") for r in found
            if not {"target", ".claude", ".git"} & set(r.parts)}


def git(*args, repo=ROOT):
    done = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True, check=False)
    return done.stdout.strip() if done.returncode == 0 else None


def sibling(src):
    """The source's checkout beside this repository's main checkout (found from a linked worktree too)."""
    common = git("rev-parse", "--path-format=absolute", "--git-common-dir")
    return (Path(common).parent.parent if common else ROOT.parent) / src["repo"]


def pin_problems(srcs, tags, run=git):
    """XR6 and XR7's newer-tag notes, `run` standing for git: ([what is wrong], [notes])."""
    out, notes = [], []
    for s in srcs:
        there = sibling(s)
        if "pin" in s:
            head = run("rev-parse", "HEAD", repo=there)
            if head is None:
                out.append(f"{s['repo']}: no checkout at ../{s['repo']} (a path source builds from it) (XR6)")
            elif head != s["pin"]:
                out.append(f"{s['repo']}: ../{s['repo']} at {head[:12]}, pinned {s['pin'][:12]}: check against it, "
                           f"then `--set {s['repo']}` (XR6)")
            elif run("status", "--porcelain", "--untracked-files=no", repo=there):
                out.append(f"{s['repo']}: ../{s['repo']} has uncommitted changes (XR6)")
        elif (listed := run("tag", "--sort=-creatordate", repo=there)) is not None:
            newest = next((t for t in listed.splitlines() if re.search(s["tag"], t)), None)
            if newest and tags.get(s["repo"]) and newest not in tags[s["repo"]]:
                notes.append(f"{s['repo']}: using {', '.join(sorted(tags[s['repo']]))}; newest release {newest}")
    return out, notes


def config_tracked(listed):
    """XR5: whether `git ls-files .cargo/config.toml` listed it."""
    return bool(listed)


def set_tag(text, src, tag):
    """The manifest's text with every dependency on `src` at `tag` (a one-line `{ git = …, tag = … }`)."""
    line = re.compile(rf'^(\s*"?{re.escape(src["prefix"])}[^=]*=.*git\s*=\s*"{re.escape(src["git"])}".*tag\s*=\s*")'
                      r'[^"]*(".*)$')
    return "\n".join(line.sub(rf"\g<1>{tag}\g<2>", row) for row in text.split("\n"))


def dev_section(srcs):
    """The [patch] tables pointing each git source's crates at its sibling checkout (crates/*/Cargo.toml)."""
    rows = [DEV_BEGIN]
    for s in srcs:
        if "git" not in s:
            continue
        crates = sorted(sibling(s).glob("crates/*/Cargo.toml"))
        rows.append(f'[patch."{s["git"]}"]')
        for c in crates:
            name = tomllib.loads(c.read_text(encoding="utf-8")).get("package", {}).get("name")
            if name and name.startswith(s["prefix"]):
                rows.append(f'{name} = {{ path = "{relpath(c.parent, ROOT)}" }}')
    rows.append(DEV_END)
    return "\n".join(rows) + "\n"


def without_dev(text):
    """The text without the section --dev wrote; everything else kept."""
    return re.sub(rf"{re.escape(DEV_BEGIN)}\n.*?{re.escape(DEV_END)}\n?", "", text, flags=re.S)


def self_test():
    url = "ssh://git@example.invalid/a.git"
    srcs = [{"repo": "a", "prefix": "a-", "git": url, "tag": r"^a-0\.1\.0-pre\.[1-9][0-9]*$"},
            {"repo": "s", "prefix": "s-", "pin": "0" * 40}]
    root = (f'[workspace]\n[workspace.dependencies]\na-x = {{ git = "{url}", tag = "a-0.1.0-pre.2" }}\n'
            'a-y = { git = "' + url + '", tag = "a-0.1.0-pre.2", features = ["f"] }\n')
    pilot = (f'[package]\nname = "p"\n[workspace]\n[dependencies]\na-x = {{ git = "{url}", tag = "a-0.1.0-pre.2" }}\n'
             's-core = { path = "../../../s/crates/s-core" }\n')
    member = '[package]\nname = "m"\n[dependencies]\na-x.workspace = true\na-y = { workspace = true, optional = true }\n'
    n = lambda found: len(found[0])  # noqa: E731

    def fake(head=None, status="", tags=None):
        return lambda *a, repo: {"rev-parse": head, "status": status, "tag": tags}[a[0]]
    cases = [
        ("XR1-XR4 a good repository", n(problems({"Cargo.toml": root, "c/m/Cargo.toml": member,
                                                  "apps/p/Cargo.toml": pilot}, srcs)), 0),
        ("XR1 spelled out in a member", n(problems({"c/m/Cargo.toml": member.replace(
            "a-x.workspace = true", f'a-x = {{ git = "{url}", tag = "a-0.1.0-pre.2" }}')}, srcs)), 1),
        ("XR1 a renamed crate is still the source's", n(problems({"c/m/Cargo.toml": '[dependencies]\nz = { package = '
                                                                  '"a-x", path = "../a" }\n'}, srcs)), 1),
        ("XR2 a branch, a path, another URL", n(problems({"Cargo.toml": '[workspace]\n[workspace.dependencies]\n'
                                                          f'a-x = {{ git = "{url}", branch = "main" }}\n'
                                                          'a-y = { path = "../a/crates/a-y" }\n'
                                                          'a-z = { git = "https://example.invalid/b", tag = '
                                                          '"a-0.1.0-pre.2" }\n'}, srcs)), 3),
        ("XR2 not a release tag", n(problems({"Cargo.toml": root.replace("pre.2", "pre.0", 1)}, srcs)), 1),
        ("XR3 two tags", n(problems({"Cargo.toml": root, "apps/p/Cargo.toml": pilot.replace("pre.2", "pre.3")}, srcs)),
         1),
        ("XR4 a path outside ../s", n(problems({"apps/p/Cargo.toml": pilot.replace("../../../s/", "../../s/")}, srcs)),
         1),
        ("XR4 a path into a lookalike ../s2", n(problems({"apps/p/Cargo.toml": pilot.replace(
            "../../../s/crates", "../../../s2/crates")}, srcs)), 1),
        ("XR10 a source named outside its only paths; inside", [n(problems({path: pilot}, [srcs[0], {**srcs[1], "only": [
            "apps/"]}])) for path in ("Cargo.toml", "apps/p/Cargo.toml")], [1, 0]),
        ("XR5 a tracked .cargo/config.toml", [config_tracked("x/.cargo/config.toml"), config_tracked("")],
         [True, False]),
        ("XR6 at the pin; moved; dirty; no checkout", [len(pin_problems(srcs[1:], {}, fake(h, ""))[0])
                                                       for h in ("0" * 40, "1" * 40, None)] +
         [len(pin_problems(srcs[1:], {}, fake("0" * 40, " M x"))[0])], [0, 1, 1, 1]),
        ("XR7 a newer release tag is a note", pin_problems(srcs[:1], {"a": {"a-0.1.0-pre.2"}},
                                                           fake(tags="a-0.1.0-pre.3\nother\n"))[1],
         ["a: using a-0.1.0-pre.2; newest release a-0.1.0-pre.3"]),
        ("XR8 set_tag moves every tag, nothing else", set_tag(root + 'b = "1"\n', srcs[0], "a-0.1.0-pre.9").count(
            "pre.9"), 2),
        ("XR9 the --dev section comes out whole, nothing else", without_dev(f"[x]\n{DEV_BEGIN}\n[patch.y]\nq = 1\n{DEV_END}\n[z]\n"),
         "[x]\n[z]\n"),
        ("the configuration: good, a source with both kinds, a short pin",
         [len(sources('[[source]]\nrepo = "a"\nprefix = "a-"\npin = "' + "0" * 40 + '"\n')[1]),
          len(sources('[[source]]\nrepo = "a"\nprefix = "a-"\npin = "x"\ngit = "u"\ntag = "t"\n')[1]),
          len(sources('[[source]]\nrepo = "a"\nprefix = "a-"\npin = "abc"\n')[1])], [0, 1, 1]),
    ]
    bad = [f"{name}: got {got!r}, want {want!r}" for name, got, want in cases if got != want]
    for line in bad:
        print(f"cross-repo-deps self-test failed: {line}")
    if not bad:
        print("cross-repo-deps self-test: ok")
    return 1 if bad else 0


def main(argv):
    if argv == ["--self-test"]:
        return self_test()
    if not (ROOT / CONF).exists():
        print(f"{CONF}: missing (the repositories this one takes crates from, decision 0047)")
        return 1
    srcs, wrong = sources((ROOT / CONF).read_text(encoding="utf-8"))
    for w in wrong:
        print(w)
    if wrong:
        return 1
    if argv[:1] == ["--set"] and len(argv) in (2, 3):
        src = next((s for s in srcs if s["repo"] == argv[1]), None)
        if src is None:
            print(f"{argv[1]}: not a source in {CONF}")
            return 1
        if "pin" in src:
            head = argv[2] if len(argv) == 3 else git("rev-parse", "HEAD", repo=sibling(src))
            if not head or not SHA.match(head):
                print(f"{argv[1]}: no full commit id to pin")
                return 1
            conf = (ROOT / CONF).read_text(encoding="utf-8")
            (ROOT / CONF).write_text(conf.replace(src["pin"], head), encoding="utf-8")
            return 0
        if len(argv) != 3 or not re.search(src["tag"], argv[2]):
            print(f"{argv[1]}: give a release tag")
            return 1
        for path, text in manifests(ROOT).items():
            moved = set_tag(text, src, argv[2])
            if moved != text:
                (ROOT / path).write_text(moved, encoding="utf-8")
        return 0
    if argv[:1] == ["--dev"] and argv[1:] in (["on"], ["off"]):
        path = ROOT / CARGO_CONFIG
        kept = without_dev(path.read_text(encoding="utf-8")) if path.exists() else ""
        text = kept + (dev_section(srcs) if argv[1] == "on" else "")
        if text:
            path.parent.mkdir(exist_ok=True)
            path.write_text(text, encoding="utf-8")
        elif path.exists():
            path.unlink()
        return 0
    if argv not in ([], ["--pins"]):
        print(__doc__.split("\n\n")[1])
        return 2
    found, tags = problems(manifests(ROOT), srcs)
    if config_tracked(git("ls-files", str(CARGO_CONFIG))):
        found.append(f"{CARGO_CONFIG} is tracked: local [patch] work stays uncommitted (XR5)")
    if argv == ["--pins"]:
        more, notes = pin_problems(srcs, tags)
        found += more
        for note in notes:
            print(f"note: {note}")
    for p in found:
        print(f"FAIL: {p}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
