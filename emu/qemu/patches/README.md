# QEMU patches for the emulated engine

Each patch here is a minimal, separately reviewable change with a header comment stating what it
does, why it is needed on bionic/Android, and which upstream behaviour it preserves.

Planned patches (Phase 2/4/7):

| Patch | Purpose |
| --- | --- |
| `0001-ui-add-android-ahb-backend.patch` | `ui/android.c`: `-display android,frame-fd=N,input-fd=M`; scanout as `AHardwareBuffer` + fence, input injection into virtio-input |
| `0002-audio-add-aaudio-backend.patch` | `audio/aaudio.c`: AAudio audiodev for `virtio-snd` |
| `00xx-android-portability.patch` | bionic gaps (`sys/shm.h`, `getloadavg`, `pthread_cancel`, `posix_memalign`, `tmpfile`, ...) |

Nothing is patched yet. QEMU is GPL-2.0; the published source offer for every release that ships
the binary must include these patches and the exact sources pinned in `../VERSIONS.toml`.
