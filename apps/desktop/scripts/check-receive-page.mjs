// Tests the phone's receive page's `answer` (crates/oopz-app/src/receive_page.html, between its "answer: begin" and
// "answer: end" lines): docs/messages.md, the receive page, rows 10 and 12. Its words are invented here.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const page = readFileSync(new URL("../../../crates/oopz-app/src/receive_page.html", import.meta.url), "utf8");
const begin = page.indexOf("// answer: begin"), end = page.indexOf("// answer: end");
assert.ok(begin > 0 && end > begin, "the page marks its answer function");
const answer = new Function(page.slice(begin, end) + "\nreturn answer;")();

const W = {
  receive_page_ok: "OK-WORDS",
  receive_page_bad: "BAD-WORDS",
  receive_refused_exists: "EXISTS-WORDS",
};
const cases = [
  // A file received whole.
  [200, "5 ab12", 5, { ok: true, text: "OK-WORDS" }],
  // Received, but not the file's size.
  [200, "4 ab12", 5, { ok: false, text: "BAD-WORDS (HTTP 200)" }],
  // A code this page knows.
  [409, "receive_refused_exists", 5, { ok: false, text: "BAD-WORDS (EXISTS-WORDS)" }],
  // Row 10: a code from a newer computer: the generic words and the code.
  [400, "receive_refused_newer", 5, { ok: false, text: "BAD-WORDS (receive_refused_newer)" }],
  // A page word or an object's own property is not a refusal.
  [400, "receive_page_ok", 5, { ok: false, text: "BAD-WORDS (receive_page_ok)" }],
  [400, "__proto__", 5, { ok: false, text: "BAD-WORDS (__proto__)" }],
  // Row 12: a proxy's error page, or no body: the HTTP status, never the body.
  [502, "<html><body>Bad gateway</body></html>", 5, { ok: false, text: "BAD-WORDS (HTTP 502)" }],
  [500, "", 5, { ok: false, text: "BAD-WORDS (HTTP 500)" }],
  [0, null, 5, { ok: false, text: "BAD-WORDS (HTTP 0)" }],
];
for (const [status, body, size, expected] of cases) {
  assert.deepEqual(answer(status, body, size, W), expected, `${status} ${body}`);
}
console.log(`check-receive-page: ${cases.length} answers as expected`);
