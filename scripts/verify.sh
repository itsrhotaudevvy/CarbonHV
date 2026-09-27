#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

echo "[1/9] Rust formatting"
cargo fmt --all -- --check

echo "[2/9] Rust tests"
cargo test

echo "[3/9] Bundle manifests"
cargo run -q -p carbon-cli -- bundle verify bundles/grapheneos/bundle.toml
cargo run -q -p carbon-cli -- bundle verify bundles/ubuntu-touch/bundle.toml

echo "[4/9] Guest contracts"
cargo run -q -p carbon-cli -- guest verify guest/android/device/cvd1.toml
cargo run -q -p carbon-cli -- guest verify guest/ubuntu-touch/device/cvd1.toml

echo "[5/9] Shell contracts"
bash -n guest/android/build.sh
bash -n guest/ubuntu-touch/build.sh
bash -n qualification/carbon-qualify.sh
bash -n qualification/test-dry-run.sh
bash -n qualification/gpu/run-minimal.sh
bash -n qualification/gpu/run-sustained.sh
bash -n qualification/gpu/test-sustained.sh
bash -n qualification/gpu/collect-report.sh

echo "[6/9] Qualification fixtures"
qualification/test-dry-run.sh

echo "[7/9] GPU proof fixtures"
gpu_pass="$(qualification/gpu/run-minimal.sh --fixture qualification/gpu/fixtures/mali-pass.env --dry-run)"
grep -qx 'gpu.vulkan=true' <<<"$gpu_pass"
grep -qx 'gpu.native-buffer=true' <<<"$gpu_pass"
grep -qx 'gpu.zero-copy=false' <<<"$gpu_pass"
grep -qx 'gpu.qualified=false' <<<"$gpu_pass"

gpu_no_proof="$(qualification/gpu/run-minimal.sh --fixture qualification/gpu/fixtures/mali-no-hardware-proof.env --dry-run)"
grep -qx 'gpu.vulkan=false' <<<"$gpu_no_proof"
grep -qx 'gpu.native-buffer=false' <<<"$gpu_no_proof"
grep -qx 'gpu.zero-copy=false' <<<"$gpu_no_proof"
grep -qx 'gpu.sustained=false' <<<"$gpu_no_proof"
grep -qx 'gpu.qualified=false' <<<"$gpu_no_proof"

bash qualification/gpu/test-sustained.sh

echo "[8/9] Android unit tests"
if [[ -z "${JAVA_HOME:-}" ]]; then
  if command -v brew >/dev/null && [[ -d "$(brew --prefix openjdk@17 2>/dev/null)/libexec/openjdk.jdk/Contents/Home" ]]; then
    export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
  fi
fi
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17 or newer before running Android verification}"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$repo_root/.gradle-local}"
./gradlew --no-daemon :android:domain:test :android:app:testDebugUnitTest

echo "[9/9] Android debug APK"
./gradlew --no-daemon :android:app:assembleDebug
test -f android/app/build/outputs/apk/debug/app-debug.apk

echo "CarbonHV verification: PASS"
