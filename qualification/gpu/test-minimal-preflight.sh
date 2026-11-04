#!/usr/bin/env bash
# Offline fixture for run-minimal.sh physical bring-up preflight on Snapdragon/Adreno (Gunyah, no
# KVM) and Pixel/Mali hosts. No device is required; adb is faked.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
run_minimal="$repo_root/qualification/gpu/run-minimal.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cat >"$tmp/adb" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail

host="${FAKE_HOST:-snapdragon}"
snap() { [[ "$host" == snapdragon ]]; }
query="${*: -1}"
case "$query" in
  *"ro.product.cpu.abi"*) printf 'arm64-v8a\n' ;;
  *"ro.product.model"*) snap && printf '25098RA98G\n' || printf 'Pixel 7a\n' ;;
  *"ro.soc.model"*) snap && printf 'SM6475\n' || printf 'GS201\n' ;;
  *"ro.hardware.egl"*) snap && printf 'adreno\n' || printf 'mali\n' ;;
  *"ro.hardware.vulkan"*) snap && printf 'adreno\n' || printf 'mali\n' ;;
  *"ro.build.fingerprint"*) printf 'fixture/device:user/release-keys\n' ;;
  *"uname -r"*) printf '6.6-test\n' ;;
  *"apex-info-list.xml"*)
    printf '<apex-info moduleName="com.android.virt" versionCode="3" isActive="true"/>\n'
    ;;
  *"SurfaceFlinger"*)
    if snap; then
      printf 'GLES: Qualcomm, Adreno (TM) 710, OpenGL ES 3.2 V@0800.70\n'
    else
      printf '  Name: Mali-G710\n  Driver version: 226504704\n'
    fi
    ;;
  *"vkjson"*) snap && printf '"driverVersion" : 2150760518.0,\n' || true ;;
  *"dumpsys display"*) printf '1080x2400@60\n' ;;
  *"/dev/gunyah"*) snap && printf 'yes\n' || printf 'no\n' ;;
  *"qvirt"*) snap && printf 'Service vendor.qti.qvirt.IVirtualizationService/default: found\n' || true ;;
  *"vm info"*) snap && printf 'VMs are not supported.\n' || true ;;
  *"/dev/kvm"*) snap && printf 'no\n' || printf 'yes\n' ;;
  *"/apex/com.android.virt/bin/vm"*) printf 'yes\n' ;;
  *) exit 0 ;;
esac
FAKE
chmod +x "$tmp/adb"
export PATH="$tmp:$PATH"

# Snapdragon: preflight yields an experimental blocker report that names the hypervisor gap.
set +e
report="$(FAKE_HOST=snapdragon "$run_minimal" --adb fake --renderer gfxstream-vulkan-adreno 2>&1)"
status=$?
set -e
test "$status" -eq 4
grep -Fxq 'soc=SM6475' <<<"$report"
grep -Fxq 'gpu=Adreno (TM) 710' <<<"$report"
grep -Fxq 'graphics_driver_version=2150760518' <<<"$report"
grep -Fxq 'renderer_id=gfxstream-vulkan-adreno' <<<"$report"
grep -Fxq 'bringup_status=BLOCKED' <<<"$report"
grep -Eq '^blocker=host-virtualization-unsupported-gunyah,' <<<"$report"
for capability in gpu.basic gpu.vulkan gpu.native-buffer gpu.zero-copy gpu.sustained gpu.qualified; do
  grep -Fxq "$capability=false" <<<"$report"
done
grep -Fxq 'qualification=EXPERIMENTAL' <<<"$report"

# Profile and host GPU family must agree in both directions.
set +e
FAKE_HOST=snapdragon "$run_minimal" --adb fake --renderer gfxstream-vulkan-mali >/dev/null 2>&1
mismatch_a=$?
FAKE_HOST=pixel "$run_minimal" --adb fake --renderer gfxstream-vulkan-adreno >/dev/null 2>&1
mismatch_b=$?
set -e
test "$mismatch_a" -eq 65
test "$mismatch_b" -eq 65

# Pixel/Mali keeps working and does not gain a hypervisor blocker.
set +e
pixel="$(FAKE_HOST=pixel "$run_minimal" --adb fake --renderer gfxstream-vulkan-mali 2>&1)"
pixel_status=$?
set -e
test "$pixel_status" -eq 4
! grep -Fq 'host-virtualization-unsupported' <<<"$pixel"

# Evidence mode stays strict: an unsupported host cannot yield graphics evidence.
printf 'schema_version=1\n' >"$tmp/evidence.env"
set +e
FAKE_HOST=snapdragon "$run_minimal" --adb fake --evidence "$tmp/evidence.env" >/dev/null 2>&1
evidence_status=$?
set -e
test "$evidence_status" -eq 3

echo "gpu minimal preflight fixture: PASS"
