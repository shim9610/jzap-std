#!/usr/bin/env bash
# Default is quick; --profile full requests the global replay. No remote CI.
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 scripts/verify.py "$@"
