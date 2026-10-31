# Security

oops handles people's private backups and their keys, so we take reports seriously.

## Reporting a vulnerability

Please report privately, not in a public issue: use GitHub's **"Report a vulnerability"** button on the repository's
Security tab (private vulnerability reporting). Describe the problem and how to reproduce it **with synthetic data
only**. Never send a real backup, a real key, real media or anyone's phone number; if a real backup is the only way you
found it, describe its shape (WhatsApp version, which tables or columns) instead.

We aim to acknowledge a report within a week and to publish a fix and an advisory once it is ready, crediting you if
you wish.

## In scope

- A key, message text, media or phone number appearing in oops's output, errors or files it writes.
- An original backup or `Media/` folder being modified, or an existing file overwritten.
- A crafted backup file making oops crash unsafely, write outside the paths it was given, or produce a result that
  passes its checks while being damaged.
- Weaknesses in how keys are read, held or wiped.

## Out of scope

- Getting into a backup without its owner's key. oops never tries to, and won't accept changes that do.
