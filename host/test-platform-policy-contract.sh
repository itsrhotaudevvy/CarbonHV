#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
service_contexts="$repo_root/host/service_contexts"
file_contexts="$repo_root/host/file_contexts"
seapp_contexts="$repo_root/host/seapp_contexts"
property_contexts="$repo_root/host/property_contexts"
daemon_policy="$repo_root/host/carbonhvd.te"
app_policy="$repo_root/host/carbonhv_host_app.te"
manifest="$repo_root/host/android/AndroidManifest.xml"
init_rc="$repo_root/host/carbonhvd.rc"

for file in "$service_contexts" "$file_contexts" "$seapp_contexts" "$property_contexts" "$app_policy"; do
  [[ -f "$file" ]]
done

grep -Eq '^dev\.carbonhv\.host\.runtime[[:space:]]+u:object_r:carbonhv_host_runtime_service:s0$' "$service_contexts"
grep -Eq '^dev\.carbonhv\.broker[[:space:]]+u:object_r:carbonhv_broker_service:s0$' "$service_contexts"
grep -Eq '^/system_ext/bin/carbonhvd[[:space:]]+u:object_r:carbonhvd_exec:s0$' "$file_contexts"
grep -Eq '^/data/misc/carbonhv\(/\.\*\)\?[[:space:]]+u:object_r:carbonhv_data_file:s0$' "$file_contexts"

grep -Fq 'user=system seinfo=platform isPrivApp=true name=dev.carbonhv.host domain=carbonhv_host_app type=system_app_data_file levelFrom=all' "$seapp_contexts"
grep -Eq '^ro\.carbonhv\.ui_signing_sha256[[:space:]]+u:object_r:carbonhv_config_prop:s0[[:space:]]+exact[[:space:]]+string$' "$property_contexts"
grep -Fq 'android:sharedUserId="android.uid.system"' "$manifest"
python3 - "$manifest" <<'PY'
import sys
import xml.etree.ElementTree as ET

android = "{http://schemas.android.com/apk/res/android}"
root = ET.parse(sys.argv[1]).getroot()
application = root.find("application")
if application is None:
    raise SystemExit("host manifest has no application")

receiver = application.find("receiver[@android:name='.CarbonHvHostBootReceiver']", {"android": android[1:-1]})
service = application.find("service[@android:name='.CarbonHvHostService']", {"android": android[1:-1]})
if receiver is None or receiver.get(android + "exported") != "false":
    raise SystemExit("CarbonHvHostBootReceiver must not be exported")
if service is None or service.get(android + "exported") != "true":
    raise SystemExit("CarbonHvHostService must remain exported for the ordinary UI Binder client")
PY

grep -Fq 'type carbonhv_host_runtime_service, service_manager_type;' "$app_policy"
grep -Fq 'type carbonhv_broker_service, service_manager_type;' "$daemon_policy"
grep -Fq 'type carbonhv_host_app, domain, coredomain;' "$app_policy"
grep -Fq 'app_domain(carbonhv_host_app)' "$app_policy"
grep -Fq 'system_internal_prop(carbonhv_config_prop)' "$app_policy"
grep -Fq 'get_prop(carbonhv_host_app, carbonhv_config_prop)' "$app_policy"
grep -Fq 'add_service(carbonhv_host_app, carbonhv_host_runtime_service)' "$app_policy"
grep -Fq 'allow carbonhv_host_app carbonhv_broker_service:service_manager find;' "$app_policy"
grep -Fq 'binder_call(carbonhv_host_app, carbonhvd)' "$app_policy"
grep -Fq 'allow carbonhv_host_app activity_service:service_manager find;' "$app_policy"
grep -Fq 'allow carbonhv_host_app package_service:service_manager find;' "$app_policy"
grep -Fq 'allow carbonhv_host_app carbonhv_data_file:dir r_dir_perms;' "$app_policy"
grep -Fq 'allow carbonhv_host_app carbonhv_data_file:file rw_file_perms;' "$app_policy"

grep -Fq 'type carbonhvd, domain, coredomain;' "$daemon_policy"
grep -Fq 'type carbonhv_data_file, file_type, data_file_type, core_data_file_type;' "$daemon_policy"
grep -Fq 'allow carbonhvd carbonhv_host_runtime_service:service_manager find;' "$daemon_policy"
grep -Fq 'binder_call(carbonhvd, carbonhv_host_app)' "$daemon_policy"
grep -Fq 'add_service(carbonhvd, carbonhv_broker_service)' "$daemon_policy"
grep -Fq 'allow carbonhvd carbonhv_data_file:dir create_dir_perms;' "$daemon_policy"
grep -Fq 'allow carbonhvd carbonhv_data_file:file create_file_perms;' "$daemon_policy"
grep -Fq 'allow crosvm_domain carbonhv_data_file:file { getattr read write ioctl lock };' "$daemon_policy"
grep -Fq 'get_prop(carbonhvd, hypervisor_prop)' "$daemon_policy"

grep -Fq 'mkdir /data/misc/carbonhv 0770 system system' "$init_rc"
grep -Fq 'restorecon_recursive /data/misc/carbonhv' "$init_rc"

if grep -Fq 'add_service(carbonhv_host_app, carbonhv_broker_service)' "$app_policy"; then
  echo "host app must not publish the broker service" >&2
  exit 1
fi
if grep -Fq 'allow carbonhvd carbonhv_broker_service:service_manager find;' "$daemon_policy"; then
  echo "carbonhvd must not discover its own broker service as a client" >&2
  exit 1
fi
if grep -Fq 'add_service(carbonhvd, carbonhv_host_runtime_service)' "$daemon_policy"; then
  echo "carbonhvd must not publish the host runtime service" >&2
  exit 1
fi
if grep -Fq 'allow carbonhv_host_app carbonhv_host_runtime_service:service_manager find;' "$app_policy"; then
  echo "host app must not consume its own runtime service" >&2
  exit 1
fi

echo "platform policy contract: PASS"
