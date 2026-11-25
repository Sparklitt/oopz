#!/usr/bin/env python3
"""Prose check for AI tells (rule ids from pstack's unslop list, MIT, Copyright (c) 2026 Lauren Tan).
Exit 0 clean, 1 hits, 2 bad config.
"""
import re
import sys
from pathlib import Path

NAMES = {7: "ai vocabulary", 13: "dash", 19: "curly quote", 20: "chatbot phrase", 23: "filler", 31: "plain word"}
VOCAB = "additionally crucial delve enhance foster landscape pivotal showcase testament underscore vibrant"
PLAIN = "utilize utilise leverage"
PHRASES = {20: ["i hope this helps", "let me know if", "feel free to", "happy to help"],
           23: ["in order to", "it is important to note", "it['’]s worth noting"]}
DASH_CHARS = "—–"
CURLY = "‘’“”"
INLINE_CODE = re.compile(r"(`+).+?\1")
URL = re.compile(r"https?://\S+")
SPACED_HYPHEN = re.compile(r"(?<=\w) - (?=\w)")
FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
WORD = re.compile(r"[\w'.-]+\Z")
SCISSORS = "# ------------------------ >8 ------------------------"  # git drops this line and all below it


def forms(word):
    """Whole-word pattern for a word and its inflections (delve: delves, delved, delving)."""
    w = re.escape(word)
    if word.endswith("e"):
        return r"\b(?:" + w + r"[sd]?|" + re.escape(word[:-1]) + r"ing)\b"
    return r"\b(?:" + w + r"(?:s|ed)?|" + w + r"ing)\b"


def build_rules(extra_words=()):
    """(rule id, compiled regex) pairs; the dash rule is handled apart because of its table exemption."""
    rules = [(7, re.compile("|".join(forms(w) for w in VOCAB.split() + list(extra_words)), re.I)),
             (31, re.compile("|".join(forms(w) for w in PLAIN.split()), re.I)),
             (19, re.compile("[" + CURLY + "]"))]
    for rid, items in PHRASES.items():
        rules.append((rid, re.compile(r"\b(?:" + "|".join(items) + r")\b", re.I)))
    return rules


def mask(line):
    """The line with inline code and URLs blanked (columns stay put)."""
    for pat in (INLINE_CODE, URL):
        line = pat.sub(lambda m: " " * len(m.group()), line)
    return line


def scan_text(text, rules, markdown=True, comments=False):
    """Hits as (line, col, rule id), 1-based, sorted."""
    hits, fence = [], None
    lines = text.split("\n")
    start = 0
    if markdown and lines and lines[0].strip() == "---":
        end = next((i for i, l in enumerate(lines[1:], 1) if l.strip() == "---"), None)
        start = 0 if end is None else end + 1
    for n in range(start, len(lines)):
        raw = lines[n]
        if comments and raw == SCISSORS:
            break
        m = FENCE.match(raw) if markdown else None
        if m and fence is None:
            fence = (m.group(1)[0], len(m.group(1)))
            continue
        if m and fence and m.group(1)[0] == fence[0] and len(m.group(1)) >= fence[1] and not m.group(2).strip():
            fence = None
            continue
        if fence or (comments and raw.startswith("#")):
            continue
        line = mask(raw)
        for rid, rx in rules:
            hits += [(n + 1, m.start() + 1, rid) for m in rx.finditer(line)]
        hits += [(n + 1, i + 1, 13) for i, c in enumerate(line) if c in DASH_CHARS]
        if not line.lstrip().startswith("|"):
            hits += [(n + 1, m.start() + 2, 13) for m in SPACED_HYPHEN.finditer(line)]
    return sorted(set(hits))


def glob_regex(glob):
    out, i = "", 0
    while i < len(glob):
        if glob.startswith("**/", i):
            out, i = out + "(?:.*/)?", i + 3
        elif glob.startswith("**", i):
            out, i = out + ".*", i + 2
        elif glob[i] == "*":
            out, i = out + "[^/]*", i + 1
        elif glob[i] == "?":
            out, i = out + "[^/]", i + 1
        else:
            out, i = out + re.escape(glob[i]), i + 1
    return re.compile(out + r"\Z")


def parse_conf(path):
    """(sources, allows, words); allows are (line no, glob regex, rule id). ValueError names the line."""
    sources, allows, words = [], [], []
    for n, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        key, _, val = line.partition("=")
        key, val = key.strip(), val.strip()
        if key == "source":
            sources.append(glob_regex(val))
        elif key == "words":
            if not all(WORD.match(w) for w in val.split()):
                raise ValueError(f"{path.name}:{n}: words are letters, digits, ' . and - only")
            words += val.split()
        elif key == "allow":
            parts = [p.strip() for p in val.split("|", 2)]
            if len(parts) != 3 or not parts[2] or not parts[1].isdigit() or int(parts[1]) not in NAMES:
                raise ValueError(f"{path.name}:{n}: an allow entry is `glob | rule id | reason`")
            allows.append((n, glob_regex(parts[0]), int(parts[1])))
        else:
            raise ValueError(f"{path.name}:{n}: unknown key")
    return sources, allows, words


def project_files(root, sources):
    files = sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and ".git" not in p.parts)
    return [f for f in files if any(s.match(f) for s in sources)]


def run_project(root):
    conf = root / ".claude" / "unslop.conf"
    if not conf.is_file():
        return 0
    try:
        sources, allows, words = parse_conf(conf)
    except ValueError as e:
        print(e)
        return 2
    rules, used, out = build_rules(words), set(), []
    for rel in project_files(root, sources):
        text = (root / rel).read_text(encoding="utf-8", errors="replace")
        for ln, col, rid in scan_text(text, rules, rel.endswith(".md")):
            hit = [a for a in allows if a[2] == rid and a[1].match(rel)]
            used.update(a[0] for a in hit)
            if not hit:
                out.append(f"{rel}:{ln}:{col} rule {rid} {NAMES[rid]}")
    out += [f"unslop.conf:{a[0]}: stale allow entry" for a in allows if a[0] not in used]
    if out:
        print("\n".join(out))
    return 1 if out else 0


def run_message(path):
    conf, words = Path(".claude/unslop.conf"), []
    if conf.is_file():
        try:
            words = parse_conf(conf)[2]
        except ValueError as e:
            print(e)
            return 2
    text = Path(path).read_text(encoding="utf-8", errors="replace")
    out = [f"{path}:{ln}:{col} rule {rid} {NAMES[rid]}" for ln, col, rid in scan_text(text, build_rules(words), False, True)]
    if out:
        print("\n".join(out))
    return 1 if out else 0


# Known answers, written by hand from the rule list: (text, markdown, [(line, col, rule id)]).
CASES = [
    ("We delve into it.", True, [(1, 4, 7)]),
    ("It showcased and fostered; delving on.", True, [(1, 4, 7), (1, 18, 7), (1, 28, 7)]),
    ("An enhancement and a landscaper.", True, []),
    ("Use leverage, utilize it.", True, [(1, 5, 31), (1, 15, 31)]),
    ("a — b", True, [(1, 3, 13)]),
    ("1–2", True, [(1, 2, 13)]),
    ("one - two", True, [(1, 5, 13)]),
    ("- one\n| a - b |\nnot-a-dash", True, []),
    ("say “hi”", True, [(1, 5, 19), (1, 8, 19)]),
    ("I hope this helps. Feel free to ask.", True, [(1, 1, 20), (1, 20, 20)]),
    ("Do it in order to win; it's worth noting.", True, [(1, 7, 23), (1, 24, 23)]),
    ("`delve` and https://x.test/crucial-a — b", True, [(1, 38, 13)]),
    ("```\ndelve\n```\nok", True, []),
    ("---\ntitle: crucial\n---\nvibrant", True, [(4, 1, 7)]),
    ("delve", False, [(1, 1, 7)]),
    ("````\n```\n````\ndelve here", True, [(4, 1, 7)]),
    ("```\n~~~\ndelve\n```\nok", True, []),
    ("~~~\n```\ndelve\n~~~\nok", True, []),
]


def self_test():
    bad = 0
    for i, (text, md, want) in enumerate(CASES):
        got = scan_text(text, build_rules(), md)
        if got != want:
            bad += 1
            print(f"unslop self-test: case {i}: expected {want}, got {got}")
    if scan_text("# crucial\nfine", build_rules(), False, True):
        bad += 1
        print("unslop self-test: a # line in a message must be skipped")
    if scan_text("s\n\n" + SCISSORS + "\n+We delve here", build_rules(), False, True):
        bad += 1
        print("unslop self-test: nothing below the scissors line is scanned")
    if scan_text("moreover robust", build_rules(["robust"]), False) != [(1, 10, 7)]:
        bad += 1
        print("unslop self-test: words = must add a banned word")
    print("unslop self-test: ok" if not bad else f"unslop self-test: {bad} failed")
    return 1 if bad else 0


def main(argv):
    if argv[:1] == ["--self-test"]:
        return self_test()
    if argv[:1] == ["--message"] and len(argv) == 2:
        return run_message(argv[1])
    if len(argv) <= 1 and not (argv and argv[0].startswith("-")):
        return run_project(Path(argv[0] if argv else "."))
    print("usage: unslop.py [root] | --message FILE | --self-test")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
