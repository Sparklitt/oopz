"""The comment scan: the mechanical half of the house comment rule (claude-setup README, "Comment scan").

    python3 -I -B scripts/comments [dir]              scan dir (default: here); exit 1 on a problem, 2 on bad config
    python3 -I -B scripts/comments --baseline [dir]   record today's hits per file and rule (turning it on)
    python3 -I -B scripts/comments --self-test        check this scanner against known answers

Rules, on the comments of Rust, Kotlin, Java, JS/TS, Python, Go, shell and SQL files (the literal scan's lexer, so
a // inside a string is no comment):
  block       a plain comment block (not a doc comment) over max_lines lines; a licence header is exempt
  workaround  a comment saying why the root cause wasn't fixed: for now, workaround, in case, hack, quick fix, …
  todo        a TODO, FIXME or XXX whose line names no issue (#123, PROJ-12 or a link)
Config: <dir>/.claude/comments.conf (no file, no scan), one `key = value` per line:
  source = <glob>         the files to scan (default: every file of a known language); ** spans folders
  max_lines = 4           the longest plain comment block
  phrase = <words>        one more workaround phrase (letters, spaces, ' and -)
  allow = <glob> | <rule or *> | <reason>     an entry that allows nothing is stale and fails
Baseline: <dir>/.claude/comments-baseline.txt, `<count> <rule> <file>` per line, for a project that turns the scan on
with hits. A count may only fall: more hits than the line fails, and so do fewer (lower the line).
Prints positions, the rule and the block's length or the listed phrase, never the comment's text.

Needs scripts/literals beside it (claude-setup scaffold writes both): the lexer and the file list are its.
"""

import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ beside the project's scripts: set before the imports below
sys.path.insert(1, str(Path(__file__).resolve().parent.parent / "literals"))

from baseline import judge, read_baseline, write_baseline  # literals' modules, found through the line above
from config import files, glob_re
from scan import HINTS, MAX_LINES, PHRASES, comment_hits
from tokens import LANGS

CONF = ".claude/comments.conf"
BASELINE = ".claude/comments-baseline.txt"
KEYS = ("source", "max_lines", "phrase", "allow")
PHRASE = re.compile(r"[A-Za-z][A-Za-z' -]*\Z")


def read_conf(text):
    conf = {"source": [], "max_lines": MAX_LINES, "phrase": [], "allow": []}
    for number, raw in enumerate(text.splitlines(), 1):
        raw = raw.strip()
        if not raw or raw.startswith("#"):
            continue
        key, sep, value = (p.strip() for p in raw.partition("="))
        if not sep or key not in KEYS or not value:
            raise ValueError(f"{CONF}:{number}: expected `<key> = <value>`, the key one of {', '.join(KEYS)}")
        if key == "allow":
            parts = [p.strip() for p in value.split(" | ", 2)]
            if len(parts) != 3 or not all(parts) or parts[1] not in (*HINTS, "*"):
                raise ValueError(f"{CONF}:{number}: expected `allow = <glob> | <{' or '.join(HINTS)} or *> | <reason>`")
            conf[key].append((number, glob_re(parts[0]), parts[1]))
        elif key == "max_lines":
            if not value.isdigit() or int(value) < 1:
                raise ValueError(f"{CONF}:{number}: max_lines is a whole number of lines, at least 1")
            conf[key] = int(value)
        elif key == "phrase":
            if not PHRASE.match(value):
                raise ValueError(f"{CONF}:{number}: a phrase is letters, spaces, ' and - only")
            conf[key].append(value.lower())
        else:
            conf[key].append(glob_re(value))
    return conf


def scan(root, conf):
    """Every hit in the project: (file, line, col, rule, what)."""
    found = []
    for rel in files(root):
        lang = LANGS.get(Path(rel).suffix)
        if lang is None or conf["source"] and not any(g.fullmatch(rel) for g in conf["source"]):
            continue
        src = (root / rel).read_bytes().decode("utf-8", "replace")
        found += [(rel, *h) for h in comment_hits(src, lang, conf["max_lines"], PHRASES + tuple(conf["phrase"]))]
    return found


def show(rel, line, col, rule, what):
    return f"{rel}:{line}:{col}: {rule} " + (f"{what} lines" if rule == "block" else repr(what))


def main(argv):
    if argv == ["--self-test"]:
        from known import self_test
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
            (root / BASELINE).read_text(encoding="utf-8"), BASELINE, HINTS)
    except ValueError as e:
        print(e)
        return 2
    problems, counts = judge(scan(root, conf), conf["allow"], base, (CONF, BASELINE), show,
                             lambda value, hit: value in ("*", hit[3]))
    if write:
        header = ("Comment scan baseline (comments --baseline): hits per file and rule when it was turned on."
                  " Counts only fall.")
        print(f"{BASELINE}: {write_baseline(root / BASELINE, counts, header)}")
        problems = [p for p in problems if p.startswith(CONF)]
    for p in problems:
        print(p)
    risen = sorted({rule for (rel, rule), n in counts.items() if n > base.get((rel, rule), 0)} if not write else ())
    for rule in risen:
        print(f"  {rule}: {HINTS[rule]}")
    if risen:
        print(f"  or, when the comment is right: `allow = <glob> | <rule> | <reason>` in {CONF}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
