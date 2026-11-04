#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
backend="$repo_root/host/android/src/dev/carbonhv/host/Android17GraphicalVmBackend.kt"
bridge="$repo_root/host/android/src/android/system/virtualmachine/CarbonHvVirtualizationServiceBridge.java"

if grep -q 'AssignedDevices' "$backend"; then
  echo "backend still depends on the removed Android 17 AssignedDevices raw-config field" >&2
  exit 1
fi

if grep -q 'VirtualMachineRawConfig()' "$backend"; then
  echo "backend still constructs the drift-prone AVF raw config directly" >&2
  exit 1
fi

grep -q 'CarbonHvVirtualizationServiceBridge.buildRawConfig' "$backend"
grep -q 'import android.system.virtualizationcommon.IGuestAgent' "$backend"
grep -Eq 'override fun onGuestAgentRegistered\(cid: Int, guestAgent: IGuestAgent\)' "$backend"
grep -q 'VirtualMachineRawConfig buildRawConfig' "$bridge"
grep -q '\.toVsRawConfig()' "$bridge"
grep -q 'rawConfig.inputDevices' "$bridge"

# Broker-planned performance fields must reach the AVF raw config, not be dropped at the bridge.
grep -q 'rawConfig.hugePages = hugePages;' "$bridge"
grep -q 'rawConfig.boostUclamp = boostUclamp;' "$bridge"
grep -q 'rawConfig.balloon = balloon;' "$bridge"
grep -q 'config.performance.boostUclamp' "$backend"
aidl="$repo_root/host/android/aidl/runtime/dev/carbonhv/host/GraphicalVmLaunchRequest.aidl"
grep -q 'boolean hugePages;' "$aidl"
grep -q 'boolean boostUclamp;' "$aidl"
grep -q 'boolean balloon;' "$aidl"

echo "framework-owned raw-config contract: PASS"
