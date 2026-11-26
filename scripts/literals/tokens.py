"""Per-language tokenisers: the strings, numbers, names and operators of a source; comments kept apart."""

import re

LANGS = {".rs": "rust", ".kt": "kotlin", ".kts": "kotlin", ".java": "java", ".py": "python", ".go": "go",
         ".sh": "shell", ".bash": "shell", ".sql": "sql", ".js": "js", ".jsx": "js", ".mjs": "js", ".cjs": "js",
         ".ts": "js", ".tsx": "js", ".mts": "js", ".cts": "js"}
LINE_COMMENT = {"python": "#", "shell": "#", "sql": "--"}
NESTED_COMMENTS = {"rust", "kotlin"}
CHARS = {"rust", "kotlin", "java", "go"}
TRIPLE = {"python": True, "java": True, "kotlin": False}  # """…""": escapes or not
PY_PREFIX = {"r", "u", "b", "f", "br", "rb", "fr", "rf"}
RUST_PREFIX = {"b", "r", "br", "c", "cr"}
NUMBER = re.compile(r"(0[xX][0-9a-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?)"
                    r"([A-Za-z_]\w*)?")
DIGITS = re.compile(r"\d+")
CHAR = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'")
SHELL_VAR = re.compile(r"\$(?:\d+|[@#?*!$-]|[A-Za-z_]\w*)")
TEMPLATE_VAR = re.compile(r"\$([A-Za-z_]\w*)")
HEREDOC = re.compile(r"<<(-?)[ \t]*(['\"]?)([A-Za-z_]\w*)\2")
HASHES = re.compile(r"#*")
WORD = re.compile(r"\w+")
JS_WORD = re.compile(r"[\w$]+")
JS_REGEX_AFTER = set("(,=:[!&|?{};+-*%~^") | {"return", "typeof", "case", "in", "of", "void", "yield"}  # not < >: </div>


class Tok:
    __slots__ = ("kind", "text", "line", "col", "end", "start", "stop", "depth", "interp", "attr")

    def __init__(self, kind, text, line, col, start, stop, depth, interp):
        self.kind, self.text, self.line, self.col, self.start, self.stop = kind, text, line, col, start, stop
        self.end = line + text.count("\n")
        self.depth, self.interp, self.attr = depth, interp, False


def is_op(t, chars):
    """t is an operator among chars (a string token "(" is not one)."""
    return t is not None and t.kind == "op" and t.text in chars


def until(src, i, close, escapes=True):
    """Index of the first unescaped `close` at or after i (len(src) when there is none)."""
    while i < len(src):
        if escapes and src[i] == "\\":
            i += 2
        elif src.startswith(close, i):
            return i
        else:
            i += 1
    return len(src)


def brace_end(src, i, opening, closing):
    """Index just past the `closing` that matches the `opening` at i; quoted text inside skipped."""
    depth = 0
    while i < len(src):
        c = src[i]
        if c in "\"'`":
            i = until(src, i + 1, c) + 1
            continue
        depth += (c == opening) - (c == closing)
        i += 1
        if depth == 0:
            break
    return i


def interpolations(body, lang, python_f=False):
    """The expressions a string splices in (`${x}`, `$x`, `{x}`, `$(…)`), escapes skipped."""
    found, i = [], 0
    while i < len(body):
        c = body[i]
        if c == "\\" and lang != "kotlin-raw":
            i += 2
        elif python_f and body.startswith(("{{", "}}"), i):
            i += 2
        elif python_f and c == "{":
            j = brace_end(body, i, "{", "}")
            found.append(re.split(r"!|:|=$", body[i + 1:j - 1].strip())[0])
            i = j
        elif not python_f and body.startswith("${", i):
            j = brace_end(body, i + 1, "{", "}")
            found.append(body[i + 2:j - 1])
            i = j
        elif lang == "shell" and (body.startswith("$(", i) or c == "`"):
            j = brace_end(body, i + 1, "(", ")") if c == "$" else until(body, i + 1, "`") + 1
            found.append(body[i:j])
            i = j
        elif not python_f and c == "$" and (m := (SHELL_VAR if lang == "shell" else TEMPLATE_VAR).match(body, i)):
            name = m.group(0)[1:]
            found.append(name if name[0].isalpha() or name[0] == "_" else m.group(0))
            i = m.end()
        else:
            i += 1
    return found


def tokenize(src, lang):
    return Lexer(src, lang).run()


class Lexer:
    """One pass over a source. Each step reads one construct at self.i and says whether it did. The comments it
    skips are kept in self.comments, for the comment scan."""

    def __init__(self, src, lang):
        self.src, self.lang, self.n = src, lang, len(src)
        self.i, self.line, self.depth, self.toks, self.heredocs = 0, 1, 0, [], []
        self.comments = []

    def run(self):
        steps = (self.blank, self.comment, self.shell, self.char, self.quoted, self.backtick, self.js_regex,
                 self.number, self.word, self.operator)
        while self.i < self.n:
            next(step for step in steps if step())
        return self.toks

    def add(self, kind, text, start, stop, interp=None):
        col = start - self.src.rfind("\n", 0, start)
        self.toks.append(Tok(kind, text, self.line, col, start, stop, self.depth, interp))
        self.line += self.src.count("\n", start, stop)
        self.i = stop

    def skip(self, j):
        self.line += self.src.count("\n", self.i, j)
        self.i = j

    def blank(self):
        c = self.src[self.i]
        if c == "\n":
            self.line, self.i = self.line + 1, self.i + 1
            for delimiter, dash, quoted in self.heredocs:
                self.heredoc(delimiter, dash, quoted)
            self.heredocs = []
            return True
        if c in " \t\r\f":
            self.i += 1
            return True
        return False

    def heredoc(self, delimiter, dash, quoted):
        src, start, i = self.src, self.i, self.i
        while i < self.n and src[i:until(src, i, "\n", False)].lstrip("\t" if dash else "") != delimiter:
            i = until(src, i, "\n", False) + 1
        self.add("str", src[start:i], start, i, None if quoted else interpolations(src[start:i], self.lang))
        self.i = until(src, i, "\n", False)

    def comment(self):
        src, i, lang = self.src, self.i, self.lang
        if src.startswith(LINE_COMMENT.get(lang, "//"), i) and (
                lang != "shell" or i == 0 or src[i - 1] in " \t\n;|&()"):
            self.i = until(src, i, "\n", False)
            self.note(i, self.i)
            return True
        if lang in ("python", "shell") or not src.startswith("/*", i):
            return False
        level, j = 0, i
        while j < self.n:
            if src.startswith("/*", j) and (level == 0 or lang in NESTED_COMMENTS):
                level, j = level + 1, j + 2
            elif src.startswith("*/", j):
                level, j = level - 1, j + 2
                if level == 0:
                    break
            else:
                j += 1
        self.note(i, j)
        self.skip(j)
        return True

    def note(self, start, stop):
        col = start - self.src.rfind("\n", 0, start)
        self.comments.append(Tok("comment", self.src[start:stop], self.line, col, start, stop, self.depth, None))

    def shell(self):
        src, i = self.src, self.i
        if self.lang != "shell" or src[i] not in "<$":
            return False
        if src.startswith("<<", i) and not src.startswith("<<<", i) and (m := HEREDOC.match(src, i)):
            self.heredocs.append((m.group(3), m.group(1) == "-", bool(m.group(2))))
            self.i = m.end()
        elif src.startswith("$'", i):
            j = until(src, i + 2, "'")
            self.add("str", src[i + 2:j], i, j + 1)
        elif m := SHELL_VAR.match(src, i):
            self.add("id", m.group(0), i, m.end())
        else:
            return False
        return True

    def char(self):
        if self.src[self.i] != "'" or self.lang not in CHARS:
            return False
        m = CHAR.match(self.src, self.i)
        if m:
            self.i = m.end()
        else:  # a Rust lifetime
            self.add("op", "'", self.i, self.i + 1)
        return True

    def quoted(self):
        src, i, lang, c = self.src, self.i, self.lang, self.src[self.i]
        if c not in "\"'":
            return False
        if src.startswith(c * 3, i) and (lang in TRIPLE and c == '"' or lang == "python"):
            self.string(i, 3, i, TRIPLE[lang], lang == "kotlin")
        elif lang == "sql" and c == '"':
            j = until(src, i + 1, '"', False)
            self.add("id", src[i + 1:j], i, j + 1)
        elif lang == "sql":
            j = i + 1
            while (j := until(src, j, "'", False)) + 1 < self.n and src[j + 1] == "'":
                j += 2
            self.add("str", src[i + 1:j], i, j + 1)
        elif lang == "shell" and c == "'":
            j = until(src, i + 1, "'", False)
            self.add("str", src[i + 1:j], i, j + 1)
        elif c == '"' or lang in ("python", "js"):
            self.string(i, 1, i, True, lang in ("kotlin", "shell"))
        else:
            return False
        return True

    def string(self, i, quotes, start, escapes, interpolates, python_f=False):
        """The string whose quotes begin at i (its prefix, if any, at start)."""
        close = self.src[i] * quotes
        j = until(self.src, i + quotes, close, escapes)
        body = self.src[i + quotes:j]
        kind = "kotlin-raw" if self.lang == "kotlin" and quotes == 3 else self.lang
        interp = interpolations(body, kind, python_f) if interpolates or python_f else None
        self.add("str", body, start, j + quotes, interp)

    def backtick(self):
        src, i = self.src, self.i
        if src[i] != "`" or self.lang not in ("js", "go"):
            return False
        j = self.template_end(i) if self.lang == "js" else until(src, i + 1, "`", False)
        tagged = self.toks and self.toks[-1].kind == "id" and self.toks[-1].stop == i
        body = src[i + 1:j]
        self.add("str", body, i, j + 1, None if tagged or self.lang == "go" else interpolations(body, "js"))
        return True

    def template_end(self, i):
        """Index of the backtick closing the JS template literal opened at i."""
        src, j = self.src, i + 1
        while j < self.n and src[j] != "`":
            if src[j] == "\\":
                j += 2
            elif src.startswith("${", j):
                j = brace_end(src, j + 1, "{", "}")
            else:
                j += 1
        return j

    def js_regex(self):
        src, i, prev = self.src, self.i, self.toks[-1] if self.toks else None
        if src[i] != "/" or self.lang != "js" or src[i + 1:i + 2] == "/" or prev is not None and (
                prev.kind not in ("op", "id") or prev.text not in JS_REGEX_AFTER):
            return False
        j, klass = i + 1, False
        while j < self.n and src[j] != "\n" and (src[j] != "/" or klass):
            klass = (klass or src[j] == "[") and src[j] != "]"
            j += 2 if src[j] == "\\" else 1
        self.i = j + (j < self.n and src[j] == "/")
        return True

    def number(self):
        src, i = self.src, self.i
        if not src[i].isdigit():
            return False
        prev = self.toks[-2:]
        if len(prev) == 2 and is_op(prev[1], ".") and prev[1].stop == i and (
                prev[0].kind == "id" or is_op(prev[0], ")]")):
            m = DIGITS.match(src, i)  # a tuple field: t.0
            self.add("field", m.group(0), i, m.end())
        else:
            m = NUMBER.match(src, i)
            self.add("num", m.group(0), i, m.end())
        return True

    def word(self):
        src, i, lang = self.src, self.i, self.lang
        if not (src[i].isalpha() or src[i] == "_" or src[i] == "$" and lang in ("js", "java")):
            return False
        m = (JS_WORD if lang in ("js", "java") else WORD).match(src, i)
        word, j = m.group(0), m.end()
        nxt = src[j:j + 1]
        if lang == "python" and word.lower() in PY_PREFIX and nxt in ("'", '"'):
            self.string(j, 3 if src.startswith(nxt * 3, j) else 1, i, True, False, "f" in word.lower())
        elif lang == "rust" and word in RUST_PREFIX and (nxt == '"' or "r" in word and nxt == "#"):
            hashes = HASHES.match(src, j).group(0) if "r" in word else ""
            close = '"' + hashes
            k = until(src, j + len(hashes) + 1, close, "r" not in word)
            self.add("str", src[j + len(hashes) + 1:k], i, k + len(close))
        elif lang == "rust" and word == "b" and nxt == "'" and CHAR.match(src, j):
            self.i = CHAR.match(src, j).end()
        else:
            self.add("id", word, i, j)
        return True

    def operator(self):
        c = self.src[self.i]
        self.depth = max(0, self.depth - (c in ")]}"))
        self.add("op", c, self.i, self.i + 1)
        self.depth += c in "([{"
        return True
