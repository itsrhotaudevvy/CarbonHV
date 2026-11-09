#!/usr/bin/env bash
# Boots one guest inside the emulated engine on the attached phone and brings its first frames
# back to the build host, unrooted and without installing an APK.
#
#   qualification/emu/device-frame-probe/run-on-phone.sh \
#       --kernel ~/carbonhv-cache/alpine/vmlinuz-virt \
#       --initrd ~/carbonhv-cache/alpine/initramfs-virt \
#       [--serial 8b227126] [--memory 1024] [--frames 6] [--seconds 240]
#
# The kernel and initramfs are arguments rather than downloads: nothing here reaches the network.
# Output goes to qualification/emu/reports/data/<date>-frame-probe/ so the frames and the serial
# log are reviewable artifacts next to the write-up.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"
ndk="${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK pinned in emu/qemu/VERSIONS.toml}"

serial="8b227126"
kernel=""
initrd=""
memory=1024
frames=6
seconds=240

while [[ $# -gt 0 ]]; do
  case "$1" in
    --serial) serial="$2"; shift 2 ;;
    --kernel) kernel="$2"; shift 2 ;;
    --initrd) initrd="$2"; shift 2 ;;
    --memory) memory="$2"; shift 2 ;;
    --frames) frames="$2"; shift 2 ;;
    --seconds) seconds="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done

[[ -n "$kernel" && -f "$kernel" ]] || { echo "--kernel must name an existing kernel" >&2; exit 64; }
[[ -n "$initrd" && -f "$initrd" ]] || { echo "--initrd must name an existing initramfs" >&2; exit 64; }

# bash 3.2 on the build host treats an empty array expansion under `set -u` as an error, so the
# serial is always present rather than optionally added.
adb() { command adb -s "$serial" "$@"; }

[[ "$(adb get-state 2>/dev/null || true)" == "device" ]] || { echo "no device attached" >&2; exit 69; }
echo "device: $(adb shell getprop ro.product.device | tr -d '\r') ($(adb shell getprop ro.build.version.release | tr -d '\r'))"

"$here/build.sh" >/dev/null

qemu_binary="$repo_root/android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so"
[[ -f "$qemu_binary" ]] || { echo "missing $qemu_binary; run emu/qemu/build-android.sh --build" >&2; exit 66; }

remote=/data/local/tmp/carbon-emu
stamp="$(date +%Y%m%d-%H%M%S)"
out="$repo_root/qualification/emu/reports/data/$stamp-frame-probe"
mkdir -p "$out"

echo "pushing emulator + probe + ${kernel##*/} + ${initrd##*/}"
adb shell "rm -rf $remote && mkdir -p $remote/frames" >/dev/null
adb push -q "$qemu_binary" "$remote/libqemu_system_aarch64.so"
adb push -q "$here/build/carbon-frame-probe" "$remote/carbon-frame-probe"
adb push -q "$kernel" "$remote/kernel"
adb push -q "$initrd" "$remote/initramfs"
adb shell "chmod 755 $remote/carbon-frame-probe $remote/libqemu_system_aarch64.so"

# The guest writes its console to both the framebuffer (tty0, which is what produces frames) and
# the serial port, so a failed graphics path still leaves a readable boot log.
cmd="cd $remote && timeout -s TERM $seconds ./carbon-frame-probe frames --max-frames $frames -- \
./libqemu_system_aarch64.so \
-M virt,gic-version=3,highmem=on -accel tcg,thread=multi -cpu max -smp 2 -m $memory \
-nodefaults -no-user-config -no-reboot \
-device virtio-gpu-pci -device virtio-rng-pci \
-kernel kernel -initrd initramfs \
-append 'console=ttyAMA0 console=tty0 loglevel=7 modules=virtio-gpu' \
-serial file:$remote/serial.log"
echo "running: $cmd"
set +e
adb shell "$cmd" 2>&1 | tee "$out/probe.log"
status=${PIPESTATUS[0]}
set -e

adb shell "pkill -f libqemu_system_aarch64.so" >/dev/null 2>&1 || true
adb pull -q "$remote/serial.log" "$out/serial.log" 2>/dev/null || echo "no serial log" >&2
for file in $(adb shell "ls $remote/frames" | tr -d '\r'); do
  adb pull -q "$remote/frames/$file" "$out/$file"
done

if compgen -G "$out/frame-*.rgba" >/dev/null; then
  python3 "$here/rgba-to-png.py" "$out"/frame-*.rgba
fi

echo
echo "probe exit=$status, artifacts in $out"
[[ "$status" == 0 ]] || exit "$status"
