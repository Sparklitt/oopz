# Next session: start here

**Since session 8 (2026-11-11) the plan for oopz, hilo and msgkinds lives in one place:
`~/projects/msgkinds/docs/roadmap.md`.** Read it first: state of each repository, checks and their last results,
what was done, the next steps in order (hilo's Kotlin kinds, then decision 0002 phases 1–3), what makes the project
professional, what's still to verify on real data, and the decisions waiting on the owner. What follows is the
plan from before session 7, kept for the product items (2–6) the roadmap points back to.

State (2026-11-11): main at the commit after ee2245a, not pushed. All checks green at 341042d (workspace and
src-tauri fmt/clippy/test; pnpm check, build, check:notes, check:commands). A dev build is installed on the user's
Windows (per-user, `oopz` on PATH). On the S22+ both the Wi-Fi page and the adb copy (wireless debugging, 998 MB of
Databases) worked.

Read first: [gaps.md](gaps.md) (what's missing and every edge case), [data-types.md](data-types.md), decisions
0013 (one model), 0006 (restore refuses groups), 0019 (platform-tools on request), hilo's 0002, 0028, 0029, 0034.

Order of work:
1. **The shared data-type registry**, for oopz and hilo: check both licences, decide the shared crate's home (its
   own repository, a git dependency for both), map every type per app and per app version, and the tests (every
   adapter row maps, every entry renders and exports, unknown types show a labelled card, unknown versions open).
   Fixes on the way: stickers ("0:00", no image), every type listed in gaps.md.
2. **Names and photos**: the phone's address book over adb (every account: Google, Samsung, SIM…), vCard 2.1/3.0/4.0,
   Google and Outlook CSV; the full number when there's no name.
3. **The phone page**: the app's design (it's still navy and coral; long paths overflow), and "Send to a computer"
   in the oopz phone app scanning the same QR; the browser page stays.
4. **Library speed** on real data (measure first), and "Show as one chat" for a linked contact.
5. **Moving to another phone**, fully flexible (any contact, group, chat, dates, types, media): a decision first.
6. The edge cases in gaps.md, then the release (CHANGELOG, 0.2.0).

Rules: invented data only in tests and pictures; the S9+ (`adb -s 224ab8793b027ece`) for automated phone tests, the
user's S22+ only with them; never print keys; small commits; push only when asked; Windows builds need
`--runner cargo-xwin` for `pnpm sidecar` too (see the handoff memory).
