#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
ui_aidl_root="$repo_root/host/android/aidl/ui"
request="$ui_aidl_root/dev/carbonhv/host/HostGraphicalVmStartRequest.aidl"
interface="$ui_aidl_root/dev/carbonhv/host/ICarbonHvHostService.aidl"
client="$repo_root/host/android/src/dev/carbonhv/host/PlatformCarbonHvBrokerClient.kt"
service="$repo_root/host/android/src/dev/carbonhv/host/CarbonHvHostService.kt"

sdk_root="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
aidl_bin="${AIDL:-}"
if [[ -z "$aidl_bin" ]]; then
    aidl_bin="$(ls -1 "$sdk_root"/build-tools/*/aidl 2>/dev/null | sort -V | tail -1)"
fi
framework_aidl="$(ls -1 "$sdk_root"/platforms/android-*/framework.aidl 2>/dev/null | sort -V | tail -1)"
[[ -x "$aidl_bin" ]] || { echo "Android AIDL compiler not found" >&2; exit 1; }
[[ -f "$framework_aidl" ]] || { echo "Android framework.aidl not found" >&2; exit 1; }
[[ -f "$request" ]]
[[ -f "$interface" ]]
[[ -f "$client" ]]

tmp="$(mktemp -d)"
trap 'find "$tmp" -type f -delete; find "$tmp" -depth -type d -empty -delete' EXIT

"$aidl_bin" --lang=java --structured -p "$framework_aidl" -I "$ui_aidl_root" \
    -o "$tmp/java" "$request" "$interface"
"$aidl_bin" --dumpapi -p "$framework_aidl" -I "$ui_aidl_root" \
    --out="$tmp/api" "$request" "$interface"

api_dump="$(find "$tmp/api" -type f -name '*.aidl' -print -exec cat {} \;)"
grep -q 'long startGraphicalVm' <<<"$api_dump"
grep -q 'void stopVm' <<<"$api_dump"
grep -q 'long activeDisplayGeneration' <<<"$api_dump"
if grep -Eqi '\b(path|executable|command|serviceName|objectId)\b' <<<"$api_dump"; then
    echo "UI lifecycle AIDL leaked a privileged transport detail" >&2
    exit 1
fi

grep -q 'ServiceManager.waitForService(BROKER_SERVICE_NAME)' "$client"
grep -q 'BROKER_SERVICE_NAME = "dev.carbonhv.broker"' "$client"
grep -q 'ICarbonHvBroker.Stub.asInterface' "$client"
grep -q 'brokerClient.startGraphicalVm' "$service"
grep -q 'brokerClient.stopVm' "$service"
grep -q 'brokerClient.activeDisplayGeneration' "$service"
grep -q 'brokerClient.notifyRuntimeDied' "$service"

assert_authenticated_forward() {
    local signature="$1"
    local forward="$2"
    local body
    body="$(awk -v signature="$signature" '
        index($0, signature) { capture=1 }
        capture { print }
        capture && /^        }$/ { exit }
    ' "$service")"
    [[ -n "$body" ]]
    grep -q 'enforceUiCaller()' <<<"$body"
    grep -q "$forward" <<<"$body"
    local auth_line forward_line
    auth_line="$(grep -n 'enforceUiCaller()' <<<"$body" | head -1 | cut -d: -f1)"
    forward_line="$(grep -n "$forward" <<<"$body" | head -1 | cut -d: -f1)"
    (( auth_line < forward_line ))
}

assert_authenticated_forward 'override fun startGraphicalVm(request: HostGraphicalVmStartRequest)' 'brokerClient.startGraphicalVm'
assert_authenticated_forward 'override fun stopVm' 'brokerClient.stopVm'
assert_authenticated_forward 'override fun activeDisplayGeneration' 'brokerClient.activeDisplayGeneration'

echo "Host broker forwarding contract: PASS"
