#!/bin/bash
# Fail the commit when product code is staged without a CHANGELOG diff, or
# when the version files disagree. Mirrors the sibling repos' pre-commit rule.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

staged=$(git diff --cached --name-only)
[ -n "$staged" ] || exit 0

product=$(printf '%s\n' "$staged" | grep -E '^(core/|cli/|app/src/|app/src-tauri/(src|capabilities)/|app/src-tauri/tauri\.conf\.json|scripts/|\.github/workflows/)' || true)
changelog=$(printf '%s\n' "$staged" | grep -x 'CHANGELOG.md' || true)

if [ -n "$product" ] && [ -z "$changelog" ]; then
  echo "pre-commit: product changes need a CHANGELOG.md entry in the same commit." >&2
  echo "  bump: scripts/bump-version.sh <x.y.z> \"<one-line change>\"" >&2
  echo "  staged product files:" >&2
  printf '    %s\n' $product >&2
  exit 1
fi

# Version consistency across every mirrored location.
cargo_v=$(python3 - <<'PY'
import re
s = open("Cargo.toml").read()
m = re.search(r'\[workspace\.package\][^\[]*?version\s*=\s*"([^"]+)"', s, re.S)
print(m.group(1) if m else "")
PY
)
conf_v=$(python3 -c 'import json;print(json.load(open("app/src-tauri/tauri.conf.json"))["version"])')
pkg_v=$(python3 -c 'import json;print(json.load(open("app/package.json"))["version"])')
app_v=$(grep -oE 'const VERSION = "[0-9.]+"' app/src/App.tsx | grep -oE '[0-9.]+' || true)

# App.tsx only carries a version when the UI shows a hardcoded one; this app
# reads it from the backend (snap.version), so an empty match is fine.
if [ "$cargo_v" != "$conf_v" ] || [ "$cargo_v" != "$pkg_v" ] || { [ -n "$app_v" ] && [ "$cargo_v" != "$app_v" ]; }; then
  echo "pre-commit: version mismatch (Cargo.toml=$cargo_v tauri=$conf_v package=$pkg_v App.tsx=$app_v)" >&2
  echo "  run: scripts/bump-version.sh $cargo_v" >&2
  exit 1
fi

# The changelog's top header must match the current version.
top=$(grep -m1 -oE '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' CHANGELOG.md | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true)
if [ -n "$top" ] && [ "$top" != "$cargo_v" ]; then
  echo "pre-commit: CHANGELOG top header ($top) does not match version $cargo_v" >&2
  exit 1
fi
exit 0
