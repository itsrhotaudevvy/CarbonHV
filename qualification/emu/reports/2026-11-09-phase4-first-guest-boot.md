# First guest boot: Linux on the phone, frames drawn by the guest kernel

- **Date:** 2026-11-09
- **Plan:** `docs/superpowers/plans/2026-11-08-qemu-emulated-engine.md`, Phase 4 verification
- **Gate:** the part of G4 that was missing after the build is now met on hardware: **a guest boots
  inside the emulated engine on the phone and its frames reach a reader outside QEMU.** The guest
  is Linux, **not Android** — see "What this is not" below.

## What ran, and on what

| Item | Value |
| --- | --- |
| Device | Redmi `kunzite`, `25098RA98G`, Android 16, serial `8b227126` |
| Privilege | unprivileged `adb shell` (`shell` user). No root, no APK, no property or package changed |
| Emulator | `libqemu_system_aarch64.so`, sha256 `7ce45c39…`, TCG only (`-accel tcg,thread=multi -cpu max`) |
| Guest | Alpine Linux kernel `6.12.115-0-virt` + `initramfs-virt` (aarch64), no root filesystem |
| Devices | `virtio-gpu-pci`, `virtio-rng-pci`, `-M virt,gic-version=3 -m 1024 -smp 2` |
| Display | QEMU's `-display android,frame-fd=3`, readback presentation, three-buffer ring |
| Reader | `qualification/emu/device-frame-probe`, a host-side stand-in for the app's frame reader |
| Result | 640x480 placeholder frames, then `mode 1280x800`, then continuous guest frames; console text captured |

The guest's own log, from the serial port:

```
[    0.000000] Linux version 6.12.115-0-virt (buildozer@build-3-22-aarch64) …
[    5.364947] [drm] pci: virtio-gpu-pci detected at 0000:00:01.0
[    5.367654] [drm] features: -virgl +edid -resource_blob -host_visible
[    5.434944] [drm] Initialized virtio_gpu 0.1.0 for 0000:00:01.0 on minor 0
[    5.598012] Console: switching to colour frame buffer device 160x50
[    5.657184] virtio-pci 0000:00:01.0: [drm] fb0: virtio_gpudrmfb frame buffer device
[   21.077665] Mounting boot media: failed.
```

So the path is real end to end: guest kernel writes text through `virtio_gpudrmfb` → virtio-gpu
2D commands → QEMU's pixman surface → `android_capture()` → `AHardwareBuffer` → socket →
reader process → PNG. The captured frames are a black screen with grey console text, which is
exactly what the guest drew. Artifacts:
`qualification/emu/reports/data/20261109-143608-frame-probe/`.

`AHardwareBuffer_allocate` therefore works from a QEMU process running as `shell` on this phone,
which was the open question in the Phase 4 report.

## Three bugs the phone found that the build host could not

1. **The backend closed its own frame descriptor.** The probe handed QEMU fd 3 with
   `posix_spawn` file actions; `addclose(pair[0])` ran *after* the `dup2` onto the same number,
   so the child ended up with fd 3 closed, and QEMU's `-serial file:…` took that number. QEMU
   then failed with `cannot write the greeting to fd 3: Socket operation on non-socket`. The
   probe now forks and installs fd 3 itself, clearing close-on-exec explicitly, and nothing in
   the child can reuse the descriptor.
2. **Frames were only readable with `CPU_READ_NEVER`.** The ring was allocated with
   `CPU_WRITE_OFTEN | GPU_SAMPLED_IMAGE`, which is right for the app (it samples the buffer) but
   makes `AHardwareBuffer_lock(CPU_READ_…)` fail for any diagnostic reader. The usage is now
   `CPU_READ_RARELY | CPU_WRITE_OFTEN | GPU_SAMPLED_IMAGE` (`0x132`); it costs nothing on this
   hardware and is what makes screenshots possible. Golden bytes and the Rust test were updated
   with it.
3. **The frames were channel-swapped, and the reason was a macro that is always defined.**
   `qemu/compiler.h` defines `HOST_BIG_ENDIAN` as `(__BYTE_ORDER__ == __ORDER_BIG_ENDIAN__)` —
   a *value*, not a presence flag. The backend chose its pixman format with `#ifdef
   HOST_BIG_ENDIAN`, so every little-endian build silently took the big-endian branch and wrote
   `A,B,G,R` into a buffer advertised as `R,G,B,A`. On screen: a black console rendered solid
   red. Fixed with `#if HOST_BIG_ENDIAN` and a comment explaining the trap; the compiled object
   now carries `PIXMAN_a8b8g8r8` (`0x20038888`), verified by disassembling
   `libsystem.a.p/ui_android.c.o` before re-running on the phone.
4. Minor: the reader must consume one handle datagram per frame message, even for frames it does
   not keep, or the stream goes one datagram out of step and every later read fails. The probe
   now drains the handle as well as sending `RELEASE`.

## What this is not

- **Not an Android guest.** The AOSP cuttlefish arm64 image is not on this Mac and cannot be
  fetched here: `ci.android.com` and the `androidbuildinternal` v3 API both answer
  `403 Rate limit exceeded for legacy API` to a non-browser client, and no AVD system image is
  installed. Android userspace adds requirements this boot did not exercise: `bootconfig`,
  `vendor_boot`, the `super` partition, and an init that expects a `virtio-gpu` context.
- **No app involved.** `:vm`, the JNI frame reader and `GraphicsSession` belong to `android/emu`,
  which does not exist yet (Phase 9). The probe is a process, not the service.
- **Not fast yet.** TCG on the phone boots a kernel in ~6 s of guest time and drew ~900 frames.
  Frame rate, readback cost and the zero-copy comparison still belong with Phase 6.
- **`RESIZE` is still only logged**, and input is still Phase 5's problem.

## Reproduce

```bash
export ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/28.2.13676358
emu/qemu/build-android.sh --build                      # ~3.5 min, applies patch 0003
qualification/emu/device-frame-probe/run-on-phone.sh \
    --kernel  ~/carbonhv-cache/alpine/vmlinuz-virt \
    --initrd  ~/carbonhv-cache/alpine/initramfs-virt \
    --frames 10 --seconds 120                          # serial 8b227126 by default
```

The Alpine kernel and initramfs are deliberately *not* downloaded by the script; pass paths.
`/data/local/tmp/carbon-emu` on the phone is left in place between runs to keep them short.
