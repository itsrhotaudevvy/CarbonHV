#!/usr/bin/env bash
# Pulls the evidence of a stand-in boot started by run-on-phone.sh into
# qualification/emu/reports/data/<stamp>-android-standin/: kernel log (serial.log), the Android
# logcat stream (hvc2), the serial console transcript (hvc1), the probe log, the exact launch
# script, disk layout, kernel command line and bootconfig, and every frame as PNG.
#
#   qualification/emu/android-standin/collect.sh [--serial 8b227126] [--viewer] [--stop] [--out <dir>]
#
# --viewer collects from the qualification viewer app (run-on-phone.sh --viewer) instead: the same
# logs read out of its private storage with run-as, plus a screenshot of the phone's own screen
# (screen.png), which is the evidence that the guest is drawn there.
# --stop asks QEMU to quit over QMP after collecting. Without it the guest keeps running.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"
serial="8b227126"
stop=0
viewer=0
out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --serial) serial="$2"; shift 2 ;;
    --stop) stop=1; shift ;;
    --viewer) viewer=1; shift ;;
    --out) out="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done
adb() { command adb -s "$serial" "$@"; }
remote=/data/local/tmp/carbon-android
stage="${CARBONHV_STANDIN_STAGE:-$repo_root/target/android-standin}"
out="${out:-$repo_root/qualification/emu/reports/data/$(date +%Y%m%d-%H%M%S)-android-standin}"
mkdir -p "$out"

if [[ "$viewer" == 1 ]]; then
  app=dev.carbonhv.emuviewer
  for file in serial.log hvc0-kernel.log logcat.log console.log qemu.log qemu.args; do
    adb exec-out "run-as $app cat files/guest/$file" >"$out/$file" 2>/dev/null \
      || echo "no $file in $app" >&2
  done
  adb logcat -d -s CarbonEmuViewer >"$out/viewer-logcat.txt" || true
  adb exec-out screencap -p >"$out/screen.png"
else
  for file in serial.log hvc0-kernel.log logcat.log console.log probe.log launch.sh; do
    adb pull -q "$remote/$file" "$out/$file" 2>/dev/null || echo "no $file on the phone" >&2
  done
fi
for file in layout.txt cmdline bootconfig; do
  cp "$stage/$file" "$out/$file"
done
tr '\0' '\n' <"$out/bootconfig" | sed '/^$/d' >"$out/bootconfig.txt" && rm "$out/bootconfig"
for file in $( [[ "$viewer" == 1 ]] || adb shell "ls $remote/frames" | tr -d '\r'); do
  adb pull -q "$remote/frames/$file" "$out/$file"
done
if compgen -G "$out/frame-*.rgba" >/dev/null; then
  python3 "$repo_root/qualification/emu/device-frame-probe/rgba-to-png.py" "$out"/frame-*.rgba
  rm -f "$out"/frame-*.rgba
fi
if adb shell "pgrep -f libqemu_system_aarch64.so" >/dev/null 2>&1; then
  echo "QEMU still running on the phone"
  if [[ "$stop" == 1 ]]; then
    adb shell "printf '{\"execute\":\"qmp_capabilities\"}{\"execute\":\"quit\"}' | nc 127.0.0.1 5561" \
      >/dev/null 2>&1 || adb shell "pkill -f libqemu_system_aarch64.so" || true
    echo "asked QEMU to quit"
  fi
fi
echo "evidence in $out"
ls -la "$out"
