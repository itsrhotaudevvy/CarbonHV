#!/usr/bin/env bash
# Cross-builds the on-device frame probe for aarch64 Android with the pinned NDK.
#
# The probe's wire format is the same ui/android-proto.h the QEMU backend ships, extracted from
# emu/qemu/patches/qemu/0003-ui-add-android-ahb-backend.patch here rather than copied, so a
# protocol change cannot leave the probe reading stale bytes.
#
# usage: ANDROID_NDK_HOME=… qualification/emu/device-frame-probe/build.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"
patch_file="$repo_root/emu/qemu/patches/qemu/0003-ui-add-android-ahb-backend.patch"
ndk="${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK pinned in emu/qemu/VERSIONS.toml}"
bin="$(ls -d "$ndk"/toolchains/llvm/prebuilt/*/bin | head -n 1)"
cc="$bin/aarch64-linux-android30-clang"
out="$here/build"

[[ -f "$patch_file" ]] || { echo "missing $patch_file" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
(cd "$work" && git apply --include='*android-proto.h' "$patch_file")
[[ -f "$work/ui/android-proto.h" ]] || { echo "patch no longer delivers ui/android-proto.h" >&2; exit 2; }

mkdir -p "$out"
"$cc" -O2 -Wall -Wextra -Werror -Wno-unused-parameter -fPIE -pie \
  -Wl,-z,max-page-size=16384 \
  -I"$work" -o "$out/carbon-frame-probe" "$here/probe.c" -landroid -llog
"$bin/llvm-strip" "$out/carbon-frame-probe"
ls -l "$out/carbon-frame-probe"
