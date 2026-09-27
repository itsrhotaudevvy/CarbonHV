#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: collect-report.sh --input <normalized-evidence.env>" >&2
}

input=""
while (($#)); do
  case "$1" in
    --input)
      [[ $# -ge 2 ]] || { usage; exit 64; }
      input="$2"
      shift 2
      ;;
    *)
      usage
      exit 64
      ;;
  esac
done

[[ -n "$input" && -f "$input" ]] || { usage; exit 66; }

input_value() {
  local key="$1"
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
  ' "$input"
}

require_value() {
  local key="$1" value
  value="$(input_value "$key")" || { echo "missing evidence field: $key" >&2; exit 65; }
  [[ -n "$value" ]] || { echo "empty evidence field: $key" >&2; exit 65; }
  printf '%s' "$value"
}

required_input_fields=(
  evidence_source
  carbonhv_commit device_model soc gpu host_build_fingerprint kernel_version avf_revision
  renderer_id renderer_version graphics_driver_version guest_bundle_id guest_bundle_version
  cvd_major display_mode workload_version renderer_process_identity guest_gpu_transport guest_renderer
  hardware_render_evidence hardware_render_source software_renderer_detected host_gpu_busy_delta_pct
  presented_frames native_buffer_presented_frames explicit_sync_evidence
  full_frame_cpu_copies gpu_side_full_frame_copies frame_samples presented_fps
  frame_p50_ms frame_p95_ms frame_p99_ms dropped_frames max_queue_depth
  guest_gles_workload guest_vulkan_workload sustained_seconds sustained_workload
)

printf 'schema_version=1\n'
for key in "${required_input_fields[@]}"; do
  printf '%s=%s\n' "$key" "$(require_value "$key")"
done

presented_frames="$(require_value presented_frames)"
native_frames="$(require_value native_buffer_presented_frames)"
cpu_copies="$(require_value full_frame_cpu_copies)"
gpu_copies="$(require_value gpu_side_full_frame_copies)"
sustained_seconds="$(require_value sustained_seconds)"

for numeric in "$presented_frames" "$native_frames" "$cpu_copies" "$gpu_copies" "$sustained_seconds"; do
  [[ "$numeric" =~ ^[0-9]+$ ]] || { echo "invalid numeric evidence: $numeric" >&2; exit 65; }
done

hardware_proven=false
if [[ "$(require_value hardware_render_evidence)" == positive \
   && "$(require_value hardware_render_source)" != unknown \
   && "$(require_value software_renderer_detected)" == false \
   && "$(require_value guest_gpu_transport)" == virtio-gpu \
   && "$presented_frames" -gt 0 ]]; then
  hardware_proven=true
fi

basic=false
vulkan=false
native_buffer=false
zero_copy=false
sustained=false

if [[ "$(require_value guest_gpu_transport)" == virtio-gpu && "$presented_frames" -gt 0 ]]; then
  basic=true
fi
if [[ "$hardware_proven" == true && "$(require_value guest_vulkan_workload)" == pass ]]; then
  vulkan=true
fi
if [[ "$vulkan" == true \
   && "$native_frames" -gt 0 \
   && "$(require_value explicit_sync_evidence)" == positive ]]; then
  native_buffer=true
fi
if [[ "$native_buffer" == true && "$cpu_copies" -eq 0 && "$gpu_copies" -eq 0 ]]; then
  zero_copy=true
fi
if [[ "$native_buffer" == true \
   && "$sustained_seconds" -ge 1800 \
   && "$(require_value sustained_workload)" == pass ]]; then
  sustained=true
fi

printf 'gpu.basic=%s\n' "$basic"
printf 'gpu.vulkan=%s\n' "$vulkan"
printf 'gpu.native-buffer=%s\n' "$native_buffer"
printf 'gpu.zero-copy=%s\n' "$zero_copy"
printf 'gpu.sustained=%s\n' "$sustained"
printf 'gpu.qualified=false\n'
printf 'qualification=EXPERIMENTAL\n'
