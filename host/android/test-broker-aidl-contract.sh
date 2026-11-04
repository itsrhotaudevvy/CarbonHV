#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
aidl_root="$repo_root/host/android/aidl/broker"
interface="$aidl_root/dev/carbonhv/broker/ICarbonHvBroker.aidl"
request="$aidl_root/dev/carbonhv/broker/BrokerStartRequest.aidl"

sdk_root="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
aidl_bin="${AIDL:-}"
if [[ -z "$aidl_bin" ]]; then
    aidl_bin="$(ls -1 "$sdk_root"/build-tools/*/aidl 2>/dev/null | sort -V | tail -1)"
fi
[[ -x "$aidl_bin" ]] || { echo "Android AIDL compiler not found" >&2; exit 1; }

[[ -f "$interface" ]]
[[ -f "$request" ]]

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

"$aidl_bin" --lang=java --structured -I "$aidl_root" -o "$tmp/java" "$request" "$interface"
"$aidl_bin" --lang=rust --structured -I "$aidl_root" -o "$tmp/rust" "$request" "$interface"
"$aidl_bin" --dumpapi -I "$aidl_root" --out="$tmp/api" "$request" "$interface"

api_dump="$(find "$tmp/api" -type f -name '*.aidl' -print -exec cat {} \;)"
grep -q 'long startGraphicalVm' <<<"$api_dump"
grep -q 'void stopVm' <<<"$api_dump"
grep -q 'long activeDisplayGeneration' <<<"$api_dump"
grep -q 'void notifyRuntimeDied' <<<"$api_dump"

if grep -Eqi '\b(path|executable|command|serviceName|objectId)\b' <<<"$api_dump"; then
    echo "broker AIDL leaked a privileged transport detail" >&2
    exit 1
fi

echo "Broker AIDL contract: PASS"
