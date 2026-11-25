"""The three rules on a file's tokens: prose, number (outside a named constant) and sql (a value spliced in)."""

import re

from tokens import NUMBER, is_op, tokenize

ESCAPE = re.compile(r"\\(?:u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|.)", re.S)
PLACEHOLDER = re.compile(r"\{[^{}\s]*\}|%[-+#0]*\d*(?:\.\d+)?[sdifvqxXeEgGcpob]")
INLINE = re.compile(r"(?<!\{)\{([A-Za-z_]\w*)[^{}]*\}")
LETTER = re.compile(r"[^\W\d_]")
SPACE = re.compile(r"\s")
SQL = re.compile(r"^\s*(?:\{[^{}]*\}\s*)?(?:SELECT|INSERT|UPDATE|DELETE|WITH|CREATE|ALTER|DROP|PRAGMA|FROM|WHERE|AND"
                 r"|OR|SET|VALUES|ORDER BY|GROUP BY|LIMIT|JOIN|LEFT JOIN|ON CONFLICT|RETURNING)\b"
                 r"|\b(?:FROM|WHERE|VALUES|INTO)\b")
CONSTANT = re.compile(r"(?:[A-Za-z_]\w*(?:\.|::))*[A-Z][A-Z0-9_]*")
UPPER = re.compile(r"[A-Z][A-Z0-9_]*")
GO_TAG = re.compile(r"(?:\w+:\"[^\"]*\"\s*)+")
CLASS_ATTRS = {"className", "class"}
FORMAT_CALLS = {"format", "Sprintf", "sprintf"}
FORMAT_MACROS = {"format", "format_args"}
# Words before a declared name that change nothing about it being a constant.
MODIFIERS = {"pub", "export", "private", "internal", "public", "protected", "override", "transient", "volatile"}
NOT_A_CONSTANT = {"fn", "unsafe", "async", "extern", "impl", "trait"}  # Rust: `const fn`, `static` before them
# A line ending in one of these goes on: `val X =` then the value on the next line.
CONTINUES = set("=+-*/,.([{:|&?<")


def end_of_group(toks, k):
    """Index just past the bracket closing the one at k."""
    depth = 0
    for j in range(k, len(toks)):
        depth += is_op(toks[j], "([{") - is_op(toks[j], ")]}")
        if depth == 0:
            return j + 1
    return len(toks)


def end_of_item(toks, k):
    """Index just past the Rust item starting at k (to its `;` or its closing brace)."""
    for j in range(k, len(toks)):
        if is_op(toks[j], ";"):
            return j + 1
        if is_op(toks[j], "{"):
            return end_of_group(toks, j)
    return len(toks)


def mark_attributes(toks, lang):
    """Marks the tokens of attributes and annotations; drops Rust's test items. Returns the tokens kept."""
    kept, k = [], 0
    while k < len(toks):
        t = toks[k]
        rust = lang == "rust" and is_op(t, "#") and k + 1 < len(toks) and is_op(toks[k + 1], "[!")
        jvm = lang in ("kotlin", "java") and is_op(t, "@") and k + 1 < len(toks) and toks[k + 1].kind == "id"
        if not (rust or jvm):
            kept.append(t)
            k += 1
            continue
        j = k + 1
        while jvm and j + 2 < len(toks) and is_op(toks[j + 1], ".:"):
            j += 2
        if jvm and (j + 1 >= len(toks) or not is_op(toks[j + 1], "(")):
            kept.extend(toks[k:j + 1])
            k = j + 1
            continue
        bracket = j + 1 if jvm else (k + 1 if is_op(toks[k + 1], "[") else k + 2)
        j = end_of_group(toks, bracket)
        body = "".join(x.text for x in toks[bracket + 1:j - 1])
        if rust and (body in ("cfg(test)", "test") or body.endswith("::test")):
            k = end_of_item(toks, j)
            continue
        for x in toks[k:j]:
            x.attr = True
        kept.extend(toks[k:j])
        k = j
    return kept


def text(toks, j):
    return toks[j].text if j < len(toks) else ""


def declares(toks, i, lang, top):
    """A named constant is declared by the statement starting at i. `top`: outside any function body."""
    j = i
    while text(toks, j) in MODIFIERS or lang == "java" and text(toks, j) in ("static", "final"):
        j = end_of_group(toks, j + 1) if is_op(toks[j + 1] if j + 1 < len(toks) else None, "(") else j + 1
    word, name = text(toks, j), text(toks, j + 1)
    if lang == "rust":
        return word in ("const", "static") and name not in NOT_A_CONSTANT
    if lang == "kotlin":
        return word == "const" or word == "val" and top and bool(UPPER.fullmatch(name))
    if lang == "java":
        mods = {x.text for x in toks[i:j]}
        head = toks[j:statement_end(toks, j, lang)]
        head = head[:next((n for n, x in enumerate(head) if is_op(x, "=")), len(head))]
        return {"static", "final"} <= mods and not any(is_op(x, "(") for x in head)  # a ( before = : a method
    if lang == "js":
        return word == "const" and top and bool(UPPER.fullmatch(name))
    if lang == "python":
        return toks[i].col == 1 and bool(UPPER.fullmatch(word)) and is_op(toks[j + 1] if j + 1 < len(toks) else None, "=:")
    if lang == "go":
        return word == "const"
    if lang == "shell":
        return word == "readonly" or bool(UPPER.fullmatch(word)) and name == "=" and toks[j + 1].start == toks[j].stop
    return False


def statement_end(toks, i, lang):
    """Index just past the statement starting at i: its `;`, or the next line that starts at its depth."""
    d = toks[i].depth
    for j in range(i + 1, len(toks)):
        t, prev = toks[j], toks[j - 1]
        if is_op(t, ";") and t.depth == d:
            return j + 1
        if lang not in ("rust", "java") and t.depth == d and t.line > prev.end and not is_op(prev, CONTINUES):
            return j
    return len(toks)


def declared(toks, lang):
    """Indices of the tokens inside named constant declarations."""
    inside, scopes, end = set(), [], 0
    for i, t in enumerate(toks):
        starts = i == 0 or toks[i - 1].end < t.line or is_op(toks[i - 1], ";{}")
        if i >= end and starts and t.kind == "id" and declares(toks, i, lang, not scopes or scopes[-1]):
            end = statement_end(toks, i, lang)
            inside.update(range(i, end))
        if is_op(t, "{"):  # Kotlin: an object's body is as good as the top level; JS: any brace is not
            scopes.append(lang == "kotlin" and any(x.text == "object" for x in toks[max(0, i - 4):i]))
        elif is_op(t, "}") and scopes:
            scopes.pop()
    return inside


def constant(expr):
    """An expression that names a constant: a string or number token, or an UPPER_CASE path."""
    if isinstance(expr, str):
        return bool(CONSTANT.fullmatch(expr.strip()))
    if len(expr) == 1 and expr[0].kind in ("str", "num"):
        return True
    return bool(CONSTANT.fullmatch("".join(t.text for t in expr)))


def arguments(toks, k):
    """The arguments of the call whose `(` is at k, each a token list, `name = value` as its value."""
    args, current, depth = [], [], 0
    for t in toks[k + 1:end_of_group(toks, k) - 1]:
        if is_op(t, ",") and depth == 0:
            args.append(current)
            current = []
            continue
        depth += is_op(t, "([{") - is_op(t, ")]}")
        current.append(t)
    return [a[2:] if len(a) > 2 and a[0].kind == "id" and is_op(a[1], "=") else a for a in args + [current] if a]


def named(toks, k):
    """The names a call (its `(` at k) gives with `name = value`: Rust's format!, Python's str.format."""
    return {toks[j].text for j in range(k + 1, end_of_group(toks, k) - 2) if toks[j].kind == "id"
            and is_op(toks[j + 1], "=") and not is_op(toks[j + 2], "=") and is_op(toks[j - 1], "(,")}


def operand(toks, k):
    """The run of names, dots and :: from k: `a.b`, `T::X`."""
    j = k
    while j < len(toks) and (toks[j].kind == "id" or is_op(toks[j], ".:") and j > k):
        j += 1
    return toks[k:j]


def formatted(toks, k, lang):
    """The values a formatting call or operator splices into the string at k, or None when there is none."""
    t, nxt = toks[k], toks[k + 1:k + 4]
    if lang == "python" and nxt and is_op(nxt[0], "%"):
        return arguments(toks, k + 2) if len(nxt) > 1 and is_op(nxt[1], "(") else [operand(toks, k + 2)]
    if len(nxt) == 3 and is_op(nxt[0], ".") and nxt[1].text in ("format", "formatted") and is_op(nxt[2], "("):
        call, args = k + 3, arguments(toks, k + 3)
    elif k >= 2 and is_op(toks[k - 1], "(") and (toks[k - 2].kind == "id" and toks[k - 2].text in FORMAT_CALLS or
                                                  k >= 3 and is_op(toks[k - 2], "!") and toks[k - 3].text in FORMAT_MACROS):
        call, args = k - 1, arguments(toks, k - 1)[1:]
    else:
        return None
    names = named(toks, call)
    return args + [name for name in INLINE.findall(t.text) if name not in names]


def spliced(toks, k, lang):
    """The SQL-looking string at k takes a value that isn't a constant: interpolated, formatted or joined."""
    t, nxt = toks[k], toks[k + 1:k + 3]
    if t.interp and not all(constant(e) for e in t.interp):
        return True
    if any(not constant(v) for v in formatted(toks, k, lang) or []):
        return True
    if len(nxt) == 2 and is_op(nxt[0], "+") and not is_op(nxt[1], "="):
        if nxt[1].kind in ("str", "num"):
            return False
        chain = operand(toks, k + 2)
        after = toks[k + 2 + len(chain)] if k + 2 + len(chain) < len(toks) else None
        return not chain or is_op(after, "([") or not constant(chain)
    return False


def prose(t):
    words = t.text
    for e in t.interp or []:
        words = words.replace(e, "#")
    words = PLACEHOLDER.sub("#", ESCAPE.sub("", words)).strip()
    return bool(SPACE.search(words) and LETTER.search(words))


def number_value(written):
    m = NUMBER.fullmatch(written)
    digits = (m.group(1) if m else written).replace("_", "")
    try:
        return float(int(digits, 0)) if digits[:2].lower() in ("0x", "0b", "0o") else float(digits)
    except ValueError:
        return None


def not_words(toks, k, lang, src):
    """The string at k is no text for a person: CSS classes, a Go struct tag, a Python docstring."""
    t, prev = toks[k], [x.text for x in toks[max(0, k - 3):k]]
    if lang == "js" and (prev[-2:-1] and prev[-1] == "=" and prev[-2] in CLASS_ATTRS or
                         prev[-1:] == ["{"] and prev[-3:-1] and prev[-2] == "=" and prev[-3] in CLASS_ATTRS):
        return True
    if lang == "go" and src[t.start] == "`" and GO_TAG.fullmatch(t.text.strip()):
        return True
    return lang == "python" and t.depth == 0 and (k == 0 or toks[k - 1].end < t.line) and (
        k + 1 == len(toks) or toks[k + 1].line > t.end)


def file_hits(src, lang, check_prose=True, check_numbers=True, allowed=None, constants_only=False):
    """[(line, col, rule, literal)] for one file's source."""
    allowed = {0.0, 1.0, 2.0} if allowed is None else allowed
    toks = mark_attributes(tokenize(src, lang), lang)
    named_constants = set() if constants_only else declared(toks, lang)
    found = []
    for k, t in enumerate(toks):
        if t.kind == "num":
            if check_numbers and k not in named_constants and number_value(t.text) not in allowed:
                found.append((t.line, t.col, "number", t.text))
        elif t.kind == "str" and SQL.search(t.text):
            if spliced(toks, k, lang):
                found.append((t.line, t.col, "sql", t.text))
        elif t.kind == "str" and check_prose and not t.attr and prose(t) and not not_words(toks, k, lang, src):
            found.append((t.line, t.col, "prose", t.text))
    return found
