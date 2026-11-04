#!/usr/bin/env bash
# Pins the shared AOSP-derived ROM overlay to CVD1's single graphics transport and honest identity.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
overlay="$root/guest/aosp-overlay/device/carbonhv/cvd1"
common="$overlay/carbonhv_cvd1.mk"
script="$root/guest/aosp-overlay/build-cvd1.sh"

# One base for every ROM: the gfxstream-equipped AOSP cuttlefish arm64-only phone.
grep -Fq 'inherit-product, device/google/cuttlefish/vsoc_arm64_only/phone/aosp_cf.mk' "$common"
grep -Fq 'ro.carbonhv.graphics.transport=gfxstream' "$common"
grep -Fq 'PRODUCT_MODEL := CarbonHV Virtual Device (CVD1)' "$common"
grep -Fq 'device/carbonhv/cvd1/device.toml:$(TARGET_COPY_OUT_VENDOR)/etc/carbonhv/device.toml' "$common"

# ROM device trees that change the transport or boot flow must not be pulled in.
forbidden='device/virt/|mesa|virgl|venus|swiftshader|BOARD_MESA3D'
if cat "$overlay"/*.mk | grep -v '^[[:space:]]*#' | grep -Eiq "$forbidden"; then
  grep -EinH "$forbidden" "$overlay"/*.mk | grep -v ':[[:space:]]*#' >&2
  echo "overlay must not use ROM virtual-device trees or Mesa/SwiftShader graphics" >&2
  exit 1
fi

for product in lineage_carbonhv_cvd1 aosp_carbonhv_cvd1; do
  grep -Fq "$product:\$(LOCAL_DIR)/$product.mk" "$overlay/AndroidProducts.mk"
  grep -Fq 'inherit-product, device/carbonhv/cvd1/carbonhv_cvd1.mk' "$overlay/$product.mk"
  grep -Fq "PRODUCT_NAME := $product" "$overlay/$product.mk"
  grep -Fq "PRODUCT=$product" "$script"
done
grep -Fq 'vendor/lineage/config/common_full_phone.mk' "$overlay/lineage_carbonhv_cvd1.mk"
grep -Fq 'vendor/custom/config/common_full_phone.mk' "$overlay/aosp_carbonhv_cvd1.mk"

# Latest Android 17 branches by default, the tree's own release config, direct-boot artifacts.
grep -Fq 'DEFAULT_REF=lineage-24.0' "$script"
grep -Fq 'DEFAULT_REF=seventeen' "$script"
grep -Fq 'source vendor/lineage/vars/aosp_target_release' "$script"
grep -Fq 'cat "$PRODUCT_OUT/vendor_ramdisk.img" "$PRODUCT_OUT/ramdisk.img" > "$OUTPUT/initrd.img"' "$script"

for rom in lineageos pixelos; do
  grep -Fq "aosp-overlay/build-cvd1.sh\" --rom $rom" "$root/guest/$rom/build.sh"
  grep -Fq "id = \"$rom-cvd1\"" "$root/guest/$rom/device/cvd1.toml"
  grep -Fq 'required_capability = "gpu.vulkan"' "$root/guest/$rom/device/cvd1.toml"
  grep -Fq 'required_capability = "enforcing-selinux"' "$root/guest/$rom/device/cvd1.toml"
  grep -Fq "out/carbonhv/$rom/super.img" "$root/bundles/$rom/bundle.toml"
  grep -Fq '(unofficial CarbonHV virtual-device build)' "$root/bundles/$rom/bundle.toml"
done

echo "AOSP-derived ROM overlay contract: PASS"
