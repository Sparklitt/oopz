#!/usr/bin/env bash
# Every check: the definition of "done" (CLAUDE.md); CI runs exactly this, `--quick` (pre-commit) the fast ones.
# Written by `claude-setup scaffold` from claude-setup/templates/checks; this copy is the project's own: edit it
# freely (the project's own checks go at the end of quick_project/full_project).
set -euo pipefail
cd "$(dirname "$0")/.."

quick=false
[[ "${1:-}" == "--quick" ]] && quick=true
failed=0
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1"; failed=1; }
has() { command -v "$1" >/dev/null 2>&1; }

check_doc_links() {
  step "doc links"
  local md dir link target
  while IFS= read -r md; do
    dir=$(dirname "$md")
    while IFS= read -r link; do
      target=${link%%#*}
      [[ -z "$target" ]] && continue
      [[ -e "$dir/$target" ]] || fail "$md links to missing $target"
    # Markdown shown as code (fenced blocks, `spans`) links nowhere: dropped before looking for links.
    done < <(awk '/^[[:space:]]*(```|~~~)/ { fence = !fence; next } !fence' "$md" | sed -E 's/`[^`]*`//g' |
             grep -oE '\]\([^)[:space:]]+\)' | sed -E 's/^\]\((.*)\)$/\1/' | grep -vE '^(https?:|mailto:|#)' || true)
  done < <(git ls-files --cached --others --exclude-standard '*.md')
}

# Secrets: gitleaks when installed (staged changes in --quick, the whole tree otherwise), else a pattern scan.
# Never print a match itself: only the file and line.
check_secrets() {
  step "secrets"
  if has gitleaks; then
    if $quick; then
      gitleaks git --pre-commit --staged --redact --no-banner -l warn || fail "gitleaks: possible secret staged"
    else
      # gitleaks dir doesn't read .gitignore, and ignored folders (target/, node_modules/, agent worktrees)
      # can be tens of GB of copies: skip what git ignores, so the tree scanned is the one a commit can hold.
      local cfg ignored
      cfg=$(mktemp)
      ignored=$(git ls-files --others --ignored --exclude-standard --directory |
        sed -e 's|/$||' -e 's/[][\\.^$*+?(){}|]/\\&/g' | paste -sd'|' -)
      printf '[extend]\nuseDefault = true\n[allowlist]\npaths = [%s]\n' \
        "'''^(\\./)?(\\.git${ignored:+|$ignored})(/|$)'''" >"$cfg"
      gitleaks dir --redact --no-banner -l warn --config "$cfg" . || fail "gitleaks: possible secret in the tree"
      rm -f "$cfg"
    fi
    return
  fi
  local patterns hits
  patterns='BEGIN [A-Z ]*PRIVATE KEY|AKIA[0-9A-Z]{16}|sk_live_[0-9A-Za-z]{10,}|gh[pousr]_[0-9A-Za-z]{30,}|xox[bpa]-[0-9A-Za-z-]{10,}|(api[_-]?key|secret|passw(or)?d|token)[[:space:]]*[:=][[:space:]]*"[^"$ {]{16,}"'
  hits=$(git ls-files --cached --others --exclude-standard -z | xargs -0 grep -nIEl "$patterns" -- 2>/dev/null \
    | grep -v '^scripts/check.sh$' || true)
  [[ -z "$hits" ]] || fail "possible secret in: $hits"
}

# Layering: rules in .claude/layers.conf (format: scripts/layers.sh). No file, no rules.
check_layers() {
  [[ -f .claude/layers.conf ]] || return 0
  step "layering (.claude/layers.conf)"
  bash scripts/layers.sh . || fail "layering"
}

# Literals: rules in .claude/literals.conf (format: claude-setup's analyzer/literals.example.conf). No file, no
# scan. Prose belongs in the catalogue, numbers in named constants, values in SQL as bound parameters.
check_literals() {
  [[ -f .claude/literals.conf ]] || return 0
  step "literals (.claude/literals.conf)"
  has python3 || { fail "the literal scan needs python3"; return; }
  python3 -I -B scripts/literals --self-test || fail "the literal scan fails its own known answers"
  python3 -I -B scripts/literals . || fail "literals: hits above (an allow entry needs its reason)"
}

# Unslop: AI-tell prose (rules and format: claude-setup's analyzer/unslop.example.conf). No file, no check.
check_unslop() {
  [[ -f .claude/unslop.conf ]] || return 0
  step "unslop (.claude/unslop.conf)"
  has python3 || { fail "the prose check needs python3"; return; }
  python3 -I -B scripts/unslop.py --self-test >/dev/null || fail "the prose check fails its own known answers"
  python3 -I -B scripts/unslop.py . || fail "unslop: hits above (an allow entry needs its reason)"
}

# Comments: the mechanical half of the comment rule (format: claude-setup's analyzer/comments.example.conf). No
# file, no scan. Needs scripts/literals beside it: the lexer is the literal scan's.
check_comments() {
  [[ -f .claude/comments.conf ]] || return 0
  step "comments (.claude/comments.conf)"
  has python3 || { fail "the comment scan needs python3"; return; }
  python3 -I -B scripts/comments --self-test >/dev/null || fail "the comment scan fails its own known answers"
  python3 -I -B scripts/comments . || fail "comments: hits above (an allow entry needs its reason)"
}

# Invariants: each item under CLAUDE.md's "## Invariants" heading names its enforcer on a `Check: <the check>` line,
# `Check: none (review)` when only a review can; a `Check:` in a code span or a fence is no check. Prints line numbers.
check_invariants() {
  [[ -f CLAUDE.md ]] || return 0
  step "invariants name their check (CLAUDE.md)"
  local missing
  missing=$(awk '
    function flush() { if (item && !ok) printf "%s ", item; item = 0 }
    { sub(/\r$/, "") }
    match($0, /^[[:space:]]*(```+|~~~+)/) {
      m = substr($0, RSTART, RLENGTH); sub(/^[[:space:]]*/, "", m)
      if (fence == "") fence = m; else if (index(m, fence) == 1 && substr(m, 1, 1) == substr(fence, 1, 1)) fence = ""
      blank = 0; next
    }
    fence != "" { next }
    { gsub(/`[^`]*`/, "C") }
    /^ {0,3}#{1,2}[[:space:]]/ { flush(); inv = ($0 ~ /^ {0,3}##[[:space:]]+Invariants([^[:alnum:]]|$)/); next }
    !inv { next }
    /^[[:space:]]*$/ { blank = 1; next }
    # A list line indented less than the text of the current item (at most 3) starts an item; deeper, it is nested.
    match($0, /^ {0,3}([0-9]+[.)]|[-*+])[ \t]+/) && (!item || match($0, /[^ ]/) - 1 < text) {
      flush(); item = NR; ok = 0; match($0, /^ *([0-9]+[.)]|[-*+])[ \t]+/); text = RLENGTH
    }
    blank && /^[^[:space:]]/ && NR != item { flush() }
    { blank = 0 }
    item && /Check:[[:space:]]*[^[:space:]]/ { ok = 1 }
    END { flush() }' CLAUDE.md)
  [[ -z "$missing" ]] || fail "CLAUDE.md, line ${missing% }: an invariant names no check: end it with \
'Check: <the check>' (write the check first; 'Check: none (review)' when only a review can)"
}

quick_project() { :; } # the project's own fast checks
# The desktop app's own scripts (as CI runs them): types, the build, notes and commands in step with Rust.
full_project() {
  local s
  for s in check build check:notes check:commands check:receive-page; do
    step "pnpm $s (apps/desktop)"
    (cd apps/desktop && pnpm -s "$s") || fail "pnpm $s in apps/desktop"
  done
}

STACKS=(rust pnpm)

# --- rust: cargo fmt, clippy with warnings as errors, tests (nextest when installed), cargo deny
# apps/desktop/src-tauri is its own workspace (excluded from the root one: it needs a webview toolkit).
rust_roots=(. apps/desktop/src-tauri)
# An exception to a lint is an #[expect] with a reason: it fails once it no longer fires.
clippy_deny=(-D warnings -D clippy::allow_attributes -D clippy::allow_attributes_without_reason)
# The packages under root $1 that staged or unstaged changes touch: per file, the nearest Cargo.toml above it with a
# [package], unless the file is in a workspace nested inside root (that one's own). No Rust package touched: none.
touched_packages() {
  local root=$1 f d o
  { git diff --cached --name-only; git diff --name-only; } | sort -u | while IFS= read -r f; do
    [[ "$root" == "." || "$f" == "$root"/* ]] || continue
    for o in "${rust_roots[@]}"; do
      [[ "$o" != "$root" && "$o" != "." && "$f" == "$o"/* ]] && continue 2
    done
    d=$(dirname "$f")
    while :; do
      if [[ -f "$d/Cargo.toml" ]] && grep -q '^\[package\]' "$d/Cargo.toml"; then
        sed -n 's/^name *= *"\(.*\)"/\1/p' "$d/Cargo.toml" | head -1
        break
      fi
      [[ "$d" == "." || "$d" == "$root" ]] && break
      d=$(dirname "$d")
    done
  done | sort -u
}
# Tracked .rs files under root $1 with a crate- or module-level #![allow(…)], as file:line: clippy's allow_attributes
# reads outer attributes only.
inner_allows() {
  # git grep exits 1 when nothing matches, the usual case: not a failure under set -euo pipefail.
  git grep -nE '^[[:space:]]*#!\[allow\(' -- ":(glob)$1/**/*.rs" | cut -d: -f1,2 || (($? == 1))
}
quick_rust() {
  local r p pkgs found
  for r in "${rust_roots[@]}"; do
    step "rust lint exceptions expire ($r)"
    found=$(inner_allows "$r")
    [[ -z "$found" ]] || fail "#![allow] in ${found//$'\n'/ }: write #![expect(<lint>, reason = \"…\")], \
which fails once it no longer fires"
    step "rust format ($r)"
    (cd "$r" && cargo fmt --all --check) || fail "cargo fmt in $r (run: cargo fmt --all)"
    # Clippy on the crates this commit touches: the full run lints the workspace, but a finding caught here costs
    # seconds instead of a failed full check later (claude-setup, 2026-11-16).
    pkgs=()
    while IFS= read -r p; do [[ -n "$p" ]] && pkgs+=(-p "$p"); done < <(touched_packages "$r")
    if ((${#pkgs[@]})); then
      step "rust clippy, touched crates ($r)"
      (cd "$r" && cargo clippy "${pkgs[@]}" --all-targets -- "${clippy_deny[@]}") ||
        fail "clippy in $r (touched crates)"
    fi
  done
  shared_rust_quick
}
full_rust() {
  local r
  for r in "${rust_roots[@]}"; do
    step "rust clippy ($r)"
    (cd "$r" && cargo clippy --workspace --all-targets -- "${clippy_deny[@]}") || fail "clippy in $r"
    step "rust tests ($r)"
    if has cargo-nextest; then
      # nextest doesn't run doctests: they run on their own.
      (cd "$r" && cargo nextest run --workspace --no-fail-fast && cargo test --workspace --doc -q) || fail "tests in $r"
    else
      (cd "$r" && cargo test --workspace -q) || fail "tests in $r"
    fi
    step "rust dependencies: licences, advisories, sources ($r)"
    if ! has cargo-deny; then
      echo "skipped: cargo-deny not installed (claude-setup tools)"
    elif [[ -f "$r/deny.toml" ]]; then
      (cd "$r" && cargo deny --log-level error check) || fail "cargo deny in $r"
    else
      (cd "$r" && cargo deny --log-level error check advisories) || fail "cargo deny advisories in $r (no deny.toml)"
    fi
  done
  shared_rust_full
}
# sparklitt's shared checks, copied by `claude-setup scaffold` (sparklitt decisions 0045-0047): copies unchanged,
# build budgets (.claude/build-budget.toml), another repository's crates (.claude/cross-repo.toml).
shared_rust_quick() {
  if [[ -f scripts/copies.py ]]; then
    step "copied scripts match their source (scripts/copies.py)"
    python3 -I -B scripts/copies.py --self-test >/dev/null || fail "scripts/copies.py fails its own known answers"
    python3 -I -B scripts/copies.py || fail "a copied script changed or behind its source"
  fi
  if [[ -f scripts/build-budget.py && -f .claude/build-budget.toml ]]; then
    step "build budgets: crate graphs (.claude/build-budget.toml)"
    python3 -I -B scripts/build-budget.py --self-test >/dev/null || fail "scripts/build-budget.py fails its own known answers"
    python3 -I -B scripts/build-budget.py --graph || fail "a crate graph over its ceiling or forbidden"
  fi
  if [[ -f scripts/cross-repo-deps.py && -f .claude/cross-repo.toml ]]; then
    step "another repository's crates: pinned, once per workspace (.claude/cross-repo.toml)"
    python3 -I -B scripts/cross-repo-deps.py --self-test >/dev/null || fail "scripts/cross-repo-deps.py fails its own known answers"
    python3 -I -B scripts/cross-repo-deps.py || fail "another repository's crates taken another way"
  fi
}
shared_rust_full() {
  if [[ -f scripts/build-budget.py && -f .claude/build-budget.toml ]]; then
    step "build budgets: any compile's memory, rebuild times"
    python3 -I -B scripts/build-budget.py || fail "a build over its budget (scripts/build-budget.py)"
  fi
  if [[ -f scripts/cross-repo-deps.py && -f .claude/cross-repo.toml ]]; then
    step "another repository's checkouts at their pins"
    python3 -I -B scripts/cross-repo-deps.py --pins || fail "a path source's checkout off its pin"
  fi
}

# --- pnpm: a package.json script runs only if the project defines it (--if-present), so the same block fits
# a single package and a workspace (-r).
pnpm_roots=(apps/desktop)
pnpm_run() { # <root> <script>
  local flag=()
  [[ -f "$1/pnpm-workspace.yaml" ]] && flag=(-r)
  # Flags after `run`: pnpm 11 reads `pnpm --if-present run` as a bare `pnpm` (an install).
  (cd "$1" && pnpm "${flag[@]}" run --if-present -s "$2")
}
quick_pnpm() {
  local r
  for r in "${pnpm_roots[@]}"; do
    step "pnpm format:check ($r)"
    pnpm_run "$r" format:check || fail "format in $r (run: pnpm format)"
  done
}
full_pnpm() {
  local r s
  for r in "${pnpm_roots[@]}"; do
    step "pnpm lockfile is current ($r)"
    (cd "$r" && pnpm -s install --frozen-lockfile --prefer-offline >/dev/null) || fail "pnpm-lock.yaml out of date in $r"
    for s in lint typecheck test; do
      step "pnpm $s ($r)"
      pnpm_run "$r" "$s" || fail "$s in $r"
    done
    step "pnpm audit ($r)"
    (cd "$r" && pnpm audit --audit-level high) || fail "pnpm audit in $r (high or critical advisories)"
  done
}

check_doc_links
check_secrets
check_layers
check_literals
check_comments
check_unslop
check_invariants
for s in "${STACKS[@]}"; do "quick_$s"; done
quick_project
if ! $quick; then
  for s in "${STACKS[@]}"; do "full_$s"; done
  full_project
fi

if [[ $failed -ne 0 ]]; then printf '\nSome checks failed.\n'; exit 1; fi
printf '\nAll checks passed.\n'
