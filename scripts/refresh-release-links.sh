#!/bin/bash
# Entry point for release publishing (launchd calls this).
# The real work lives in publish-release.sh: build or fetch the artifacts,
# upload them to Cloudflare R2, repoint the README table and manifest, commit.
set -euo pipefail
SELF=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SELF/.." && pwd)
cd "$REPO"

# R2 links never expire, so there is nothing to renew on a timer. Publish only
# when the repo version is ahead of the live feed; FORCE=1 overrides.
v=$(python3 -c 'import json;print(json.load(open("app/src-tauri/tauri.conf.json"))["version"])')
live=$(curl -fsS "https://ghostyshell.github.io/stripchat-record/downloads.json?v=$(date +%s)" 2>/dev/null \
  | python3 -c 'import json,sys
try: print(json.load(sys.stdin).get("version",""))
except Exception: print("")' 2>/dev/null || true)
if [ "${FORCE:-0}" != 1 ] && [ "$live" = "$v" ]; then
  printf '[%s] %s is already live; nothing to publish\n' "$(date '+%F %T')" "$v"
  exit 0
fi
exec "$REPO/scripts/publish-release.sh" "$@"
