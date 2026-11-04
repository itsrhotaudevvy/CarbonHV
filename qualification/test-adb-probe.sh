#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
qualify="$repo_root/qualification/carbon-qualify.sh"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

cat >"$tmp_dir/adb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

query="${*: -1}"
case "$query" in
  *"ro.product.cpu.abi"*) printf 'arm64-v8a\n' ;;
  *"ro.soc.model"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf 'SM6475\n'; else printf 'GS201\n'; fi
    ;;
  *"ro.hardware.egl"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf 'adreno\n'; else printf 'mali\n'; fi
    ;;
  *"ro.hardware.vulkan"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf 'adreno\n'; else printf 'mali\n'; fi
    ;;
  *"ls -Z /dev/gunyah"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf 'crw-rw---- 1 system system u:object_r:gunyah_device:s0 10, 55 2026-09-30 /dev/gunyah\n'; fi
    ;;
  *"ro.boot.hypervisor.vm.supported"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf '%s\n' "${FAKE_VM_SUPPORTED:-}"; else printf '1\n'; fi
    ;;
  *"ro.boot.hypervisor.protected_vm.supported"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf '\n'; else printf '1\n'; fi
    ;;
  *"ro.boot.hypervisor.version"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf '%s\n' "${FAKE_HV_VERSION:-}"; else printf 'kvm.arm-protected\n'; fi
    ;;
  *"cpufreq/policy"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf '0 1 2 3\n4 5 6 7\n'; else printf '0 1 2 3\n4 5\n6 7\n'; fi
    ;;
  *"transparent_hugepage"*) printf 'always [madvise] never\n' ;;
  *"/dev/gunyah"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf 'yes\n'; else printf 'no\n'; fi
    ;;
  *"qvirt"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then
      printf 'Service vendor.qti.qvirt.IVirtualizationService/default: found\n'
    fi
    ;;
  *"cpu_capacity"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf '405\n405\n405\n405\n1024\n1024\n1024\n1024\n'; fi
    ;;
  *"vkjson"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then
      printf '\t\t\t"deviceName" : "Adreno (TM) 710",\n\t\t\t"driverVersion" : 2150760518.0,\n'
    fi
    ;;
  *"vm info"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf 'VMs are not supported.\n'; fi
    ;;
  *"ro.build.fingerprint"*) printf 'google/lynx/test:user/release-keys\n' ;;
  *"uname -r"*) printf '6.1-test\n' ;;
  *"apexservice"*) exit 0 ;;
  *"apex-info-list.xml"*)
    printf '<apex-info moduleName="com.android.virt" versionCode="371899999" isActive="true"/>\n'
    ;;
  *"SurfaceFlinger"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then
      printf 'GLES: Qualcomm, Adreno (TM) 710, OpenGL ES 3.2 V@0800.70 (GIT@4e394f1848)\n'
      exit 0
    fi
    printf '  Name: Mali-G710\n'
    if [[ "${FAKE_DRIVER_VERSION:-present}" == present ]]; then
      printf '  Driver version: 226504704\n'
    fi
    ;;
  *"dumpsys display"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then
      printf '  DisplayDeviceInfo{"Built-in Screen": 1080 x 2392, modeId 3, supportedRefreshRates [120.00001, 90.0, 60.000004], defaultModeId 1}\n'
    else
      printf '1080x2400@90;1080x2400@60\n'
    fi
    ;;
  *"/apex/com.android.virt/bin/vm"*) printf 'yes\n' ;;
  *"/dev/kvm"*)
    if [[ "${FAKE_HOST:-pixel}" == snapdragon ]]; then printf 'no\n'; else printf 'yes\n'; fi
    ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$tmp_dir/adb"

set +e
output="$(PATH="$tmp_dir:$PATH" "$qualify" --adb fake-serial 2>&1)"
status=$?
set -e

test "$status" -eq 4
grep -Fq 'soc=GS201' <<<"$output"
grep -Fq 'gpu_renderer=Mali-G710' <<<"$output"
grep -Fq 'egl_provider=mali' <<<"$output"
grep -Fq 'vulkan_provider=mali' <<<"$output"
grep -Fq 'vulkan_driver_version=226504704' <<<"$output"
grep -Fq 'kernel=6.1-test' <<<"$output"
grep -Fq 'build_fingerprint=google/lynx/test:user/release-keys' <<<"$output"
grep -Fq 'avf_revision=com.android.virt@371899999' <<<"$output"
grep -Fq 'display_modes=1080x2400@90;1080x2400@60' <<<"$output"
grep -Fq 'graphics=unverified' <<<"$output"
grep -Fq 'qualification=EXPERIMENTAL' <<<"$output"
! grep -Fq 'gpu.vulkan=true' <<<"$output"

set +e
missing_driver_output="$(FAKE_DRIVER_VERSION=missing PATH="$tmp_dir:$PATH" "$qualify" --adb fake-serial 2>&1)"
missing_driver_status=$?
set -e

test "$missing_driver_status" -eq 4
grep -Fq 'vulkan_driver_version=unknown' <<<"$missing_driver_output"
! grep -Fq 'gpu.vulkan=true' <<<"$missing_driver_output"

set +e
gunyah_output="$(FAKE_HOST=snapdragon PATH="$tmp_dir:$PATH" "$qualify" --adb fake-serial 2>&1)"
gunyah_status=$?
set -e

test "$gunyah_status" -eq 3
grep -Fq 'soc=SM6475' <<<"$gunyah_output"
grep -Fq 'gpu_renderer=Adreno (TM) 710' <<<"$gunyah_output"
grep -Fq 'vulkan_driver_version=2150760518' <<<"$gunyah_output"
grep -Fq 'kvm_device=no' <<<"$gunyah_output"
grep -Fq 'gunyah_device=yes' <<<"$gunyah_output"
grep -Fq 'hypervisor=gunyah' <<<"$gunyah_output"
grep -Fq 'vendor_virtualization_service=yes' <<<"$gunyah_output"
grep -Fq 'avf_vm_info=VMs are not supported.' <<<"$gunyah_output"
grep -Fq 'cpu_capacities=405,405,405,405,1024,1024,1024,1024' <<<"$gunyah_output"
grep -Fq 'display_modes=1080x2392@120.00001,90.0,60.000004' <<<"$gunyah_output"
grep -Fq 'qualification=UNSUPPORTED' <<<"$gunyah_output"
grep -Fq 'reason=Gunyah hypervisor present but this build does not declare AVF VM support' <<<"$gunyah_output"
grep -Fq 'grant crosvm u:object_r:gunyah_device:s0' <<<"$gunyah_output"
grep -Fq 'hypervisor_vm_supported=unset' <<<"$gunyah_output"
grep -Fq 'gunyah_selinux_label=u:object_r:gunyah_device:s0' <<<"$gunyah_output"
grep -Fq 'avf_gunyah_path=blocked:ro.boot.hypervisor.vm.supported=unset' <<<"$gunyah_output"
grep -Fq 'cpu_freq_domains=0,1,2,3;4,5,6,7' <<<"$gunyah_output"
grep -Fq 'thp_mode=madvise' <<<"$gunyah_output"
! grep -Fq 'qualification=READY' <<<"$gunyah_output"
! grep -Fq 'cpu=hardware-virtualized' <<<"$gunyah_output"

# A controlled Snapdragon build that declares AVF VM support is EXPERIMENTAL, never READY.
set +e
declared_output="$(FAKE_HOST=snapdragon FAKE_VM_SUPPORTED=1 FAKE_HV_VERSION=gunyah PATH="$tmp_dir:$PATH" "$qualify" --adb fake-serial 2>&1)"
declared_status=$?
set -e
test "$declared_status" -eq 4
grep -Fq 'avf_gunyah_path=declared' <<<"$declared_output"
grep -Fq 'qualification=EXPERIMENTAL' <<<"$declared_output"
grep -Fq 'reason=Gunyah AVF path declared' <<<"$declared_output"
! grep -Fq 'qualification=READY' <<<"$declared_output"

# The KVM Pixel fixture must report its hypervisor as kvm and stay EXPERIMENTAL.
grep -Fq 'avf_gunyah_path=not-applicable' <<<"$output"
grep -Fq 'cpu_freq_domains=0,1,2,3;4,5;6,7' <<<"$output"
grep -Fq 'hypervisor=kvm' <<<"$output"
grep -Fq 'gunyah_device=no' <<<"$output"

echo "qualification adb probe fixture: PASS"
