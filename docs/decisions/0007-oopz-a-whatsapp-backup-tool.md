# 0007: oopz, a tool for one's own WhatsApp backups

Status: accepted (2026-10-24); point 7 changed 2026-10-31 (published under AGPL-3.0); point 1 changed 2026-10-31
(renamed from `oops` to `oopz`)

Restoring a lost chat (0006) is mostly about WhatsApp backups themselves: opening a crypt15 file with its owner's key,
reading its structure, checking it, putting a chat back. That deserves a tool of its own for one's own backups:
manage them (list, inspect, check health) and restore lost conversations (from the other person's backup, later from
an older backup of one's own, 0008, one chat at a time).

## Decision

1. **Name: `oopz`.** "Oops", what you say when a chat is gone, spelled with a z so the name is its own (it was
   `oops` until publishing). Published at `github.com/sparklitt/oopz`.
2. **A library and an `oopz` binary**: `crates/oopz` and `crates/oopz-cli`, so the library takes no CLI crates.
3. **oopz owns WhatsApp's backup format**: crypt15 (0004), schema detection, opening a backup and reading its chats,
   facts and tables, the merge with its checks and media pack (0006), and the synthetic fixtures. Other programs
   (such as the hilo project oopz came from) read backups through it.
4. **CLI first.** A UI later, if ever. (A desktop app came on 2026-10-31: 0009.)
5. **The rules**: vetted crypto only (no crate beyond 0004 and 0006 without a new record), originals never touched,
   results are new files, output is counts and masked ids, never content, numbers or keys; synthetic data in tests.
6. **Consent.** Used only on a backup with its owner's own key and consent. oopz never tries to get around
   WhatsApp's encryption of anyone's account.
7. **Published** under AGPL-3.0-or-later (was: private). Not affiliated with WhatsApp.

## Consequences

- A new WhatsApp version means updating oopz (format, schema).
- WhatsApp's terms may not allow changing its data: `oopz restore` says so in the README, and a person uses it on
  their own account at their own risk.
