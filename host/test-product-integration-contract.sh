#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fragment="$repo_root/host/product/carbonhv.mk"

[[ -f "$fragment" ]]
grep -Fq 'CARBONHV_UI_SIGNING_SHA256 must be set' "$fragment"
grep -Eq '^[[:space:]]*carbonhvd \\' "$fragment"
grep -Fq 'CarbonHvHostIntegration' "$fragment"
grep -Fq 'SYSTEM_EXT_PRIVATE_SEPOLICY_DIRS += $(LOCAL_PATH)/..' "$fragment"
grep -Fq 'PRODUCT_SYSTEM_EXT_PROPERTIES +=' "$fragment"
grep -Fq 'ro.carbonhv.ui_signing_sha256=$(CARBONHV_UI_SIGNING_SHA256)' "$fragment"

# Product wiring must name the same modules and property that the source policy owns.
grep -Fq 'name: "CarbonHvHostIntegration"' "$repo_root/host/android/Android.bp"
grep -Fq 'name: "carbonhvd"' "$repo_root/Android.bp"
grep -Fq 'ro.carbonhv.ui_signing_sha256' "$repo_root/host/property_contexts"

echo 'product integration contract: PASS'
