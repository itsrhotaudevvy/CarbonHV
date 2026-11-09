# Phase 4: the Android display backend (`-display android`)

- **Date:** 2026-11-09
- **Plan:** `docs/superpowers/plans/2026-11-08-qemu-emulated-engine.md`, Phase 4
- **Gate:** **G4 partially met.** The QEMU half of G4 is built, compiled into the Gate 2 binary
  and golden-tested against the Rust reader. It is **not yet met end to end**: no guest image is
  installed on this Mac's AVDs (there are no AVD system images at all), and the app-side reader
  (JNI → `GraphicsSession`) belongs to the `android/emu` module that does not exist yet
  (Phase 9). What is proven and what is not is listed at the bottom.

## What was added

| File | What it does |
| --- | --- |
| `emu/qemu/patches/qemu/0003-ui-add-android-ahb-backend.patch` | The whole backend as one reviewable patch |
| ↳ `ui/android-proto.h` | Wire format + three-slot buffer ring. **No QEMU and no Android headers**, so the build host compiles it |
| ↳ `ui/android.c` | `-display android,frame-fd=N`: binds a `DisplayChangeListener` to the default console, converts the scanout into a ring of three `AHardwareBuffer`s, sends `HELLO`/`MODE`/`FRAME`/`CURSOR`, applies `RELEASE`, logs `RESIZE` |
| ↳ `qapi/ui.json` | `DisplayType.android` + `DisplayAndroid{frame-fd}`, gated on `CONFIG_ANDROID_DISPLAY` |
| ↳ `meson.build`, `ui/meson.build` | `CONFIG_ANDROID_DISPLAY` is `cc.get_define('__ANDROID__') != ''`; links `-landroid` |
| `qualification/emu/frame-proto/` | Host test: extracts `ui/android-proto.h` from the patch with `git apply --include`, compiles it with `cc`, checks 111 assertions |
| `qualification/emu/frame-proto/golden-messages.txt` | The bytes QEMU emits, pinned. Read by both the C test and the Rust test |
| `crates/carbon-emu/src/frame_proto.rs` | New `agrees_with_the_qemu_golden_messages` test: decodes every golden message, re-encodes it, and requires byte equality |

Design decisions worth recording:

- **Readback first.** This is v1: the pixman scanout is converted (`PIXMAN_OP_SRC` when the format
  differs, a row copy when it does not) into a CPU-writable, GPU-samplable
  `AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM` buffer with usage `0x130`. The plan's Phase 6 replaces
  the *conversion* with the gfxstream composer scanout; the transport, ring and backpressure stay.
- **The backend never blocks a vCPU.** Three buffers, then frames are dropped and counted. A
  `RELEASE` for a sequence a mode change already invalidated is counted as unknown, not fatal.
- **The console may not exist.** `qemu_console_lookup_default()` may return NULL (a `-nodefaults`
  machine with no display device). `qemu_console_register_listener` then goes through QEMU's
  placeholder-surface path, which emits a real `MODE` + `FRAME` ("This VM has no graphic display
  device.") — the frame path can therefore be exercised with no guest at all.
- **`RESIZE` is parsed and logged, not applied.** v1 has no guest display hotplug, and the app's
  own surface scaling covers rotation, so pretending otherwise would be a lie in the protocol.
- **Directions are enforced.** Only `RELEASE` and `RESIZE` decode on the app→QEMU direction; a
  `FRAME` or a `HELLO` arriving there is rejected. Every field is bounds-checked
  (`MAX_DIMENSION 16384`, `format ≤ 1024`, `dpi ≤ 10000`, `seq != 0`), because the service treats
  QEMU's socket peer as semi-trusted and QEMU treats the app's as untrusted.

## Build result

`emu/qemu/build-android.sh --build` with `CARBON_EMU_SKIP_DEPS=1` (deps unchanged) succeeds; the
new file appears as `libsystem.a.p/ui_android.c.o` with no warnings from either new file.

| Item | Gate 2 | Phase 4 |
| --- | --- | --- |
| sha256 | `d98e38f3…ffd5d` | `86aa496e64dbfc4d71764a35b1b4fcc048bd789e3203b15447373a359ed01836` |
| bytes (stripped) | 43,808,392 | 43,811,368 (**+2,976**) |

Four build iterations were needed and each one is a real finding:

1. QAPI lint rejects documentation lines over 70 characters (twice).
2. `DisplayOptions` union members are values, not pointers here: `opts->u.android.frame_fd`.
3. **`host_os` is `linux`, not `android`, in QEMU's meson build** when cross-compiling with the
   NDK, because `configure` overrides the cross file's `host_machine.system`. Gating on
   `host_os == 'android'` silently excluded the file (`CONFIG_ANDROID_DISPLAY` was `#undef` and
   `android.c` never reached `build.ninja`). The gate is `cc.get_define('__ANDROID__')`, the same
   idiom the existing bionic patch uses. **This is the kind of silent no-op that a build-only
   check cannot catch**, which is why the protocol test exists.

## Protocol test (no QEMU, no device, no NDK)

```
$ qualification/emu/frame-proto/run.sh
android-proto: 111 checks passed
$ cargo test -p carbon-emu
43 passed
```

The C test fails if the producer and `golden-messages.txt` disagree; the Rust test fails if the
reader and the same file disagree, or if re-encoding does not reproduce the bytes. An earlier
revision of the test had an off-by-offset truncation bug in its own golden reader and the test
caught it, which is the reason it compares full hex rather than only lengths.

Also wired into `scripts/verify.sh` step 7, so a protocol change that touches one side only fails
CI.

## What this does NOT prove

- **No guest booted, and no frame reached a Surface.** No AVD system image is installed on this
  Mac; the phone path needs the `:vm` service and the app's frame reader (Phase 9).
- **No `AHardwareBuffer_allocate` from inside QEMU has run on the phone yet.** Gate 3 proved the
  reverse direction (a child exported a driver-allocated image and the app read it back); QEMU now
  needs to *allocate* the buffer itself. That is the next device check, and it is cheap: one
  `adb shell` run, no APK.
- **Readback cost per frame is unmeasured** (the plan asks for it in G4). It is a full-frame CPU
  copy of the scanout; the number belongs with Phase 6's zero-copy comparison.
- **`CURSOR` is defined and wired to `dpy_mouse_set`, but no touch guest will use it.** Phase 5
  owns input.

## Reproduce

```bash
emu/qemu/build-android.sh --build                     # rebuilds with 0003 applied
qualification/emu/frame-proto/run.sh                   # protocol + ring, on the build host
cargo test -p carbon-emu frame_proto                   # the reader side of the same bytes
```

`qualification/qemu-frame-probe/` (added with the guest bring-up) runs the binary on the phone as
`adb shell` and prints the frames it receives.
