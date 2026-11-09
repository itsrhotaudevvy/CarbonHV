# Handoff prompt — boot Android in the emulated engine on the phone

> **Status 2026-11-09 (later the same day): done to `system_server`.** The stand-in boots on the phone
> and draws on its screen, as the shell user and inside `qualification/emu/viewer-app`
> (`untrusted_app`). Blockers 1 and 2 below are solved by `carbonctl emu boot-assets` and
> `qualification/emu/android-standin/`. What remains is boot speed to `boot_completed`. See
> `qualification/emu/reports/2026-11-09-android-standin-boot-on-phone.md`. The owner has dropped
> custom ROM builds (GrapheneOS/LineageOS/PixelOS) for now.

Paste everything below the line into a fresh agent session started in `/Volumes/SSD/Code/CarbonHV`.

---

## Job

Get a real Android guest booting inside CarbonHV's QEMU emulated engine on the owner's phone, with
its frames reaching the app (or the device probe, until `:vm` exists). The end goal the owner stated
is "GrapheneOS running as intended on release in the phone". The GrapheneOS-derived image does not
exist yet — a Google-built AOSP Cuttlefish image is the plan's sanctioned stand-in and must always be
labelled as such (`id = "android-standin"`), never as GrapheneOS.

Working directory: `/Volumes/SSD/Code/CarbonHV` (repo, `main`, everything below is pushed).
Phone: Redmi `kunzite`, serial `8b227126`, Android 16, unrooted, `adb shell` works as `shell` user.
The owner has said: use this phone, and keep `/data/local/tmp/carbon-emu` (93 MB) on it.

## Where things stand

Done and verified:

- **Phase 4 display backend** — QEMU patch `emu/qemu/patches/qemu/0003-ui-add-android-ahb-backend.patch`
  adds `-display android,frame-fd=N`: three-slot `AHardwareBuffer` ring over a
  `SOCK_SEQPACKET` socketpair, readback presentation, wire format in `ui/android-proto.h`.
  Host test `qualification/emu/frame-proto/run.sh` → "111 checks passed"; Rust reader in
  `crates/carbon-emu/src/frame_proto.rs` asserts the same golden bytes.
- **A Linux guest has booted on the phone** through this backend: Alpine 6.12.115 kernel with
  `virtio-gpu-pci`, console text captured as real frames. Report:
  `qualification/emu/reports/2026-11-09-phase4-first-guest-boot.md`, artifacts in
  `qualification/emu/reports/data/20261109-143608-frame-probe/`.
- **Device probe** `qualification/emu/device-frame-probe/` (`build.sh`, `run-on-phone.sh`,
  `probe.c`, `rgba-to-png.py`): forks QEMU, owns fd 3, saves frames as PNG.
  Run: `ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/28.2.13676358 qualification/emu/device-frame-probe/run-on-phone.sh --kernel ~/carbonhv-cache/alpine/vmlinuz-virt --initrd ~/carbonhv-cache/alpine/initramfs-virt --frames 10 --seconds 120`
- **Release APK pipeline**: `scripts/build-apk.sh --release`, env-driven signing
  (`CARBONHV_KEYSTORE`…), debug-key builds named `-release-debugkey.apk`. Installed on the phone
  (`dev.carbonhv` 0.1.0, non-debuggable, activity `dev.carbonhv/.app.MainActivity`, cold start 418 ms).
- **Three guest CI repositories**, one per guest (recipes stay here, each repo checks out CarbonHV at
  a pinned commit):
  - `itsrhotaudevvy/carbonhv-guest-android` — **works**: run `42641974882` fetched AOSP Cuttlefish
    `aosp_cf_arm64_only_phone-userdebug` build **16644040**, verified, repacked. Artifact
    `carbonhv-guest-android-standin-16644040` has `boot.img` (64 MiB), `init_boot.img`, `vendor_boot.img`
    (64 MiB), `super.img` (1.75 GB), `userdata.img`, `vbmeta*.img`, `partitions.txt` (real hashes),
    `bundle.toml`, `provenance.txt` (source zip sha256 `a9314183…`).
  - `itsrhotaudevvy/carbonhv-guest-ubuntu-touch` — initramfs job **passes**; kernel job was still
    compiling kernel 6.12.10 at handoff (check whether it finished and what
    `carbonctl guest kernel-config` said); rootfs job waits on repo variables
    `CARBONHV_UT_ROOTFS_URL` + `CARBONHV_UT_ROOTFS_SHA256`.
  - `itsrhotaudevvy/carbonhv-guest-grapheneos` — recipe only: `ci/preflight.sh` measures the runner
    (~70 GB free on `ubuntu-24.04` vs ≈400 GB needed) and exits 3 with the options. Nothing built.

## The immediate blockers, in order

1. **QEMU 11.1.3 has no Android boot-image parser** (`hw/arm/boot.c` has no `ANDROID!` / `VNDRBOOT`),
   so `-kernel boot.img` cannot work. Extract instead: boot image v4 header gives kernel + ramdisk;
   `vendor_boot.img` v4 gives vendor ramdisk + a **bootconfig** section (header offsets for v4:
   `magic[8]`, `header_version`@8, `page_size`@12, `kernel_addr`@16, `ramdisk_addr`@20,
   `vendor_ramdisk_size`@24, `cmdline[2048]`@28, `tags_addr`@2076, `name[16]`@2080,
   `header_size`@2096, `dtb_size`@2100, `dtb_addr`@2104, then `vendor_ramdisk_table_size`@2112,
   `…entry_num`@2116, `…entry_size`@2120, `bootconfig_size`@2124; sections start at the next page
   boundary after the header, which is 4096 for `page_size` 2048). A quick header parse is already
   half-written in the session log; do not fight `cvdhost/bin/unpack_bootimg.py`, it imports the
   native binary of the same name and fails with "bad magic number".
2. **Partition layout.** Cuttlefish's first-stage init finds partitions by name (GPT symlinks or
   `androidboot.*` bootconfig keys) and expects the disk ordering `assemble_cvd` gives crosvm. Raw
   `super.img` behind `virtio-blk` is not automatically that. Get this right and log it; a wrong
   layout looks like a boot that stops early with no obvious error.
3. The emulator binary on the phone and locally is
   `android/emu/src/main/jniLibs/arm64-v8a/libqemu_system_aarch64.so`
   (sha256 `2ba9534a…` was current at handoff; rebuild with
   `ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/28.2.13676358 CARBON_EMU_SKIP_DEPS=1 emu/qemu/build-android.sh --build`).

## Hard-won facts — do not rediscover these

- **`#if HOST_BIG_ENDIAN`, never `#ifdef`.** `qemu/compiler.h` defines it as a *value*, so `#ifdef`
  takes the big-endian branch on every little-endian build. That bug rendered a black console solid
  red and was invisible to every host-side test. The AHB is `R8G8B8A8`, so the pixman target is
  `PIXMAN_a8b8g8r8` on little-endian.
- **Frame usage is `0x132`** (`CPU_READ_RARELY | CPU_WRITE_OFTEN | GPU_SAMPLED_IMAGE`). Without the
  read flag no diagnostic reader can lock the buffer. Golden bytes carry it.
- **Reader protocol order**: one FRAME message datagram, then one `AHardwareBuffer` handle datagram;
  the reader must consume the handle even for frames it drops, and must send `RELEASE`, or the ring
  stalls or every later read is one datagram out of step.
- **The probe must own fd 3 itself** (fork + `dup2` + clear close-on-exec). `posix_spawn` file
  actions ran `addclose` after `adddup2` and closed the socket, after which QEMU's `-serial file:`
  took fd 3 and the first write died with `ENOTSOCK`.
- **Google's legacy Build API 403-rate-limits everyone** (`Rate limit exceeded for legacy API`), from
  laptops and GitHub runners alike. `ci.android.com/builds/branches/<branch>/status.json` gives
  `last_known_good_build` without a key.
- **`ci.android.com/builds/submitted/<id>/<target>/latest/<artifact>` returns HTML**, not bytes: an
  "Artifact Viewer" page with the object's signed `storage.googleapis.com/android-build/…?GoogleAccessId=…`
  URL embedded. Follow that URL in the same run; it expires.
- A standard runner cannot build AOSP. Do not "fix" `ci/preflight.sh` to make it try.

## Environment

```
export ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/28.2.13676358
export JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home
export GRADLE_USER_HOME=/Volumes/SSD/Code/CarbonHV/.gradle-local
```
`~/carbonhv-cache/alpine/` holds `vmlinuz-virt` + `initramfs-virt` (known-good Linux guest).
`scripts/verify.sh` must stay green (it builds both APKs and runs the protocol test).
Guest images live in CI: `gh run download <run-id> --repo itsrhotaudevvy/carbonhv-guest-android --dir /tmp/standin`.

## Definitions of done for this stretch

1. Android (the `android-standin` image) reaches a running userspace on the phone with its frames
   visible, and the evidence — serial log, `adb` output, one captured frame as PNG — is committed
   under `qualification/emu/reports/` with a report that states plainly what is Android and what is
   not.
2. `scripts/verify.sh` PASS, everything pushed.
3. Only then, the GrapheneOS track: either a runner with ≈400 GB is available
   (`gh variable set CARBONHV_BUILDER_LABEL`) and the image builds, or it is explicitly reported as
   still blocked. Never describe the stand-in image as GrapheneOS, and never forge device identity,
   attestation or security claims for a virtual device (ADR/architecture §17.5).
