#!/usr/bin/env bash
# Boots the AOSP Cuttlefish stand-in guest (NOT GrapheneOS) inside the emulated engine on the
# attached phone, unrooted and without installing an APK, with its frames going through the
# Phase 4 `-display android` backend into the device frame probe.
#
#   qualification/emu/android-standin/run-on-phone.sh \
#       --bundle /tmp/standin/carbonhv-guest-android-standin-16644040 \
#       [--serial 8b227126] [--memory 3072] [--vcpus 6] [--frames 60] [--interval 30] \
#       [--seconds 5400] [--keep-disk] [--viewer]
#
# --viewer runs the guest inside the qualification viewer app (qualification/emu/viewer-app,
# dev.carbonhv.emuviewer) instead of as the adb shell user: QEMU is a child of an untrusted_app
# foreground service and the guest is drawn on the phone's screen. The guest is staged into the
# app's private storage with `run-as` (the viewer is a debug build), from the same pushed files.
#
# The bundle is the CI artifact from itsrhotaudevvy/carbonhv-guest-android. Nothing here reaches
# the network. QEMU runs detached on the phone (it outlives this script); follow it with
# collect.sh, which pulls serial/logcat/console logs and frames into qualification/emu/reports.
#
# The QEMU command line mirrors Cuttlefish's own QEMU launcher (base/cvd/cuttlefish/host/libs/
# vm_manager/qemu_manager.cpp) for an arm64 guest with gpu_mode=guest_swiftshader: GPU at PCI
# slot 2, one virtio-serial device with 31 ports where hvc0 = kernel log, hvc1 = Android serial
# console, hvc2 = logcat, the rest sinks; then the disk, rng, input, balloon, and three NICs in
# mobile/ethernet/wifi order. Differences, all forced by an unrooted phone: no vsock (no
# /dev/vhost-vsock), no vhost-user input, no AC97/xHCI, direct kernel boot instead of U-Boot
# (bootconfig.txt supplies what U-Boot would), and the kernel log on the PL011 (ttyAMA0) so it
# survives a failure before the virtio_console module loads.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK pinned in emu/qemu/VERSIONS.toml}"

serial="8b227126"
bundle=""
memory=3072
vcpus=6
frames=60
interval=30
seconds=5400
keep_disk=0
viewer=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --serial) serial="$2"; shift 2 ;;
    --bundle) bundle="$2"; shift 2 ;;
    --memory) memory="$2"; shift 2 ;;
    --vcpus) vcpus="$2"; shift 2 ;;
    --frames) frames="$2"; shift 2 ;;
    --interval) interval="$2"; shift 2 ;;
    --seconds) seconds="$2"; shift 2 ;;
    --keep-disk) keep_disk=1; shift ;;
    --viewer) viewer=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done
[[ -n "$bundle" && -f "$bundle/bundle.toml" ]] || { echo "--bundle must name a CI bundle dir" >&2; exit 64; }

adb() { command adb -s "$serial" "$@"; }
[[ "$(adb get-state 2>/dev/null || true)" == "device" ]] || { echo "no device attached" >&2; exit 69; }

remote=/data/local/tmp/carbon-android
stage="${CARBONHV_STANDIN_STAGE:-$repo_root/target/android-standin}"
mkdir -p "$stage"

echo "== verifying bundle (sha256 of every artifact against bundle.toml)"
cargo build -q -p carbon-cli --manifest-path "$repo_root/Cargo.toml"
carbonctl="$repo_root/target/debug/carbonctl"
"$carbonctl" bundle verify "$bundle/bundle.toml"
grep -q '^id = "android-standin"' "$bundle/bundle.toml" \
  || { echo "bundle is not the android-standin guest" >&2; exit 65; }

echo "== boot assets (carbon-emu bootimg: kernel, initrd + bootconfig, cmdline)"
art="$bundle/artifacts"
"$carbonctl" emu boot-assets --boot "$art/boot.img" --init-boot "$art/init_boot.img" \
  --vendor-boot "$art/vendor_boot.img" --bootconfig "$here/bootconfig.txt" \
  --append console=ttyAMA0 --append earlycon=pl011,mmio32,0x9000000 --out "$stage"

echo "== OS disk (Cuttlefish os_composite layout)"
for image in super userdata; do
  if [[ ! -f "$stage/$image.raw" || "$art/$image.img" -nt "$stage/$image.raw" ]]; then
    simg2img "$art/$image.img" "$stage/$image.raw"
  fi
done
python3 -I "$here/make-disk.py" --bundle "$bundle" --raw "$stage" --out "$stage"

"$repo_root/qualification/emu/device-frame-probe/build.sh" >/dev/null
qemu_binary="$repo_root/android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so"
[[ -f "$qemu_binary" ]] || { echo "missing $qemu_binary; run emu/qemu/build-android.sh --build" >&2; exit 66; }

# One argument per line, in Cuttlefish's device order. `@GUEST@` is the guest directory: the shell
# run fills in $remote, the viewer app its own files/guest. The frame reader (probe or viewer)
# adds -display android,frame-fd=3.
qemu_args=(
  -name guest=carbonhv-android-standin,debug-threads=on
  -machine virt,gic-version=3,highmem=on,usb=off
  -accel tcg,thread=multi,tb-size=512
  -cpu max,pauth-impdef=on
  -smp "$vcpus"
  -m "$memory"
  -nodefaults -no-user-config -no-reboot
  -rtc base=utc
  -msg timestamp=on
  # The shell user may not bind Unix sockets under /data/local/tmp (SELinux), so QMP is TCP.
  -qmp tcp:127.0.0.1:5561,server=on,wait=off
  -serial file:@GUEST@/serial.log
  -device virtio-gpu-pci,id=gpu0,addr=02.0,xres=720,yres=1280
  -device virtio-serial-pci-non-transitional,max_ports=31,id=virtio-serial
  -chardev file,id=hvc0,path=@GUEST@/hvc0-kernel.log
  -device virtconsole,bus=virtio-serial.0,chardev=hvc0
  -chardev socket,id=hvc1,host=127.0.0.1,port=5560,server=on,wait=off,logfile=@GUEST@/console.log
  -device virtconsole,bus=virtio-serial.0,chardev=hvc1
  -chardev file,id=hvc2,path=@GUEST@/logcat.log
  -device virtconsole,bus=virtio-serial.0,chardev=hvc2
)
for port in $(seq 3 30); do
  qemu_args+=(-chardev "null,id=hvc$port" -device "virtconsole,bus=virtio-serial.0,chardev=hvc$port")
done
qemu_args+=(
  -drive file=@GUEST@/disk.img,if=none,id=drive-virtio-disk0,format=raw,aio=threads
  -device virtio-blk-pci-non-transitional,drive=drive-virtio-disk0,id=virtio-disk0,bootindex=1
  -object rng-random,id=objrng0,filename=/dev/urandom
  -device virtio-rng-pci-non-transitional,rng=objrng0,id=rng0,max-bytes=1024,period=2000
  -device virtio-mouse-pci,disable-legacy=on
  -device virtio-keyboard-pci,disable-legacy=on
  -device virtio-balloon-pci-non-transitional,id=balloon0
  -netdev user,id=hostnet0,net=10.0.0.0/24
  -netdev user,id=hostnet1,net=10.0.1.0/24,hostfwd=tcp:127.0.0.1:6555-:5555
  -netdev user,id=hostnet2,net=10.0.2.0/24,hostfwd=tcp:127.0.0.1:7555-:5555
  -device virtio-net-pci-non-transitional,netdev=hostnet0,id=net0,mac=02:ca:4b:00:00:01,romfile=
  -device virtio-net-pci-non-transitional,netdev=hostnet1,id=net1,mac=02:ca:4b:00:00:02,romfile=
  -device virtio-net-pci-non-transitional,netdev=hostnet2,id=net2,mac=02:ca:4b:00:00:03,romfile=
  -kernel @GUEST@/kernel
  -initrd @GUEST@/initrd
  -append "$(cat "$stage/cmdline")"
)

{
  echo '#!/system/bin/sh'
  echo '# Generated by qualification/emu/android-standin/run-on-phone.sh. AOSP Cuttlefish stand-in, NOT GrapheneOS.'
  echo "cd $remote"
  echo 'rm -rf frames && mkdir frames'
  echo 'rm -f serial.log hvc0-kernel.log logcat.log console.log'
  printf 'exec timeout -s TERM %s ./carbon-frame-probe frames --max-frames %s --interval %s -- \\\n' \
    "$seconds" "$frames" "$interval"
  printf '  ./libqemu_system_aarch64.so'
  for arg in "${qemu_args[@]}"; do
    printf ' \\\n  %q' "${arg//@GUEST@/$remote}"
  done
  echo
} >"$stage/launch.sh"
printf '%s\n' "${qemu_args[@]}" >"$stage/qemu.args"

push_if_changed() {  # <local> <remote-name>
  local want have
  want="$(shasum -a 256 "$1" | cut -d' ' -f1)"
  have="$(adb shell "sha256sum $remote/$2 2>/dev/null" | cut -d' ' -f1 || true)"
  if [[ "$want" != "$have" ]]; then
    echo "pushing $2 ($(du -h "$1" | cut -f1))"
    adb push -q "$1" "$remote/$2"
  else
    echo "unchanged $2"
  fi
}

echo "== pushing to $remote"
adb shell "pkill -f libqemu_system_aarch64.so; mkdir -p $remote" >/dev/null 2>&1 || true
push_if_changed "$qemu_binary" libqemu_system_aarch64.so
push_if_changed "$repo_root/qualification/emu/device-frame-probe/build/carbon-frame-probe" carbon-frame-probe
for file in kernel initrd chunks.bin assemble.sh launch.sh qemu.args; do
  push_if_changed "$stage/$file" "$file"
done
adb shell "chmod 755 $remote/carbon-frame-probe $remote/libqemu_system_aarch64.so $remote/assemble.sh $remote/launch.sh"

if [[ "$viewer" == 1 ]]; then
  app=dev.carbonhv.emuviewer
  apk="$repo_root/qualification/emu/viewer-app/build/outputs/apk/debug/viewer-app-debug.apk"
  echo "== viewer app: build, install, stage the guest into its private storage"
  "$repo_root/qualification/emu/viewer-app/build-native.sh" >/dev/null
  (cd "$repo_root" && ./gradlew -q --no-daemon :qualification:emu:viewer-app:assembleDebug)
  adb install -r -t "$apk" >/dev/null
  adb shell "am force-stop $app"
  # `[/]` keeps pgrep from matching the `sh -c` that runs it, whose argv contains the pattern.
  adb shell "pgrep -f '[/]${remote#/}/disk.img' >/dev/null" \
    && { echo "a shell-run guest is still running; stop it first (collect.sh --stop)" >&2; exit 1; }
  guest="files/guest"
  # assemble.sh rebuilds disk.img next to itself from ./chunks.bin; point that at the pushed copy.
  stage_cmd="mkdir -p $guest && cd $guest && cp $remote/kernel $remote/initrd $remote/qemu.args . \
    && sed 's|if=chunks.bin|if=$remote/chunks.bin|' $remote/assemble.sh > assemble.sh"
  if [[ "$keep_disk" == 0 ]] || ! adb shell "run-as $app test -f $guest/disk.img"; then
    stage_cmd="$stage_cmd && sh assemble.sh"
  fi
  adb shell "run-as $app sh -c \"$stage_cmd\""
  adb shell "run-as $app sh -c 'cd $guest && rm -f serial.log hvc0-kernel.log logcat.log console.log qemu.log'"
  echo "== launching in $app (on screen)"
  adb shell "am start -W -n $app/.ViewerActivity" >/dev/null
  sleep 8
  adb shell "pidof libqemu_system_aarch64.so" >/dev/null \
    || { echo "the viewer's emulator is not running:" >&2;
         adb shell "run-as $app cat $guest/qemu.log" >&2; adb logcat -d -s CarbonEmuViewer >&2; exit 1; }
  echo "running on screen. evidence: $here/collect.sh --viewer"
  exit 0
fi

if [[ "$keep_disk" == 0 ]] || ! adb shell "test -f $remote/disk.img"; then
  echo "== assembling disk.img on the phone (fresh first boot)"
  adb shell "sh $remote/assemble.sh"
fi

echo "== launching (detached)"
# `adb shell` does not return while a child it started is alive, even one in its own session with
# every descriptor redirected. QEMU runs under setsid on the phone, so the local client is
# started in the background here and dropped after the liveness check without touching the VM.
adb shell "cd $remote && setsid sh ./launch.sh >probe.log 2>&1 </dev/null &" </dev/null >/dev/null 2>&1 &
launcher=$!
# QEMU refuses bad arguments within a second or two; the probe logs its exit status.
sleep 8
if adb shell "grep -qE 'qemu exit|killed by signal' $remote/probe.log" \
  || ! adb shell "pgrep -f libqemu_system_aarch64.so >/dev/null"; then
  echo "QEMU is not running:" >&2
  adb shell "cat $remote/probe.log" >&2
  kill "$launcher" 2>/dev/null || true
  exit 1
fi
kill "$launcher" 2>/dev/null || true
echo "running. follow with: $here/collect.sh [--serial $serial]"
echo "  kernel log:  adb shell tail -f $remote/serial.log"
echo "  logcat:      adb shell tail -f $remote/logcat.log"
echo "  guest adb:   adb forward tcp:6555 tcp:6555 && adb connect 127.0.0.1:6555"
