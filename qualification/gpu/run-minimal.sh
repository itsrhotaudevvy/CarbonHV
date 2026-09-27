#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat >&2 <<'EOF'
usage:
  run-minimal.sh --fixture <evidence.env> --dry-run [--output <report>]
  run-minimal.sh --adb <serial> --renderer <profile> [--output <report>]
  run-minimal.sh --adb <serial> --evidence <guest-renderer.env> [--output <report>]

Fixture mode is deterministic and never counts as physical qualification.
Physical mode performs read-only host probing and combines it with evidence captured from the
guest virtio-gpu -> carbon-renderer -> Android presentation path.
The --renderer form performs the physical bring-up preflight and emits an EXPERIMENTAL blocker
report when the guest/renderer path cannot yet be launched end to end.
EOF
}

fixture=""
serial=""
evidence=""
renderer=""
output=""
dry_run=false

while (($#)); do
  case "$1" in
    --fixture)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      fixture="$2"
      shift 2
      ;;
    --adb)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      serial="$2"
      shift 2
      ;;
    --evidence)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      evidence="$2"
      shift 2
      ;;
    --renderer)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      renderer="$2"
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      output="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    *)
      usage
      exit 64
      ;;
  esac
done

report_value() {
  local key="$1" report="$2"
  awk -F= -v wanted="$key" '$1 == wanted { sub(/^[^=]*=/, "", $0); print; found=1; exit } END { if (!found) exit 2 }' <<<"$report"
}

validate_report() {
  local report="$1" key value
  while IFS= read -r key; do
    [[ -n "$key" && "$key" != \#* ]] || continue
    value="$(report_value "$key" "$report")" || {
      echo "report missing schema field: $key" >&2
      exit 65
    }
    [[ -n "$value" ]] || { echo "report has empty schema field: $key" >&2; exit 65; }
  done < "$script_dir/schema/report-v1.txt"

  if [[ "$(report_value hardware_render_evidence "$report")" != positive ]]; then
    for capability in gpu.vulkan gpu.native-buffer gpu.zero-copy gpu.sustained gpu.qualified; do
      [[ "$(report_value "$capability" "$report")" != true ]] || {
        echo "unsafe capability grant without positive hardware-render evidence: $capability" >&2
        exit 65
      }
    done
  fi
}

collect_from_input() {
  "$script_dir/collect-report.sh" --input "$1"
}

probe_physical_host() {
  command -v adb >/dev/null || { echo "adb not found" >&2; exit 69; }
  set +e
  host_report="$("$repo_root/qualification/carbon-qualify.sh" --adb "$serial" 2>&1)"
  host_status=$?
  set -e
  [[ $host_status -eq 0 || $host_status -eq 4 ]] || {
    printf '%s\n' "$host_report" >&2
    exit "$host_status"
  }
  device_model="$(adb -s "$serial" shell getprop ro.product.model 2>/dev/null | tr -d '\r')"
  [[ -n "$device_model" ]] || device_model=unknown
}

host_value() {
  local key="$1"
  report_value "$key" "$host_report"
}

write_host_prefix() {
  printf 'evidence_source=physical-adb\n'
  printf 'carbonhv_commit=%s\n' "$(git -C "$repo_root" rev-parse HEAD)"
  printf 'device_model=%s\n' "$device_model"
  printf 'soc=%s\n' "$(host_value soc)"
  printf 'gpu=%s\n' "$(host_value gpu_renderer)"
  printf 'host_build_fingerprint=%s\n' "$(host_value build_fingerprint)"
  printf 'kernel_version=%s\n' "$(host_value kernel)"
  printf 'avf_revision=%s\n' "$(host_value avf_revision)"
  printf 'graphics_driver_version=%s\n' "$(host_value vulkan_driver_version)"
}

physical_exit=0

if [[ -n "$fixture" && -z "$serial" && -z "$evidence" && -z "$renderer" && "$dry_run" == true ]]; then
  report="$(collect_from_input "$fixture")"
elif [[ -n "$serial" && -n "$evidence" && -z "$fixture" && "$dry_run" == false ]]; then
  [[ -f "$evidence" ]] || { echo "evidence file not found: $evidence" >&2; exit 66; }
  probe_physical_host

  merged="$(mktemp "${TMPDIR:-/tmp}/carbonhv-gpu-evidence.XXXXXX")"
  trap 'rm -f "$merged"' EXIT
  {
    write_host_prefix
    cat "$evidence"
  } > "$merged"
  report="$(collect_from_input "$merged")"
elif [[ -n "$serial" && -n "$renderer" && -z "$evidence" && -z "$fixture" && "$dry_run" == false ]]; then
  [[ "$renderer" == gfxstream-vulkan-mali ]] || {
    echo "unsupported physical renderer profile: $renderer" >&2
    exit 65
  }
  probe_physical_host

  blockers=()
  for artifact in out/grapheneos/boot.img out/grapheneos/system.img; do
    [[ -f "$repo_root/$artifact" ]] || blockers+=("missing-$artifact")
  done
  renderer_present="$(adb -s "$serial" shell 'if [ -x /system/bin/carbon-renderer ]; then echo yes; else echo no; fi' 2>/dev/null | tr -d '\r')"
  [[ "$renderer_present" == yes ]] || blockers+=("carbon-renderer-not-installed")
  blockers+=("avf-renderer-resource-transport-not-wired")

  guest_version="$(awk -F= '$1 ~ /^[[:space:]]*version[[:space:]]*$/ { value=$2; gsub(/^[[:space:]\"]+|[[:space:]\"]+$/, "", value); print value; exit }' "$repo_root/bundles/grapheneos/bundle.toml")"
  [[ -n "$guest_version" ]] || guest_version=unknown

  merged="$(mktemp "${TMPDIR:-/tmp}/carbonhv-gpu-preflight.XXXXXX")"
  trap 'rm -f "$merged"' EXIT
  {
    write_host_prefix
    printf 'renderer_id=%s\n' "$renderer"
    printf 'renderer_version=crosvm-88c1385+rutabaga-0.1.80+gfxstream-ebe5b424\n'
    printf 'guest_bundle_id=grapheneos\n'
    printf 'guest_bundle_version=%s\n' "$guest_version"
    printf 'cvd_major=1\n'
    printf 'display_mode=not-presented\n'
    printf 'workload_version=minimal-gpu-v1\n'
    printf 'renderer_process_identity=not-running\n'
    printf 'guest_gpu_transport=unverified\n'
    printf 'guest_renderer=unverified\n'
    printf 'hardware_render_evidence=unknown\n'
    printf 'hardware_render_source=none\n'
    printf 'software_renderer_detected=unknown\n'
    printf 'host_gpu_busy_delta_pct=unknown\n'
    printf 'presented_frames=0\n'
    printf 'native_buffer_presented_frames=0\n'
    printf 'explicit_sync_evidence=unknown\n'
    printf 'full_frame_cpu_copies=0\n'
    printf 'gpu_side_full_frame_copies=0\n'
    printf 'frame_samples=0\n'
    printf 'presented_fps=0\n'
    printf 'frame_p50_ms=unknown\n'
    printf 'frame_p95_ms=unknown\n'
    printf 'frame_p99_ms=unknown\n'
    printf 'dropped_frames=0\n'
    printf 'max_queue_depth=0\n'
    printf 'guest_gles_workload=not-run\n'
    printf 'guest_vulkan_workload=not-run\n'
    printf 'sustained_seconds=0\n'
    printf 'sustained_workload=not-run\n'
  } > "$merged"
  report="$(collect_from_input "$merged")"
  blocker_text="$(IFS=,; echo "${blockers[*]}")"
  report="${report}
bringup_status=BLOCKED
blocker=${blocker_text}"
  physical_exit=4
else
  usage
  exit 64
fi

validate_report "$report"

if [[ -n "$output" ]]; then
  mkdir -p "$(dirname "$output")"
  printf '%s\n' "$report" > "$output"
fi
printf '%s\n' "$report"
exit "$physical_exit"
