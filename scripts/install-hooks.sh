#!/bin/bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
git config core.hooksPath .githooks
chmod +x .githooks/pre-commit scripts/*.sh
echo "installed .githooks pre-commit hook"
