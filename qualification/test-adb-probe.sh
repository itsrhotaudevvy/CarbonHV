#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
qualify="$repo_root/qualification/carbon-qualify.sh"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

# Fake adb answering the probe's queries for one host per SoC family. Values follow each
# family's public configuration; driver numbers and labels are fixture data, not measurements.
cat >"$tmp_dir/adb" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

host="${FAKE_HOST:-pixel}"
kvm=no gzvm=no halla=no gunyah=no qvirt=no vm_info="" vm_supported="" hv_version=""
label="" sf="" vkjson="" display="" policies="" parts="" capacities=""
case "$host" in
  pixel) # Pixel 7a, Tensor G2: KVM/pKVM + Mali
    soc=GS201 manufacturer=Google egl=mali kvm=yes vm_supported=1 hv_version=kvm.arm-protected
    sf='  Name: Mali-G710'
    [[ "${FAKE_DRIVER_VERSION:-present}" == present ]] && sf+=$'\n  Driver version: 226504704'
    display='1080x2400@90;1080x2400@60'
    policies=$'0 1 2 3\n4 5\n6 7'
    parts=$'0xd05\n0xd05\n0xd05\n0xd05\n0xd41\n0xd41\n0xd44\n0xd44'
    ;;
  tensor-g5) # Pixel 10, Tensor G5: KVM/pKVM + Imagination PowerVR
    soc="Tensor G5" manufacturer=Google egl=powervr kvm=yes vm_supported=1 hv_version=kvm.arm-protected
    sf='GLES: Imagination Technologies, PowerVR D-Series DXT-48-1536, OpenGL ES 3.2 build 24.3@6603887'
    vkjson=$'\t\t\t"deviceName" : "PowerVR D-Series DXT-48-1536",\n\t\t\t"driverVersion" : 1602400.0,'
    display='1080x2424@120;1080x2424@60'
    policies=$'0 1\n2 3 4 5 6\n7'
    parts=$'0xd80\n0xd80\n0xd87\n0xd87\n0xd87\n0xd87\n0xd87\n0xd82'
    ;;
  exynos) # Galaxy S24, Exynos 2400: Halla + Samsung Xclipse
    soc=s5e9945 manufacturer=Samsung egl=samsung halla=yes
    vm_supported="${FAKE_VM_SUPPORTED:-}" hv_version="${FAKE_HV_VERSION:-}"
    label='u:object_r:halla_device:s0'
    sf='GLES: Samsung Electronics Co., Ltd., Samsung Xclipse 940, OpenGL ES 3.2'
    vkjson=$'\t\t\t"deviceName" : "Samsung Xclipse 940",\n\t\t\t"driverVersion" : 8388608.0,'
    display='1080x2340@120;1080x2340@60'
    policies=$'0 1 2 3\n4 5 6\n7 8\n9'
    parts=$'0xd80\n0xd80\n0xd80\n0xd80\n0xd81\n0xd81\n0xd81\n0xd81\n0xd81\n0xd82'
    ;;
  mediatek) # Dimensity 9400: GenieZone + Arm Immortalis
    soc=MT6991 manufacturer=Mediatek egl=mali gzvm=yes
    vm_supported="${FAKE_VM_SUPPORTED:-}" hv_version="${FAKE_HV_VERSION:-}"
    label='u:object_r:gzvm_device:s0'
    sf='GLES: ARM, Immortalis-G925, OpenGL ES 3.2 v1.r52p0'
    vkjson=$'\t\t\t"deviceName" : "Immortalis-G925",\n\t\t\t"driverVersion" : 213909504.0,'
    display='1260x2800@120;1260x2800@60'
    policies=$'0 1 2 3\n4 5 6\n7'
    parts=$'0xd81\n0xd81\n0xd81\n0xd81\n0xd82\n0xd82\n0xd82\n0xd85'
    ;;
  snapdragon) # Redmi kunzite, Snapdragon 6 Gen 3: Gunyah + Adreno
    soc=SM6475 manufacturer=QTI egl=adreno gunyah=yes qvirt=yes vm_info='VMs are not supported.'
    vm_supported="${FAKE_VM_SUPPORTED:-}" hv_version="${FAKE_HV_VERSION:-}"
    label='u:object_r:gunyah_device:s0'
    sf='GLES: Qualcomm, Adreno (TM) 710, OpenGL ES 3.2 V@0800.70 (GIT@4e394f1848)'
    vkjson=$'\t\t\t"deviceName" : "Adreno (TM) 710",\n\t\t\t"driverVersion" : 2150760518.0,'
    display='  DisplayDeviceInfo{"Built-in Screen": 1080 x 2392, modeId 3, supportedRefreshRates [120.00001, 90.0, 60.000004], defaultModeId 1}'
    policies=$'0 1 2 3\n4 5 6 7'
    capacities=$'405\n405\n405\n405\n1024\n1024\n1024\n1024'
    parts=$'0xd05\n0xd05\n0xd05\n0xd05\n0xd41\n0xd41\n0xd41\n0xd41'
    ;;
  *) echo "unknown FAKE_HOST $host" >&2; exit 1 ;;
esac

query="${*: -1}"
case "$query" in
  *"ro.product.cpu.abi"*) printf 'arm64-v8a\n' ;;
  *"ro.soc.model"*) printf '%s\n' "$soc" ;;
  *"ro.soc.manufacturer"*) printf '%s\n' "$manufacturer" ;;
  *"ro.hardware.egl"*|*"ro.hardware.vulkan"*) printf '%s\n' "$egl" ;;
  *"ls -Z"*) [[ -z "$label" ]] || printf 'crw-rw---- 1 system system %s 10, 55 2026-09-30 /dev/x\n' "$label" ;;
  *"ro.boot.hypervisor.vm.supported"*) printf '%s\n' "$vm_supported" ;;
  *"ro.boot.hypervisor.protected_vm.supported"*) printf '\n' ;;
  *"ro.boot.hypervisor.version"*) printf '%s\n' "$hv_version" ;;
  *"cpufreq/policy"*) printf '%s\n' "$policies" ;;
  *"CPU part"*) while read -r part; do printf 'CPU part\t: %s\n' "$part"; done <<<"$parts" ;;
  *"transparent_hugepage"*) printf 'always [madvise] never\n' ;;
  *"/dev/gunyah"*) printf '%s\n' "$gunyah" ;;
  *"/dev/gzvm"*) printf '%s\n' "$gzvm" ;;
  *"/dev/halla"*) printf '%s\n' "$halla" ;;
  *"qvirt"*) [[ "$qvirt" == no ]] || printf 'Service vendor.qti.qvirt.IVirtualizationService/default: found\n' ;;
  *"cpu_capacity"*) [[ -z "$capacities" ]] || printf '%s\n' "$capacities" ;;
  *"vkjson"*) [[ -z "$vkjson" ]] || printf '%s\n' "$vkjson" ;;
  *"vm info"*) [[ -z "$vm_info" ]] || printf '%s\n' "$vm_info" ;;
  *"ro.build.fingerprint"*) printf 'google/lynx/test:user/release-keys\n' ;;
  *"uname -r"*) printf '6.1-test\n' ;;
  *"apexservice"*) exit 0 ;;
  *"apex-info-list.xml"*)
    printf '<apex-info moduleName="com.android.virt" versionCode="371899999" isActive="true"/>\n'
    ;;
  *"SurfaceFlinger"*) printf '%s\n' "$sf" ;;
  *"dumpsys display"*) printf '%s\n' "$display" ;;
  *"/apex/com.android.virt/bin/vm"*) printf 'yes\n' ;;
  *"/dev/kvm"*) printf '%s\n' "$kvm" ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$tmp_dir/adb"

probe() {
  set +e
  probe_output="$(PATH="$tmp_dir:$PATH" "$qualify" --adb fake-serial 2>&1)"
  probe_status=$?
  set -e
}

# --- Pixel 7a (Tensor G2, KVM, Mali) -------------------------------------------------------
probe
output="$probe_output"
test "$probe_status" -eq 4
grep -Fq 'soc=GS201' <<<"$output"
grep -Fq 'soc_manufacturer=Google' <<<"$output"
grep -Fq 'gpu_renderer=Mali-G710' <<<"$output"
grep -Fq 'renderer_profile=gfxstream-vulkan-mali' <<<"$output"
grep -Fq 'egl_provider=mali' <<<"$output"
grep -Fq 'vulkan_provider=mali' <<<"$output"
grep -Fq 'vulkan_driver_version=226504704' <<<"$output"
grep -Fq 'kernel=6.1-test' <<<"$output"
grep -Fq 'build_fingerprint=google/lynx/test:user/release-keys' <<<"$output"
grep -Fq 'avf_revision=com.android.virt@371899999' <<<"$output"
grep -Fq 'display_modes=1080x2400@90;1080x2400@60' <<<"$output"
grep -Fq 'hypervisor=kvm' <<<"$output"
grep -Fq 'gunyah_device=no' <<<"$output"
grep -Fq 'avf_vendor_path=not-applicable' <<<"$output"
grep -Fq 'aosp_apex_crosvm_backend=yes' <<<"$output"
grep -Fq 'cpu_freq_domains=0,1,2,3;4,5;6,7' <<<"$output"
grep -Fq 'cpu_parts=0xd05,0xd05,0xd05,0xd05,0xd41,0xd41,0xd44,0xd44' <<<"$output"
grep -Fq 'graphics=unverified' <<<"$output"
grep -Fq 'qualification=EXPERIMENTAL' <<<"$output"
! grep -Fq 'gpu.vulkan=true' <<<"$output"

FAKE_DRIVER_VERSION=missing probe
test "$probe_status" -eq 4
grep -Fq 'vulkan_driver_version=unknown' <<<"$probe_output"
! grep -Fq 'gpu.vulkan=true' <<<"$probe_output"

# --- Pixel 10 (Tensor G5, KVM, PowerVR) ----------------------------------------------------
FAKE_HOST=tensor-g5 probe
test "$probe_status" -eq 4
grep -Fq 'soc=Tensor G5' <<<"$probe_output"
grep -Fq 'gpu_renderer=PowerVR D-Series DXT-48-1536' <<<"$probe_output"
grep -Fq 'renderer_profile=gfxstream-vulkan-powervr' <<<"$probe_output"
grep -Fq 'vulkan_driver_version=1602400' <<<"$probe_output"
grep -Fq 'hypervisor=kvm' <<<"$probe_output"
grep -Fq 'cpu_freq_domains=0,1;2,3,4,5,6;7' <<<"$probe_output"
grep -Fq 'qualification=EXPERIMENTAL' <<<"$probe_output"

# --- Snapdragon 6 Gen 3 (Gunyah, Adreno) ---------------------------------------------------
FAKE_HOST=snapdragon probe
gunyah_output="$probe_output"
test "$probe_status" -eq 3
grep -Fq 'soc=SM6475' <<<"$gunyah_output"
grep -Fq 'gpu_renderer=Adreno (TM) 710' <<<"$gunyah_output"
grep -Fq 'renderer_profile=gfxstream-vulkan-adreno' <<<"$gunyah_output"
grep -Fq 'vulkan_driver_version=2150760518' <<<"$gunyah_output"
grep -Fq 'kvm_device=no' <<<"$gunyah_output"
grep -Fq 'gunyah_device=yes' <<<"$gunyah_output"
grep -Fq 'hypervisor=gunyah' <<<"$gunyah_output"
grep -Fq 'vendor_virtualization_service=yes' <<<"$gunyah_output"
grep -Fq 'avf_vm_info=VMs are not supported.' <<<"$gunyah_output"
grep -Fq 'cpu_capacities=405,405,405,405,1024,1024,1024,1024' <<<"$gunyah_output"
grep -Fq 'display_modes=1080x2392@120.00001,90.0,60.000004' <<<"$gunyah_output"
grep -Fq 'qualification=UNSUPPORTED' <<<"$gunyah_output"
grep -Fq 'reason=gunyah hypervisor present but this build does not declare AVF VM support' <<<"$gunyah_output"
grep -Fq 'grant crosvm u:object_r:gunyah_device:s0' <<<"$gunyah_output"
grep -Fq 'hypervisor_vm_supported=unset' <<<"$gunyah_output"
grep -Fq 'vendor_hypervisor_selinux_label=u:object_r:gunyah_device:s0' <<<"$gunyah_output"
grep -Fq 'avf_vendor_path=blocked:ro.boot.hypervisor.vm.supported=unset' <<<"$gunyah_output"
grep -Fq 'cpu_freq_domains=0,1,2,3;4,5,6,7' <<<"$gunyah_output"
grep -Fq 'thp_mode=madvise' <<<"$gunyah_output"
! grep -Fq 'qualification=READY' <<<"$gunyah_output"
! grep -Fq 'cpu=hardware-virtualized' <<<"$gunyah_output"

# A controlled Snapdragon build that declares AVF VM support is EXPERIMENTAL, never READY.
FAKE_HOST=snapdragon FAKE_VM_SUPPORTED=1 FAKE_HV_VERSION=gunyah probe
test "$probe_status" -eq 4
grep -Fq 'avf_vendor_path=declared' <<<"$probe_output"
grep -Fq 'qualification=EXPERIMENTAL' <<<"$probe_output"
grep -Fq 'reason=gunyah AVF path declared' <<<"$probe_output"
! grep -Fq 'qualification=READY' <<<"$probe_output"

# --- Exynos 2400 (Halla, Xclipse) ----------------------------------------------------------
FAKE_HOST=exynos probe
test "$probe_status" -eq 3
grep -Fq 'soc=s5e9945' <<<"$probe_output"
grep -Fq 'soc_manufacturer=Samsung' <<<"$probe_output"
grep -Fq 'halla_device=yes' <<<"$probe_output"
grep -Fq 'hypervisor=halla' <<<"$probe_output"
grep -Fq 'gpu_renderer=Samsung Xclipse 940' <<<"$probe_output"
grep -Fq 'renderer_profile=gfxstream-vulkan-xclipse' <<<"$probe_output"
grep -Fq 'vendor_hypervisor_selinux_label=u:object_r:halla_device:s0' <<<"$probe_output"
grep -Fq 'aosp_apex_crosvm_backend=vendor-build-required' <<<"$probe_output"
grep -Fq 'cpu_freq_domains=0,1,2,3;4,5,6;7,8;9' <<<"$probe_output"
grep -Fq 'reason=halla hypervisor present but this build does not declare AVF VM support' <<<"$probe_output"

FAKE_HOST=exynos FAKE_VM_SUPPORTED=1 probe
test "$probe_status" -eq 4
grep -Fq 'avf_vendor_path=declared' <<<"$probe_output"
! grep -Fq 'qualification=READY' <<<"$probe_output"

# --- Dimensity 9400 (GenieZone, Immortalis) ------------------------------------------------
FAKE_HOST=mediatek probe
test "$probe_status" -eq 3
grep -Fq 'soc=MT6991' <<<"$probe_output"
grep -Fq 'gzvm_device=yes' <<<"$probe_output"
grep -Fq 'hypervisor=geniezone' <<<"$probe_output"
grep -Fq 'gpu_renderer=Immortalis-G925' <<<"$probe_output"
grep -Fq 'renderer_profile=gfxstream-vulkan-immortalis' <<<"$probe_output"
grep -Fq 'aosp_apex_crosvm_backend=yes' <<<"$probe_output"
grep -Fq 'cpu_parts=0xd81,0xd81,0xd81,0xd81,0xd82,0xd82,0xd82,0xd85' <<<"$probe_output"
grep -Fq 'avf_vendor_path=blocked:ro.boot.hypervisor.vm.supported=unset' <<<"$probe_output"

FAKE_HOST=mediatek FAKE_VM_SUPPORTED=1 FAKE_HV_VERSION=geniezone probe
test "$probe_status" -eq 4
grep -Fq 'reason=geniezone AVF path declared' <<<"$probe_output"
! grep -Fq 'qualification=READY' <<<"$probe_output"

echo "qualification adb probe fixture: PASS"
