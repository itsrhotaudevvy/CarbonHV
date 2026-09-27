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
  *"ro.soc.model"*) printf 'GS201\n' ;;
  *"ro.hardware.egl"*) printf 'mali\n' ;;
  *"ro.hardware.vulkan"*) printf 'mali\n' ;;
  *"ro.build.fingerprint"*) printf 'google/lynx/test:user/release-keys\n' ;;
  *"uname -r"*) printf '6.1-test\n' ;;
  *"apexservice"*) exit 0 ;;
  *"apex-info-list.xml"*)
    printf '<apex-info moduleName="com.android.virt" versionCode="371899999" isActive="true"/>\n'
    ;;
  *"SurfaceFlinger"*)
    printf '  Name: Mali-G710\n'
    if [[ "${FAKE_DRIVER_VERSION:-present}" == present ]]; then
      printf '  Driver version: 226504704\n'
    fi
    ;;
  *"dumpsys display"*) printf '1080x2400@90;1080x2400@60\n' ;;
  *"/apex/com.android.virt/bin/vm"*) printf 'yes\n' ;;
  *"/dev/kvm"*) printf 'yes\n' ;;
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

echo "qualification adb probe fixture: PASS"
