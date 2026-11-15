#!/bin/bash
# Weekly entry point for the release mirror refresh (launchd calls this).
# The real work lives in publish-release.sh: fetch the latest CI artifacts,
# re-upload to GoFile and file.kiwi, repoint the README table, commit.
set -euo pipefail
SELF=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SELF/.." && pwd)
cd "$REPO"
printf '[%s] refresh start (%s)\n' "$(date '+%F %T')" "$REPO"
exec "$REPO/scripts/publish-release.sh" "$@"
