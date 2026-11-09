#!/usr/bin/env bash
# Builds the Gate 3 probe's native pieces with the pinned NDK and stages them, together with the
# Gate 2 QEMU binary, into src/main/jniLibs/arm64-v8a (git-ignored). Executables are named lib*.so
# so the packager extracts them into nativeLibraryDir, which is exactly what V7 tests.
#
# usage: ANDROID_NDK_HOME=… qualification/emu/probe-app/build-native.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"
ndk="${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK pinned in emu/qemu/VERSIONS.toml}"
bin="$(ls -d "$ndk"/toolchains/llvm/prebuilt/*/bin | head -n 1)"
cc="$bin/aarch64-linux-android30-clang"
out="$here/src/main/jniLibs/arm64-v8a"
qemu="$repo_root/android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so"

if [[ ! -f "$qemu" ]]; then
  echo "missing $qemu; run emu/qemu/build-android.sh --build first" >&2
  exit 2
fi

mkdir -p "$out"
cflags=(-O2 -Wall -Wextra -Wno-unused-parameter -fPIC -Wl,-z,max-page-size=16384)
"$cc" "${cflags[@]}" -shared -o "$out/libemuprobe_jni.so" "$here/native/probe_jni.c" -landroid -lnativewindow -llog
"$cc" "${cflags[@]}" -fPIE -pie -o "$out/libemuprobe_child.so" "$here/native/probe_child.c" -lvulkan -lnativewindow
cp "$qemu" "$out/libqemu_system_aarch64.so"
"$bin/llvm-strip" "$out/libemuprobe_jni.so" "$out/libemuprobe_child.so"
ls -l "$out"
