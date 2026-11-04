#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
runtime="$repo_root/crates/carbon-broker/src/android_host_runtime.rs"
broker_lib="$repo_root/crates/carbon-broker/src/lib.rs"
daemon="$repo_root/crates/carbon-broker/src/main.rs"
soong="$repo_root/Android.bp"

[[ -f "$runtime" ]]
[[ -f "$soong" ]]

grep -q 'dev_carbonhv_host_runtime::aidl::dev::carbonhv::host' "$runtime"
grep -q 'dev.carbonhv.host.runtime' "$runtime"
grep -q 'wait_for_interface::<dyn ICarbonHvHostRuntime>' "$runtime"
grep -q 'struct LazyAndroidHostRuntimePublisher' "$runtime"
grep -q 'AndroidHostRuntimePublisher::connect()' "$runtime"
grep -q 'r#startGraphicalVm' "$runtime"
grep -q 'r#stopGraphicalVm' "$runtime"
grep -q 'i64::try_from(identity.generation.0)' "$runtime"

grep -q '#\[cfg(target_os = "android")\]' "$broker_lib"
grep -q 'LazyAndroidHostRuntimePublisher' "$broker_lib"
grep -q 'run_android_broker_service' "$daemon"

grep -q 'name: "libcarbon_broker"' "$soong"
grep -q 'name: "carbonhvd"' "$soong"
grep -q '"libbinder_rs"' "$soong"
grep -q '"dev.carbonhv.host.runtime-rust"' "$soong"
grep -q 'init_rc: \["host/carbonhvd.rc"\]' "$soong"
grep -q 'system_ext_specific: true' "$soong"

echo "Rust runtime Binder contract: PASS"
