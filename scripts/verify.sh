#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

echo "[1/8] Rust formatting"
cargo fmt --all -- --check

echo "[2/8] Rust tests"
cargo test

echo "[3/8] Bundle manifests"
cargo run -q -p carbon-cli -- bundle verify bundles/grapheneos/bundle.toml
cargo run -q -p carbon-cli -- bundle verify bundles/ubuntu-touch/bundle.toml

echo "[4/8] Guest contracts"
cargo run -q -p carbon-cli -- guest verify guest/android/device/cvd1.toml
cargo run -q -p carbon-cli -- guest verify guest/ubuntu-touch/device/cvd1.toml

echo "[5/8] Shell contracts"
bash -n guest/android/build.sh
bash -n guest/ubuntu-touch/build.sh
bash -n qualification/carbon-qualify.sh
bash -n qualification/test-dry-run.sh

echo "[6/8] Qualification fixtures"
qualification/test-dry-run.sh

echo "[7/8] Android unit tests"
if [[ -z "${JAVA_HOME:-}" ]]; then
  if command -v brew >/dev/null && [[ -d "$(brew --prefix openjdk@17 2>/dev/null)/libexec/openjdk.jdk/Contents/Home" ]]; then
    export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
  fi
fi
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17 or newer before running Android verification}"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$repo_root/.gradle-local}"
./gradlew --no-daemon :android:domain:test :android:app:testDebugUnitTest

echo "[8/8] Android debug APK"
./gradlew --no-daemon :android:app:assembleDebug
test -f android/app/build/outputs/apk/debug/app-debug.apk

echo "CarbonHV verification: PASS"
