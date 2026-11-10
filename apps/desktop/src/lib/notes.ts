// Release notes are Markdown (the release's latest.json). They're shown formatted but never as HTML: this turns the
// few things release notes use (headings, lists, **bold**, *italic*, `code`, [links](…)) into plain pieces that the
// sheet renders as text. A link shows its words and its address; nothing is clickable.

export type Span = { text: string; bold?: boolean; italic?: boolean; code?: boolean };
export type Block = { kind: "heading" | "item" | "para"; spans: Span[] };

const INLINE = /(\*\*([^*]+)\*\*|__([^_]+)__|\*([^*\s][^*]*)\*|_([^_\s][^_]*)_|`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\))/g;

export function spans(line: string): Span[] {
  const out: Span[] = [];
  let at = 0;
  // exec in a loop, not matchAll: old phones' WebViews (Chrome 67) don't have it.
  INLINE.lastIndex = 0;
  for (let m = INLINE.exec(line); m; m = INLINE.exec(line)) {
    if (m.index! > at) out.push({ text: line.slice(at, m.index) });
    if (m[2] ?? m[3]) out.push({ text: (m[2] ?? m[3])!, bold: true });
    else if (m[4] ?? m[5]) out.push({ text: (m[4] ?? m[5])!, italic: true });
    else if (m[6]) out.push({ text: m[6], code: true });
    else if (m[7]) out.push({ text: m[7] === m[8] ? m[8] : `${m[7]} (${m[8]})` });
    at = m.index! + m[0].length;
  }
  if (at < line.length) out.push({ text: line.slice(at) });
  return out;
}

export function blocks(markdown: string): Block[] {
  const out: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length) out.push({ kind: "para", spans: spans(para.join(" ")) });
    para = [];
  };
  for (const raw of markdown.replace(/\r\n?/g, "\n").split("\n")) {
    const line = raw.trim();
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    const item = /^(?:[-*+]|\d+[.)])\s+(.*)$/.exec(line);
    if (!line) flush();
    else if (heading) {
      flush();
      out.push({ kind: "heading", spans: spans(heading[1]) });
    } else if (item) {
      flush();
      out.push({ kind: "item", spans: spans(item[1]) });
    } else para.push(line);
  }
  flush();
  return out;
}
