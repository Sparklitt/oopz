#!/usr/bin/env -S python3 -I -B
# copy of sparklitt scripts/build-budget.py sha256 4b10fc5d56e54a07550bf20b3184c15c2594953792c29cb897a88796e270b6d8
"""A Rust build stays small and fast: its crates, each compile's memory, a rebuild's time (decision 0046).

    python3 -I -B scripts/build-budget.py              measure every [[budget]] of .claude/build-budget.toml
    python3 -I -B scripts/build-budget.py --graph      the crate graphs only, no build (quick check)
    python3 -I -B scripts/build-budget.py --self-test  check this checker against known answers (BG1-BG12)

The numbers are the repository's, in `.claude/build-budget.toml`; this checker is sparklitt's, copied whole
(`scripts/copies.py`). A [[budget]] names a package and its limits:

    [[budget]]
    package = "app"                      # the graph's root and what is built (a member of the workspace)
    manifest = "Cargo.toml"              # optional: the workspace holding it
    build = ["--example", "r1"]          # optional: more `cargo build` arguments
    crate_ceiling = 450                  # crates in its normal and build graph (this host's platform)
    own = ["apps/app/"]                  # optional: the repository's own crates under these paths aren't counted
    dev = false                          # optional: the package's own dev-dependencies counted (an example builds them)
    forbidden = ["slint"]                # optional: packages it must never build, and name prefixes
    forbidden_prefixes = ["i-slint-"]
    peak_rss_mib = 2048                  # any one compile's largest process
    keep = false                         # keep the measuring folder between runs: faster, takes disk
    rebuilds = [{ touch = "src/lib.rs", budget_ms = 5000, peak_rss_mib = 512 }]  # the cap optional

Measured in a folder of its own (`<target>/build-budget-<package>`), each rustc run straight through this same file
as the rustc wrapper (no compile cache, so a compile is measured, not a cache hit): the build (through `cargo`, so
the machine's build slots apply), then each rebuild after touching its file (through the toolchain's own cargo, so
a slot's wait isn't timed; the file's time put back after), timed while this checker holds every build slot of the
machine's cargo shim, so no other compile shares the CPU (pitfall P10). With `keep`, a crate not rebuilt keeps the peak its last
build measured. Fails, naming the number and its budget, on a graph over its ceiling, a forbidden package, a compile
over the memory budget, a rebuild over its time, or a build that fails.
"""
import fcntl
import json
import os
import resource
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONF = Path(".claude/build-budget.toml")
# Set for the measured cargo only: this file then runs as its rustc wrapper and appends to the log it names.
LOG_ENV = "BUILD_BUDGET_LOG"
CRATE_FLAG = "--crate-name"
# What a measured folder is named under the target folder, and the peaks a kept folder remembers.
FOLDER_PREFIX, PEAKS = "build-budget-", "peaks.txt"
KIB = 1024
MIB_IN_KIB = 1024
NS_PER_S = 1e9
MS_PER_S = 1000
REQUIRED = {"package": str, "crate_ceiling": int, "peak_rss_mib": int, "rebuilds": list}
OPTIONAL = {"manifest": str, "build": list, "own": list, "forbidden": list, "forbidden_prefixes": list, "keep": bool,
            "dev": bool}
REBUILD_KEYS = {"touch": str, "budget_ms": int}
# A rebuild may also cap its own compiles' memory, below the budget's (an edit is lighter than a cold build).
REBUILD_OPTIONAL = {"peak_rss_mib": int}
# The machine's cargo shim (~/.local/bin/cargo) runs a heavy cargo only while it holds one of these flock files; the
# folder and count are the shim's own defaults, read from the same variables.
SLOTS_DIR = Path(os.environ.get("CARGO_SLOTS_DIR", "/tmp/cargo-slots"))
SLOTS = int(os.environ.get("CARGO_SLOTS", "2"))


def wrap(args):
    """The rustc wrapper: runs `args` (rustc and its arguments) and logs `<package>:<crate> <start> <end> <peak KiB>`
    (monotonic ns; the largest process rustc ran, never a sum). Cargo's probes (`rustc -vV`) name no crate."""
    crate = args[args.index(CRATE_FLAG) + 1] if CRATE_FLAG in args[:-1] else None
    start = time.monotonic_ns()
    # Cargo's jobserver reaches rustc through inherited descriptors.
    code = subprocess.call(args, close_fds=False)
    end = time.monotonic_ns()
    if crate:
        peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        with open(os.environ[LOG_ENV], "a", encoding="utf-8") as out:
            out.write(f"{os.environ.get('CARGO_PKG_NAME', '-')}:{crate} {start} {end} {peak}\n")
    return code


def budgets(text):
    """The [[budget]] tables of the configuration's text, each checked: (budgets, [what is wrong])."""
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError as e:
        return [], [f"{CONF}: not TOML ({e})"]
    found, wrong = data.pop("budget", None), [f"{CONF}: unknown table {k}" for k in sorted(data)]
    if not isinstance(found, list) or not found:
        return [], [*wrong, f"{CONF}: no [[budget]]"]
    for i, b in enumerate(found, 1):
        where = f"{CONF} budget {i}"
        wrong += [f"{where}: unknown key {k}" for k in sorted(set(b) - set(REQUIRED) - set(OPTIONAL))]
        wrong += [f"{where}: no {k}" for k in REQUIRED if k not in b]
        wrong += [f"{where}: {k} isn't a {t.__name__}" for k, t in {**REQUIRED, **OPTIONAL}.items()
                  if k in b and (type(b[k]) is not t)]
        for j, r in enumerate(b.get("rebuilds", []) if isinstance(b.get("rebuilds"), list) else [], 1):
            if not isinstance(r, dict) or not set(REBUILD_KEYS) <= set(r) <= set(REBUILD_KEYS) | set(REBUILD_OPTIONAL) \
                    or any(type(r[k]) is not t for k, t in {**REBUILD_KEYS, **REBUILD_OPTIONAL}.items() if k in r):
                wrong.append(f"{where} rebuild {j}: needs touch (a path) and budget_ms (a number), may add "
                             f"peak_rss_mib (a number)")
        for k in ("crate_ceiling", "peak_rss_mib"):
            if type(b.get(k)) is int and b[k] <= 0:
                wrong.append(f"{where}: {k} must be above 0")
    return found, wrong


def graph(metadata, package, own, dev=False):
    """The crates reachable from `package` through normal and build dependencies, two versions counted twice, less
    the repository's own under the `own` paths (relative to ROOT); None when `package` isn't a workspace member."""
    by_id = {p["id"]: p for p in metadata["packages"]}
    roots = [i for i in metadata["workspace_members"] if by_id[i]["name"] == package]
    if not roots:
        return None
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    seen, stack = set(), roots[:1]
    while stack:
        node = stack.pop()
        if node in seen:
            continue
        seen.add(node)
        kinds = {None, "build", "dev"} if dev and node == roots[0] else {None, "build"}
        stack += [d["pkg"] for d in nodes[node]["deps"] if {k["kind"] for k in d["dep_kinds"]} & kinds]

    def mine(i):
        path = Path(by_id[i]["manifest_path"])
        return path.is_relative_to(ROOT) and any(str(path.relative_to(ROOT)).startswith(o) for o in own)
    return sorted(by_id[i]["name"] for i in seen if not mine(i))


def forbidden(names, exact, prefixes):
    return sorted({n for n in names if n in exact or n.startswith(tuple(prefixes))})


def read_log(text):
    """(span in seconds, {package:crate: peak KiB}, compiles) of a wrapper log; malformed lines are skipped."""
    rows = [r for r in (line.split() for line in text.splitlines()) if len(r) == 4 and all(x.isdigit() for x in r[1:])]
    if not rows:
        return 0.0, {}, 0
    span = (max(int(r[2]) for r in rows) - min(int(r[1]) for r in rows)) / NS_PER_S
    peaks = {}
    for r in rows:
        peaks[r[0]] = max(peaks.get(r[0], 0), int(r[3]))
    return span, peaks, len(rows)


def verdicts(budget, crates, peaks, rebuilds):
    """[what is over] for `crates` (a count), `peaks` ({crate: KiB}) and `rebuilds` ([(touch, ms or None, largest
    KiB of its compiles)])."""
    out = []
    if crates > budget["crate_ceiling"]:
        out.append(f"{budget['package']}: {crates} crates, over its ceiling {budget['crate_ceiling']} (a decision)")
    limit = budget["peak_rss_mib"] * MIB_IN_KIB
    out += [f"{budget['package']}: compiling {c} peaked at {kib // MIB_IN_KIB} MiB, budget {budget['peak_rss_mib']} MiB"
            for c, kib in sorted(peaks.items()) if kib > limit]
    for (touch, ms, peak), want in zip(rebuilds, budget["rebuilds"]):
        if "peak_rss_mib" in want and peak > want["peak_rss_mib"] * MIB_IN_KIB:
            out.append(f"{budget['package']}: a compile after touching {touch} peaked at {peak // MIB_IN_KIB} MiB, "
                       f"budget {want['peak_rss_mib']} MiB")
        if ms is None:
            out.append(f"{budget['package']}: the rebuild after touching {touch} failed")
        elif ms > want["budget_ms"]:
            out.append(f"{budget['package']}: the rebuild after touching {touch} took {ms} ms, budget "
                       f"{want['budget_ms']} ms")
    return out


def toolchain_cargo():
    """The toolchain's own cargo (rustup's), past any wrapper on PATH; plain `cargo` without rustup."""
    found = subprocess.run(["rustup", "which", "cargo"], capture_output=True, text=True, check=False)
    return found.stdout.strip() if found.returncode == 0 and found.stdout.strip() else "cargo"


def host():
    out = subprocess.run(["rustc", "-vV"], capture_output=True, text=True, check=True).stdout
    return next(line.split(": ", 1)[1] for line in out.splitlines() if line.startswith("host: "))


def build(budget, folder, log, cargo):
    """One measured build; (succeeded, wall ms, log text)."""
    log.write_text("", encoding="utf-8")
    env = {**os.environ, "CARGO_TARGET_DIR": str(folder), "RUSTC_WRAPPER": str(ROOT / "scripts/build-budget.py"),
           LOG_ENV: str(log)}
    command = [cargo, "build", "-q", "--manifest-path", budget.get("manifest", "Cargo.toml"),
               "-p", budget["package"], *budget.get("build", [])]
    began = time.monotonic()
    done = subprocess.run(command, cwd=ROOT, env=env, check=False)
    return done.returncode == 0, round((time.monotonic() - began) * MS_PER_S), log.read_text(encoding="utf-8")


def take_slots(folder, count, wait=True):
    """Every build slot in `folder`, held exclusively (in order, as the shim tries them): their open files, closed to
    release; None when one is busy and not `wait`; [] on a machine without the shim's folder."""
    if not folder.is_dir():
        return []
    held = []
    for i in range(1, count + 1):
        handle = open(folder / f"slot{i}", "a", encoding="utf-8")  # noqa: SIM115 - held past this function
        try:
            fcntl.flock(handle, fcntl.LOCK_EX if wait else fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            handle.close()
            for h in held:
                h.close()
            return None
        held.append(handle)
    return held


def touched(budget, folder, log, path):
    """The rebuild after touching `path`, its time put back after, timed holding every build slot (P10);
    (wall ms or None when it failed, log text)."""
    before = (ROOT / path).stat()
    held = take_slots(SLOTS_DIR, SLOTS)
    os.utime(ROOT / path)
    try:
        ok, ms, text = build(budget, folder, log, toolchain_cargo())
    finally:
        os.utime(ROOT / path, ns=(before.st_atime_ns, before.st_mtime_ns))
        for handle in held:
            handle.close()
    return (ms if ok else None), text


def remembered(text):
    return {k: int(v) for k, v in (line.split() for line in text.splitlines() if len(line.split()) == 2) if v.isdigit()}


def measure(budget, crates):
    """Builds and rebuilds `budget`'s package; [what is over]."""
    folder = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / f"{FOLDER_PREFIX}{budget['package']}"
    if not budget.get("keep"):
        shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True, exist_ok=True)
    log, kept = folder / "compiles.log", folder / PEAKS
    try:
        ok, ms, text = build(budget, folder, log, "cargo")
        _, peaks, compiles = read_log(text)
        print(f"{budget['package']}: build {ms / MS_PER_S:.1f} s, {compiles} compiles (logged, not judged)")
        if not ok:
            return [f"{budget['package']}: the build failed"]
        if budget.get("keep") and kept.exists():
            peaks = {**remembered(kept.read_text(encoding="utf-8")), **peaks}
        rebuilds = []
        for r in budget["rebuilds"]:
            ms, text = touched(budget, folder, log, r["touch"])
            span, again, compiles = read_log(text)
            peaks.update(again)
            rebuilds.append((r["touch"], ms, max(again.values(), default=0)))
            print(f"{budget['package']}: rebuild after touching {r['touch']}: {ms} ms ({compiles} compiles over "
                  f"{span:.1f} s), budget {r['budget_ms']} ms")
        if budget.get("keep"):
            kept.write_text("".join(f"{c} {k}\n" for c, k in sorted(peaks.items())), encoding="utf-8")
        top = max(peaks.items(), key=lambda kv: kv[1], default=("-", 0))
        print(f"{budget['package']}: largest compile {top[0]} {top[1] // MIB_IN_KIB} MiB, budget "
              f"{budget['peak_rss_mib']} MiB")
        return verdicts(budget, crates, peaks, rebuilds)
    finally:
        if not budget.get("keep"):
            shutil.rmtree(folder, ignore_errors=True)


def check(only_graph):
    path = ROOT / CONF
    if not path.exists():
        return [f"{CONF}: missing (the repository's build budgets, decision 0046)"]
    found, wrong = budgets(path.read_text(encoding="utf-8"))
    for b in found if not wrong else []:
        wrong += [f"{CONF}: {b['package']}: {r['touch']} doesn't exist" for r in b["rebuilds"]
                  if not (ROOT / r["touch"]).is_file()]
    if wrong:
        return wrong
    failed = []
    for b in found:
        meta = subprocess.run(["cargo", "metadata", "-q", "--format-version", "1", "--manifest-path",
                               b.get("manifest", "Cargo.toml"), "--filter-platform", host()],
                              cwd=ROOT, capture_output=True, text=True, check=False)
        if meta.returncode:
            failed.append(f"{b['package']}: cargo metadata failed")
            continue
        names = graph(json.loads(meta.stdout), b["package"], b.get("own", []), b.get("dev", False))
        if names is None:
            failed.append(f"{b['package']}: not a member of {b.get('manifest', 'Cargo.toml')}'s workspace")
            continue
        print(f"{b['package']}: {len(names)} crates in its graph, ceiling {b['crate_ceiling']}")
        bad = forbidden(names, b.get("forbidden", []), b.get("forbidden_prefixes", []))
        if bad:
            failed.append(f"{b['package']}: builds {', '.join(bad)}, forbidden")
            continue
        if only_graph:
            failed += verdicts({**b, "rebuilds": []}, len(names), {}, [])
        else:
            failed += measure(b, len(names))
    return failed


def slot_answers():
    """BG12: a fake busy slot (another process's compile) refuses the hold; once freed, both slots are taken."""
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        # Another open file's lock conflicts as another process's would (flock locks belong to the open file).
        busy = open(folder / "slot2", "a", encoding="utf-8")  # noqa: SIM115 - the other compile's lock
        fcntl.flock(busy, fcntl.LOCK_EX)
        refused = take_slots(folder, 2, wait=False)
        busy.close()
        taken = take_slots(folder, 2, wait=False)
        answers = [refused, len(taken) if taken else taken, take_slots(folder / "none", 2)]
        for handle in taken or []:
            handle.close()
        return answers


def self_test():
    package = lambda i, path="/elsewhere": {"id": i, "name": i, "manifest_path": f"{path}/{i}/Cargo.toml"}  # noqa: E731
    dep = lambda pkg, kind: {"pkg": pkg, "dep_kinds": [{"kind": kind}]}  # noqa: E731
    meta = {"workspace_members": ["app", "fam"],
            "packages": [package("app", str(ROOT / "apps")), package("fam", str(ROOT / "apps")),
                         *map(package, ("core", "cc", "tempfile", "only-tests"))],
            "resolve": {"nodes": [{"id": "app", "deps": [dep("core", None), dep("cc", "build"), dep("tempfile", "dev"),
                                                         dep("fam", None)]},
                                  {"id": "fam", "deps": []}, {"id": "core", "deps": []}, {"id": "cc", "deps": []},
                                  {"id": "tempfile", "deps": [dep("only-tests", None)]},
                                  {"id": "only-tests", "deps": []}]}}
    good = {"package": "app", "crate_ceiling": 2, "peak_rss_mib": 1, "rebuilds": [{"touch": "a", "budget_ms": 10}]}
    cases = [
        ("BG1 normal and build dependencies, never dev", graph(meta, "app", []), ["app", "cc", "core", "fam"]),
        ("BG11 dev: the package's own dev-dependencies counted, with what they build",
         graph(meta, "app", [], dev=True), ["app", "cc", "core", "fam", "only-tests", "tempfile"]),
        ("BG2 the own crates aren't counted", graph(meta, "app", ["apps/"]), ["cc", "core"]),
        ("BG2 a package outside the workspace", graph(meta, "core", []), None),
        ("BG3 forbidden names and prefixes, not near names",
         forbidden(["slint", "i-slint-core", "slinte", "x"], ["slint"], ["i-slint-"]), ["i-slint-core", "slint"]),
        ("BG4 a log's span, largest peak per crate, compiles",
         read_log("p:a 1000000000 3000000000 2048\np:a 2000000000 6500000000 1024\nbad line\n"),
         (5.5, {"p:a": 2048}, 2)),
        ("BG4 an empty log", read_log(""), (0.0, {}, 0)),
        ("BG5 a valid configuration", budgets('[[budget]]\npackage = "app"\ncrate_ceiling = 2\npeak_rss_mib = 1\n'
                                             'rebuilds = [{ touch = "a", budget_ms = 10 }]\n')[1], []),
        ("BG5 an unknown key, a missing one, a wrong type, a bad rebuild",
         len(budgets('[[budget]]\npackage = 3\ncrate_ceilng = 2\npeak_rss_mib = 1\nrebuilds = [{ touch = "a" }]\n')[1]),
         4),
        ("BG5 no budget, not TOML", [len(budgets("")[1]), len(budgets("[[budget]\n")[1])], [1, 1]),
        ("BG6 at every budget passes", verdicts(good, 2, {"p:a": 1024}, [("a", 10, 9999)]), []),
        ("BG6 over each budget fails", len(verdicts(good, 3, {"p:a": 1025}, [("a", 11, 0)])), 3),
        ("BG7 a failed rebuild fails", len(verdicts(good, 1, {}, [("a", None, 0)])), 1),
        ("BG10 a rebuild's own memory cap: at it passes, over it fails",
         [len(verdicts({**good, "rebuilds": [{"touch": "a", "budget_ms": 10, "peak_rss_mib": 1}]}, 1, {}, [("a", 1, k)]))
          for k in (1024, 1025)], [0, 1]),
        ("BG10 a rebuild's cap is a number", len(budgets('[[budget]]\npackage = "a"\ncrate_ceiling = 1\npeak_rss_mib = 1\n'
                                                        'rebuilds = [{ touch = "a", budget_ms = 1, peak_rss_mib = "x" }]\n')[1]), 1),
        ("BG8 kept peaks: malformed lines skipped", remembered("p:a 5\nbad\np:b x\n"), {"p:a": 5}),
        ("BG9 the ceiling must be above 0", len(budgets('[[budget]]\npackage = "a"\ncrate_ceiling = 0\n'
                                                        'peak_rss_mib = 1\nrebuilds = []\n')[1]), 1),
        ("BG12 the slots: busy one refused, all taken once free, none without the folder", slot_answers(), [None, 2, []]),
    ]
    bad = [f"{name}: got {got!r}, want {want!r}" for name, got, want in cases if got != want]
    for line in bad:
        print(f"build-budget self-test failed: {line}")
    if not bad:
        print("build-budget self-test: ok")
    return 1 if bad else 0


def main(argv):
    if os.environ.get(LOG_ENV) and len(argv) > 1 and not argv[1].startswith("--"):
        return wrap(argv[1:])
    if argv[1:] == ["--self-test"]:
        return self_test()
    if argv[1:] not in ([], ["--graph"]):
        print(__doc__.split("\n\n")[1])
        return 2
    failed = check(argv[1:] == ["--graph"])
    for f in failed:
        print(f"FAIL: {f}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
