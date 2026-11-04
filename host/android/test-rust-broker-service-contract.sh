#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
server="$repo_root/crates/carbon-broker/src/android_broker_service.rs"
broker_lib="$repo_root/crates/carbon-broker/src/lib.rs"
daemon="$repo_root/crates/carbon-broker/src/main.rs"
root_soong="$repo_root/Android.bp"
host_soong="$repo_root/host/android/Android.bp"

[[ -f "$server" ]]

grep -q 'dev_carbonhv_broker::aidl::dev::carbonhv::broker' "$server"
grep -q 'const BROKER_SERVICE: &str = "dev.carbonhv.broker"' "$server"
grep -q 'const SYSTEM_UID: u32 = 1000' "$server"
grep -q 'ThreadState::get_calling_uid' "$server"
grep -q 'BnCarbonHvBroker::new_binder' "$server"
grep -q 'binder::add_service' "$server"
grep -q 'ProcessState::join_thread_pool' "$server"
grep -q 'r#startGraphicalVm' "$server"
grep -q 'r#stopVm' "$server"
grep -q 'r#activeDisplayGeneration' "$server"
grep -q 'r#notifyRuntimeDied' "$server"

grep -q 'mod android_broker_service' "$broker_lib"
grep -q 'run_android_broker_service' "$daemon"

grep -q '"dev.carbonhv.broker-rust"' "$root_soong"
grep -q 'name: "dev.carbonhv.broker"' "$host_soong"
grep -q '"dev.carbonhv.broker-java"' "$host_soong"

echo "Rust broker Binder service contract: PASS"
