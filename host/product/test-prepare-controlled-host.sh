#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="$repo_root/host/product/prepare-controlled-host.sh"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
android_root="$tmp/android"
mkdir -p \
  "$android_root/build" \
  "$android_root/device/google/lynx" \
  "$android_root/packages/modules/Virtualization/libs/framework-virtualization"
touch "$android_root/build/envsetup.sh"
product_mk="$android_root/device/google/lynx/aosp_lynx.mk"
printf '%s\n' 'PRODUCT_NAME := aosp_lynx' > "$product_mk"
framework_bp="$android_root/packages/modules/Virtualization/libs/framework-virtualization/Android.bp"
cat > "$framework_bp" <<'BP'
java_sdk_library {
    name: "framework-virtualization",
    impl_library_visibility: [
        "//packages/modules/Virtualization:__subpackages__",
    ],
}
BP

digest_a='0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'
digest_b='abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789'

"$script" \
  --android-root "$android_root" \
  --product-makefile device/google/lynx/aosp_lynx.mk \
  --ui-signing-sha256 "$digest_a"

staged="$android_root/external/carbonhv"
test -f "$staged/Android.bp"
test -f "$staged/host/product/carbonhv.mk"
test -f "$staged/.carbonhv-controlled-host-staging"
grep -Fq '# BEGIN CarbonHV controlled-host integration' "$product_mk"
grep -Fq "CARBONHV_UI_SIGNING_SHA256 := $digest_a" "$product_mk"
grep -Fq '$(call inherit-product, external/carbonhv/host/product/carbonhv.mk)' "$product_mk"
test "$(grep -Fc '# BEGIN CarbonHV controlled-host integration' "$product_mk")" -eq 1
grep -Fq '"//external/carbonhv:__subpackages__",' "$framework_bp"
test "$(grep -Fc '"//external/carbonhv:__subpackages__",' "$framework_bp")" -eq 1

# Re-running is idempotent and updates the digest in place.
"$script" \
  --android-root "$android_root" \
  --product-makefile device/google/lynx/aosp_lynx.mk \
  --ui-signing-sha256 "$digest_b"

test "$(grep -Fc '# BEGIN CarbonHV controlled-host integration' "$product_mk")" -eq 1
grep -Fq "CARBONHV_UI_SIGNING_SHA256 := $digest_b" "$product_mk"
! grep -Fq "$digest_a" "$product_mk"
test "$(grep -Fc '"//external/carbonhv:__subpackages__",' "$framework_bp")" -eq 1

# APK certificate mode consumes apksigner's certificate digest instead of a hardcoded value.
fake_apk="$tmp/ui.apk"
touch "$fake_apk"
fake_apksigner="$tmp/apksigner"
cat > "$fake_apksigner" <<'FAKE'
#!/usr/bin/env bash
printf '%s\n' 'Signer #1 certificate DN: CN=CarbonHV Test'
printf '%s\n' 'Signer #1 certificate SHA-256 digest: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
FAKE
chmod +x "$fake_apksigner"
"$script" \
  --android-root "$android_root" \
  --product-makefile device/google/lynx/aosp_lynx.mk \
  --ui-apk "$fake_apk" \
  --apksigner "$fake_apksigner"
grep -Fq 'CARBONHV_UI_SIGNING_SHA256 := aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' "$product_mk"

# Never delete or overwrite an unrelated pre-existing external/carbonhv directory.
foreign_root="$tmp/foreign-android"
mkdir -p \
  "$foreign_root/build" \
  "$foreign_root/device/google/lynx" \
  "$foreign_root/external/carbonhv" \
  "$foreign_root/packages/modules/Virtualization/libs/framework-virtualization"
touch "$foreign_root/build/envsetup.sh"
printf '%s\n' 'PRODUCT_NAME := aosp_lynx' > "$foreign_root/device/google/lynx/aosp_lynx.mk"
cp "$framework_bp" "$foreign_root/packages/modules/Virtualization/libs/framework-virtualization/Android.bp"
printf '%s\n' 'unrelated' > "$foreign_root/external/carbonhv/keep.txt"
if "$script" \
  --android-root "$foreign_root" \
  --product-makefile device/google/lynx/aosp_lynx.mk \
  --ui-signing-sha256 "$digest_a" 2>/dev/null; then
  echo 'expected foreign staging directory rejection' >&2
  exit 1
fi
grep -Fxq 'unrelated' "$foreign_root/external/carbonhv/keep.txt"

echo 'controlled host preparation contract: PASS'
