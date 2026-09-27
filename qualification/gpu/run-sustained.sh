#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat >&2 <<'EOF'
usage:
  run-sustained.sh --adb <serial> --minutes <n> [--profile <profile.toml>] [--output <report>]
  run-sustained.sh --fixture <evidence.env> --profile <profile.toml> --minutes <n> [--output <report>]

The physical path first proves the existing CarbonHV guest-renderer presentation path. If the
real guest/renderer prerequisites are blocked, the sustained run stops immediately and records
unknown/not-run metrics. Fixture mode exercises workload and threshold policy only; it can never
grant gpu.sustained or gpu.qualified.
EOF
}

serial=""
fixture=""
minutes=""
profile="$script_dir/profiles/pixel-7a-mali-g710.toml"
output=""

while (($#)); do
  case "$1" in
    --adb)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      serial="$2"
      shift 2
      ;;
    --fixture)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      fixture="$2"
      shift 2
      ;;
    --minutes)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      minutes="$2"
      shift 2
      ;;
    --profile)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      profile="$2"
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      output="$2"
      shift 2
      ;;
    *)
      usage
      exit 64
      ;;
  esac
done

[[ "$minutes" =~ ^[0-9]+$ && "$minutes" -ge 30 ]] || {
  echo "sustained qualification requires --minutes >= 30" >&2
  exit 64
}
[[ -f "$profile" ]] || { echo "profile not found: $profile" >&2; exit 66; }

if [[ -n "$serial" && -n "$fixture" ]] || [[ -z "$serial" && -z "$fixture" ]]; then
  usage
  exit 64
fi

kv_value() {
  local key="$1" file="$2"
  awk -F= -v wanted="$key" '
    $1 == wanted {
      value=$0
      sub(/^[^=]*=/, "", value)
      sub(/\r$/, "", value)
      print value
      found=1
      exit
    }
    END { if (!found) exit 2 }
  ' "$file"
}

toml_value() {
  local key="$1" file="$2"
  awk -F= -v wanted="$key" '
    {
      lhs=$1
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", lhs)
      if (lhs == wanted) {
        value=$0
        sub(/^[^=]*=/, "", value)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        if (value ~ /^".*"$/) {
          sub(/^"/, "", value)
          sub(/"$/, "", value)
        }
        print value
        found=1
        exit
      }
    }
    END { if (!found) exit 2 }
  ' "$file"
}

require_kv() {
  local key="$1" file="$2" value
  value="$(kv_value "$key" "$file")" || { echo "missing sustained evidence field: $key" >&2; exit 65; }
  [[ -n "$value" ]] || { echo "empty sustained evidence field: $key" >&2; exit 65; }
  printf '%s' "$value"
}

require_toml() {
  local key="$1" value
  value="$(toml_value "$key" "$profile")" || { echo "missing profile field: $key" >&2; exit 65; }
  [[ -n "$value" ]] || { echo "empty profile field: $key" >&2; exit 65; }
  printf '%s' "$value"
}

is_number() {
  [[ "$1" =~ ^[0-9]+([.][0-9]+)?$ ]]
}

numeric_le() {
  awk -v got="$1" -v max="$2" 'BEGIN { exit !(got + 0 <= max + 0) }'
}

numeric_ge() {
  awk -v got="$1" -v min="$2" 'BEGIN { exit !(got + 0 >= min + 0) }'
}

append_failure() {
  local reason="$1"
  if [[ -z "${failures:-}" ]]; then
    failures="$reason"
  else
    failures="$failures,$reason"
  fi
}

emit_report() {
  local report="$1"
  if [[ -n "$output" ]]; then
    mkdir -p "$(dirname "$output")"
    printf '%s\n' "$report" > "$output"
  fi
  printf '%s\n' "$report"
}

profile_id="$(require_toml profile_id)"
profile_status="$(require_toml status)"
profile_minimum_minutes="$(require_toml minimum_minutes)"
[[ "$profile_minimum_minutes" =~ ^[0-9]+$ ]] || { echo "invalid profile minimum_minutes" >&2; exit 65; }
[[ "$minutes" -ge "$profile_minimum_minutes" ]] || {
  echo "requested duration is shorter than profile minimum_minutes=$profile_minimum_minutes" >&2
  exit 64
}

if [[ -n "$serial" ]]; then
  set +e
  minimal="$($script_dir/run-minimal.sh --adb "$serial" --renderer gfxstream-vulkan-mali 2>&1)"
  minimal_status=$?
  set -e

  if [[ $minimal_status -ne 0 && $minimal_status -ne 4 ]]; then
    printf '%s\n' "$minimal" >&2
    exit "$minimal_status"
  fi

  get_minimal() {
    local key="$1"
    awk -F= -v wanted="$key" '$1 == wanted { sub(/^[^=]*=/, "", $0); print; found=1; exit } END { if (!found) exit 2 }' <<<"$minimal"
  }

  blocker="$(get_minimal blocker 2>/dev/null || true)"
  if [[ -z "$blocker" ]]; then
    blocker="sustained-workload-runner-not-wired"
  fi

  report="$(cat <<EOF
schema_version=1
evidence_source=physical-adb
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=BLOCKED
threshold_failure=none
requested_minutes=$minutes
workload_version=sustained-gpu-v1
device_model=$(get_minimal device_model 2>/dev/null || echo unknown)
soc=$(get_minimal soc 2>/dev/null || echo unknown)
gpu=$(get_minimal gpu 2>/dev/null || echo unknown)
host_build_fingerprint=$(get_minimal host_build_fingerprint 2>/dev/null || echo unknown)
kernel_version=$(get_minimal kernel_version 2>/dev/null || echo unknown)
avf_revision=$(get_minimal avf_revision 2>/dev/null || echo unknown)
renderer_id=gfxstream-vulkan-mali
graphics_driver_version=$(get_minimal graphics_driver_version 2>/dev/null || echo unknown)
android_guest_bundle_version=$(get_minimal guest_bundle_version 2>/dev/null || echo unknown)
ubuntu_touch_guest_bundle_version=not-run
sustained_seconds=0
presented_fps=unknown
frame_p50_ms=unknown
frame_p95_ms=unknown
frame_p99_ms=unknown
dropped_frame_pct=unknown
max_queue_depth=unknown
input_to_visible_ms=unknown
graphics_cpu_pct=unknown
renderer_rss_mib=unknown
graphics_buffer_mib=unknown
full_frame_cpu_copies=unknown
gpu_side_full_frame_copies=unknown
thermal_status_peak=unknown
power_energy_source=unavailable-not-run
recovery_result=not-run
workload_evaluation=BLOCKED
gpu.sustained=false
gpu.qualified=false
qualification=EXPERIMENTAL
sustained_status=BLOCKED
blocker=$blocker
EOF
)"
  emit_report "$report"
  exit 4
fi

[[ -f "$fixture" ]] || { echo "fixture not found: $fixture" >&2; exit 66; }
[[ "$(require_kv evidence_source "$fixture")" == fixture ]] || {
  echo "--fixture requires evidence_source=fixture" >&2
  exit 65
}

if [[ "$profile_status" != calibrated ]]; then
  report="$(cat <<EOF
schema_version=1
evidence_source=fixture
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=BLOCKED
threshold_failure=none
requested_minutes=$minutes
workload_version=$(require_kv workload_version "$fixture")
gpu.sustained=false
gpu.qualified=false
qualification=EXPERIMENTAL
sustained_status=BLOCKED
blocker=release-threshold-profile-not-calibrated
EOF
)"
  emit_report "$report"
  exit 4
fi

threshold_keys=(
  min_presented_fps
  max_frame_p95_ms
  max_frame_p99_ms
  max_dropped_frame_pct
  max_queue_depth
  max_input_to_visible_ms
  max_graphics_cpu_pct
  max_renderer_rss_mib
  max_graphics_buffer_mib
  max_thermal_status
)
for key in "${threshold_keys[@]}"; do
  value="$(require_toml "$key")"
  is_number "$value" || { echo "invalid numeric profile field: $key=$value" >&2; exit 65; }
done

required_evidence=(
  workload_version android_guest_bundle_version ubuntu_touch_guest_bundle_version sustained_seconds
  presented_fps frame_p50_ms frame_p95_ms frame_p99_ms dropped_frame_pct max_queue_depth
  input_to_visible_ms graphics_cpu_pct renderer_rss_mib graphics_buffer_mib
  full_frame_cpu_copies gpu_side_full_frame_copies thermal_status_peak power_energy_source recovery_result
)
for key in "${required_evidence[@]}"; do
  require_kv "$key" "$fixture" >/dev/null
done

workload_keys=(
  static_composition continuous_scrolling browser_rendering rapid_content gles
  multitouch_animation rotation surface_recreation background_foreground renderer_recovery
)
failures=""
for guest in android ubuntu_touch; do
  for workload in "${workload_keys[@]}"; do
    [[ "$(require_kv "$guest.$workload" "$fixture")" == pass ]] || append_failure "$guest.$workload"
  done
  vulkan_exposed="$(require_kv "$guest.vulkan_exposed" "$fixture")"
  vulkan_result="$(require_kv "$guest.vulkan" "$fixture")"
  case "$vulkan_exposed" in
    true) [[ "$vulkan_result" == pass ]] || append_failure "$guest.vulkan" ;;
    false) [[ "$vulkan_result" == not-exposed ]] || append_failure "$guest.vulkan-contract" ;;
    *) append_failure "$guest.vulkan-exposed" ;;
  esac
done

sustained_seconds="$(require_kv sustained_seconds "$fixture")"
[[ "$sustained_seconds" =~ ^[0-9]+$ ]] || { echo "invalid sustained_seconds=$sustained_seconds" >&2; exit 65; }
required_seconds=$((minutes * 60))
[[ "$sustained_seconds" -ge "$required_seconds" ]] || append_failure "sustained_seconds"

if [[ "$(require_kv recovery_result "$fixture")" != pass ]]; then
  append_failure "recovery_result"
fi

cpu_copies="$(require_kv full_frame_cpu_copies "$fixture")"
[[ "$cpu_copies" =~ ^[0-9]+$ ]] || { echo "invalid full_frame_cpu_copies=$cpu_copies" >&2; exit 65; }
[[ "$cpu_copies" -eq 0 ]] || append_failure "full_frame_cpu_copies"

check_min() {
  local evidence_key="$1" profile_key="$2" got limit
  got="$(require_kv "$evidence_key" "$fixture")"
  limit="$(require_toml "$profile_key")"
  is_number "$got" || { echo "invalid numeric evidence: $evidence_key=$got" >&2; exit 65; }
  numeric_ge "$got" "$limit" || append_failure "$evidence_key"
}

check_max() {
  local evidence_key="$1" profile_key="$2" got limit
  got="$(require_kv "$evidence_key" "$fixture")"
  limit="$(require_toml "$profile_key")"
  is_number "$got" || { echo "invalid numeric evidence: $evidence_key=$got" >&2; exit 65; }
  numeric_le "$got" "$limit" || append_failure "$evidence_key"
}

check_min presented_fps min_presented_fps
check_max frame_p95_ms max_frame_p95_ms
check_max frame_p99_ms max_frame_p99_ms
check_max dropped_frame_pct max_dropped_frame_pct
check_max max_queue_depth max_queue_depth
check_max input_to_visible_ms max_input_to_visible_ms
check_max graphics_cpu_pct max_graphics_cpu_pct
check_max renderer_rss_mib max_renderer_rss_mib
check_max graphics_buffer_mib max_graphics_buffer_mib
check_max thermal_status_peak max_thermal_status

workload_evaluation=PASS
threshold_evaluation=PASS
threshold_failure=none
exit_status=0
if [[ -n "$failures" ]]; then
  workload_evaluation=FAIL
  threshold_evaluation=FAIL
  threshold_failure="$failures"
  exit_status=4
fi

report="$(cat <<EOF
schema_version=1
evidence_source=fixture
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=$threshold_evaluation
threshold_failure=$threshold_failure
requested_minutes=$minutes
workload_version=$(require_kv workload_version "$fixture")
android_guest_bundle_version=$(require_kv android_guest_bundle_version "$fixture")
ubuntu_touch_guest_bundle_version=$(require_kv ubuntu_touch_guest_bundle_version "$fixture")
sustained_seconds=$sustained_seconds
presented_fps=$(require_kv presented_fps "$fixture")
frame_p50_ms=$(require_kv frame_p50_ms "$fixture")
frame_p95_ms=$(require_kv frame_p95_ms "$fixture")
frame_p99_ms=$(require_kv frame_p99_ms "$fixture")
dropped_frame_pct=$(require_kv dropped_frame_pct "$fixture")
max_queue_depth=$(require_kv max_queue_depth "$fixture")
input_to_visible_ms=$(require_kv input_to_visible_ms "$fixture")
graphics_cpu_pct=$(require_kv graphics_cpu_pct "$fixture")
renderer_rss_mib=$(require_kv renderer_rss_mib "$fixture")
graphics_buffer_mib=$(require_kv graphics_buffer_mib "$fixture")
full_frame_cpu_copies=$cpu_copies
gpu_side_full_frame_copies=$(require_kv gpu_side_full_frame_copies "$fixture")
thermal_status_peak=$(require_kv thermal_status_peak "$fixture")
power_energy_source=$(require_kv power_energy_source "$fixture")
recovery_result=$(require_kv recovery_result "$fixture")
workload_evaluation=$workload_evaluation
gpu.sustained=false
gpu.qualified=false
qualification=EXPERIMENTAL
sustained_status=$threshold_evaluation
EOF
)"
emit_report "$report"
exit "$exit_status"
