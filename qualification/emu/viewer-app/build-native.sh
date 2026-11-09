#!/usr/bin/env bash
# Builds the viewer's JNI library with the pinned NDK and stages it, with the QEMU binary, into
# src/main/jniLibs/arm64-v8a (git-ignored). The frame wire format is ui/android-proto.h taken from
# emu/qemu/patches/qemu/0003-ui-add-android-ahb-backend.patch at build time, as the device probe
# does, so the viewer cannot drift from the producer.
#
# usage: ANDROID_NDK_HOME=… qualification/emu/viewer-app/build-native.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"
ndk="${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK pinned in emu/qemu/VERSIONS.toml}"
bin="$(ls -d "$ndk"/toolchains/llvm/prebuilt/*/bin | head -n 1)"
cc="$bin/aarch64-linux-android30-clang"
out="$here/src/main/jniLibs/arm64-v8a"
qemu="$repo_root/android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so"
patch_file="$repo_root/emu/qemu/patches/qemu/0003-ui-add-android-ahb-backend.patch"

[[ -f "$qemu" ]] || { echo "missing $qemu; run emu/qemu/build-android.sh --build first" >&2; exit 2; }
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
(cd "$work" && git apply --include='*android-proto.h' "$patch_file")

mkdir -p "$out"
"$cc" -O2 -Wall -Wextra -Werror -Wno-unused-parameter -fPIC -Wl,-z,max-page-size=16384 \
  -I"$work" -shared -o "$out/libemuviewer_jni.so" "$here/native/viewer_jni.c" \
  -landroid -lnativewindow -llog
cp "$qemu" "$out/libqemu_system_aarch64.so"
"$bin/llvm-strip" "$out/libemuviewer_jni.so"
ls -l "$out"
