#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage:
  carbon-qualify.sh --profile <profile.toml> --dry-run
  carbon-qualify.sh --adb <serial>

Profile mode validates a recorded/fixture capability tuple. It does not probe hardware.
ADB mode probes CPU/AVF/KVM presence on a connected Android host and keeps graphics unverified.
EOF
}

profile=""
serial=""
dry_run=false

while (($#)); do
  case "$1" in
    --profile)
      [[ $# -ge 2 ]] || { usage >&2; exit 64; }
      profile="$2"
      shift 2
      ;;
    --adb)
      [[ $# -ge 2 ]] || { usage >&2; exit 64; }
      serial="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

profile_value() {
  local key="$1" file="$2"
  awk -F= -v wanted="$key" '
    {
      k=$1
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", k)
      if (k == wanted) {
        v=$0
        sub(/^[^=]*=/, "", v)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", v)
        if (v ~ /^".*"$/) {
          sub(/^"/, "", v)
          sub(/"$/, "", v)
        }
        print v
        found=1
        exit
      }
    }
    END { if (!found) exit 2 }
  ' "$file"
}

emit_profile() {
  local file="$1"
  [[ -f "$file" ]] || { echo "profile not found: $file" >&2; exit 66; }

  local id arch virt authorized gpu memory storage
  id="$(profile_value id "$file")" || { echo "profile missing id" >&2; exit 65; }
  arch="$(profile_value arch "$file")" || { echo "profile missing arch" >&2; exit 65; }
  virt="$(profile_value hardware_virtualization "$file")" || { echo "profile missing hardware_virtualization" >&2; exit 65; }
  authorized="$(profile_value broker_authorized "$file")" || { echo "profile missing broker_authorized" >&2; exit 65; }
  gpu="$(profile_value gpu_acceleration "$file")" || { echo "profile missing gpu_acceleration" >&2; exit 65; }
  memory="$(profile_value memory_mib "$file")" || { echo "profile missing memory_mib" >&2; exit 65; }
  storage="$(profile_value storage_mib "$file")" || { echo "profile missing storage_mib" >&2; exit 65; }

  printf 'source=profile:%s\n' "$id"
  printf 'arch=%s\n' "$arch"
  printf 'memory_mib=%s\n' "$memory"
  printf 'storage_mib=%s\n' "$storage"

  if [[ "$arch" != "arm64" ]]; then
    printf 'qualification=UNSUPPORTED\nreason=unsupported architecture\n'
    exit 3
  fi
  if [[ "$virt" != "true" ]]; then
    printf 'qualification=UNSUPPORTED\nreason=hardware virtualization unavailable\n'
    exit 3
  fi
  if [[ "$authorized" != "true" ]]; then
    printf 'qualification=UNSUPPORTED\nreason=broker authorization unavailable\n'
    exit 3
  fi
  if [[ "$gpu" != "true" ]]; then
    printf 'cpu=hardware-virtualized\ngraphics=unqualified\nqualification=EXPERIMENTAL\nreason=accelerated graphics unavailable\n'
    exit 4
  fi
  if (( memory < 4096 )); then
    printf 'qualification=UNSUPPORTED\nreason=insufficient memory for starter guests\n'
    exit 3
  fi
  if (( storage < 32768 )); then
    printf 'qualification=UNSUPPORTED\nreason=insufficient storage reserve\n'
    exit 3
  fi

  printf 'cpu=hardware-virtualized\ngraphics=accelerated\nqualification=READY\n'
}

emit_adb_probe() {
  local target="$1"
  command -v adb >/dev/null || { echo "adb not found" >&2; exit 69; }

  local abi vm_status kvm_status soc egl_provider vulkan_provider build_fingerprint kernel
  local sf_dump gpu_renderer vulkan_driver_version avf_revision apex_info avf_version display_modes
  abi="$(adb -s "$target" shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r')"
  [[ -n "$abi" ]] || { echo "unable to query Android host $target" >&2; exit 69; }
  vm_status="$(adb -s "$target" shell 'if [ -x /apex/com.android.virt/bin/vm ]; then echo yes; else echo no; fi' 2>/dev/null | tr -d '\r')"
  kvm_status="$(adb -s "$target" shell 'if [ -e /dev/kvm ]; then echo yes; else echo no; fi' 2>/dev/null | tr -d '\r')"
  soc="$(adb -s "$target" shell getprop ro.soc.model 2>/dev/null | tr -d '\r')"
  egl_provider="$(adb -s "$target" shell getprop ro.hardware.egl 2>/dev/null | tr -d '\r')"
  vulkan_provider="$(adb -s "$target" shell getprop ro.hardware.vulkan 2>/dev/null | tr -d '\r')"
  build_fingerprint="$(adb -s "$target" shell getprop ro.build.fingerprint 2>/dev/null | tr -d '\r')"
  kernel="$(adb -s "$target" shell 'uname -r' 2>/dev/null | tr -d '\r')"
  sf_dump="$(adb -s "$target" shell 'dumpsys SurfaceFlinger' 2>/dev/null | tr -d '\r')"
  gpu_renderer="$(awk -F: '/^[[:space:]]*Name:[[:space:]]*/ { sub(/^[[:space:]]+/, "", $2); print $2; exit }' <<<"$sf_dump")"
  vulkan_driver_version="$(awk -F: '/^[[:space:]]*Driver version:[[:space:]]*/ { sub(/^[[:space:]]+/, "", $2); print $2; exit }' <<<"$sf_dump")"
  apex_info="$(adb -s "$target" shell 'grep -m1 "moduleName=\"com.android.virt\"" /apex/apex-info-list.xml 2>/dev/null || true' 2>/dev/null | tr -d '\r')"
  avf_version="$(sed -n 's/.*versionCode="\([^"]*\)".*/\1/p' <<<"$apex_info")"
  if [[ -n "$avf_version" ]]; then
    avf_revision="com.android.virt@$avf_version"
  else
    avf_revision=unknown
  fi
  display_modes="$(adb -s "$target" shell 'dumpsys display 2>/dev/null | grep -m1 -E "supportedModes|mSupportedModes" || true' 2>/dev/null | tr -d '\r')"
  # A raw DisplayDeviceInfo line is unreadable; reduce it to WxH@rates when that shape is present.
  if [[ "$display_modes" =~ ([0-9]+)\ x\ ([0-9]+).*supportedRefreshRates\ \[([^]]*)\] ]]; then
    display_modes="${BASH_REMATCH[1]}x${BASH_REMATCH[2]}@${BASH_REMATCH[3]// /}"
  fi

  local gunyah_status qvirt_status avf_vm_info cpu_capacities vkjson hypervisor
  local hv_vm_supported hv_protected_vm_supported hv_version vendor_label cpu_freq_domains thp_mode
  local avf_vendor_path gzvm_status halla_status vendor_device soc_manufacturer cpu_parts
  local renderer_profile apex_backend
  gunyah_status="$(adb -s "$target" shell 'if [ -e /dev/gunyah ]; then echo yes; else echo no; fi' 2>/dev/null | tr -d '\r')"
  gzvm_status="$(adb -s "$target" shell 'if [ -e /dev/gzvm ]; then echo yes; else echo no; fi' 2>/dev/null | tr -d '\r')"
  halla_status="$(adb -s "$target" shell 'if [ -e /dev/halla ]; then echo yes; else echo no; fi' 2>/dev/null | tr -d '\r')"
  soc_manufacturer="$(adb -s "$target" shell getprop ro.soc.manufacturer 2>/dev/null | tr -d '\r')"
  # MIDR part numbers in CPU order: the broker classifies in-order efficiency cores from these.
  cpu_parts="$(adb -s "$target" shell 'grep "^CPU part" /proc/cpuinfo 2>/dev/null' 2>/dev/null | tr -d '\r' | awk -F: '{gsub(/[[:space:]]/, "", $2); print $2}' | paste -sd, -)"
  qvirt_status="$(adb -s "$target" shell 'service check vendor.qti.qvirt.IVirtualizationService/default 2>/dev/null' 2>/dev/null | tr -d '\r')"
  avf_vm_info="$(adb -s "$target" shell 'timeout 5 /apex/com.android.virt/bin/vm info </dev/null 2>&1 | head -1' 2>/dev/null | tr -d '\r' | tr '\n' ' ')"
  cpu_capacities="$(adb -s "$target" shell 'for d in /sys/devices/system/cpu/cpu[0-9]*; do cat $d/cpu_capacity 2>/dev/null; done' 2>/dev/null | tr -d '\r' | paste -sd, -)"
  vkjson="$(adb -s "$target" shell 'cmd gpu vkjson 2>/dev/null' 2>/dev/null | tr -d '\r')"
  # AVF refuses every VM unless the boot config declares support; Gunyah hosts need this for the
  # AVF path, and the /dev/gunyah label tells the controlled product which crosvm rule to add.
  hv_vm_supported="$(adb -s "$target" shell getprop ro.boot.hypervisor.vm.supported 2>/dev/null | tr -d '\r')"
  hv_protected_vm_supported="$(adb -s "$target" shell getprop ro.boot.hypervisor.protected_vm.supported 2>/dev/null | tr -d '\r')"
  hv_version="$(adb -s "$target" shell getprop ro.boot.hypervisor.version 2>/dev/null | tr -d '\r')"
  # Near-native placement mirrors host cpufreq policies into the guest; record them exactly.
  cpu_freq_domains="$(adb -s "$target" shell 'for p in /sys/devices/system/cpu/cpufreq/policy[0-9]*; do cat $p/related_cpus 2>/dev/null; done' 2>/dev/null | tr -d '\r' | sed 's/[[:space:]]*$//; s/[[:space:]][[:space:]]*/,/g' | paste -sd';' -)"
  thp_mode="$(adb -s "$target" shell 'cat /sys/kernel/mm/transparent_hugepage/enabled 2>/dev/null' 2>/dev/null | tr -d '\r' | sed -n 's/.*\[\([a-z]*\)\].*/\1/p')"

  # Adreno's SurfaceFlinger dump has no "Name:" line; fall back to the GLES identity, then Vulkan.
  if [[ -z "$gpu_renderer" ]]; then
    # The vendor field may itself contain commas ("Samsung Electronics Co., Ltd."), so take the
    # last comma-separated field before ", OpenGL ES".
    gpu_renderer="$(sed -n 's/^GLES: \(.*\), OpenGL ES.*/\1/p' <<<"$sf_dump" | head -1 | sed 's/.*, //')"
  fi
  if [[ -z "$gpu_renderer" ]]; then
    gpu_renderer="$(sed -n 's/.*"deviceName" : "\(.*\)".*/\1/p' <<<"$vkjson" | head -1)"
  fi
  if [[ -z "$vulkan_driver_version" ]]; then
    vulkan_driver_version="$(sed -n 's/.*"driverVersion" : \([0-9]*\).*/\1/p' <<<"$vkjson" | head -1)"
  fi

  # Same order as crosvm's default-hypervisor probe: kvm, geniezone, halla, gunyah.
  vendor_device=""
  if [[ "$kvm_status" == yes ]]; then
    hypervisor=kvm
  elif [[ "$gzvm_status" == yes ]]; then
    hypervisor=geniezone vendor_device=/dev/gzvm
  elif [[ "$halla_status" == yes ]]; then
    hypervisor=halla vendor_device=/dev/halla
  elif [[ "$gunyah_status" == yes ]]; then
    hypervisor=gunyah vendor_device=/dev/gunyah
  else
    hypervisor=none
  fi
  vendor_label=""
  if [[ -n "$vendor_device" ]]; then
    vendor_label="$(adb -s "$target" shell "ls -Z $vendor_device 2>/dev/null" 2>/dev/null | tr -d '\r' | awk '{for (i = 1; i <= NF; i++) if ($i ~ /^u:object_r:/) { print $i; exit }}')"
  fi
  # AOSP's com.android.virt crosvm is built with the geniezone and gunyah features, not halla.
  case "$hypervisor" in
    kvm|geniezone|gunyah) apex_backend=yes ;;
    halla) apex_backend=vendor-build-required ;;
    *) apex_backend=not-applicable ;;
  esac
  case "$gpu_renderer" in
    Mali-*) renderer_profile=gfxstream-vulkan-mali ;;
    Immortalis-*) renderer_profile=gfxstream-vulkan-immortalis ;;
    Adreno*) renderer_profile=gfxstream-vulkan-adreno ;;
    PowerVR*) renderer_profile=gfxstream-vulkan-powervr ;;
    "Samsung Xclipse"*) renderer_profile=gfxstream-vulkan-xclipse ;;
    *) renderer_profile=none ;;
  esac
  [[ -n "$gunyah_status" ]] || gunyah_status=unknown
  [[ "$qvirt_status" == *": found"* ]] && qvirt_status=yes || qvirt_status=no
  [[ -n "$cpu_capacities" ]] || cpu_capacities=unknown
  [[ -n "$avf_vm_info" ]] || avf_vm_info=unknown
  [[ -n "$hv_vm_supported" ]] || hv_vm_supported=unset
  [[ -n "$hv_protected_vm_supported" ]] || hv_protected_vm_supported=unset
  [[ -n "$hv_version" ]] || hv_version=unset
  [[ -n "$vendor_label" ]] || vendor_label=unknown
  [[ -n "$gzvm_status" ]] || gzvm_status=unknown
  [[ -n "$halla_status" ]] || halla_status=unknown
  [[ -n "$soc_manufacturer" ]] || soc_manufacturer=unknown
  [[ -n "$cpu_parts" ]] || cpu_parts=unknown
  [[ -n "$cpu_freq_domains" ]] || cpu_freq_domains=unknown
  [[ -n "$thp_mode" ]] || thp_mode=unknown
  if [[ -z "$vendor_device" ]]; then
    avf_vendor_path=not-applicable
  elif [[ "$vm_status" != yes ]]; then
    avf_vendor_path=blocked:avf-vm-tool-missing
  elif [[ "$hv_vm_supported" != 1 && "$hv_vm_supported" != true ]]; then
    avf_vendor_path="blocked:ro.boot.hypervisor.vm.supported=$hv_vm_supported"
  else
    avf_vendor_path=declared
  fi

  [[ -n "$soc" ]] || soc=unknown
  [[ -n "$egl_provider" ]] || egl_provider=unknown
  [[ -n "$vulkan_provider" ]] || vulkan_provider=unknown
  [[ -n "$build_fingerprint" ]] || build_fingerprint=unknown
  [[ -n "$kernel" ]] || kernel=unknown
  [[ -n "$gpu_renderer" ]] || gpu_renderer=unknown
  [[ -n "$vulkan_driver_version" ]] || vulkan_driver_version=unknown
  [[ -n "$display_modes" ]] || display_modes=unknown

  printf 'source=adb:%s\n' "$target"
  printf 'android_abi=%s\n' "$abi"
  printf 'avf_vm_tool=%s\n' "$vm_status"
  printf 'kvm_device=%s\n' "$kvm_status"
  printf 'gunyah_device=%s\n' "$gunyah_status"
  printf 'gzvm_device=%s\n' "$gzvm_status"
  printf 'halla_device=%s\n' "$halla_status"
  printf 'hypervisor=%s\n' "$hypervisor"
  printf 'vendor_virtualization_service=%s\n' "$qvirt_status"
  printf 'avf_vm_info=%s\n' "$avf_vm_info"
  printf 'cpu_capacities=%s\n' "$cpu_capacities"
  printf 'cpu_freq_domains=%s\n' "$cpu_freq_domains"
  printf 'thp_mode=%s\n' "$thp_mode"
  printf 'hypervisor_vm_supported=%s\n' "$hv_vm_supported"
  printf 'hypervisor_protected_vm_supported=%s\n' "$hv_protected_vm_supported"
  printf 'hypervisor_version=%s\n' "$hv_version"
  printf 'vendor_hypervisor_selinux_label=%s\n' "$vendor_label"
  printf 'aosp_apex_crosvm_backend=%s\n' "$apex_backend"
  printf 'avf_vendor_path=%s\n' "$avf_vendor_path"
  printf 'cpu_parts=%s\n' "$cpu_parts"
  printf 'soc=%s\n' "$soc"
  printf 'soc_manufacturer=%s\n' "$soc_manufacturer"
  printf 'gpu_renderer=%s\n' "$gpu_renderer"
  printf 'renderer_profile=%s\n' "$renderer_profile"
  printf 'egl_provider=%s\n' "$egl_provider"
  printf 'vulkan_provider=%s\n' "$vulkan_provider"
  printf 'vulkan_driver_version=%s\n' "$vulkan_driver_version"
  printf 'kernel=%s\n' "$kernel"
  printf 'build_fingerprint=%s\n' "$build_fingerprint"
  printf 'avf_revision=%s\n' "$avf_revision"
  printf 'display_modes=%s\n' "$display_modes"

  if [[ "$abi" != arm64-v8a* && "$abi" != arm64* ]]; then
    printf 'qualification=UNSUPPORTED\nreason=host is not ARM64\n'
    exit 3
  fi
  if [[ -n "$vendor_device" && "$avf_vendor_path" != declared ]]; then
    printf 'qualification=UNSUPPORTED\nreason=%s hypervisor present but this build does not declare AVF VM support; a controlled host build must set androidboot.hypervisor.vm.supported=1 and grant crosvm %s\n' "$hypervisor" "$vendor_label"
    exit 3
  fi
  if [[ -n "$vendor_device" ]]; then
    printf 'cpu=hardware-virtualized\ngraphics=unverified\nqualification=EXPERIMENTAL\nreason=%s AVF path declared; guest boot, gfxstream presentation and sustained qualification still required\n' "$hypervisor"
    exit 4
  fi
  if [[ "$vm_status" != "yes" || "$kvm_status" != "yes" ]]; then
    printf 'qualification=UNSUPPORTED\nreason=AVF or KVM unavailable\n'
    exit 3
  fi

  printf 'cpu=hardware-virtualized\ngraphics=unverified\nqualification=EXPERIMENTAL\nreason=physical GPU/display qualification still required\n'
  exit 4
}

if [[ -n "$profile" && -z "$serial" ]]; then
  [[ "$dry_run" == true ]] || { echo "profile mode requires --dry-run to make fixture use explicit" >&2; exit 64; }
  emit_profile "$profile"
elif [[ -n "$serial" && -z "$profile" ]]; then
  [[ "$dry_run" == false ]] || { echo "--dry-run is only valid with --profile" >&2; exit 64; }
  emit_adb_probe "$serial"
else
  usage >&2
  exit 64
fi
