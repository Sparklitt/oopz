"""The literal scan: every fixed value has one home (claude-setup README, "Literal scan").

    python3 -I -B scripts/literals [dir]              scan dir (default: here); exit 1 on a problem, 2 on bad config
    python3 -I -B scripts/literals --baseline [dir]   record today's hits per file and rule (turning it on)
    python3 -I -B scripts/literals --self-test        check this scanner against known answers

Rules, on the string and number literals of Rust, Kotlin, Java, JS/TS, Python, Go, shell and SQL files (comments,
docstrings, attributes' texts and tests left out; tests by each language's convention, TESTS below):
  prose   a string with a letter and a space (words a person reads) outside the catalogue files
  number  a number other than the allowed ones (default 0 1 2; -1 is - and 1) outside a named constant's declaration
          (Rust const/static, Kotlin const val or an UPPER_CASE val at the top or in an object, Java static final,
          JS/TS top-level const UPPER_CASE, Python module-level UPPER_CASE =, Go const, shell readonly or
          UPPER_CASE=) and outside the constants files
  sql     an SQL-looking string (upper-case keywords) with a value spliced in: interpolated, formatted or joined with
          +. Values are bound parameters; only constants (UPPER_CASE names, qualified or not) may be spliced in.
Config: <dir>/.claude/literals.conf (no file, no scan), one `key = value` per line:
  source = <glob>         the files to scan (default: every file of a known language); ** spans folders
  test = <glob>           more test files, besides each language's convention
  constants = <glob>      files where numbers may stand anywhere
  constants_only = true   numbers only in the constants files, not in named constants elsewhere
  catalogue = <glob>      files where prose lives
  numbers = 0 1 2         the numbers allowed anywhere
  allow = <glob> | <literal or *> | <reason>     an entry that allows nothing is stale and fails
Baseline: <dir>/.claude/literals-baseline.txt, `<count> <rule> <file>` per line, for a project that turns the scan on
with hits. A count may only fall: more hits than the line fails, and so do fewer (lower the line).
Prints positions and the literal (cut to SHOWN characters), never any other file content.

A package run as a directory: Python puts this directory first on sys.path, even under -I, so its modules import
each other by name (analyzer/test.sh runs it from elsewhere to prove it). -B: no __pycache__ in the project (Python
caches __main__ itself before this code runs; the line below covers the other modules when -B is left out).
"""

import sys
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ beside the project's scripts: set before the imports below

from baseline import write_baseline
from config import BASELINE, CONF, HINTS, NUMBERS, files, glob_re, judge, read_baseline, read_conf
from rules import file_hits, number_value
from tokens import LANGS

# Each language's convention for test code, never scanned. Rust's #[cfg(test)] and #[test] items too.
TESTS = {
    "rust": ["**/tests/**", "**/benches/**"],
    "kotlin": ["**/src/test/**", "**/src/*Test/**", "**/*Test.kt", "**/*Tests.kt"],
    "java": ["**/src/test/**", "**/src/*Test/**", "**/*Test.java", "**/*Tests.java"],
    "js": ["**/*.test.*", "**/*.spec.*", "**/__tests__/**", "**/test/**", "**/tests/**", "**/e2e/**"],
    "python": ["**/test_*.py", "**/*_test.py", "**/conftest.py", "**/test/**", "**/tests/**"],
    "go": ["**/*_test.go", "**/testdata/**"],
    "shell": ["**/*_test.sh", "**/*.bats", "**/test/**", "**/tests/**"],
    "sql": [],
}
TEST_RES = {lang: [glob_re(g) for g in globs] for lang, globs in TESTS.items()}


def scan(root, conf):
    """Every hit in the project: (file, line, col, rule, literal)."""
    allowed = set(conf["numbers"] or [number_value(v) for v in NUMBERS.split()])
    found = []
    for rel in files(root):
        lang = LANGS.get(Path(rel).suffix)
        if lang is None or conf["source"] and not any(g.fullmatch(rel) for g in conf["source"]):
            continue
        if any(g.fullmatch(rel) for g in TEST_RES[lang] + conf["test"]):
            continue
        src = (root / rel).read_bytes().decode("utf-8", "replace")
        hits = file_hits(src, lang, not any(g.fullmatch(rel) for g in conf["catalogue"]),
                         not any(g.fullmatch(rel) for g in conf["constants"]), allowed, conf["constants_only"])
        found += [(rel, *hit) for hit in hits]
    return found


def main(argv):
    if argv == ["--self-test"]:
        from selftest import self_test
        return self_test()
    write = argv[:1] == ["--baseline"]
    rest = argv[1:] if write else argv
    if len(rest) > 1 or rest and rest[0].startswith("-"):
        print(__doc__.split("\n\n")[1])
        return 2
    root = Path(rest[0] if rest else ".")
    if not (root / CONF).is_file():
        print(f"no {CONF}: nothing to scan", file=sys.stderr)
        return 0
    try:
        conf = read_conf((root / CONF).read_text(encoding="utf-8"))
        base = {} if write or not (root / BASELINE).is_file() else read_baseline(
            (root / BASELINE).read_text(encoding="utf-8"))
    except ValueError as e:
        print(e)
        return 2
    problems, counts = judge(scan(root, conf), conf["allow"], base)
    if write:
        header = ("Literal scan baseline (literals --baseline): hits per file and rule when it was turned on."
                  " Counts only fall.")
        print(f"{BASELINE}: {write_baseline(root / BASELINE, counts, header)}")
        problems = [p for p in problems if p.startswith(CONF)]
    for p in problems:
        print(p)
    risen = sorted({rule for (rel, rule), n in counts.items() if n > base.get((rel, rule), 0)} if not write else ())
    for rule in risen:
        print(f"  {rule}: {HINTS[rule]}")
    if risen:
        print(f"  or: `allow = <glob> | <literal> | <reason>` in {CONF}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
