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

mkdir -p "$tmp/bin"
cat > "$tmp/bin/adb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

query="${*: -1}"
case "$query" in
  *"ro.product.cpu.abi"*) printf 'arm64-v8a\n' ;;
  *"ro.product.model"*) printf 'Pixel 7a\n' ;;
  *"ro.soc.model"*) printf 'GS201\n' ;;
  *"ro.hardware.egl"*) printf 'mali\n' ;;
  *"ro.hardware.vulkan"*) printf 'mali\n' ;;
  *"ro.build.fingerprint"*) printf 'google/lynx/test:user/release-keys\n' ;;
  *"uname -r"*) printf '6.1-test\n' ;;
  *"apex-info-list.xml"*)
    printf '<apex-info moduleName="com.android.virt" versionCode="371899999" isActive="true"/>\n'
    ;;
  *"SurfaceFlinger"*)
    printf '  Name: Mali-G710\n'
    printf '  Driver version: 226504704\n'
    ;;
  *"dumpsys display"*) printf '1080x2400@90;1080x2400@60\n' ;;
  *"/apex/com.android.virt/bin/vm"*) printf 'yes\n' ;;
  *"/dev/kvm"*) printf 'yes\n' ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$tmp/bin/adb"

sed 's/evidence_source=fixture/evidence_source=physical-adb/' "$tmp/evidence.env" > "$tmp/physical.env"
cat >> "$tmp/physical.env" <<'EOF'
carbonhv_commit=evidence-is-bound-to-live-head-by-run-minimal
device_model=Pixel 7a
soc=GS201
gpu=Mali-G710
host_build_fingerprint=google/lynx/test:user/release-keys
kernel_version=6.1-test
avf_revision=com.android.virt@371899999
renderer_id=gfxstream-vulkan-mali
renderer_version=crosvm-88c1385+rutabaga-0.1.80+gfxstream-ebe5b424
graphics_driver_version=226504704
guest_bundle_id=grapheneos
guest_bundle_version=2026.09.27-test
cvd_major=1
display_mode=1080x2400@90
renderer_process_identity=carbon-renderer/rutabaga-gfxstream
guest_gpu_transport=virtio-gpu
guest_renderer=gfxstream-vulkan
hardware_render_evidence=positive
hardware_render_source=renderer-vulkan-physical-device+host-gpu-counter-delta
software_renderer_detected=false
host_gpu_busy_delta_pct=23.5
presented_frames=108000
native_buffer_presented_frames=108000
explicit_sync_evidence=positive
frame_samples=108000
dropped_frames=756
guest_gles_workload=pass
guest_vulkan_workload=pass
sustained_workload=pass
EOF

repo_root="$(cd "$script_dir/../.." && pwd)"
head_commit="$(git -C "$repo_root" rev-parse HEAD)"
cat > "$tmp/calibrated-physical.toml" <<EOF
profile_id = "pixel-7a-mali-g710"
status = "calibrated"
minimum_minutes = 30
baseline_report = "physical-baseline"
threshold_source = "physical-baseline"
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
carbonhv_commit = "$head_commit"
device_model = "Pixel 7a"
soc = "GS201"
gpu = "Mali-G710"
host_build_fingerprint = "google/lynx/test:user/release-keys"
kernel_version = "6.1-test"
avf_revision = "com.android.virt@371899999"
renderer_id = "gfxstream-vulkan-mali"
renderer_version = "crosvm-88c1385+rutabaga-0.1.80+gfxstream-ebe5b424"
graphics_driver_version = "226504704"
android_guest_bundle_version = "2026.09.27-test"
ubuntu_touch_guest_bundle_version = "24.04-test"
cvd_major = 1
display_mode = "1080x2400@90"
workload_version = "sustained-gpu-v1"
EOF

run_capture() {
  local expected="$1"
  shift
  set +e
  output="$("$@" 2>&1)"
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

# Break caught: a calibrated exact physical tuple with both guests and all workload gates can
# promote sustained/qualified; the same data cannot qualify through fixture mode.
run_capture 0 env PATH="$tmp/bin:$PATH" "$script_dir/run-sustained.sh" \
  --adb fake-serial --evidence "$tmp/physical.env" \
  --profile "$tmp/calibrated-physical.toml" --minutes 30
grep -qx 'evidence_source=physical-adb' <<<"$output"
grep -qx 'threshold_evaluation=PASS' <<<"$output"
grep -qx 'gpu.sustained=true' <<<"$output"
grep -qx 'gpu.qualified=true' <<<"$output"
grep -qx 'qualification=QUALIFIED' <<<"$output"

# Break caught: qualification must not carry to a materially different host tuple.
sed 's#host_build_fingerprint = "google/lynx/test:user/release-keys"#host_build_fingerprint = "google/lynx/different:user/release-keys"#' \
  "$tmp/calibrated-physical.toml" > "$tmp/wrong-host.toml"
run_capture 4 env PATH="$tmp/bin:$PATH" "$script_dir/run-sustained.sh" \
  --adb fake-serial --evidence "$tmp/physical.env" \
  --profile "$tmp/wrong-host.toml" --minutes 30
grep -qx 'threshold_evaluation=BLOCKED' <<<"$output"
grep -qx 'gpu.qualified=false' <<<"$output"
grep -q '^blocker=qualification-tuple-mismatch:' <<<"$output"

echo "GPU sustained qualification fixture: PASS"
