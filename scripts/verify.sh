#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

echo "[1/10] Rust formatting"
cargo fmt --all -- --check

echo "[2/10] Rust tests"
cargo test

echo "[3/10] Bundle manifests"
cargo run -q -p carbon-cli -- bundle verify bundles/grapheneos/bundle.toml
cargo run -q -p carbon-cli -- bundle verify bundles/ubuntu-touch/bundle.toml
cargo run -q -p carbon-cli -- bundle verify bundles/lineageos/bundle.toml
cargo run -q -p carbon-cli -- bundle verify bundles/pixelos/bundle.toml

echo "[4/10] Guest contracts"
cargo run -q -p carbon-cli -- guest verify guest/android/device/cvd1.toml
cargo run -q -p carbon-cli -- guest verify guest/ubuntu-touch/device/cvd1.toml
cargo run -q -p carbon-cli -- guest verify guest/lineageos/device/cvd1.toml
cargo run -q -p carbon-cli -- guest verify guest/pixelos/device/cvd1.toml
kernel_base="$(mktemp)"
trap 'rm -f "$kernel_base"' EXIT
printf 'CONFIG_ARM64=y\n' > "$kernel_base"
cargo run -q -p carbon-cli -- guest kernel-config android "$kernel_base" \
  guest/common/kernel/cvd1-near-native.fragment guest/android/device/kernel.fragment
for rom in lineageos pixelos; do
  cargo run -q -p carbon-cli -- guest kernel-config android "$kernel_base" \
    guest/common/kernel/cvd1-near-native.fragment "guest/$rom/device/kernel.fragment"
done
cargo run -q -p carbon-cli -- guest kernel-config ubuntu-touch "$kernel_base" \
  guest/common/kernel/cvd1-near-native.fragment guest/ubuntu-touch/device/kernel.fragment

echo "[5/10] Shell contracts"
bash -n guest/android/build.sh
bash -n guest/ubuntu-touch/build.sh
bash -n guest/aosp-overlay/build-cvd1.sh
bash -n guest/lineageos/build.sh
bash -n guest/pixelos/build.sh
guest/test-aosp-overlay-contract.sh
bash -n emu/qemu/build-android.sh
bash -n qualification/emu/frame-proto/run.sh
bash -n scripts/build-apk.sh
bash -n qualification/carbon-qualify.sh
bash -n qualification/test-dry-run.sh
bash -n qualification/test-adb-probe.sh
bash -n qualification/gpu/run-minimal.sh
bash -n qualification/gpu/run-sustained.sh
bash -n qualification/gpu/test-sustained.sh
bash -n qualification/gpu/test-minimal-preflight.sh
bash -n qualification/gpu/collect-report.sh
bash -n host/android/test-framework-raw-config-contract.sh
bash -n host/android/test-broker-aidl-contract.sh
bash -n host/android/test-host-broker-forwarding-contract.sh
bash -n host/android/test-rust-broker-service-contract.sh
bash -n host/android/test-rust-runtime-binder-contract.sh
bash -n host/test-platform-policy-contract.sh
bash -n host/test-product-integration-contract.sh
bash -n host/test-android-soong-contract.sh
bash -n host/product/prepare-controlled-host.sh
bash -n host/product/test-prepare-controlled-host.sh
bash -n host/product/snapdragon/test-snapdragon-contract.sh
python3 -I host/product/snapdragon/test_patch_vendor_boot.py
host/product/snapdragon/test-snapdragon-contract.sh
host/android/test-framework-raw-config-contract.sh
host/android/test-broker-aidl-contract.sh
host/android/test-host-broker-forwarding-contract.sh
host/android/test-rust-broker-service-contract.sh
host/android/test-rust-runtime-binder-contract.sh
host/test-platform-policy-contract.sh
host/test-product-integration-contract.sh
host/test-android-soong-contract.sh
host/product/test-prepare-controlled-host.sh

echo "[6/10] Qualification fixtures"
qualification/test-dry-run.sh
qualification/test-adb-probe.sh

echo "[7/10] Emulated-engine planning"
emu/qemu/build-android.sh --check
# The `-display android` frame protocol: the C producer extracted from the QEMU patch, checked
# against the golden bytes the Rust reader also asserts (crates/carbon-emu frame_proto tests).
qualification/emu/frame-proto/run.sh
cargo run -q -p carbon-cli -- emu plan --facts qualification/emu/fixtures/kunzite.json \
  --profile low-memory >/dev/null
# The measured 2750 MiB reading must be refused for the balanced 2048 MiB profile: the golden
# test asserts the numbers, this asserts the CLI's exit code.
if cargo run -q -p carbon-cli -- emu plan --facts qualification/emu/fixtures/kunzite.json \
  --profile balanced >/dev/null 2>&1; then
  echo "balanced emulated profile was admitted on the measured 2750 MiB reading" >&2
  exit 1
fi

echo "[8/10] GPU proof fixtures"
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
bash qualification/gpu/test-minimal-preflight.sh

echo "[9/10] Android unit tests"
if [[ -z "${JAVA_HOME:-}" ]]; then
  if command -v brew >/dev/null && [[ -d "$(brew --prefix openjdk@17 2>/dev/null)/libexec/openjdk.jdk/Contents/Home" ]]; then
    export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
  fi
fi
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17 or newer before running Android verification}"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$repo_root/.gradle-local}"
./gradlew --no-daemon :android:domain:test :android:host-contract:test :android:app:testDebugUnitTest

echo "[10/10] Android debug and release APKs"
./gradlew --no-daemon :android:app:assembleDebug
test -f android/app/build/outputs/apk/debug/app-debug.apk
# The release variant must build too. Without CARBONHV_KEYSTORE it is signed with the debug key,
# which is what makes CI artifacts installable; scripts/build-apk.sh reports which case it is.
CARBONHV_VERSION_CODE="${CARBONHV_VERSION_CODE:-1}" ./gradlew --no-daemon :android:app:assembleRelease
test -f android/app/build/outputs/apk/release/app-release.apk

echo "CarbonHV verification: PASS"
