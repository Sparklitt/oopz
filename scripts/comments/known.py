"""Known answers for the comment scan, written by hand from the rules (README, "Comment scan", edge cases C1-C26)."""

from scan import comment_hits

FIVE = "".join(f"// {n}\n" for n in "abcde")
# (edge case id, source, language, extra arguments, [(line, col, rule, what)])
CASES = [
    ("C1", 'let a = "// not a comment"; // for now\n', "rust", {}, [(1, 32, "workaround", "for now")]),
    ("C2", FIVE + "fn f() {}\n", "rust", {}, [(1, 1, "block", "5")]),
    ("C3", "// a\n// b\n// c\n// d\nfn f() {}\n", "rust", {}, []),
    ("C4", "".join(f"/// {n}\n" for n in "abcdef") + "fn f() {}\n", "rust", {}, []),
    ("C4", "/**\n * a\n * b\n * c\n * d\n */\nfunction f() {}\n", "js", {}, []),
    ("C5", "".join(f"//// {n}\n" for n in "abcde"), "rust", {}, [(1, 1, "block", "5")]),
    ("C6", "x = 1;\n/* a\n b\n c\n d\n e */\n", "java", {}, [(2, 1, "block", "5")]),
    ("C7", "".join(f"a{n} = 1; // {n}\n" for n in "abcde"), "js", {}, []),
    ("C8", "// a\n// b\n// c\n\n// d\n// e\n// f\n", "go", {}, []),
    ("C9", "// Copyright 2026 Example\n// a\n// b\n// c\n// d\nfn f() {}\n", "rust", {}, []),
    ("C9", "fn f() {}\n// Copyright 2026 Example\n// a\n// b\n// c\n// d\n", "rust", {}, [(2, 1, "block", "5")]),
    ("C10", "#!/usr/bin/env bash\n# a\n# b\n# c\n# d\necho hi\n", "shell", {}, []),
    ("C11", "// keep it for now\n// a temporary file\n// a hacker\n// HACK: x\n", "rust", {},
     [(1, 12, "workaround", "for now"), (4, 4, "workaround", "hack")]),
    ("C12", "// we work  around it\n", "rust", {}, [(1, 7, "workaround", "work around")]),
    ("C13", "// TODO fix\n// TODO(#12) fix\nx;\n// FIXME(PROJ-3)\n// XXX https://x.test/1\n// a todo list\n",
     "rust", {},
     [(1, 4, "todo", "TODO")]),
    ("C25", "// TODO: use SHA-256\n// FIXME: UTF-8 here\n// TODO(ISO-8601)\n", "rust", {},
     [(1, 4, "todo", "TODO"), (2, 4, "todo", "FIXME")]),
    ("C26", "x = 1; /* a\n b\n c\n d\n e */\n", "java", {}, [(1, 8, "block", "5")]),
    ("C14", "// `TODO` and `for now` are words\n", "rust", {}, []),
    ("C15", "x = 1; /* a\n   in case b */\n", "kotlin", {}, [(2, 4, "workaround", "in case")]),
    ("C16", 'def f():\n    """for now, a docstring"""\n    # in case\n', "python", {},
     [(3, 7, "workaround", "in case")]),
    ("C17", "echo a#for now\n", "shell", {}, []),
    ("C18", "select 1; -- quick fix\n", "sql", {}, [(1, 14, "workaround", "quick fix")]),
    ("C19", "// a\n// b\nfn f() {}\n", "rust", {"max_lines": 1}, [(1, 1, "block", "2")]),
    ("C20", "// band-aid here\n", "rust", {"phrases": ("band-aid",)}, [(1, 4, "workaround", "band-aid")]),
    ("C21", "x;\n/* a /* b\n c */ d\n e\n f\n g */\n", "rust", {}, [(2, 1, "block", "5")]),
    ("C22", "/**/\nval a = 1\n/**\n a\n b\n c\n d\n */\n", "kotlin", {}, []),
    ("C23", "// see https://x.test/hack/for-now\n", "rust", {}, []),
    ("C24", "// a\n// b\n// c\n/// d\n// e\n// f\n", "rust", {}, []),
]


def self_test():
    bad = 0
    for cid, src, lang, kw, want in CASES:
        got = comment_hits(src, lang, **kw)
        if got != want:
            bad += 1
            print(f"comments self-test {cid}: expected {want}, got {got}")
    print("comments self-test: ok" if not bad else f"comments self-test: {bad} failed")
    return 1 if bad else 0
