#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
qualify="$repo_root/qualification/carbon-qualify.sh"

ready_output="$($qualify --profile "$repo_root/qualification/profiles/reference-development.toml" --dry-run)"
grep -Fq 'qualification=READY' <<<"$ready_output"
grep -Fq 'cpu=hardware-virtualized' <<<"$ready_output"
grep -Fq 'graphics=accelerated' <<<"$ready_output"

set +e
unsupported_output="$($qualify --profile "$repo_root/qualification/profiles/unsupported.toml" --dry-run 2>&1)"
unsupported_status=$?
set -e

test "$unsupported_status" -eq 3
grep -Fq 'qualification=UNSUPPORTED' <<<"$unsupported_output"
grep -Fq 'reason=hardware virtualization unavailable' <<<"$unsupported_output"

echo "qualification dry-run fixtures: PASS"
