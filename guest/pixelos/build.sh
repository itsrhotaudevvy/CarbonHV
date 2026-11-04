#!/usr/bin/env bash
# Thin wrapper: the shared AOSP-derived ROM recipe lives in guest/aosp-overlay/build-cvd1.sh.
set -euo pipefail
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/aosp-overlay/build-cvd1.sh" --rom pixelos "$@"
