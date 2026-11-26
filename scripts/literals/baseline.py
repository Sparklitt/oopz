"""A scan's baseline and allow entries, shared by the literal and comment scans: hits beyond a file's count in the
baseline fail, and so do a count that has fallen (lower it) and an allow entry that allows nothing."""

from collections import Counter


def read_baseline(text, name, rules):
    """{(file, rule): count} from `<count> <rule> <file>` lines; ValueError names the line."""
    base = {}
    for number, raw in enumerate(text.splitlines(), 1):
        parts = raw.split(" ", 2)
        if raw.strip() and not raw.startswith("#"):
            if len(parts) != 3 or not parts[0].isdigit() or parts[1] not in rules:
                raise ValueError(f"{name}:{number}: expected `<count> <rule> <file>`")
            base[(parts[2], parts[1])] = int(parts[0])
    return base


def judge(found, allows, base, names, show, covers):
    """(problems, counts). found: (file, line, col, rule, what) hits; allows: (line no, glob regex, value);
    names: (config, baseline) file names; covers(value, hit): whether an allow entry's value covers the hit."""
    conf, baseline = names
    used, rest = set(), []
    for hit in found:
        match = [a[0] for a in allows if a[1].fullmatch(hit[0]) and covers(a[2], hit)]
        used.update(match)
        if not match:
            rest.append(hit)
    counts = Counter((h[0], h[3]) for h in rest)
    problems = []
    for key in sorted(set(counts) | set(base)):
        got, had = counts.get(key, 0), base.get(key, 0)
        if got > had:
            if had:
                problems.append(f"{key[0]}: {key[1]}: {got} hits, the baseline allows {had}; the new ones are among:")
            problems += [show(*h) for h in rest if (h[0], h[3]) == key]
        elif got < had:
            problems.append(f"{baseline}: {key[0]} {key[1]}: {got} hits, baseline {had}: lower it (--baseline)")
    problems += [f"{conf}:{a[0]}: stale allow entry (allows nothing)" for a in allows if a[0] not in used]
    return problems, counts


def write_baseline(path, counts, header):
    lines = [f"{n} {rule} {rel}" for (rel, rule), n in sorted(counts.items())]
    path.write_text(f"# {header}\n" + "\n".join(lines) + "\n", encoding="utf-8")
    return f"{sum(counts.values())} hits in {len(lines)} lines"
