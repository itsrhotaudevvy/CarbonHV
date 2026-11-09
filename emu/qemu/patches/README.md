# Patches for the emulated engine's Android build

Each patch is a minimal, separately reviewable change. Its header comment says what it does, why
bionic/Android needs it, and whether it has been submitted upstream. `build-android.sh` applies
`<project>/*.patch` in name order right after unpacking the pinned archive.

| Directory | Patches today | Later (plan phase) |
| --- | --- | --- |
| `qemu/` | `0001-android-memfd-shm` (no `shm_open`/`librt` on bionic), `0002-libvhost-user-bionic-uapi-headers` | `ui/android.c` frame/input backend (Phase 4/5), `audio/aaudio.c` (Phase 7) |
| `gfxstream/` | `0001-meson-android-host`, `0002-android-ndk-headers-no-x11`, `0003-udmabuf-require-access` | AHB-backed display ColorBuffers for zero-copy (Phase 6) |
| `rutabaga_gfx/` | `0001-ndk-build-without-aosp-nativewindow`, plus `Cargo.lock` for `cargo build --locked` | — |

QEMU is GPL-2.0. The published source offer for every release that ships the binary must include
these patches and the exact sources pinned in `../VERSIONS.toml`.
