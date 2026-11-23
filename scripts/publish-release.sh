#!/bin/bash
# Build the current version (or take an artifact directory), upload it to
# Cloudflare R2, rewrite the README table and docs/downloads.json, commit,
# push, and delete the superseded local artifacts and older R2 versions.
#
#   scripts/publish-release.sh --artifacts /path/to/dist
#   scripts/publish-release.sh                 # build the current version first
#
# Environment:
#   SKIP_PUSH=1    commit but do not push
set -euo pipefail

SELF=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SELF/.." && pwd)
cd "$REPO"

ARTIFACTS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --artifacts) ARTIFACTS=$2; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

log() { printf '[%s] %s\n' "$(date '+%F %T')" "$*" >&2; }
die() { log "FAILED: $*"; exit 1; }

for tool in python3 curl git shasum tar; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool not found on PATH ($PATH)"
done

V=$(python3 - <<'PY'
import json
print(json.load(open("app/src-tauri/tauri.conf.json"))["version"])
PY
)
[ -n "$V" ] || die "could not read the version"

RUN=".release-${V}-$(date +%Y%m%d)"
LINKS="$RUN/links.txt"
mkdir -p "$RUN"
touch "$LINKS"

LOCK=.release-publish.lock
if ! mkdir "$LOCK" 2>/dev/null; then
  pid=$(cat "$LOCK/pid" 2>/dev/null || true)
  if [ -n "${pid:-}" ] && kill -0 "$pid" 2>/dev/null; then
    log "another publish is already running (pid $pid) - exiting"
    exit 0
  fi
  rm -rf "$LOCK"
  mkdir "$LOCK"
fi
printf '%s\n' "$$" > "$LOCK/pid"
trap 'rm -rf "$LOCK"' EXIT

find . -maxdepth 1 -name '.release-*' -type d -mtime +30 -exec rm -rf {} + 2>/dev/null || true


log "publishing Stripchat Recorder $V"

# --- 1. Artifacts ---------------------------------------------------------------
if [ -n "$ARTIFACTS" ]; then
  [ -d "$ARTIFACTS" ] || die "no artifact directory at $ARTIFACTS"
  DIST=$(cd "$ARTIFACTS" && pwd)
else
  DIST="$RUN/artifacts"
  rm -rf "$DIST"
  mkdir -p "$DIST"
  ARCH=$(uname -m)
  case "$ARCH" in
    arm64) PLATARCH=arm64 ;;
    x86_64) PLATARCH=x64 ;;
    *) die "unsupported macOS arch: $ARCH" ;;
  esac
  log "building locally for macOS $PLATARCH (no CI)"
  # ffmpeg is not bundled: the app uses the system binary and shows its path.
  ( cd app && npm run build >/dev/null 2>&1 && npx tauri build --bundles app >/dev/null 2>&1 ) \
    || die "tauri build failed"
  APP=$(find target/release/bundle/macos -name "*.app" | head -1)
  [ -n "$APP" ] || die "no app bundle produced"
  codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
  STAGE=$(mktemp -d)
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  hdiutil create -volname "Stripchat Recorder" -srcfolder "$STAGE" -ov -format UDZO \
    "$DIST/stripchat-recorder-macos-$PLATARCH.dmg" >/dev/null || die "installer DMG failed"
  rm -rf "$STAGE"
  cargo build --release -p stripchat-recorder-cli >/dev/null 2>&1 || die "cli build failed"
  tar -czf "$DIST/stripchat-recorder-cli-macos-$PLATARCH.tar.gz" -C target/release stripchat-recorder
  # Cross-platform artifacts are built on demand (or copied from a previous
  # run kept in dist/); the session publishes whatever exists.
  if [ -d dist ]; then
    for f in dist/*.AppImage dist/*.deb dist/*setup.exe dist/*linux*.tar.gz dist/*windows*.zip; do
      [ -f "$f" ] && cp "$f" "$DIST/"
    done
  fi
fi

FILES=()
for f in "$DIST"/*.dmg "$DIST"/*.tar.gz "$DIST"/*.zip "$DIST"/*.AppImage "$DIST"/*.deb "$DIST"/*setup.exe; do
  [ -f "$f" ] && FILES+=("$(basename "$f")")
done
[ ${#FILES[@]} -gt 0 ] || die "no release files in $DIST"
# Normalize CI names to stripchat-recorder-<version>-<platform>.<ext>.
normalize() {
  local f=$1 base
  base=$(basename "$f")
  case "$base" in
    stripchat-recorder-macos-*)     mv "$f" "$DIST/stripchat-recorder-${V}-${base#stripchat-recorder-macos-}" ;;
    stripchat-recorder-cli-*)       case "$base" in *"${V}"*) : ;; *) mv "$f" "$DIST/stripchat-recorder-cli-${V}-${base#stripchat-recorder-cli-}" ;; esac ;;
    "Stripchat Recorder_"*amd64.AppImage) mv "$f" "$DIST/stripchat-recorder-${V}-linux-x64.AppImage" ;;
    "Stripchat Recorder_"*amd64.deb)      mv "$f" "$DIST/stripchat-recorder-${V}-linux-x64.deb" ;;
    "Stripchat Recorder_"*x64-setup.exe)  mv "$f" "$DIST/stripchat-recorder-${V}-x64-setup.exe" ;;
  esac
}
for f in "${FILES[@]}"; do normalize "$DIST/$f"; done
FILES=()
for f in "$DIST"/*.dmg "$DIST"/*.tar.gz "$DIST"/*.zip "$DIST"/*.AppImage "$DIST"/*.deb "$DIST"/*setup.exe; do
  [ -f "$f" ] && FILES+=("$(basename "$f")")
done
log "artifacts: ${FILES[*]}"

# --- 2. Upload to Cloudflare R2 ---------------------------------------------------
# R2 is the only host. Keys are versioned and immutable, so a link stays valid
# until the app ships a new version - nothing needs a timed refresh. A publish
# uploads under <prefix>/<version>/, then prunes older versions (keep 2).
R2_ENV=${R2_ENV:-$HOME/.r2.env}
R2_BUCKET=${R2_BUCKET:-torrent-search-covers-eu}
R2_PREFIX=${R2_PREFIX:-stripchat-recorder/releases}
R2_PUBLIC=${R2_PUBLIC:-https://covers.debrid.in}
[ -f "$R2_ENV" ] || die "missing $R2_ENV (R2 S3 keys)"
set -a; . "$R2_ENV"; set +a
for v in R2_S3_ENDPOINT R2_S3_ACCESS_KEY_ID R2_S3_SECRET_ACCESS_KEY; do
  eval "val=\${$v:-}"
  [ -n "$val" ] || die "$v missing from $R2_ENV"
done
command -v aws >/dev/null 2>&1 || die "aws CLI not found on PATH ($PATH)"
# R2 rejects the extra CRC headers modern aws-cli injects on some builds.
export AWS_REQUEST_CHECKSUM_CALCULATION=when_required AWS_RESPONSE_CHECKSUM_VALIDATION=when_required

r2_aws() {
  AWS_ACCESS_KEY_ID="$R2_S3_ACCESS_KEY_ID" AWS_SECRET_ACCESS_KEY="$R2_S3_SECRET_ACCESS_KEY" \
  AWS_DEFAULT_REGION=auto AWS_EC2_METADATA_DISABLED=true \
    aws "$@" --endpoint-url "$R2_S3_ENDPOINT"
}

r2_put() {  # $1=local file -> prints the public URL
  local key="$R2_PREFIX/$V/$(basename "$1")"
  log "  R2 -> s3://$R2_BUCKET/$key"
  r2_aws s3 cp "$1" "s3://$R2_BUCKET/$key" \
    --content-type application/octet-stream \
    --cache-control 'public, max-age=31536000, immutable' \
    --only-show-errors || return 1
  printf '%s\n' "$R2_PUBLIC/$key"
}

r2_prune() {  # keep the newest $1 versions under the prefix (default 2)
  local keep=${1:-2} versions total
  versions=$(r2_aws s3api list-objects-v2 --bucket "$R2_BUCKET" \
      --prefix "$R2_PREFIX/" --delimiter / \
      --query 'CommonPrefixes[].Prefix' --output text 2>/dev/null \
    | tr '\t' '\n' | sed 's#/$##' | awk -F/ '{print $NF}' \
    | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' | sort -t. -k1,1n -k2,2n -k3,3n) || true
  total=$(printf '%s\n' "$versions" | grep -c . || true)
  [ "${total:-0}" -gt "$keep" ] || return 0
  printf '%s\n' "$versions" | sed -n "1,$((total-keep))p" | while IFS= read -r old; do
    log "R2: pruning $R2_PREFIX/$old/"
    r2_aws s3 rm "s3://$R2_BUCKET/$R2_PREFIX/$old/" --recursive --only-show-errors \
      || log "WARNING: R2 prune failed for $old (old objects left in place)"
  done
}

human_size() {
  python3 - "$1" <<'PY'
import os, sys
b = os.path.getsize(sys.argv[1])
for unit in ("B", "KiB", "MiB", "GiB"):
    if b < 1024 or unit == "GiB":
        print(f"{b:.1f} {unit}" if unit != "B" else f"{b} B")
        break
    b /= 1024.0
PY
}

row() {
  grep -v -F "$1 |" "$LINKS" > "$LINKS.tmp" 2>/dev/null || true
  mv "$LINKS.tmp" "$LINKS"
  printf '%s | %s | %s | %s\n' "$1" "$2" "$3" "$4" >> "$LINKS"
}

: > "$LINKS"
for f in "${FILES[@]}"; do
  size=$(human_size "$DIST/$f")
  sha=$(shasum -a 256 "$DIST/$f" | cut -d' ' -f1)
  log "[upload] $f ($size)"
  url=$(r2_put "$DIST/$f") || die "R2 upload failed for $f"
  row "$f" "$url" "$size" "$sha"
done

# --- 3. Rewrite the README table -------------------------------------------------
python3 -I scripts/patch-release-links.py "$LINKS" "$V" || die "could not patch README"

# --- 4. Commit and push ----------------------------------------------------------
if git diff --quiet -- README.md docs/downloads.json; then
  log "download links already current - nothing to commit"
else
  git add README.md docs/downloads.json
  git commit -q -m "docs: publish ${V} downloads to Cloudflare R2

One host, versioned keys: the links stay valid until the next version, so no
weekly mirror refresh is needed any more." || die "git commit failed"
  if [ "${SKIP_PUSH:-0}" = "1" ]; then
    log "SKIP_PUSH=1: links committed locally; not pushed"
  else
    git push -q origin main || die "git push failed"
    log "pushed refreshed ${V} links"
    # Prove the live site actually serves the new feed. Pages caches for ten
    # minutes, so a cache-busted request is the only reliable check.
    log "verifying the live site feed"
    sleep 20
    LIVE=$(curl -fsS "https://ghostyshell.github.io/stripchat-record/downloads.json?v=$(date +%s)" 2>/dev/null \
      | python3 -c 'import json,sys
try:
    print(json.load(sys.stdin).get("version",""))
except Exception:
    print("")' 2>/dev/null || true)
    if [ "$LIVE" = "$V" ]; then
      log "live site feed serves $V"
    else
      log "WARNING: live site feed shows '${LIVE:-unknown}' - Pages may need a minute; re-run to confirm"
    fi
  fi
fi

log "pruning old R2 versions (keeping the current and previous)"
r2_prune 2

# The rebuild's output is disposable now that R2 has it. An external
# --artifacts directory is left alone, it is not ours to delete.
if [ "${KEEP_DIST:-0}" = "1" ]; then
  log "KEEP_DIST=1: artifacts kept at $DIST"
elif [ -z "$ARTIFACTS" ]; then
  log "removing local artifacts"
  rm -rf "$DIST"
fi
log "done"
