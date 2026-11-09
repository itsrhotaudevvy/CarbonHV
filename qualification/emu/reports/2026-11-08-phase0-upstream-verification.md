# Phase 0 — upstream verification for the emulated engine

- **Dates:** 2026-11-08 (first pass, offline), 2026-11-09 (second pass, with network and source checkouts)
- **Plan:** `docs/superpowers/plans/2026-11-08-qemu-emulated-engine.md`
- **Author:** implementation pass (agent), to be reviewed by the project owner
- **Update 2026-11-09 (later):** Gate 2 built the binary and showed V1, V3 and V13 on the phone as the adb shell user.
  See `2026-11-09-gate2-android-qemu-build.md`. V7 still needs the app sandbox (Gate 3).
- **Update 2026-11-09 (Gate 3):** V7, V8, V9, V10 and V11 were measured in the app sandbox (`2026-11-09-gate3-app-sandbox-probe.md`).
  **All four plan-changing claims (V1, V3, V7, V13) are ✅.** V12 (blocks only G1) and V14 (needs a GKI boot) stay open.
- **Original gate line (2026-11-09 morning):** **G0 NOT PASSED.** V1 and V3 are now ✅ from source. V13 has source-level evidence but no
  build. V7 still needs a device (AVD or phone) and stays blocking. G0 also requires that the owner has
  seen this report and answered the questions at the end.

This report records only what was actually run or read. Claims that need a build, an AVD or a
phone are marked **unverified**, not guessed. Nothing below is a capability claim for the product.

## Sources used

| Source | Revision | How it was obtained |
| --- | --- | --- |
| QEMU | **11.1.3** release tarball, sha256 `5a8289c7…2b84b22` (pinned in `emu/qemu/VERSIONS.toml`) | `https://download.qemu.org/qemu-11.1.3.tar.xz`, unpacked outside the repo |
| QEMU (binary probes, first pass) | Homebrew `qemu-system-aarch64` 11.1.3, built without rutabaga | local |
| Cuttlefish QEMU manager | `google/android-cuttlefish` `main` @ `a269362b` (2026-11-07), `base/cvd/cuttlefish/host/libs/vm_manager/qemu_manager.cpp`. The AOSP gitiles copy at `device/google/cuttlefish` `a1162ca7` (2025-03-26) is older: cuttlefish host code moved to GitHub | raw file fetch |
| gfxstream | `google/gfxstream` @ `ebe5b424` (2026-09-17, the `Cargo.toml` renderer pin) | GitHub tree/raw fetch |
| Source archives | glib 2.82.4, pixman 0.44.2, libslirp 4.9.1, dtc 1.7.2, zlib 1.3.1, libffi 3.4.6, pcre2 10.44 | Downloaded to `emu/qemu/downloads/` (git-ignored). glib's sha256 and pixman's sha512 match the checksums upstream publishes; the others are trust-on-first-download and recorded |

## V1–V14

| ID | Claim | Evidence | Status |
| --- | --- | --- | --- |
| V1 | `virtio-gpu-rutabaga` exists with gfxstream properties | QEMU 11.1.3 `hw/display/virtio-gpu-rutabaga.c`: properties are `gfxstream-vulkan`, `cross-domain`, **`x-gfxstream-gles`**, **`x-gfxstream-composer`**, `wayland-socket-path`, `wsi` (`surfaceless` or `headless`); `hostmem` comes from `virtio-gpu.c`. `system/qdev-monitor.c` aliases `virtio-gpu-rutabaga-pci` → `virtio-gpu-rutabaga`. `meson.build` finds it via `dependency('rutabaga_gfx_ffi')` behind `-Drutabaga_gfx`. Upstream cuttlefish's arm64 guest-ANGLE mode uses `virtio-gpu-rutabaga,gfxstream-vulkan=on,x-gfxstream-composer=on,hostmem=256M` | ✅ from source at 11.1.3 (binary still to be built in Gate 2) |
| V2 | `virtio-multitouch-*` and `virtio-sound-*` exist | Homebrew 11.1.3 `-device help` lists `virtio-multitouch-device/-pci` and `virtio-sound-device/-pci` | ✅ at 11.1.3 |
| V3 | `-accel tcg,thread=multi,tb-size=N,split-wx=on\|off` exist, and `split-wx` uses a memfd double mapping | Flags accepted by the 11.1.3 binary (first pass). Source: `accel/tcg/tcg-all.c` registers `thread`, `tb-size` and `split-wx`. `tcg/region.c` `alloc_code_gen_buffer_splitwx_memfd()` allocates the code buffer with `qemu_memfd_alloc("tcg-jit", …)` and maps an RW view and an RX view | ✅ from source and binary |
| V4 | `-cpu max` accepts `pauth-impdef=on`, `pauth=off`, `sve=off`, `sme=off`; `virt,mte=on` exists | All started cleanly on the 11.1.3 binary | ✅ at 11.1.3 |
| V5 | Cuttlefish builds an arm64 QEMU command line and boots it through a QEMU U-Boot | Upstream `qemu_manager.cpp`: arm64 uses `-bios <instance.bootloader()>` (U-Boot), `virt` with `gic-version=3` (when the host is compatible) or `2` (TCG), all virtio devices `*-pci-non-transitional`, `memory-backend-file,share=on` for vhost-user, and **`vhost-user-vsock-pci-non-transitional`** when `vhost_user_vsock` is set. The prebuilt U-Boot binary itself was not fetched | ✅ boot flow exists upstream; U-Boot binary unverified |
| V6 | Termux patches are a usable reference | Not fetched in this pass | ⚠️ unverified (reference only, not blocking) |
| V7 | Apps with `targetSdk ≥ 29` can exec from `nativeLibraryDir` with legacy packaging | Gate 3 probe (`untrusted_app`, targetSdk 36, `extractNativeLibs=true`) ran `nativeLibraryDir/libqemu_system_aarch64.so --version` → exit 0 (2026-11-09) | ✅ on kunzite |
| V8 | memfd double mapping / anonymous RWX for `targetSdk 36` | Gate 3: anonymous RWX, RW→RX `mprotect` and memfd RW+RX all allowed. QEMU TCG runs with `split-wx=on` and `off` | ✅ on kunzite (all modes) |
| V9 | `io_uring` is blocked for apps | Gate 3: `io_uring_setup` is **killed by SIGSYS** (seccomp), not refused | ✅ (stricter: a kill) |
| V10 | Apps may pin their own threads to CPUs 4–7 in the `top-app` cpuset and use `uclamp.min` | Gate 3, from a foreground service (`/foreground` cpuset): affinity to 4–7 ok. `uclamp.min` → EPERM | ✅ affinity / ❌ uclamp |
| V11 | `isolatedProcess` services cannot open the GPU device or sockets | Gate 3: `isolated_app` → KGSL EACCES, `AF_INET` EACCES, Vulkan instance fails. Exec and JIT work | ✅ |
| V12 | A public AOSP CI arm64 cuttlefish build is downloadable without login | `ci.android.com` answered **HTTP 403** to automated requests on 2026-11-09 for `aosp-main` and `aosp-android-latest-release` (`aosp_cf_arm64_only_phone*` targets). It was not bypassed. A person may still download it in a browser | ⚠️ unverified; **blocks G1 until the owner downloads the stand-in guest by hand or names another source** |
| V13 | gfxstream host builds for an Android host (bionic) | gfxstream @ `ebe5b424` `host/Android.bp`: `libgfxstream_backend_defaults` is `vendor_available: true` and has an `android:` target linking `libnativewindow`, so Soong builds the host backend for Android devices (the AVF virt-APEX route). gfxstream's `meson.build` only branches on `linux`/`darwin`/`windows`/`qnx` for `host_machine.system()`, so an NDK meson build (`system = 'android'`) is **unproven** and may need a patch or a Soong-free build recipe | ⚠️ source evidence for Android; NDK build unproven (Gate 2 decides) |
| V14 | Android 12+ GKI reads `androidboot.*` from an initrd bootconfig trailer | Implemented and unit-tested in `crates/carbon-emu/src/bootimg.rs`; not yet exercised against a GKI kernel | ⚠️ implementation tested, kernel unverified |

## Findings that change the plan or the code

1. **Wrong rutabaga property (fixed 2026-11-09).** The plan's draft and `carbon-emu` passed
   `gfxstream-gles=on`. QEMU 11.1.3 has no such property, so QEMU would have refused to start. The
   hardware path also asks for exactly `gfxstream-vulkan` + `gfxstream-composer`
   (`GFXSTREAM_CONTEXT_TYPES` in `carbon-broker`): the CVD1 guest renders GLES through ANGLE on
   Vulkan. The emulated argv now emits
   `virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,cross-domain=off,hostmem=…,wsi=surfaceless`,
   which matches the hardware path's capsets and upstream cuttlefish's guest-ANGLE mode. Golden tests are updated.
2. **QEMU's stock rutabaga scanout is CPU readback.** `rutabaga_cmd_resource_flush` calls
   `rutabaga_resource_transfer_read` into a pixman image and then `qemu_console_update_full`. There is no
   dmabuf/GL scanout hook in this device at 11.1.3. Phase 4's readback mode works with an unpatched
   device. Phase 6 zero-copy needs a QEMU or gfxstream patch, as the plan assumed (R5).
3. **Cuttlefish guests expect a fixed virtio-console layout.** Upstream reserves hvc0–hvc19: kernel
   console, serial console, logcat, keymaster, gatekeeper, bt, gnss, location, confirmationui,
   uwb, oemlock, keymint, nfc, weaver, MCU control/UART, Ti50 TPM, jcardsim keymint and sensors
   control/data. Ports without a host service are null sinks. Several of these (keymint,
   gatekeeper, oemlock, weaver, sensors) are served by host processes (`secure_env`, the sensors
   simulator) that CarbonHV does not have. `carbon-emu` currently adds one console. The
   hardware path (`direct_crosvm.rs`) does not model these ports either, so this is a **shared
   CVD1 contract gap**, not an emulated-engine issue. Phase 7 (G7) must decide the port map and which
   HALs CVD1 serves on the host, and it must be identical on both engines.
4. **Upstream cuttlefish already runs `vhost-user-vsock` under QEMU**, which supports §3.6 option 1.
5. **The memory budget in §5.5 does not admit its own default on the measured phone** (from the
   first pass, still true). It needs 2048 + 768 MiB but 2750 MiB was measured. The `low-memory` profile (1536 MiB guest) fits.
   Phase 11 decides from measured data. No silent clamp.
6. **Pin drift.** The plan says "latest stable 10.x"; the pin is 11.1.3. The NDK pin is
   `28.0.13004108`, while this Mac has `28.2.13676358` (and `27.2`). Gate 2 must either install the
   pinned NDK or move the pin, and record which.
7. **This Mac can do Gate 2.** It has NDK r28.2, meson, ninja and 443 GB free. The plan's Linux builder is
   needed for Phase 10 (the 400 GB guest build), not necessarily for the QEMU cross-build.

## Pins recorded

`emu/qemu/VERSIONS.toml` now records the sha256 of every source archive except gfxstream. GitHub
snapshot tarballs are not byte-stable, so gfxstream stays pinned by git revision until Gate 2 picks a fetch
method. `build-android.sh --check` passes and verifies 8 archives. `--check --require-pins` still
refuses to build, on `gfxstream.sha256` alone.

The archive check had a latent bug: it never ran with archives present before. It looked for the
archive's file name inside the hash value, so every archive counted as "unrecorded". It now maps
each archive to the section whose `url` ends in that file name. A tampered copy of
`zlib-1.3.1.tar.gz` is reported as a sha256 mismatch.

## Reproduce

```bash
# Source checks (outside the repo)
tar -xf emu/qemu/downloads/qemu-11.1.3.tar.xz -C <scratch>
sed -n '/virtio_gpu_rutabaga_properties/,/^};/p' <scratch>/qemu-11.1.3/hw/display/virtio-gpu-rutabaga.c
grep -n 'split-wx\|tb-size\|"thread"' <scratch>/qemu-11.1.3/accel/tcg/tcg-all.c
grep -n 'splitwx_memfd\|tcg-jit' <scratch>/qemu-11.1.3/tcg/region.c

# Binary probes (first pass)
qemu-system-aarch64 -device help | grep -iE 'rutabaga|multitouch|virtio-sound|vhost-user-vsock'
timeout 5 qemu-system-aarch64 -accel tcg,thread=multi,tb-size=256,split-wx=on \
  -machine virt,gic-version=3,highmem=on,mte=on -cpu max,pauth-impdef=on,sve=off,sme=off \
  -smp 2 -m 128 -display none -monitor none -serial none -S

# Pins
emu/qemu/build-android.sh --check
```

## G0 decision

**Not passed.** V7 remains blocking. It needs the Gate 3 probe APK on a device. V13 needs a real
NDK build of gfxstream (Gate 2). V12 needs a person to download the stand-in guest.

Questions for the project owner:

1. **Builder:** may Gate 2 (QEMU + deps + rutabaga/gfxstream cross-build with the NDK) run on this
   Mac? For Phase 10, is there a Linux x86_64 machine with ~400 GB free (a cloud VM costs money)?
2. **Stand-in guest (V12):** `ci.android.com` refuses automated downloads. Will you download the
   `aosp_cf_arm64_only_phone` `*-img-*.zip` and `cvd-host_package.tar.gz` in a browser (into
   `~/carbonhv-cache/`), or name another source?
3. **Device testing (V7):** may V7/V8 be checked first on an arm64 AVD (this Mac has `omnijay`
   and `WearTune_Round` AVDs, plus the SDK emulator), with `kunzite` only for Gate 3 after explicit consent?
4. **Licensing:** confirm the GPL-2.0 QEMU approach: a separate executable, a source offer, `NOTICE`, and
   the tarballs + patches published with every release, before any binary ships.
5. **Product framing:** confirm the wording and behaviour "Run emulated (slow)" as an explicit per-instance opt-in that is always labelled *Emulated*.
