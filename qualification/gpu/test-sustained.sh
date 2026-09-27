#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/carbonhv-sustained-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/evidence.env" <<'EOF'
evidence_source=fixture
workload_version=sustained-gpu-v1
android_guest_bundle_version=2026.09.27-test
ubuntu_touch_guest_bundle_version=24.04-test
android.static_composition=pass
android.continuous_scrolling=pass
android.browser_rendering=pass
android.rapid_content=pass
android.gles=pass
android.vulkan_exposed=true
android.vulkan=pass
android.multitouch_animation=pass
android.rotation=pass
android.surface_recreation=pass
android.background_foreground=pass
android.renderer_recovery=pass
ubuntu_touch.static_composition=pass
ubuntu_touch.continuous_scrolling=pass
ubuntu_touch.browser_rendering=pass
ubuntu_touch.rapid_content=pass
ubuntu_touch.gles=pass
ubuntu_touch.vulkan_exposed=false
ubuntu_touch.vulkan=not-exposed
ubuntu_touch.multitouch_animation=pass
ubuntu_touch.rotation=pass
ubuntu_touch.surface_recreation=pass
ubuntu_touch.background_foreground=pass
ubuntu_touch.renderer_recovery=pass
sustained_seconds=1800
presented_fps=58.4
frame_p50_ms=16.8
frame_p95_ms=19.4
frame_p99_ms=24.0
dropped_frame_pct=0.7
max_queue_depth=2
input_to_visible_ms=54.0
graphics_cpu_pct=32.0
renderer_rss_mib=184
graphics_buffer_mib=96
full_frame_cpu_copies=0
gpu_side_full_frame_copies=1800
thermal_status_peak=2
power_energy_source=fixture-meter
recovery_result=pass
EOF

cat > "$tmp/calibrated.toml" <<'EOF'
profile_id = "fixture-mali"
status = "calibrated"
minimum_minutes = 30
baseline_report = "fixture-baseline"
threshold_source = "fixture-only"
min_presented_fps = 55
max_frame_p95_ms = 22
max_frame_p99_ms = 28
max_dropped_frame_pct = 2
max_queue_depth = 3
max_input_to_visible_ms = 80
max_graphics_cpu_pct = 60
max_renderer_rss_mib = 512
max_graphics_buffer_mib = 256
max_thermal_status = 3
EOF

cat > "$tmp/uncalibrated.toml" <<'EOF'
profile_id = "pixel-7a-mali-g710"
status = "uncalibrated"
minimum_minutes = 30
baseline_report = "pending"
threshold_source = "pending-physical-baseline"
EOF

run_capture() {
  local expected="$1"
  shift
  set +e
  output="$($@ 2>&1)"
  status=$?
  set -e
  if [[ $status -ne $expected ]]; then
    printf 'expected exit %s, got %s\n%s\n' "$expected" "$status" "$output" >&2
    exit 1
  fi
}

# Break caught: an uncalibrated release profile must never manufacture sustained/qualified state.
run_capture 4 "$script_dir/run-sustained.sh" \
  --fixture "$tmp/evidence.env" --profile "$tmp/uncalibrated.toml" --minutes 30
grep -qx 'threshold_profile_status=uncalibrated' <<<"$output"
grep -qx 'threshold_evaluation=BLOCKED' <<<"$output"
grep -qx 'gpu.sustained=false' <<<"$output"
grep -qx 'gpu.qualified=false' <<<"$output"
grep -qx 'qualification=EXPERIMENTAL' <<<"$output"

# Break caught: fixture data may exercise threshold math but can never qualify hardware.
run_capture 0 "$script_dir/run-sustained.sh" \
  --fixture "$tmp/evidence.env" --profile "$tmp/calibrated.toml" --minutes 30
grep -qx 'threshold_evaluation=PASS' <<<"$output"
grep -qx 'gpu.sustained=false' <<<"$output"
grep -qx 'gpu.qualified=false' <<<"$output"
grep -qx 'qualification=EXPERIMENTAL' <<<"$output"

# Break caught: a measured value outside the locked baseline budget must fail closed.
sed 's/frame_p95_ms=19.4/frame_p95_ms=29.4/' "$tmp/evidence.env" > "$tmp/slow.env"
run_capture 4 "$script_dir/run-sustained.sh" \
  --fixture "$tmp/slow.env" --profile "$tmp/calibrated.toml" --minutes 30
grep -qx 'threshold_evaluation=FAIL' <<<"$output"
grep -q '^threshold_failure=.*frame_p95_ms' <<<"$output"
grep -qx 'gpu.qualified=false' <<<"$output"

# Break caught: a shorter run cannot satisfy the mandatory 30-minute class.
run_capture 64 "$script_dir/run-sustained.sh" \
  --fixture "$tmp/evidence.env" --profile "$tmp/calibrated.toml" --minutes 29

echo "GPU sustained qualification fixture: PASS"
