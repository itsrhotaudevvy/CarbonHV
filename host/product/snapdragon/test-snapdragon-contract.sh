#!/usr/bin/env bash
# Pins the controlled-Snapdragon fragment to the smallest change that enables AVF on Gunyah.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
dir="$root/host/product/snapdragon"
report="$root/qualification/gpu/reports/redmi-kunzite-sm6475-host-probe-2026-11-04.txt"

grep -Fxq 'BOARD_BOOTCONFIG += androidboot.hypervisor.vm.supported=1' "$dir/BoardConfigCarbonHvGunyah.mk"
grep -Fxq 'BOARD_VENDOR_SEPOLICY_DIRS += external/carbonhv/host/product/snapdragon/sepolicy' "$dir/BoardConfigCarbonHvGunyah.mk"
if grep -v '^[[:space:]]*#' "$dir"/*.mk | grep -Eq 'protected_vm|qcom_trusted'; then
  echo "the Snapdragon fragment must never enable protected or trusted VMs" >&2
  exit 1
fi

# The policy uses AOSP's VM-manager hook on the exact label the physical probe observed.
observed_label="$(sed -n 's/^vendor_hypervisor_selinux_label=u:object_r:\([a-z0-9_]*\):s0$/\1/p' "$report")"
test -n "$observed_label"
policy="$(grep -v '^[[:space:]]*#' "$dir/sepolicy/carbonhv_gunyah.te" | grep -v '^[[:space:]]*$')"
test "$policy" = "typeattribute $observed_label vm_manager_device_type;"
if ls "$dir"/sepolicy/*_contexts >/dev/null 2>&1; then
  echo "do not relabel the vendor Gunyah node" >&2
  exit 1
fi

grep -Fxq '    write /sys/kernel/mm/transparent_hugepage/enabled madvise' "$dir/init.carbonhv.gunyah.rc"
grep -Fq 'init.carbonhv.gunyah.rc:$(TARGET_COPY_OUT_SYSTEM_EXT)/etc/init/init.carbonhv.gunyah.rc' "$dir/carbonhv_gunyah.mk"

echo "Snapdragon controlled-host contract: PASS"
