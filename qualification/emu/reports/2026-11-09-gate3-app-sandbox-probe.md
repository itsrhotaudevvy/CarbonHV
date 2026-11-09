# Gate 3: on-device feasibility in the app sandbox (`kunzite`)

- **Date:** 2026-11-09
- **Plan:** `docs/superpowers/plans/2026-11-08-qemu-emulated-engine.md`, Phase 3
- **Gate:** **G3 PASSED except P5.** P1, P2 and P4 pass. P5 ("2 GiB guest viable") was **not run**
  as the plan describes it, because allocating and touching 2.5 GiB on the owner's daily phone would make the
  low-memory killer evict their apps. It needs a separate yes. P3, P6, P7, P8 and P9 are recorded below.
- **Consent:** the owner said "you can install". The probe was installed, run once, and **uninstalled**
  (`adb uninstall dev.carbonhv.emuprobe` → `Success`, 0 packages left).

## How it ran

| Item | Value |
| --- | --- |
| Device | Redmi `kunzite`, `Redmi/kunzite/kunzite:16/BP2A.250605.031.A3/OS3.0.304.0.WPQCNXM:user/release-keys`, stock and unrooted, SELinux enforcing |
| Probe | `qualification/emu/probe-app` (`dev.carbonhv.emuprobe`, debug, `targetSdk 36`, `minSdk 30`, `extractNativeLibs=true`) carrying the Gate 2 QEMU (`sha256 d98e38f3…ffd5d`) |
| Process | **`u:r:untrusted_app:s0:c74,c258,c512,c768`**, uid 10586 (the real app domain) |
| Entry point | `adb shell am start-foreground-service -n dev.carbonhv.emuprobe/.ProbeService`: a `specialUse` foreground service, the process role the engine's `:vm` service will have. The owner was using another app, so no Activity was put in front of them. Cpuset: `/foreground` |
| Duration | about 30 s for every check |
| Raw output | `reports/data/2026-11-09-gate3-kunzite-fgs.json` |

The first attempt (as an Activity) crashed the probe in P6. That crash is itself the P6 result (see below). The probe was then
changed to save its report after every step and to run the `io_uring` check in a child.

## Results

| Check | Result | Plan claim |
| --- | --- | --- |
| **P1 exec** | `ProcessBuilder(nativeLibraryDir/libqemu_system_aarch64.so --version)` → `QEMU emulator version 11.1.3`, exit 0 | **V7 ✅** |
| **P2 JIT, native** | anonymous RWX `mmap`: **ok**. RW→RX `mprotect`: **ok**. memfd RW+RX double mapping: **ok** (stub returned 42 in each case) | **V8 ✅**: all three allowed for `targetSdk 36` on this build |
| **P2 JIT, QEMU** | TCG on an empty `virt` machine, `-smp 2`, with `split-wx=on` and `split-wx=off`: **both still running at the 5 s deadline**, no output | V3 ✅ in the app domain |
| P3 placement | `sched_setaffinity` on own thread to CPUs 4–7: **ok** (read back `[4,5,6,7]`). `sched_setattr` with `uclamp.min=512`: **EPERM**. `/dev/cpuset/top-app/cpus`: EACCES (unreadable for apps) | **V10: affinity ✅, uclamp ❌** |
| **P4 gfxstream in QEMU** | `virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,…`: `Selecting Vulkan device: Adreno (TM) 710, Version: 1.3.284`, `externalMemoryMode: AndroidAHB`, **`Gfxstream initialized successfully!`**, still running at the 8 s deadline | V13 ✅ in the app domain |
| **P4 zero-copy handoff** | A separately exec'd child creates a Vulkan device on the Adreno, clears an AHB-exportable `R8G8B8A8` image on the GPU, exports it with `vkGetMemoryAndroidHardwareBufferANDROID` (256×256, format 1, usage `0x300`) and sends it with `AHardwareBuffer_sendHandleToUnixSocket` over an abstract `SOCK_SEQPACKET` socket. The app receives it (`AHardwareBuffer_recvHandleFromUnixSocket` → `HardwareBuffer`), wraps it with `Bitmap.wrapHardwareBuffer`, and reads the centre pixel: **`0xFF3366CC`, exactly the GPU clear colour** | Phase 6's transport is de-risked |
| P5 memory | `MemTotal` 7,568,976 kB. **`MemAvailable` 2,951,360 kB (≈2882 MiB)**. `SwapFree` 6,126,728 kB. AM threshold 216 MiB, `lowMemory=false`. The allocate-and-touch test was **not run** | — |
| P6 O_DIRECT | 4 KiB aligned write + read on app storage: **ok, data matched** | `cache=none` usable |
| P6 io_uring | `io_uring_setup` in a child: **exit 159 = killed by SIGSYS**. In the first run, inside the app process, it killed the probe: `signal 31 (SIGSYS), code 1 (SYS_SECCOMP) … disallowed arm64 system call 425` | **V9 ✅, and stricter than assumed: a kill, not EPERM** |
| P7 isolated process | `u:r:isolated_app:s0:c512,c768`: exec QEMU **ok**, memfd JIT **ok**, `socket(AF_INET)` **EACCES**, `open(/dev/kgsl-3d0)` **EACCES**, gfxstream: `Failed to create Vulkan instance. Error VK_ERROR_INITIALIZATION_FAILED` | **V11 ✅**: an isolated QEMU can run TCG but not the GPU |
| P7 app process (comparison) | `open(/dev/kgsl-3d0)`: **opened**. `socket(AF_INET)`: EPERM, because the probe does not request `android.permission.INTERNET` | — |
| P8 `-sandbox on` | `-sandbox support is not enabled in this QEMU binary` (Gate 2 builds without libseccomp) | recorded |
| P9 thermal | `currentThermalStatus` 0 (none), `getThermalHeadroom(10)` 0.124. No load test | baseline only |

## What changes because of this

1. **`kunzite.json` fixture is measured now.** `w_xor_x`, `o_direct`, Vulkan AHB export and affinity
   are confirmed. **`uclamp_allowed` is now `false`.** `carbonctl emu plan` reports `uclamp=off`.
   The plan's §5.1 uclamp boost lever is unavailable to apps on this phone. Affinity to the A78s works.
2. **A blocked syscall kills the emulator.** The app seccomp filter answers at least `io_uring_setup`
   with SIGSYS. The QEMU build already excludes io_uring. The `:vm` supervisor (Phase 9) must report
   exit 159 / SIGSYS as "blocked system call" with the tombstone's syscall number, not as a generic crash.
3. **Networking needs `android.permission.INTERNET`** in the app that runs QEMU, because slirp opens
   host sockets. Without it `socket(AF_INET)` is EPERM. CarbonHV's app manifest does not request it
   today. Phase 7 must add it, or keep the emulated engine "Offline".
4. **Phase 14 split confirmed.** `isolated_app` can exec QEMU and JIT but can open neither KGSL nor
   inet sockets. The plan's hardening split (QEMU isolated, GPU through vhost-user-gpu in `:vm`) is the
   only way to keep hardware graphics in an isolated emulator.
5. **Memory:** 2882 MiB was free at run time, just above the Balanced requirement (2048 + 768 =
   2816 MiB). The fixture keeps the lower 2026-11-04 reading (2750). Admission uses live values.

## What this does NOT prove

- **No guest booted.** No frame was produced by a guest, and gfxstream initialized with no guest driving it.
- **No sync fence crossed processes.** The child waited on a `VkFence` before sending. Passing a
  `sync_file` is still Phase 4/6 work.
- **Placement in the foreground app cpuset (top-app) was not measured.** The run used a foreground service (`/foreground` cpuset).
- **P5 viability** (a 2 GiB guest resident under real pressure) and the **P9 load test** were not run.
- One device and one run. These results are not a support claim for other phones.

## Reproduce

```bash
emu/qemu/build-android.sh --build                                  # Gate 2 binary
ANDROID_NDK_HOME=… qualification/emu/probe-app/build-native.sh     # stages jniLibs
./gradlew :qualification:emu:probe-app:assembleDebug
adb install qualification/emu/probe-app/build/outputs/apk/debug/probe-app-debug.apk
adb shell am start-foreground-service -n dev.carbonhv.emuprobe/.ProbeService
adb logcat -s CarbonEmuProbe:I            # wait for REPORT-COMPLETE (~30 s)
adb shell run-as dev.carbonhv.emuprobe cat files/report.json
adb uninstall dev.carbonhv.emuprobe
```
