#!/usr/bin/env bash
# Cross-build QEMU (TCG + rutabaga/gfxstream) for aarch64 Android, for the emulated engine.
#
# This produces the executable the emulated engine ships as
# `android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so`. It is a PIE named `lib*.so`
# so the Android packager extracts it into the app's nativeLibraryDir, which is the only place an
# untrusted app may execute from (untrusted_app* W^X for app data).
#
# Usage:
#   emu/qemu/build-android.sh --check [--require-pins] [--downloads-dir DIR]
#   emu/qemu/build-android.sh --build [--downloads-dir DIR] [--jobs N] [--output PATH]
#
# --check never touches the network and never needs the NDK; it is what `scripts/verify.sh` runs.
# Nothing here is claimed to have been exercised: the first real build is Gate 2, and its binary
# size, `-device help` output and NDK provenance belong in a qualification report.

set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
qemu_dir="$repo_root/emu/qemu"
versions="$qemu_dir/VERSIONS.toml"
downloads_dir="$qemu_dir/downloads"
output="$repo_root/android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so"
mode=""
require_pins="no"
jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"

usage() {
  sed -n '3,18p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) mode="check" ;;
    --build) mode="build" ;;
    --require-pins) require_pins="yes" ;;
    --downloads-dir) downloads_dir="${2:?--downloads-dir needs a directory}"; shift ;;
    --jobs) jobs="${2:?--jobs needs a number}"; shift ;;
    --output) output="${2:?--output needs a path}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 64 ;;
  esac
  shift
done

if [[ -z "$mode" ]]; then
  usage >&2
  exit 64
fi

if [[ ! -f "$versions" ]]; then
  echo "missing pins file: $versions" >&2
  exit 2
fi

# Prints `<section> <key> <value>` for every string entry, so the shell can verify archives and
# the NDK without a TOML parser of its own.
pins() {
  python3 - "$versions" <<'PY'
import sys, tomllib

with open(sys.argv[1], "rb") as handle:
    data = tomllib.load(handle)
for section, values in data.items():
    if not isinstance(values, dict):
        continue
    for key, value in values.items():
        if isinstance(value, (str, int)):
            print(f"{section}\t{key}\t{value}")
PY
}

pin() {
  local section="$1" key="$2"
  pins | awk -F '\t' -v section="$section" -v key="$key" '$1 == section && $2 == key { print $3 }'
}

check() {
  local version
  version="$(pin qemu version)"
  echo "emulated engine QEMU pins"
  echo "  qemu        $(pin qemu version)   $(pin qemu url)"
  echo "  rutabaga    $(pin rutabaga_gfx version) (must equal Cargo.lock)"
  echo "  gfxstream   $(pin gfxstream revision)"
  echo "  glib        $(pin glib version)"
  echo "  pixman      $(pin pixman version)"
  echo "  libslirp    $(pin libslirp version)"
  echo "  libfdt      $(pin libfdt version)"
  echo "  zlib        $(pin zlib version)"
  echo "  libffi      $(pin libffi version)"
  echo "  pcre2       $(pin pcre2 version)"
  echo "  ndk         $(pin ndk version) (minSdk $(pin ndk min_sdk), target $(pin ndk target))"

  local locked
  locked="$(grep -A1 '^name = "rutabaga_gfx"' "$repo_root/Cargo.lock" | awk -F'"' '/version/ { print $2 }')"
  if [[ -n "$locked" && "$locked" != "$(pin rutabaga_gfx version)" ]]; then
    echo "rutabaga_gfx pin $(pin rutabaga_gfx version) does not match Cargo.lock ($locked)" >&2
    return 1
  fi

  if [[ "$require_pins" == "yes" ]]; then
    local missing=0 section key value
    while IFS=$'\t' read -r section key value; do
      if [[ "$key" == "sha256" && -z "$value" ]]; then
        echo "unpinned source hash: $section.sha256" >&2
        missing=1
      fi
    done < <(pins)
    if [[ "$missing" != 0 ]]; then
      echo "refusing to build from sources with unrecorded hashes (use the pins in VERSIONS.toml)" >&2
      return 1
    fi
  fi

  local checked=0 failed=0 archive recorded actual
  if [[ -d "$downloads_dir" ]]; then
    while IFS= read -r archive; do
      # The archive belongs to the section whose `url` ends in its file name; compare against
      # that section's `sha256`.
      recorded="$(pins | awk -F '\t' -v name="$(basename "$archive")" '
        $2 == "url" { n = split($3, parts, "/"); if (parts[n] == name) owner = $1 }
        $2 == "sha256" { hash[$1] = $3 }
        END { if (owner != "") print hash[owner] }')"
      if [[ -z "$recorded" ]]; then
        echo "  present but unrecorded: $(basename "$archive")" >&2
        failed=1
        continue
      fi
      actual="$(shasum -a 256 "$archive" | awk '{ print $1 }')"
      checked=$((checked + 1))
      if [[ "$actual" != "$recorded" ]]; then
        echo "  sha256 mismatch: $(basename "$archive")" >&2
        echo "    recorded $recorded" >&2
        echo "    actual   $actual" >&2
        failed=1
      fi
    done < <(find "$downloads_dir" -maxdepth 1 -type f \( -name '*.tar.*' -o -name '*.zip' \) | sort)
  fi
  echo "  archives verified: $checked (in ${downloads_dir#"$repo_root"/})"
  return "$failed"
}

build() {
  local ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
  if [[ -z "$ndk" ]]; then
    echo "set ANDROID_NDK_HOME to an NDK r28 or newer" >&2
    exit 2
  fi
  local toolchain="$ndk/toolchains/llvm/prebuilt"
  local host_tag
  host_tag="$(ls "$toolchain" 2>/dev/null | head -n 1)"
  if [[ -z "$host_tag" ]]; then
    echo "no LLVM toolchain under $toolchain" >&2
    exit 2
  fi
  export PATH="$toolchain/$host_tag/bin:$PATH"
  export CC="aarch64-linux-android$(pin ndk min_sdk)-clang"
  export CXX="${CC}++"
  export AR=llvm-ar
  export STRIP=llvm-strip

  check

  local build_dir="$qemu_dir/build-android"
  local deps_dir="$build_dir/deps"
  mkdir -p "$deps_dir" "$(dirname "$output")"
  echo "build directory: $build_dir"

  # Dependencies are built in this order because each later one needs the earlier headers.
  local deps=(zlib libffi pcre2 glib pixman libslirp libfdt)
  local dep
  for dep in "${deps[@]}"; do
    echo "== dependency: $dep $(pin "$dep" version)"
    echo "   unpack $(pin "$dep" url) into $build_dir/src/$dep, configure with the android cross file,"
    echo "   then install into $deps_dir. This step is intentionally not automated until the first"
    echo "   Gate 2 build records the exact recipe that worked."
    if [[ ! -f "$downloads_dir/$(basename "$(pin "$dep" url)")" ]]; then
      echo "   missing archive: $(basename "$(pin "$dep" url)")" >&2
      exit 2
    fi
  done

  echo "== gfxstream ${pins:+$(pin gfxstream revision)} and rutabaga_gfx_ffi $(pin rutabaga_gfx version)"
  echo "   build rutabaga_gfx_ffi from the pinned crate revision with the gfxstream feature, against"
  echo "   the Android Vulkan loader, exactly as crates/carbon-renderer's native-gfxstream path does."

  echo "== QEMU $(pin qemu version)"
  echo "   meson setup $build_dir/qemu \\"
  echo "     --cross-file $qemu_dir/cross/aarch64-android.ini \\"
  echo "     --target-list=aarch64-softmmu --enable-tcg --enable-rutabaga-gfx --enable-slirp \\"
  echo "     --enable-fdt=system --disable-kvm --disable-xen --disable-docs --disable-tools \\"
  echo "     --disable-guest-agent --disable-sdl --disable-gtk --disable-vnc --disable-spice \\"
  echo "     --disable-opengl --disable-virglrenderer --disable-libusb --disable-usb-redir \\"
  echo "     --disable-linux-io-uring --disable-curl --disable-capstone"
  echo "   ninja -C $build_dir/qemu -j$jobs"
  echo "   strip and copy qemu-system-aarch64 to $output"

  echo >&2
  echo "This recipe stops here on purpose: the dependency and QEMU steps above have not been" >&2
  echo "exercised in this repository yet (Gate 2). Record the working commands, the sha256 of every" >&2
  echo "archive and the binary's size in qualification/emu/reports/ before claiming a build." >&2
  exit 3
}

case "$mode" in
  check) check ;;
  build) build ;;
esac
