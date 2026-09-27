#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"

usage() {
  cat >&2 <<'EOF'
usage:
  run-sustained.sh --adb <serial> --minutes <n> [--profile <profile.toml>] [--output <report>]
  run-sustained.sh --adb <serial> --evidence <physical-sustained.env> --minutes <n> [--profile <profile.toml>] [--output <report>]
  run-sustained.sh --fixture <evidence.env> --profile <profile.toml> --minutes <n> [--output <report>]

The ADB preflight form records blockers while the real guest/renderer path is unavailable. The
ADB+evidence form binds captured guest/renderer evidence to the currently probed physical host,
requires a calibrated exact-tuple profile, and is the only path that may grant gpu.sustained and
gpu.qualified. Fixture mode exercises policy math only and can never grant physical capabilities.
EOF
}

serial=""
fixture=""
evidence=""
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
    --evidence)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      evidence="$2"
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

if [[ -n "$fixture" ]]; then
  [[ -z "$serial" && -z "$evidence" ]] || { usage; exit 64; }
elif [[ -n "$serial" ]]; then
  [[ -z "$fixture" ]] || { usage; exit 64; }
else
  usage
  exit 64
fi
[[ -z "$evidence" || -n "$serial" ]] || { usage; exit 64; }

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

report_value() {
  local key="$1" report="$2"
  awk -F= -v wanted="$key" '
    $1 == wanted {
      value=$0
      sub(/^[^=]*=/, "", value)
      print value
      found=1
      exit
    }
    END { if (!found) exit 2 }
  ' <<<"$report"
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

require_report() {
  local key="$1" report="$2" value
  value="$(report_value "$key" "$report")" || { echo "minimal GPU report missing field: $key" >&2; exit 65; }
  [[ -n "$value" ]] || { echo "minimal GPU report has empty field: $key" >&2; exit 65; }
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

# No captured evidence means this is the physical bring-up/preflight entrypoint. Keep its report
# useful even while the release threshold profile is intentionally uncalibrated.
if [[ -n "$serial" && -z "$evidence" ]]; then
  set +e
  minimal="$("$script_dir/run-minimal.sh" --adb "$serial" --renderer gfxstream-vulkan-mali 2>&1)"
  minimal_status=$?
  set -e

  if [[ $minimal_status -ne 0 && $minimal_status -ne 4 ]]; then
    printf '%s\n' "$minimal" >&2
    exit "$minimal_status"
  fi

  blocker="$(report_value blocker "$minimal" 2>/dev/null || true)"
  [[ -n "$blocker" ]] || blocker="sustained-workload-runner-not-wired"
  report="$(cat <<EOF
schema_version=1
evidence_source=physical-adb
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=BLOCKED
threshold_failure=none
requested_minutes=$minutes
workload_version=sustained-gpu-v1
device_model=$(report_value device_model "$minimal" 2>/dev/null || echo unknown)
soc=$(report_value soc "$minimal" 2>/dev/null || echo unknown)
gpu=$(report_value gpu "$minimal" 2>/dev/null || echo unknown)
host_build_fingerprint=$(report_value host_build_fingerprint "$minimal" 2>/dev/null || echo unknown)
kernel_version=$(report_value kernel_version "$minimal" 2>/dev/null || echo unknown)
avf_revision=$(report_value avf_revision "$minimal" 2>/dev/null || echo unknown)
renderer_id=gfxstream-vulkan-mali
graphics_driver_version=$(report_value graphics_driver_version "$minimal" 2>/dev/null || echo unknown)
android_guest_bundle_version=$(report_value guest_bundle_version "$minimal" 2>/dev/null || echo unknown)
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

physical=false
minimal=""
if [[ -n "$serial" ]]; then
  physical=true
  [[ -f "$evidence" ]] || { echo "evidence file not found: $evidence" >&2; exit 66; }
  [[ "$(require_kv evidence_source "$evidence")" == physical-adb ]] || {
    echo "--adb --evidence requires evidence_source=physical-adb" >&2
    exit 65
  }
  evaluation_evidence="$evidence"
else
  [[ -f "$fixture" ]] || { echo "fixture not found: $fixture" >&2; exit 66; }
  [[ "$(require_kv evidence_source "$fixture")" == fixture ]] || {
    echo "--fixture requires evidence_source=fixture" >&2
    exit 65
  }
  evaluation_evidence="$fixture"
fi

if [[ "$profile_status" != calibrated ]]; then
  source=fixture
  [[ "$physical" == true ]] && source=physical-adb
  report="$(cat <<EOF
schema_version=1
evidence_source=$source
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=BLOCKED
threshold_failure=none
requested_minutes=$minutes
workload_version=$(require_kv workload_version "$evaluation_evidence")
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

if [[ "$physical" == true ]]; then
  set +e
  minimal="$("$script_dir/run-minimal.sh" --adb "$serial" --evidence "$evidence" 2>&1)"
  minimal_status=$?
  set -e
  if [[ $minimal_status -ne 0 ]]; then
    printf '%s\n' "$minimal" >&2
    exit "$minimal_status"
  fi

  for capability in gpu.basic gpu.vulkan gpu.native-buffer; do
    [[ "$(require_report "$capability" "$minimal")" == true ]] || {
      report="$(cat <<EOF
schema_version=1
evidence_source=physical-adb
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=BLOCKED
threshold_failure=none
requested_minutes=$minutes
workload_version=$(require_kv workload_version "$evaluation_evidence")
gpu.sustained=false
gpu.qualified=false
qualification=EXPERIMENTAL
sustained_status=BLOCKED
blocker=minimal-gpu-proof-missing:$capability
EOF
)"
      emit_report "$report"
      exit 4
    }
  done
  [[ "$(require_report hardware_render_evidence "$minimal")" == positive ]] || {
    echo "physical sustained evidence lacks positive hardware render proof" >&2
    exit 65
  }
  [[ "$(require_report explicit_sync_evidence "$minimal")" == positive ]] || {
    echo "physical sustained evidence lacks explicit synchronization proof" >&2
    exit 65
  }

  tuple_mismatch=""
  compare_tuple() {
    local profile_key="$1" actual="$2" expected
    expected="$(require_toml "$profile_key")"
    if [[ "$actual" != "$expected" ]]; then
      if [[ -z "$tuple_mismatch" ]]; then
        tuple_mismatch="$profile_key"
      else
        tuple_mismatch="$tuple_mismatch,$profile_key"
      fi
    fi
  }

  compare_tuple carbonhv_commit "$(require_report carbonhv_commit "$minimal")"
  compare_tuple device_model "$(require_report device_model "$minimal")"
  compare_tuple soc "$(require_report soc "$minimal")"
  compare_tuple gpu "$(require_report gpu "$minimal")"
  compare_tuple host_build_fingerprint "$(require_report host_build_fingerprint "$minimal")"
  compare_tuple kernel_version "$(require_report kernel_version "$minimal")"
  compare_tuple avf_revision "$(require_report avf_revision "$minimal")"
  compare_tuple renderer_id "$(require_report renderer_id "$minimal")"
  compare_tuple renderer_version "$(require_report renderer_version "$minimal")"
  compare_tuple graphics_driver_version "$(require_report graphics_driver_version "$minimal")"
  compare_tuple android_guest_bundle_version "$(require_report guest_bundle_version "$minimal")"
  compare_tuple ubuntu_touch_guest_bundle_version "$(require_kv ubuntu_touch_guest_bundle_version "$evaluation_evidence")"
  compare_tuple cvd_major "$(require_report cvd_major "$minimal")"
  compare_tuple display_mode "$(require_report display_mode "$minimal")"
  compare_tuple workload_version "$(require_report workload_version "$minimal")"

  if [[ -n "$tuple_mismatch" ]]; then
    report="$(cat <<EOF
schema_version=1
evidence_source=physical-adb
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=BLOCKED
threshold_failure=none
requested_minutes=$minutes
workload_version=$(require_kv workload_version "$evaluation_evidence")
gpu.sustained=false
gpu.qualified=false
qualification=EXPERIMENTAL
sustained_status=BLOCKED
blocker=qualification-tuple-mismatch:$tuple_mismatch
EOF
)"
    emit_report "$report"
    exit 4
  fi
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
  require_kv "$key" "$evaluation_evidence" >/dev/null
done

workload_keys=(
  static_composition continuous_scrolling browser_rendering rapid_content gles
  multitouch_animation rotation surface_recreation background_foreground renderer_recovery
)
failures=""
for guest in android ubuntu_touch; do
  for workload in "${workload_keys[@]}"; do
    [[ "$(require_kv "$guest.$workload" "$evaluation_evidence")" == pass ]] || append_failure "$guest.$workload"
  done
  vulkan_exposed="$(require_kv "$guest.vulkan_exposed" "$evaluation_evidence")"
  vulkan_result="$(require_kv "$guest.vulkan" "$evaluation_evidence")"
  case "$vulkan_exposed" in
    true) [[ "$vulkan_result" == pass ]] || append_failure "$guest.vulkan" ;;
    false) [[ "$vulkan_result" == not-exposed ]] || append_failure "$guest.vulkan-contract" ;;
    *) append_failure "$guest.vulkan-exposed" ;;
  esac
done

sustained_seconds="$(require_kv sustained_seconds "$evaluation_evidence")"
[[ "$sustained_seconds" =~ ^[0-9]+$ ]] || { echo "invalid sustained_seconds=$sustained_seconds" >&2; exit 65; }
required_seconds=$((minutes * 60))
[[ "$sustained_seconds" -ge "$required_seconds" ]] || append_failure "sustained_seconds"

[[ "$(require_kv recovery_result "$evaluation_evidence")" == pass ]] || append_failure "recovery_result"

cpu_copies="$(require_kv full_frame_cpu_copies "$evaluation_evidence")"
[[ "$cpu_copies" =~ ^[0-9]+$ ]] || { echo "invalid full_frame_cpu_copies=$cpu_copies" >&2; exit 65; }
[[ "$cpu_copies" -eq 0 ]] || append_failure "full_frame_cpu_copies"

check_min() {
  local evidence_key="$1" profile_key="$2" got limit
  got="$(require_kv "$evidence_key" "$evaluation_evidence")"
  limit="$(require_toml "$profile_key")"
  is_number "$got" || { echo "invalid numeric evidence: $evidence_key=$got" >&2; exit 65; }
  numeric_ge "$got" "$limit" || append_failure "$evidence_key"
}

check_max() {
  local evidence_key="$1" profile_key="$2" got limit
  got="$(require_kv "$evidence_key" "$evaluation_evidence")"
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

threshold_evaluation=PASS
threshold_failure=none
workload_evaluation=PASS
exit_status=0
if [[ -n "$failures" ]]; then
  threshold_evaluation=FAIL
  threshold_failure="$failures"
  workload_evaluation=FAIL
  exit_status=4
fi

gpu_sustained=false
gpu_qualified=false
qualification=EXPERIMENTAL
source=fixture
if [[ "$physical" == true ]]; then
  source=physical-adb
  if [[ "$threshold_evaluation" == PASS ]]; then
    gpu_sustained=true
    gpu_qualified=true
    qualification=QUALIFIED
  fi
fi

identity_lines=""
if [[ "$physical" == true ]]; then
  identity_lines="$(cat <<EOF
carbonhv_commit=$(require_report carbonhv_commit "$minimal")
device_model=$(require_report device_model "$minimal")
soc=$(require_report soc "$minimal")
gpu=$(require_report gpu "$minimal")
host_build_fingerprint=$(require_report host_build_fingerprint "$minimal")
kernel_version=$(require_report kernel_version "$minimal")
avf_revision=$(require_report avf_revision "$minimal")
renderer_id=$(require_report renderer_id "$minimal")
renderer_version=$(require_report renderer_version "$minimal")
graphics_driver_version=$(require_report graphics_driver_version "$minimal")
cvd_major=$(require_report cvd_major "$minimal")
display_mode=$(require_report display_mode "$minimal")
EOF
)"
fi

report="$(cat <<EOF
schema_version=1
evidence_source=$source
profile_id=$profile_id
threshold_profile_status=$profile_status
threshold_evaluation=$threshold_evaluation
threshold_failure=$threshold_failure
requested_minutes=$minutes
$identity_lines
workload_version=$(require_kv workload_version "$evaluation_evidence")
android_guest_bundle_version=$(require_kv android_guest_bundle_version "$evaluation_evidence")
ubuntu_touch_guest_bundle_version=$(require_kv ubuntu_touch_guest_bundle_version "$evaluation_evidence")
sustained_seconds=$sustained_seconds
presented_fps=$(require_kv presented_fps "$evaluation_evidence")
frame_p50_ms=$(require_kv frame_p50_ms "$evaluation_evidence")
frame_p95_ms=$(require_kv frame_p95_ms "$evaluation_evidence")
frame_p99_ms=$(require_kv frame_p99_ms "$evaluation_evidence")
dropped_frame_pct=$(require_kv dropped_frame_pct "$evaluation_evidence")
max_queue_depth=$(require_kv max_queue_depth "$evaluation_evidence")
input_to_visible_ms=$(require_kv input_to_visible_ms "$evaluation_evidence")
graphics_cpu_pct=$(require_kv graphics_cpu_pct "$evaluation_evidence")
renderer_rss_mib=$(require_kv renderer_rss_mib "$evaluation_evidence")
graphics_buffer_mib=$(require_kv graphics_buffer_mib "$evaluation_evidence")
full_frame_cpu_copies=$cpu_copies
gpu_side_full_frame_copies=$(require_kv gpu_side_full_frame_copies "$evaluation_evidence")
thermal_status_peak=$(require_kv thermal_status_peak "$evaluation_evidence")
power_energy_source=$(require_kv power_energy_source "$evaluation_evidence")
recovery_result=$(require_kv recovery_result "$evaluation_evidence")
workload_evaluation=$workload_evaluation
gpu.sustained=$gpu_sustained
gpu.qualified=$gpu_qualified
qualification=$qualification
sustained_status=$threshold_evaluation
EOF
)"
emit_report "$report"
exit "$exit_status"
