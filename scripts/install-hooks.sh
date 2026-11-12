#!/usr/bin/env bash
# Turns on this repo's git hooks (.githooks/) for this clone. Run once after cloning.
# A script on purpose: Claude's guard-git hook blocks setting the hooks path inline, so it can't be pointed
# elsewhere to skip them; running this file is the one sanctioned way to turn them on.
set -euo pipefail
cd "$(dirname "$0")/.."
chmod +x .githooks/*
# A linked worktree shares its config with the main checkout, which may not have .githooks (another
# branch): there the setting goes in the worktree's own config.
if [ "$(git rev-parse --git-dir)" != "$(git rev-parse --git-common-dir)" ]; then
  git config extensions.worktreeConfig true
  git config --worktree core.hooksPath .githooks
else
  git config core.hooksPath .githooks
fi
echo "hooks on: $(git config --get core.hooksPath)"
