#!/bin/bash
# Bump the version everywhere and open a dated CHANGELOG header.
#
#   scripts/bump-version.sh 0.2.1 "Fixed: DMG signature"
#
# Updates Cargo.toml, tauri.conf.json, package.json, App.tsx VERSION and the
# Settings about line; prepends the release header when missing.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

V=${1:?usage: scripts/bump-version.sh <x.y.z> [\"one-line change\"]}
NOTE=${2:-}
[[ "$V" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "bad version: $V" >&2; exit 2; }

python3 - "$V" <<'PY'
import json, re, sys
v = sys.argv[1]
s = open("Cargo.toml").read()
s = re.sub(r'(\[workspace\.package\][^\[]*?version\s*=\s*")[^"]+(")', r'\g<1>' + v + r'\g<2>', s, count=1, flags=re.S)
open("Cargo.toml", "w").write(s)
for p in ("app/src-tauri/tauri.conf.json", "app/package.json"):
    d = json.load(open(p))
    d["version"] = v
    open(p, "w").write(json.dumps(d, indent=2) + "\n")
a = open("app/src/App.tsx").read()
a = re.sub(r'const VERSION = "[0-9.]+"', f'const VERSION = "{v}"', a)
open("app/src/App.tsx", "w").write(a)
st = open("app/src/pages/Settings.tsx").read()
st = re.sub(r'stripchat-recorder [0-9]+\.[0-9]+\.[0-9]+', f'stripchat-recorder {v}', st)
open("app/src/pages/Settings.tsx", "w").write(st)
PY

today=$(date +%F)
python3 - "$V" "$today" "$NOTE" <<'PY'
import re, sys
v, today, note = sys.argv[1], sys.argv[2], sys.argv[3]
s = open("CHANGELOG.md").read()
header = f"## [{v}] - {today}"
if header not in s:
    s = re.sub(r'\n## \[Unreleased\]\n(?:.*?\n)*?(?=\n## \[)', "\n", s)
    entry = f"\n{header}\n\n"
    if note:
        kind, _, text = note.partition(":")
        kind = kind.strip().lower()
        section = {"fixed": "Fixed", "changed": "Changed", "added": "Added",
                   "removed": "Removed", "security": "Security"}.get(kind, "Changed")
        bullet = text.strip() if text.strip() else note
        entry += f"### {section}\n\n- {bullet}\n\n"
    m = re.search(r'(# Changelog\n\n(?:[^#\n][^\n]*\n\n)?)', s)
    if m:
        s = s[:m.end()] + entry + s[m.end():]
    else:
        s = entry + s
open("CHANGELOG.md", "w").write(s)
PY
echo "bumped to $V"
