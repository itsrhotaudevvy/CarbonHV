#!/usr/bin/env bash
# Builds an AOSP-derived custom ROM (LineageOS, PixelOS) as a CarbonHV CVD1 guest.
#
#   guest/aosp-overlay/build-cvd1.sh --rom lineageos|pixelos
#
# Environment (all optional unless noted):
#   CARBON_ROM_SOURCE        source checkout (default .work/<rom>)
#   CARBON_ROM_REF           manifest branch (default: the ROM's latest Android 17 branch)
#   CARBON_ROM_VARIANT       userdebug (default) or user
#   CARBON_ROM_KERNEL_CONFIG .config of the guest kernel; verified with carbonctl when set
#   CARBON_BUILD_JOBS        parallel jobs (default 8)
#   CARBONCTL                carbonctl binary (default carbonctl on PATH)
set -euo pipefail

usage() { echo "usage: $0 --rom lineageos|pixelos" >&2; exit 64; }
[[ "${1:-}" == --rom && -n "${2:-}" ]] || usage
ROM="$2"

case "$ROM" in
  lineageos)
    MANIFEST=https://github.com/LineageOS/android.git
    DEFAULT_REF=lineage-24.0   # Android 17; latest stable line is lineage-23.2 (Android 16)
    PRODUCT=lineage_carbonhv_cvd1
    ;;
  pixelos)
    MANIFEST=https://github.com/PixelOS-AOSP/android_manifest.git
    DEFAULT_REF=seventeen      # Android 17; latest stable line is sixteen-qpr2 (PixelOS 16.2)
    PRODUCT=aosp_carbonhv_cvd1
    ;;
  *) usage ;;
esac

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo "$ROM CVD1 builds require a Linux x86_64 Android build host." >&2
  exit 2
fi
for tool in repo rsync; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 2; }
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROOT="${CARBON_ROM_SOURCE:-$PWD/.work/$ROM}"
REF="${CARBON_ROM_REF:-$DEFAULT_REF}"
VARIANT="${CARBON_ROM_VARIANT:-userdebug}"
JOBS="${CARBON_BUILD_JOBS:-8}"
OUTPUT="${CARBON_ROM_OUTPUT:-$PWD/out/carbonhv/$ROM}"
CARBONCTL="${CARBONCTL:-carbonctl}"
case "$VARIANT" in userdebug|user) ;; *) echo "unsupported variant $VARIANT" >&2; exit 64 ;; esac

mkdir -p "$ROOT" "$OUTPUT"
cd "$ROOT"
if [[ ! -d .repo ]]; then
  repo init -u "$MANIFEST" -b "$REF" --git-lfs --no-clone-bundle
fi
repo sync -c -j"$JOBS" --force-sync --no-tags --no-clone-bundle

# Stage the shared CarbonHV product overlay and this ROM's validated guest contract.
rsync -a --delete "$REPO_ROOT/guest/aosp-overlay/device/carbonhv/" "$ROOT/device/carbonhv/"
cp "$REPO_ROOT/guest/$ROM/device/cvd1.toml" "$ROOT/device/carbonhv/cvd1/device.toml"

set +u
source build/envsetup.sh
# Lineage-based trees (LineageOS and PixelOS) pin their AOSP release config here (cp2a for A17).
if [[ -f vendor/lineage/vars/aosp_target_release ]]; then
  source vendor/lineage/vars/aosp_target_release
fi
set -u
: "${aosp_target_release:?the $ROM tree does not declare vendor/lineage/vars/aosp_target_release}"
if ! lunch "$PRODUCT-$aosp_target_release-$VARIANT"; then
  echo "lunch $PRODUCT-$aosp_target_release-$VARIANT failed; the CarbonHV overlay may need updating for $REF." >&2
  exit 3
fi

m -j"$JOBS" bootimage initbootimage vendorbootimage superimage

PRODUCT_OUT="${ANDROID_PRODUCT_OUT:?Android build did not set ANDROID_PRODUCT_OUT}"
for artifact in kernel ramdisk.img vendor_ramdisk.img super.img; do
  [[ -f "$PRODUCT_OUT/$artifact" ]] || { echo "missing build output $PRODUCT_OUT/$artifact" >&2; exit 3; }
done

if [[ -n "${CARBON_ROM_KERNEL_CONFIG:-}" ]]; then
  if ! "$CARBONCTL" guest kernel-config android "$CARBON_ROM_KERNEL_CONFIG"; then
    echo "CARBON_ROM_KERNEL_CONFIG does not meet CVD1 near-native requirements" >&2
    exit 3
  fi
  cp "$CARBON_ROM_KERNEL_CONFIG" "$OUTPUT/kernel.config"
else
  echo "warning: CARBON_ROM_KERNEL_CONFIG unset; release automation must verify the guest kernel config" >&2
fi

# CarbonHV boots the kernel directly (no bootloader): the initrd is the vendor ramdisk followed by
# the generic ramdisk, the same concatenation a bootloader performs for vendor_boot + init_boot.
cp "$PRODUCT_OUT/kernel" "$OUTPUT/kernel"
cat "$PRODUCT_OUT/vendor_ramdisk.img" "$PRODUCT_OUT/ramdisk.img" > "$OUTPUT/initrd.img"
cp "$PRODUCT_OUT/super.img" "$OUTPUT/super.img"

# Provenance: exact sources, product, release config and the boot parameters the build declares.
repo manifest -r -o "$OUTPUT/source-manifest.xml"
printf '%s\n' "$PRODUCT-$aosp_target_release-$VARIANT" > "$OUTPUT/product.txt"
{
  printf 'cmdline=%s\n' "$(get_build_var BOARD_KERNEL_CMDLINE)"
  printf 'bootconfig=%s\n' "$(get_build_var BOARD_BOOTCONFIG)"
} > "$OUTPUT/boot-parameters.txt"
( cd "$OUTPUT" && sha256sum kernel initrd.img super.img > SHA256SUMS )

echo "$ROM CVD1 artifacts staged in $OUTPUT"
