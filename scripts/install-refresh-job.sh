#!/bin/bash
# Install (or remove) the weekly download-link refresh as a user launchd agent.
# Default: every Sunday at 03:47, wrapped in caffeinate -i.
#
#   scripts/install-refresh-job.sh
#   scripts/install-refresh-job.sh --time 04:15 --weekday 1
#   scripts/install-refresh-job.sh --uninstall
set -euo pipefail

SELF=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SELF/.." && pwd)
LABEL=click.ghostyshell.stripchatrecorder.weekly-refresh
WEEKDAY=0
HOUR=3
MINUTE=47
UNINSTALL=0
LOAD=1

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) REPO=$2; shift 2 ;;
    --label) LABEL=$2; shift 2 ;;
    --time) HOUR=${2%%:*}; MINUTE=${2##*:}; shift 2 ;;
    --weekday) WEEKDAY=$2; shift 2 ;;
    --uninstall) UNINSTALL=1; shift ;;
    --no-load) LOAD=0; shift ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
UID_NUM=$(id -u)

if [ "$UNINSTALL" = "1" ]; then
  launchctl bootout "gui/$UID_NUM/$LABEL" 2>/dev/null || true
  rm -f "$PLIST"
  echo "removed $LABEL"
  exit 0
fi

mkdir -p "$HOME/Library/LaunchAgents"
cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/bin/caffeinate</string>
    <string>-i</string>
    <string>$REPO/scripts/refresh-release-links.sh</string>
  </array>
  <key>WorkingDirectory</key><string>$REPO</string>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Weekday</key><integer>$WEEKDAY</integer>
    <key>Hour</key><integer>$HOUR</integer>
    <key>Minute</key><integer>$MINUTE</integer>
  </dict>
  <key>StandardOutPath</key><string>$REPO/.release-refresh.log</string>
  <key>StandardErrorPath</key><string>$REPO/.release-refresh.log</string>
  <key>RunAtLoad</key><false/>
</dict>
</plist>
EOF

if [ "$LOAD" = "1" ]; then
  launchctl bootout "gui/$UID_NUM/$LABEL" 2>/dev/null || true
  launchctl bootstrap "gui/$UID_NUM" "$PLIST"
fi
echo "installed $LABEL (weekday $WEEKDAY at $(printf '%02d:%02d' "$HOUR" "$MINUTE"))"
