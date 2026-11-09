# QEMU cross-build recipe for aarch64 Android (emulated engine)

This directory builds the executable the **Emulated** engine ships as
`android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so`. It is a PIE named `lib*.so`
so the Android packager extracts it into the app's `nativeLibraryDir`, the only directory an
untrusted app may execute from.

**Nothing here has produced a binary yet.** The recipe, the pins and the `--check` gate exist;
the first real build is Gate 2 and must be recorded in `qualification/emu/reports/` with the
commands that worked, the sha256 of every archive, the NDK version and the stripped binary size
(target: under 40 MiB excluding gfxstream).

## Files

| File | Purpose |
| --- | --- |
| `VERSIONS.toml` | Exact pins for QEMU, rutabaga/gfxstream, glib, pixman, libslirp, dtc/libfdt, zlib, libffi, pcre2 and the NDK |
| `build-android.sh` | `--check` (offline, no NDK) and `--build` (cross-compile) |
| `cross/aarch64-android.ini` | meson cross file for `aarch64-linux-android30` (`minSdk 30`) |
| `patches/` | Android portability and the two new backends (`ui/android.c`, `audio/aaudio.c`) |

`patches/` is empty on purpose. QEMU is GPL-2.0 and CarbonHV is Apache-2.0; a separately executed
program with a published source offer is the aggregate boundary (ADR-017). Every patch must carry
a header comment explaining it, and the licensing approach must be confirmed by the project owner
before a release ships the binary.

## What QEMU must provide

- `aarch64-softmmu` with TCG (`-accel tcg,thread=multi`), `--enable-rutabaga-gfx`,
  `--enable-slirp`, `--enable-fdt=system`.
- No KVM, Xen, SDL, GTK, VNC, SPICE, OpenGL/virglrenderer, USB, liburing, curl or capstone. The
  emulated session exposes exactly the CVD1 devices: virtio-gpu-rutabaga, virtio-input
  (multitouch + keyboard), virtio-net (slirp), virtio-blk, virtio-snd (AAudio backend),
  virtio-serial for the console, and vhost-user-vsock.
- Two CarbonHV backend patches: `ui/android.c` (scanout as `AHardwareBuffer` + `sync_file` to a
  socketpair; see `crates/carbon-emu/src/frame_proto.rs`) and `audio/aaudio.c` (NDK AAudio
  audiodev).

## Verify the pins

```bash
emu/qemu/build-android.sh --check                 # offline; what CI runs
emu/qemu/build-android.sh --check --require-pins  # refuses unrecorded source hashes
```

`--check` never touches the network. It verifies the sha256 of archives already present in
`downloads/`, refuses a `rutabaga_gfx` pin that disagrees with `Cargo.lock`, and exits non-zero on
a mismatch. Do not commit archives, build trees or the packaged binary (see `.gitignore`).

## Reproducing the "we own the device list" claim

After a build, on an arm64 AVD (never the user's phone without consent):

```bash
qemu-system-aarch64 --version
qemu-system-aarch64 -device help | grep -E 'rutabaga|virtio-multitouch|virtio-sound|vhost-user-vsock'
qemu-system-aarch64 -machine help | grep '^virt '
```

The rutabaga device is what makes hardware graphics possible. A build without it can still boot a
guest with `-display android` readback, but it is not the engine this project plans for.
