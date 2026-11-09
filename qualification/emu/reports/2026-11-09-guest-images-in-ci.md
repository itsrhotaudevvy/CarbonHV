# Guest images built by CI, one repository per guest

- **Date:** 2026-11-09
- **Plan:** `docs/superpowers/plans/2026-11-08-qemu-emulated-engine.md`, Phases 10 and 13
- **Why this exists:** a GrapheneOS-derived image needs a Linux x86_64 host with ≈400 GB of disk
  and ≥32 GB of RAM for many hours. That machine does not exist here. GitHub Actions is the
  builder, and each guest gets its own repository so a build for one cannot break another and no
  multi-gigabyte artifact enters the CarbonHV tree.

## What was created

| Repository | Runs on | What it does | Status |
| --- | --- | --- | --- |
| [carbonhv-guest-grapheneos](https://github.com/itsrhotaudevvy/carbonhv-guest-grapheneos) | a runner with ≈400 GB disk, ≥32 GB RAM, Linux x86_64 (`vars.CARBONHV_BUILDER_LABEL`) | runs CarbonHV's `guest/android/build.sh` at a pinned CarbonHV commit | **recipe only.** `ci/preflight.sh` measures the runner and refuses with the options; no run has had enough disk yet |
| [carbonhv-guest-ubuntu-touch](https://github.com/itsrhotaudevvy/carbonhv-guest-ubuntu-touch) | standard `ubuntu-24.04` | builds the ARM64 kernel with the CVD1 fragments, a statically linked initramfs, and assembles from a pinned UBports rootfs | initramfs **built**; kernel building (6.12.10, contract check); rootfs waits on two pinned variables |
| [carbonhv-guest-android](https://github.com/itsrhotaudevvy/carbonhv-guest-android) | standard `ubuntu-24.04` | fetches an official AOSP Cuttlefish ARM64 image, verifies, repacks into the CarbonHV layout | **built: build 16644040**, artifact `carbonhv-guest-android-standin-16644040` |

The recipes stay in this tree. Each repository checks out CarbonHV at a pinned commit and runs
`guest/android/build.sh`, `guest/ubuntu-touch/build.sh` or `ci/repack.sh`, and writes the pin into
`provenance.txt`. Duplicating a build script into a CI repository would create a second source of
truth for the guest ABI.

## What the CI runs actually found

1. **Google's legacy Build API rate-limits every unauthenticated client**, with
   `403 Rate limit exceeded for legacy API … migrate to Build API v4`. It does this from a laptop
   *and* from a GitHub runner, so "just do it in CI" does not help. The public
   `ci.android.com/builds/branches/<branch>/status.json` — the feed the build dashboard itself
   reads — carries `last_known_good_build`, which is all the build id we need, with no key.
2. **`ci.android.com/builds/submitted/<id>/<target>/latest/<artifact>` no longer serves bytes.** A
   `HEAD` claims `application/zip` and 1.1 GB, but a `GET` returns 4.3 kB of HTML: an "Artifact
   Viewer" page. The page embeds the object's **signed Cloud Storage URL**
   (`storage.googleapis.com/android-build/builds/…?GoogleAccessId=…&Expires=…&Signature=…`), and
   that URL does serve the zip (verified: `206`, `application/zip`). The resolver now follows it
   and is careful not to cache it, because the signature expires.
3. **A standard runner cannot build AOSP.** 4 vCPU / 16 GB / ~70 GB free versus ≈400 GB of disk.
   `ci/preflight.sh` prints the measurement table and exits 3 with the three options instead of
   starting a six-hour build that would die at 70 GB. Paid larger runners exist and cost money;
   that is the owner's decision, so the workflow does not silently opt in.
4. **Three small bugs, all caught by the first CI runs:** a script committed without its executable
   bit (`exit 126`), a Ubuntu Touch kernel checked against the *Android* contract (SELinux instead
   of AppArmor), and a cross compiler installed without `libc6-dev-arm64-cross`, so it could not
   find `bits/wordsize.h`.

## The stand-in guest this produced

`aosp_cf_arm64_only_phone-userdebug` build `16644040`, repacked with real hashes:

| Role | File | Bytes |
| --- | --- | --- |
| boot | `boot.img` | 67,108,864 |
| init_boot | `init_boot.img` | 8,388,608 |
| vendor_boot | `vendor_boot.img` | 67,108,864 |
| super | `super.img` | 1,746,554,644 |
| userdata | `userdata.img` | 2,249,012 |
| vbmeta, vbmeta_system, vbmeta_{system,vendor}_dlkm | `vbmeta*.img` | 12,288 / 4,096 |

Source zip sha256 `a9314183f3cf4d15a0a2121a85f8300ca7990788c52474d322c6fe9320b40802`. The manifest
says `id = "android-standin"` in its own file, because a Google-built image is not GrapheneOS and
must never be described as such.

## What is still missing before Android runs on the phone

1. **QEMU has no Android boot-image support.** `hw/arm/boot.c` in the pinned 11.1.3 contains no
   `ANDROID!` / `VNDRBOOT` parser, so `-kernel boot.img` cannot work. The kernel and ramdisk must be
   extracted (a header parse, not a build) and the cuttlefish bootconfig fed through `-append`.
2. **Partition layout.** Cuttlefish's first-stage init discovers partitions by name via GPT symlinks
   or `androidboot.*` bootconfig keys; the raw `super.img` needs the same disk ordering crosvm gets
   from `assemble_cvd`. Getting this wrong looks like a boot that stops early, so it is the next
   thing to do and to log carefully.
3. **GrapheneOS itself** still needs the 400 GB runner. Nothing in this report changes that, and no
   GrapheneOS claim is made anywhere for the stand-in image.

## Reproduce

```
gh workflow run fetch-standin.yml --repo itsrhotaudevvy/carbonhv-guest-android
gh run download <run-id> --repo itsrhotaudevvy/carbonhv-guest-android --dir /tmp/standin
gh workflow run build.yml --repo itsrhotaudevvy/carbonhv-guest-ubuntu-touch
gh workflow run build.yml --repo itsrhotaudevvy/carbonhv-guest-grapheneos   # fails at preflight until a big runner exists
```
