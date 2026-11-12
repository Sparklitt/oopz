#!/usr/bin/env bash
# Layering check: which manifests may not depend on what. Rules in <dir>/.claude/layers.conf, one per line:
#   <glob of manifest files> !-> <ERE of dependency names> [+dev]
# (format and examples: analyzer/layers.example.conf). Standalone use, exit 1 on a violation:
#   bash analyzer/lib/layers.sh [dir]
# project-report sources this file and calls check_layers.

# Dependency names a manifest declares, one per line. With dev=1 also dev-dependencies.
manifest_deps() {
  local f=$1 dev=$2
  case ${f##*/} in
    Cargo.toml)
      # [dependencies], [build-dependencies], [target.'cfg(..)'.dependencies], [dependencies.foo]
      awk -v dev="$dev" '
        /^[[:space:]]*\[/ {
          s = $0; gsub(/^[[:space:]]*\[+|\]+[[:space:]]*(#.*)?$/, "", s)
          on = 0; name = ""
          n = split(s, p, ".")
          for (i = 1; i <= n; i++) {
            if (p[i] == "dependencies" || p[i] == "build-dependencies" || (dev == 1 && p[i] == "dev-dependencies")) {
              on = 1; if (i < n) { name = p[i + 1]; gsub(/"/, "", name); print name; on = 0 }
              break
            }
          }
          next
        }
        on && /^[[:space:]]*"?[A-Za-z0-9_-]+"?[[:space:]]*(\.|=)/ {
          s = $0; sub(/^[[:space:]]*"?/, "", s); sub(/"?[[:space:]]*(\.|=).*/, "", s); print s
        }' "$f" ;;
    package.json)
      jq -r --argjson dev "$dev" '((.dependencies // {}) + (.peerDependencies // {})
        + (if $dev == 1 then .devDependencies // {} else {} end)) | keys[]' "$f" ;;
    build.gradle|build.gradle.kts)
      grep -oE "project\([[:space:]]*(path[[:space:]]*[:=][[:space:]]*)?[\"'][^\"']+" "$f" | sed -E "s/.*[\"']//" ;;
  esac
}

# check_layers <dir> [rules file]: prints "manifest: depends on X (rule N)" per violation, N being the
# rule's line in the file. Returns 1 on a violation, 2 on a bad rule.
check_layers() {
  local dir=$1 conf=${2:-$1/.claude/layers.conf} line n=0 glob re dev bad=0 f d rc
  [ -f "$conf" ] || { echo "no rules: ${conf#"$dir"/} not found"; return 0; }
  while IFS= read -r line || [ -n "$line" ]; do
    n=$((n + 1)); line=${line%$'\r'}
    [[ $line =~ ^[[:space:]]*(#|$) ]] && continue
    if [[ ! $line =~ ^[[:space:]]*([^[:space:]]+)[[:space:]]+!-\>[[:space:]]+(.+[^[:space:]])[[:space:]]*$ ]]; then
      echo "layers.conf:$n: not '<glob> !-> <regex>'"; return 2
    fi
    glob=${BASH_REMATCH[1]} re=${BASH_REMATCH[2]} dev=0
    if [[ $re == *" +dev" ]]; then dev=1; re=${re% +dev}; re=${re%"${re##*[![:space:]]}"}; fi
    # grep -E exits 2 on a bad pattern: catch it once, before the manifests.
    rc=0; grep -qE -- "$re" <<<"" 2>/dev/null || rc=$?
    [ "$rc" != 2 ] || { echo "layers.conf:$n: bad regex: $re"; return 2; }
    while IFS= read -r -d '' f; do
      while IFS= read -r d; do
        if [ -n "$d" ] && grep -qE -- "$re" <<<"$d"; then echo "$f: depends on $d (rule $n)"; bad=1; fi
      done < <(manifest_deps "$dir/$f" "$dev")
    done < <(cd "$dir" && shopt -s globstar nullglob && IFS= && for f in $glob; do [ -f "$f" ] && printf '%s\0' "$f"; done)
  done <"$conf"
  [ "$bad" = 0 ]
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  set -euo pipefail
  check_layers "${1:-.}"
fi
