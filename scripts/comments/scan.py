"""The comment rules on one source: the part of the house comment rule that needs no judgment (comment-reviewer
keeps the rest)."""

import re

from tokens import Lexer

# A plain comment block over this many lines fails: the rule is "what and why in a line or two", and twice that
# leaves room for a spec reference or an invariant. A project sets its own with `max_lines`.
MAX_LINES = 4
# Words that justify not fixing the root cause. "temporary" alone is not one: it names temporary files and folders.
PHRASES = ("for now", "workaround", "work around", "in case", "hack", "hacks", "hacky", "quick fix",
           "temporary fix", "temporary workaround", "temporary solution", "until we")
TODO = re.compile(r"\b(?:TODO|FIXME|XXX)\b")
# What names an issue on a TODO's line: #123 or a link anywhere, PROJ-12 only as TODO(PROJ-12) (SHA-256 is no issue).
ISSUE = re.compile(r"#\d+|https?://|\b(?:TODO|FIXME|XXX)\([A-Z][A-Z0-9]+-\d+\)")
# A leading comment block holding one of these is a licence header, whose length is not the author's to choose.
LEGAL = re.compile(r"SPDX-License-Identifier|Copyright")
MASK = re.compile(r"(`+).+?\1|https?://\S+")
DOC_PREFIXES = {"rust": ("///", "//!", "/**", "/*!"), "kotlin": ("/**",), "java": ("/**",), "js": ("/**",)}
NOT_DOC = ("////", "/**/", "/***")
HINTS = {"block": "a comment says what and why in a line or two: the code says the rest, history and plans go in docs",
         "workaround": "fix the root cause instead of explaining why not; if it can't be fixed now, open an issue",
         "todo": "a TODO names its issue on the same line: TODO(#123), TODO(PROJ-12) or a link"}


def phrase_re(phrases):
    return re.compile(r"\b(?:" + "|".join(re.escape(p).replace(r"\ ", r"\s+") for p in phrases) + r")\b", re.I)


def is_doc(text, lang):
    return text.startswith(DOC_PREFIXES.get(lang, ())) and not text.startswith(NOT_DOC)


def own_line(src, start):
    return not src[src.rfind("\n", 0, start) + 1:start].strip()


def runs(src, comments, lang):
    """Plain (not doc) comments grouped into blocks: [(first tok, lines, text)]. Line comments standing alone on
    consecutive lines are one block; a /* */ comment is its own, wherever it starts."""
    out = []
    for t in comments:
        line_comment = not t.text.startswith("/*")
        shebang = t.start == 0 and t.text.startswith("#!")
        if is_doc(t.text, lang) or shebang or line_comment and not own_line(src, t.start):
            continue
        last = out[-1] if out else None
        if line_comment and last and last[3] and last[0].line + last[1] == t.line:
            out[-1] = (last[0], last[1] + 1, last[2] + "\n" + t.text, True)
        else:
            out.append((t, t.end - t.line + 1, t.text, line_comment))
    return [r[:3] for r in out]


def comment_hits(src, lang, max_lines=MAX_LINES, phrases=PHRASES):
    """Hits as (line, col, rule, what): what is the block's length or the phrase from the list, never source text."""
    lexer = Lexer(src, lang)
    first_code = (lexer.run() or [None])[0]
    hits = []
    for first, lines, text in runs(src, lexer.comments, lang):
        legal = (first_code is None or first.start < first_code.start) and LEGAL.search(text)
        if lines > max_lines and not legal:
            hits.append((first.line, first.col, "block", str(lines)))
    words = phrase_re(phrases)
    for t in lexer.comments:
        for k, raw in enumerate(t.text.split("\n")):
            col0 = t.col if k == 0 else 1
            masked = MASK.sub(lambda m: " " * len(m.group()), raw)
            for m in words.finditer(masked):
                hits.append((t.line + k, col0 + m.start(), "workaround", " ".join(m.group().lower().split())))
            if not ISSUE.search(raw):
                hits += [(t.line + k, col0 + m.start(), "todo", m.group()) for m in TODO.finditer(masked)]
    return sorted(hits)
