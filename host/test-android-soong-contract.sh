#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workspace_edition="$(python3 - "$repo_root/Cargo.toml" <<'PY'
import sys, tomllib
from pathlib import Path
with Path(sys.argv[1]).open('rb') as f:
    data = tomllib.load(f)
print(data['workspace']['package']['edition'])
PY
)"

seen=0
while IFS= read -r edition; do
  seen=$((seen + 1))
  if [[ "$edition" != "$workspace_edition" ]]; then
    echo "Android.bp Rust edition $edition does not match workspace edition $workspace_edition" >&2
    exit 1
  fi
done < <(sed -n 's/^[[:space:]]*edition: "\([0-9][0-9]*\)",/\1/p' "$repo_root/Android.bp")

if [[ "$seen" -lt 4 ]]; then
  echo "expected Rust edition declarations for all CarbonHV Android Rust modules" >&2
  exit 1
fi

grep -Fq '"dev.carbonhv.broker-rust"' "$repo_root/Android.bp"
grep -Fq '"dev.carbonhv.host.runtime-rust"' "$repo_root/Android.bp"
grep -Fq '"libbinder_rs"' "$repo_root/Android.bp"

echo 'Android Soong contract: PASS'
