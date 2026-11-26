"""The project's side: its config, its baseline, which files to scan, and what is a problem."""

import re
import subprocess

import baseline
from rules import number_value

CONF = ".claude/literals.conf"
BASELINE = ".claude/literals-baseline.txt"
# A literal is printed up to this many characters: enough to find it, not a page of SQL.
SHOWN = 60
# The numbers allowed anywhere when the config names none (-1 is written - and 1).
NUMBERS = "0 1 2"
KEYS = ("source", "test", "constants", "constants_only", "catalogue", "numbers", "allow")
SWITCHES = {"true": True, "false": False}
# Folders never scanned when the project is no git repository (in one, what git ignores is skipped).
SKIP = {".git", "target", "node_modules", "build", "dist", ".gradle", "vendor", "__pycache__", ".venv"}
HINTS = {"prose": "words a person reads are message ids in the catalogue",
         "number": "a fixed value is a named, documented constant",
         "sql": "values are bound parameters; only constants (UPPER_CASE names) are spliced into SQL"}


def glob_re(glob):
    """A path glob as a regex: ** spans folders, * and ? stay inside one."""
    out, i = "", 0
    while i < len(glob):
        if glob.startswith("**/", i):
            out, i = out + "(?:.*/)?", i + 3
        elif glob.startswith("**", i):
            out, i = out + ".*", i + 2
        else:
            out, i = out + {"*": "[^/]*", "?": "[^/]"}.get(glob[i], re.escape(glob[i])), i + 1
    return re.compile(out)


def read_conf(text):
    conf = {key: [] for key in KEYS}
    conf["constants_only"] = False
    for number, raw in enumerate(text.splitlines(), 1):
        raw = raw.strip()
        if not raw or raw.startswith("#"):
            continue
        key, sep, value = (p.strip() for p in raw.partition("="))
        if not sep or key not in conf or not value:
            raise ValueError(f"{CONF}:{number}: expected `<key> = <value>`, the key one of {', '.join(KEYS)}")
        if key == "allow":
            parts = [p.strip() for p in value.split(" | ")]
            if len(parts) != 3 or not all(parts):
                raise ValueError(f"{CONF}:{number}: expected `allow = <glob> | <literal or *> | <reason>`")
            conf[key].append((number, glob_re(parts[0]), parts[1]))
        elif key == "numbers":
            values = [number_value(v) for v in value.split()]
            if None in values:
                raise ValueError(f"{CONF}:{number}: numbers are written as in code: `numbers = 0 1 2`")
            conf[key].extend(values)
        elif key == "constants_only":
            if value not in SWITCHES:
                raise ValueError(f"{CONF}:{number}: `constants_only = true` or `false`")
            conf[key] = SWITCHES[value]
        else:
            conf[key].append(glob_re(value))
    return conf


def read_baseline(text):
    return baseline.read_baseline(text, BASELINE, HINTS)


def show(rel, line, col, rule, literal):
    shown = literal if rule == "number" else repr(literal if len(literal) <= SHOWN else literal[:SHOWN] + "…")
    return f"{rel}:{line}:{col}: {rule} {shown}"


def judge(found, allows, base):
    """(problems, counts): hits beyond the baseline, stale allow entries and stale baseline lines."""
    return baseline.judge(found, allows, base, (CONF, BASELINE), show, lambda value, hit: value in ("*", hit[4]))


def files(root):
    """The project's files, relative to root: what git knows (ignored ones left out), else a walk."""
    try:
        out = subprocess.run(["git", "-C", str(root), "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
                             capture_output=True, check=True).stdout.decode("utf-8", "replace")
        return sorted(p for p in out.split("\0") if p and (root / p).is_file())
    except (OSError, subprocess.CalledProcessError):
        return sorted(p.relative_to(root).as_posix() for p in root.rglob("*")
                      if p.is_file() and not SKIP & set(p.relative_to(root).parts[:-1]))
