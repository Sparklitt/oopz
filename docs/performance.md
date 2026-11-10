# Performance

The library (decision 0016) at one million messages: `cargo run --release -p oopz-core --example perf [MESSAGES]`
builds an invented source (500 conversations, 200 people, text in several scripts), imports it into a temporary
library and times the calls the screens and `oopz lib` make. Set `OOPZ_PERF_KEEP=1` to keep the library for a look.

Measured 2026-11-10 on a laptop (WSL2, release build), 1,000,000 messages, `library.db` 571 MB:

| Call | Before | Now |
|---|---|---|
| import | 103 s | 55–80 s (about 15,000 messages a second, with progress and stop) |
| search, a word in a third of the messages | 20–25 s | 0.07–0.09 s |
| search, a rare word or CJK text | < 5 ms | < 5 ms |
| timeline page at offset 900,000 | 20 s | 0.14 s |
| timeline first page | 0.5 s | 1 ms |
| stats (Insights) | 4–6 s | 4 s (one pass over every row) |
| count, conversation list | 0.1 s, 0.8 s | same |
| open a conversation, jump to a date | < 1 ms | same |

What changed: pages and search hits pick their ids first and build rows (message body, names, replies) only for those;
"nulls last" orders are written as `(ts IS NULL), ts …` so the `msg_time` and `msg_newest` indexes serve them; a word
in many messages walks the newest-first index instead of sorting every match; snippets are marked in a small
in-memory index of the hits' own texts; stats group once and sum in Rust instead of six passes.
