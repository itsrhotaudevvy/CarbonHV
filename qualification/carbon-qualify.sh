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
  printf 'soc=%s\n' "$soc"
  printf 'gpu_renderer=%s\n' "$gpu_renderer"
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
