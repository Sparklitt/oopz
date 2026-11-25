"""Known answers, written by hand: one source per language with every hit it must give, and the parts around them."""

from config import BASELINE, CONF, glob_re, judge, read_conf
from rules import file_hits

SAMPLES = {
    "rust": ('''//! "doc words"
#[allow(clippy::x, reason = "a reason here")]
fn f(t: (u8, u8)) -> &'static str {
    let c = '"';
    let k = "a-key";
    let n = t.0 + 42;
    let q = format!("SELECT a FROM t WHERE id = {id}");
    let ok = format!("SELECT {COLS} FROM {TABLE} WHERE id = ?1");
    let r = r#"Raw "two" words"#;
    "Two words"
}
#[cfg(test)]
mod tests {
    fn t() { let _ = ("test words", 99); }
}
pub(crate) static SIZES: [u32; 3] = [8, 16, 32];
impl A {
    const LIMIT: usize = 4096;
}
const fn g() -> u32 { 77 }
''', [(6, "number", "42"), (7, "sql", "SELECT a FROM t WHERE id = {id}"), (9, "prose", 'Raw "two" words'),
      (10, "prose", "Two words"), (20, "number", "77")]),
    "kotlin": ('''package a
@Suppress("x y")
fun f(id: Long) {
    // "in a comment"
    /* "in /* nested */ block" */
    val c = '"'
    Text("Hello there")
    tag("id-1")
    val w = 16.dp + 2.dp
    val s = "SELECT * FROM t WHERE id = $id"
    val ok = "SELECT * FROM t WHERE id = ? AND kind = '${Kinds.QUOTE}'"
    val raw = """Two $id words"""
    val LOCAL = 9
}
const val MAX = 300
val PAGE_SIZE = 50
class B {
    companion object {
        val GAP = 12
    }
}
''', [(7, "prose", "Hello there"), (9, "number", "16"), (10, "sql", "SELECT * FROM t WHERE id = $id"),
      (12, "prose", "Two $id words"), (13, "number", "9")]),
    "java": ('''class A {
    @Deprecated(since = "a b")
    static final int MAX = 300;
    String q(String id) {
        char c = '"';
        String s = """
            Text block words""";
        return "SELECT a FROM t WHERE id = '" + id + "'";
    }
    String ok() { return String.format("SELECT %s FROM t", COLUMNS) + "x y"; }
    private static final List<Integer> SIZES = List.of(8, 16);
    int size() { return 64; }
}
''', [(6, "prose", "\n            Text block words"), (8, "sql", "SELECT a FROM t WHERE id = '"), (10, "prose", "x y"),
      (12, "number", "64")]),
    "js": ('''const label = "Save the draft";
const cls = <div className="flex items-center">{n}</div>;
const q = `SELECT * FROM t WHERE id = ${id}`;
const ok = sql`SELECT * FROM t WHERE id = ${id}`;
const re = /["']/g;
const t = setTimeout(f, 5000);
// "comment words"
export const TIMEOUT_MS: number = 5000;
function g() {
  const LIMIT = 50;
}
''', [(1, "prose", "Save the draft"), (3, "sql", "SELECT * FROM t WHERE id = ${id}"), (6, "number", "5000"),
      (10, "number", "50")]),
    "python": ('''"""Module docstring words."""
import os

TIMEOUT = 30


def f(cur, name):
    """Function docstring words."""
    print(f"Hello {name}")
    cur.execute(f"SELECT * FROM t WHERE name = '{name}'")
    cur.execute("SELECT * FROM t WHERE name = %s", (name,))
    cur.execute("SELECT %s FROM t" % COLUMNS)
    cur.execute("SELECT * FROM t WHERE id = %s" % name)
    msg = ("two words "
           "more words")
    RETRY = 7
    return 0x1F  # "comment"
SIZES: tuple = (8,
                16)
''', [(9, "prose", "Hello {name}"), (10, "sql", "SELECT * FROM t WHERE name = '{name}'"),
      (13, "sql", "SELECT * FROM t WHERE id = %s"), (14, "prose", "two words "), (15, "prose", "more words"),
      (16, "number", "7"), (17, "number", "0x1F")]),
    "go": ('''package a

type T struct {
	Name string `json:"name" db:"name"`
}

func q(db *sql.DB, id string) {
	db.Query(fmt.Sprintf("SELECT * FROM t WHERE id = %s", id))
	db.Query("SELECT * FROM t WHERE id = $1", id)
	log.Print("two words")
	time.Sleep(250 * time.Millisecond)
	_ = 'x'
}

const Retries = 3

const (
	PageSize = 50
	Gap      = 12
)
''', [(8, "sql", "SELECT * FROM t WHERE id = %s"), (10, "prose", "two words"), (11, "number", "250")]),
    "shell": ('''#!/usr/bin/env bash
# "comment words"
echo "Usage: tool <file>"
n=${#args[@]}
sqlite3 db "SELECT * FROM t WHERE id = $id"
sqlite3 db "SELECT * FROM $TABLE"
sleep 30
echo "$1" >&2
cat <<EOF
Here doc words
EOF
readonly WAIT=10
MAX_TRIES=4
tries=5
''', [(3, "prose", "Usage: tool <file>"), (5, "sql", "SELECT * FROM t WHERE id = $id"), (7, "number", "30"),
      (10, "prose", "Here doc words\n"), (14, "number", "5")]),
    "sql": ('''-- 'comment words'
CREATE TABLE t (
  name TEXT NOT NULL DEFAULT 'Two words',
  "quoted id" INTEGER CHECK (n < 100)
);
INSERT INTO t VALUES ('it''s here');
''', [(3, "prose", "Two words"), (4, "number", "100"), (6, "prose", "it''s here")]),
}

# A named constant passes; with `constants_only = true` its number is a hit (it belongs in a constants file).
DECLARATIONS = {"rust": "const LIMIT: usize = 300;\n", "kotlin": "const val MAX = 300\n",
                "java": "class A { static final int MAX = 300; }\n", "js": "export const MAX = 300;\n",
                "python": "MAX = 300\n", "go": "const Max = 300\n", "shell": "readonly MAX=300\n"}


def self_test():
    failed = 0

    def expect(what, got, want):
        nonlocal failed
        if got != want:
            print(f"self-test: {what}: expected {want}, got {got}")
            failed = 1

    def hits(src, lang, constants_only=False):
        return [(line, rule, text) for line, _, rule, text in file_hits(src, lang, constants_only=constants_only)]

    for lang, (src, want) in SAMPLES.items():
        expect(lang, hits(src, lang), want)
    for lang, src in DECLARATIONS.items():
        expect(f"{lang} named constant", hits(src, lang), [])
        expect(f"{lang} constants_only", hits(src, lang, True), [(1, "number", "300")])
    expect("glob", [bool(glob_re(g).fullmatch(p)) for g, p in [("**/tests/**", "tests/a.rs"),
                                                                ("**/tests/**", "a/tests/b/c.rs"),
                                                                ("**/tests/**", "a/testsx/b.rs"), ("*.rs", "a/b.rs")]],
           [True, True, False, False])
    found = [("a.py", 1, 1, "number", "5"), ("a.py", 2, 1, "number", "6"), ("b.py", 1, 1, "prose", "x y")]
    allows = [(1, glob_re("b.py"), "x y"), (2, glob_re("c.py"), "*")]
    expect("baseline equal", judge(found, allows, {("a.py", "number"): 2})[0],
           [f"{CONF}:2: stale allow entry (allows nothing)"])
    expect("baseline risen", judge(found, [], {("a.py", "number"): 1})[0][:2],
           ["a.py: number: 2 hits, the baseline allows 1; the new ones are among:", "a.py:1:1: number 5"])
    expect("baseline fallen", judge(found[2:], allows[:1], {("a.py", "number"): 2})[0],
           [f"{BASELINE}: a.py number: 0 hits, baseline 2: lower it (--baseline)"])
    expect("constants_only", read_conf("constants_only = true\n")["constants_only"], True)
    for bad in ("allow = a | b\n", "colour = red\n", "numbers = ten\n", "constants_only = yes\n"):
        try:
            read_conf(bad)
            expect(f"bad config {bad!r}", "accepted", "refused")
        except ValueError:
            pass
    return failed
