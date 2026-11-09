# Gate 2: QEMU + rutabaga/gfxstream cross-built for Android, smoke-tested on `kunzite`

- **Date:** 2026-11-09
- **Plan:** `docs/superpowers/plans/2026-11-08-qemu-emulated-engine.md`, Phase 2 (and the first Phase 6 check)
- **Gate:** **G2 PASSED.** One deviation: the plan says "on an arm64 AVD". The owner said to
  "use the connected phone instead", so the smoke test ran on the phone. No AVD system image
  is installed on this Mac.

## What was built

| Item | Value |
| --- | --- |
| Builder | macOS, Apple Silicon (`Darwin arm64`); meson 1.12.1, ninja 1.13.2, rustup stable 1.98.1 |
| NDK | **28.2.13676358** (r28c). The pin moved from `28.0.13004108`, which is not installed here |
| Target | `aarch64-linux-android30` (`minSdk 30`), PIE, 16 KiB max page size |
| Recipe | `emu/qemu/build-android.sh --build`, from a clean tree: **3 min 25 s** wall-clock |
| Output | `android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so` (git-ignored) |
| sha256 | `d98e38f35fe896b812d0e396184eca7caecd3df4a229510f0f119c19c90ffd5d` |
| Size (stripped) | **43,808,392 bytes** including gfxstream + rutabaga. The TCG-only build was 35,566,168 bytes, so QEMU itself is under the plan's 40 MiB target |
| Runtime libraries | `libc`, `libm`, `libdl`, `liblog`, `libnativewindow`, all Android system libraries. glib, pixman, slirp, libfdt, zlib, gfxstream, rutabaga and libc++ are linked statically |
| Sources | All 10 archives are pinned by sha256 in `emu/qemu/VERSIONS.toml`. `--check --require-pins` passes |

QEMU configure: `--without-default-features --enable-tcg --enable-slirp --enable-fdt=system
--enable-pixman --enable-rutabaga-gfx --enable-vhost-user`. Not built: KVM, Xen, HVF, SDL, GTK,
VNC, SPICE, OpenGL, virglrenderer, USB, io_uring, curl, capstone, seccomp, tools, docs.

### Patches (each has a header explaining it; none are submitted upstream)

| Patch | Why |
| --- | --- |
| `patches/qemu/0001-android-memfd-shm.patch` | bionic has no `shm_open`/`librt`. On Android `qemu_shm_alloc()` uses an anonymous memfd |
| `patches/qemu/0002-libvhost-user-bionic-uapi-headers.patch` | bionic's `_UAPI_*` virtio headers clash with QEMU's `standard-headers` copies. Needed for `--enable-vhost-user` (vhost-user-vsock) |
| `patches/gfxstream/0001-meson-android-host.patch` | gfxstream's meson build had no Android host. The new branch mirrors Soong's `target: { android }` settings |
| `patches/gfxstream/0002-android-ndk-headers-no-x11.patch` | NDK `<android/hardware_buffer.h>` instead of `<vndk/…>`. No X11/GLX on Android. EGL-on-EGL as on Soong |
| `patches/gfxstream/0003-udmabuf-require-access.patch` | `/dev/udmabuf` exists on kunzite but is root-only. gfxstream enabled udmabuf on `F_OK` and then aborted. Now `R_OK` |
| `patches/rutabaga_gfx/0001-ndk-build-without-aosp-nativewindow.patch` | AOSP-only `nativewindow` crate. AHB blob export is gated behind a Soong-only cfg |
| `patches/rutabaga_gfx/Cargo.lock` | Locks the FFI's crate graph (`cargo build --locked`) |

The C FFI (`rutabaga_gfx_ffi`) is not in the crates.io `rutabaga_gfx` 0.1.80 crate. It is built
from git tag `v0.1.80` (`d1b06416`), the commit the crate's `.cargo_vcs_info.json` names.

## Smoke test on the phone

**Device:** Redmi `kunzite`, `Redmi/kunzite/kunzite:16/BP2A.250605.031.A3/OS3.0.304.0.WPQCNXM:user/release-keys`,
kernel `6.6.118-android15-8`, SELinux `Enforcing`. Stock and unrooted.

**How:** `adb push` to `/data/local/tmp/carbonhv-emu/`, run as **`uid=2000(shell)`,
`u:r:shell:s0`**. No APK was installed, no UI was touched and no settings were changed. The
directory is removed after testing.

| Check | Result |
| --- | --- |
| `--version` | `QEMU emulator version 11.1.3` |
| `-machine help` | `virt` (alias of `virt-11.1`) |
| `-accel help` | `tcg` |
| `-device help` | `virtio-gpu-rutabaga-pci` (alias `virtio-gpu-rutabaga`), `virtio-multitouch-pci`, `virtio-sound-pci`, `vhost-user-vsock-pci`, `vhost-user-vsock-pci-non-transitional` |
| TCG runs, `split-wx=on` | `-accel tcg,thread=multi,tb-size=64,split-wx=on`, `-cpu max,pauth-impdef=on,sve=off,sme=off`, `-smp 2`: ran until the 4 s timeout, no error |
| TCG runs, `split-wx=off` | same, ran until the timeout, no error |
| **gfxstream on Adreno** | `-device virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,cross-domain=off,hostmem=256M,wsi=surfaceless`. gfxstream logged `Selecting Vulkan device: Adreno (TM) 710, Version: 1.3.284`, `Using Vulkan externalMemoryMode: AndroidAHB`, `Created CompositorVk`, `Gfxstream initialized successfully!`, and QEMU ran until the timeout |
| Planned argv subset | The `carbonctl emu plan` argv **without** `-display android`, `-audiodev aaudio` (Phase 4/7 backends), vhost-user-vsock and QMP (need live socket peers), with three fd-backed raw disks (2 ro, 1 rw) via `-add-fd` + `/dev/fdset`: ran until the timeout with gfxstream initialized, after the fixes below |

Script: `argv-subset.sh`, pushed with the binary. Its content is reproduced in the
"Reproduce" section below.

### Findings that changed code

1. **`/dev/fdset` drives need exact open flags.** QEMU matches `O_ACCMODE | O_DIRECT`
   (`monitor/fds.c`). `cache=none` asks for `O_DIRECT`. A writable `-drive` defaults to
   `auto-read-only=on` and first asks for `O_RDONLY`, which failed with *"Failed to find file
   descriptor with matching flags=0x0"*. Fixed in `carbon-emu`: writable drives emit
   `auto-read-only=off`, and `EmuDisk::open_flags()` tells the `:vm` service how to `open(2)` each
   disk (`O_RDWR`/`O_RDONLY`, plus `O_DIRECT` exactly when `cache=none`).
2. **`virtio-net-pci` failed realize** looking for the PXE ROM `efi-virtio.rom`. The app ships no
   QEMU data files and the guest never PXE-boots, so the argv now emits `romfile=`.
3. **udmabuf** (patch above). Stock Android 16 on kernel 6.6 has a root-only `/dev/udmabuf`. Without the
   patch gfxstream aborts with `GFXSTREAM_FATAL("udmabuf failed to initialize")`.
4. Harmless warning: gfxstream first tries `<exe dir>/lib64/vulkan/libvulkan.so` and logs
   `VK_BASE_VERSION_1_0 check failed`, then uses the system `libvulkan.so` and selects the Adreno.

## What this does NOT prove

- **Not the app sandbox.** Everything ran as `shell` (`u:r:shell:s0`), which has different
  SELinux rules from `untrusted_app`. V7 (exec from `nativeLibraryDir`), V8 (JIT memory for
  `targetSdk 36`) and GPU access from an app process are still Gate 3 questions. The binary is ready for the
  probe APK.
- **No guest was booted.** No kernel, firmware or disk image with an OS was used. TCG executed only
  what an empty `virt` machine runs. Boot needs Phase 1 (stand-in guest) or Phase 10.
- **No frame reached the screen.** `-display android` (Phase 4) does not exist yet. gfxstream initialized
  its compositor, but nothing was rendered or presented.
- No performance, thermal or memory numbers. The runs lasted seconds.

## Reproduce

```bash
emu/qemu/build-android.sh --check --require-pins
ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/28.2.13676358 emu/qemu/build-android.sh --build
cat emu/qemu/build-android/provenance.txt

# On a device (here: the owner's phone, as the shell user)
adb push android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so /data/local/tmp/carbonhv-emu/qemu-system-aarch64
adb shell 'cd /data/local/tmp/carbonhv-emu && chmod 755 qemu-system-aarch64 &&
  ./qemu-system-aarch64 -device help | grep -E "rutabaga|multitouch|virtio-sound|vhost-user-vsock" &&
  timeout 6 ./qemu-system-aarch64 -nodefaults -no-user-config -machine virt,gic-version=3,highmem=on \
    -accel tcg,thread=multi,tb-size=64,split-wx=on -cpu max,pauth-impdef=on,sve=off,sme=off -smp 2 -m 256 \
    -object memory-backend-memfd,id=mem,size=256M,share=on -numa node,memdev=mem \
    -display none -monitor none -serial none \
    -device virtio-gpu-rutabaga-pci,gfxstream-vulkan=on,x-gfxstream-composer=on,cross-domain=off,hostmem=256M,wsi=surfaceless'
adb shell rm -rf /data/local/tmp/carbonhv-emu
```

## Plan claims updated

| ID | Before | Now |
| --- | --- | --- |
| V1 | ✅ from source | ✅ **in the built binary**, on the phone |
| V3 | ✅ from source | ✅ TCG with `split-wx=on` and `off` runs on kunzite (shell domain) |
| V13 | ⚠️ Soong evidence only | ✅ **gfxstream builds with the NDK (3 patches) and initializes Vulkan on the Adreno 710 inside the QEMU process** (shell domain; the app domain is Gate 3) |
| V7, V8, V10, V11 | unverified | unchanged. They need the Gate 3 probe APK |
