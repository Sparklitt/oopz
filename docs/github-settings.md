# GitHub settings

What to set by hand on GitHub after the repository is first pushed to `github.com/sparklitt/oopz`; none of it lives in
the code.

## About (the gear next to "About" on the repository page)

- **Description:** Open your own WhatsApp backups with your own key, check them, and bring a lost chat back. Offline.
- **Website:** `https://WEBSITE` (placeholder until the landing page is live; leave empty otherwise).
- **Topics:** `whatsapp-backup`, `backup`, `crypt15`, `data-recovery`, `chat-recovery`, `privacy`, `offline-first`,
  `sqlite`, `cli`, `rust`, `command-line-tool`, `agpl`
- Tick **Releases**; untick **Packages** and **Deployments** until they are used.

## Social preview

Settings → General → Social preview → **Edit** → **Upload an image** → `assets/social-preview.png` (1280×640). Rebuild
it after changing the banner:

```sh
python3 assets/make_assets.py
pnpm dlx @resvg/resvg-js-cli --fit-width 1280 assets/banner-light.svg assets/social-preview.png
```

## Features (Settings → General)

- Issues: on. Discussions: optional. Wiki: off (docs live in the repository). Projects: off.
- Pull requests: allow **squash merging** only, with "Default to pull request title"; tick **Always suggest updating
  pull request branches** and **Automatically delete head branches**.

## Branch protection (Settings → Rules → Rulesets → New branch ruleset)

Target: the default branch (`main`).

- Restrict deletions; block force pushes.
- Require a pull request before merging (1 approval once there is more than one maintainer; dismiss stale approvals).
- Require status checks to pass, up to date with `main`: `fmt`, `clippy`, `test (ubuntu-latest)`,
  `test (macos-latest)`, `test (windows-latest)`, `msrv`.
- Require linear history. Optionally require signed commits.

## Security (Settings → Code security)

- **Private vulnerability reporting: enable.** `SECURITY.md` and the issue chooser send reporters to it.
- Dependency graph and **Dependabot alerts**: on. **Dependabot security updates**: on (version updates come from
  `.github/dependabot.yml`, weekly, for cargo and GitHub Actions).
- **Secret scanning** and **push protection**: on.
- Code scanning with CodeQL (it supports Rust): optional, default setup.

## Actions (Settings → Actions → General)

- Workflow permissions: **Read repository contents** (the workflow asks for no more).
- Tick "Require approval for all outside collaborators" for fork pull requests.

## Community

- `CODE_OF_CONDUCT.md`: replace `CONTACT-EMAIL` with an address the maintainers read before going public.
- `.github/FUNDING.yml`: uncomment the platforms used, or delete the file.
- Check Insights → Community standards: every item should be ticked.
