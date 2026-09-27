#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

usage() {
  cat >&2 <<'EOF'
usage:
  run-minimal.sh --fixture <evidence.env> --dry-run [--output <report>]
  run-minimal.sh --adb <serial> --evidence <guest-renderer.env> [--output <report>]

Fixture mode is deterministic and never counts as physical qualification.
Physical mode performs read-only host probing and combines it with evidence captured from the
guest virtio-gpu -> carbon-renderer -> Android presentation path.
EOF
}

fixture=""
serial=""
evidence=""
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

if [[ -n "$fixture" && -z "$serial" && -z "$evidence" && "$dry_run" == true ]]; then
  report="$(collect_from_input "$fixture")"
elif [[ -n "$serial" && -n "$evidence" && -z "$fixture" && "$dry_run" == false ]]; then
  command -v adb >/dev/null || { echo "adb not found" >&2; exit 69; }
  [[ -f "$evidence" ]] || { echo "evidence file not found: $evidence" >&2; exit 66; }

  set +e
  host_report="$("$repo_root/qualification/carbon-qualify.sh" --adb "$serial" 2>&1)"
  host_status=$?
  set -e
  [[ $host_status -eq 0 || $host_status -eq 4 ]] || {
    printf '%s\n' "$host_report" >&2
    exit "$host_status"
  }

  host_value() {
    local key="$1"
    report_value "$key" "$host_report"
  }

  device_model="$(adb -s "$serial" shell getprop ro.product.model 2>/dev/null | tr -d '\r')"
  [[ -n "$device_model" ]] || device_model=unknown

  merged="$(mktemp "${TMPDIR:-/tmp}/carbonhv-gpu-evidence.XXXXXX")"
  trap 'rm -f "$merged"' EXIT
  {
    printf 'evidence_source=physical-adb\n'
    printf 'carbonhv_commit=%s\n' "$(git -C "$repo_root" rev-parse HEAD)"
    printf 'device_model=%s\n' "$device_model"
    printf 'soc=%s\n' "$(host_value soc)"
    printf 'gpu=%s\n' "$(host_value gpu_renderer)"
    printf 'host_build_fingerprint=%s\n' "$(host_value build_fingerprint)"
    printf 'kernel_version=%s\n' "$(host_value kernel)"
    printf 'avf_revision=%s\n' "$(host_value avf_revision)"
    printf 'graphics_driver_version=%s\n' "$(host_value vulkan_driver_version)"
    cat "$evidence"
  } > "$merged"
  report="$(collect_from_input "$merged")"
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
