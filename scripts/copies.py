#!/usr/bin/env python3
# copy of sparklitt scripts/copies.py sha256 6b5fc3bd17a19bff391a0df914846747be7bb5a36a8af78e9c9b0a55bc71baf2
"""A script copied from another repository stays its source's copy (decision 0045, docs/checks.md §2.10).

    python3 -I -B scripts/copies.py                            check every copy in this repository
    python3 -I -B scripts/copies.py --copy <repo> <path> <to>  copy <repo>/<path> (a sibling checkout) to <to>
    python3 -I -B scripts/copies.py --self-test                check this checker against known answers (CP1–CP7)

A copy carries one header line, after a shebang or first: `# copy of <repo> <path> sha256 <hex>`, the source's
repository, its path there and the SHA-256 of the source file when it was copied. The check finds every tracked file
holding such a line and refuses a copy whose text, without that line, no longer hashes to the record (changed here:
change the source, then copy again), and, when the source's checkout sits beside this repository's main checkout
(`../<repo>`), a record that isn't the source's hash today (the copy is behind its source). A source checkout that
isn't there is said, not failed. Shared the way it shares: this file is copied whole, with its header.
"""

import hashlib
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEADER = re.compile(r"^# copy of (?P<repo>[a-z0-9][a-z0-9_.-]*) (?P<path>[^\s]+) sha256 (?P<hash>[0-9a-f]{64})$")
# A line that means to be a header: one not matching HEADER exactly is refused, never skipped.
HEADER_START = "# copy of "


def sha256(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def header(text):
    """(repo, path, hash, the text without the header line), or None for a file that isn't a copy (CP4, CP5)."""
    lines = text.splitlines(keepends=True)
    found = [i for i, line in enumerate(lines) if line.startswith(HEADER_START)]
    if not found:
        return None
    if len(found) > 1:
        raise ValueError(f"lines {found[0] + 1} and {found[1] + 1}: two copy headers")
    match = HEADER.match(lines[found[0]].rstrip("\r\n"))
    path = match.group("path") if match else ""
    if not match or path.startswith("/") or ".." in path.split("/"):
        raise ValueError(f"line {found[0] + 1}: not `# copy of <repo> <path> sha256 <64 hex>`")
    return match.group("repo"), path, match.group("hash"), "".join(lines[:found[0]] + lines[found[0] + 1:])


def problems(name, text, source):
    """What is wrong with the copy `name`, given its source's text today (None: no checkout) (CP1–CP3)."""
    try:
        found = header(text)
    except ValueError as error:
        return [f"{name}: {error}"]
    if found is None:
        return []
    repo, path, recorded, body = found
    out = []
    if sha256(body) != recorded:
        out.append(f"{name}: changed since it was copied from {repo} {path} (change it there, then copy again)")
    if source is not None and sha256(source) != recorded:
        out.append(f"{name}: copy is behind {repo} {path} (copy it again: scripts/copies.py --copy {repo} {path} {name})")
    return out


def with_header(source, repo, path):
    """The copy of `source`: its header after a shebang, or first (CP6)."""
    if header(source) is not None:
        raise ValueError("the source is itself a copy: copy from its own source")
    line = f"{HEADER_START}{repo} {path} sha256 {sha256(source)}\n"
    if not HEADER.match(line.rstrip("\n")):
        raise ValueError("a repository or a path that can't be written in a header")
    lines = source.splitlines(keepends=True)
    at = 1 if lines and lines[0].startswith("#!") else 0
    return "".join(lines[:at] + [line] + lines[at:])


def siblings():
    """The folder holding this repository's main checkout and its siblings (a linked worktree's too)."""
    common = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--path-format=absolute", "--git-common-dir"],
                            capture_output=True, text=True, check=False).stdout.strip()
    return Path(common).parent.parent if common else ROOT.parent


def read(path):
    return path.read_bytes().decode("utf-8")


def check():
    files = subprocess.run(["git", "-C", str(ROOT), "ls-files"], capture_output=True, text=True,
                           check=True).stdout.split("\n")
    base, out, unchecked = siblings(), [], set()
    for name in filter(None, files):
        path = ROOT / name
        if not path.is_file():
            continue
        try:
            text = read(path)
        except UnicodeDecodeError:
            continue  # not text: never a copied script
        try:
            found = header(text)
        except ValueError:
            found = None  # problems() reports it
        source = None
        if found and (base / found[0] / found[1]).is_file():
            source = read(base / found[0] / found[1])
        elif found:
            unchecked.add(found[0])
        out += problems(name, text, source)
    for repo in sorted(unchecked):
        print(f"copies: no checkout of {repo} beside this one: its copies checked against their records only")
    for problem in out:
        print(problem)
    return 1 if out else 0


def copy(repo, path, to):
    (ROOT / to).write_bytes(with_header(read(siblings() / repo / path), repo, path).encode("utf-8"))
    return 0


def self_test():
    source = "#!/usr/bin/env python3\nprint(1)\n"
    copied = with_header(source, "sparklitt", "scripts/a.py")
    record = sha256(source)

    def refused(call):
        try:
            call()
        except ValueError:
            return True
        return False

    cases = [
        ("CP1", problems("a.py", copied, None) == [] and copied.split("\n")[1].endswith(record)),
        ("CP2", "changed since it was copied" in " ".join(problems("a.py", copied + "x = 2\n", None))),
        ("CP3", problems("a.py", copied, source) == []
         and problems("a.py", copied, source + "y\n") == [
             "a.py: copy is behind sparklitt scripts/a.py (copy it again: scripts/copies.py --copy sparklitt "
             "scripts/a.py a.py)"]),
        ("CP4", all(len(found := problems("a.py", t, None)) == 1
                    and ("two copy headers" in found[0] or "not `# copy of" in found[0]) for t in (
            copied + copied.split("\n")[1] + "\n", f"# copy of sparklitt scripts/a.py sha256 {record[:10]}\n",
            f"# copy of sparklitt sha256 {record}\n", f"# copy of sparklitt ../x.py sha256 {record}\n",
            f"# copy of sparklitt /x.py sha256 {record}\n"))),
        ("CP5", header(source) is None and problems("a.py", source, "other") == []),
        ("CP6", with_header("print(1)\n", "r", "p.ps1").startswith("# copy of r p.ps1 sha256 ")
         and copied.startswith("#!/usr/bin/env python3\n# copy of sparklitt scripts/a.py sha256 ")
         and refused(lambda: with_header(copied, "x", "y")) and refused(lambda: with_header(source, "Bad Repo", "p"))),
        ("CP7", problems("a.ps1", with_header("a\r\nb\r\n", "r", "a.ps1"), "a\r\nb\r\n") == []),
    ]
    failed = [label for label, ok in cases if not ok]
    for label in failed:
        print(f"self-test {label}: wrong answer")
    return 1 if failed else 0


if __name__ == "__main__":
    args = sys.argv[1:]
    if args == ["--self-test"]:
        sys.exit(self_test())
    if len(args) == 4 and args[0] == "--copy":
        sys.exit(copy(*args[1:]))
    if not args:
        sys.exit(check())
    print(__doc__.split("\n\n")[1], file=sys.stderr)
    sys.exit(2)
