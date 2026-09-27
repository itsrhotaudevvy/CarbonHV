#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "GrapheneOS guest builds require a Linux x86_64 build host." >&2
  exit 2
fi

for tool in repo git python3; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 2; }
done

ROOT="${CARBON_GRAPHENE_SOURCE:-$PWD/.work/grapheneos}"
REF="${CARBON_GRAPHENE_REF:-17}"
TARGET="${CARBON_GRAPHENE_TARGET:-aosp_cf_arm64_phone}"
JOBS="${CARBON_BUILD_JOBS:-8}"
OUTPUT="${CARBON_GRAPHENE_OUTPUT:-$PWD/out/carbonhv/grapheneos}"

mkdir -p "$ROOT" "$OUTPUT"
cd "$ROOT"

if [[ ! -d .repo ]]; then
  repo init -u https://github.com/GrapheneOS/platform_manifest.git -b "$REF"
fi
repo sync -j"$JOBS"

source build/envsetup.sh
if ! lunch "$TARGET-userdebug"; then
  echo "The pinned GrapheneOS tree does not expose $TARGET; add/update the CarbonHV CVD1 product overlay before building." >&2
  exit 3
fi

m -j"$JOBS" bootimage systemimage target-files-package

PRODUCT_OUT="${ANDROID_PRODUCT_OUT:?Android build did not set ANDROID_PRODUCT_OUT}"
cp "$PRODUCT_OUT/boot.img" "$OUTPUT/boot.img"
cp "$PRODUCT_OUT/system.img" "$OUTPUT/system.img"
repo manifest -r -o "$OUTPUT/source-manifest.xml"
printf '%s\n' "$TARGET" > "$OUTPUT/product.txt"

echo "GrapheneOS-derived CVD1 artifacts staged in $OUTPUT"

