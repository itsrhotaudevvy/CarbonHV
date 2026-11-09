# Guest image builds in CI

The guest images are multi-gigabyte build artifacts, not source, so they are built by GitHub
Actions and consumed by hash. Each guest has its own repository, so a build for one guest cannot
break another and no image blob enters this repository.

| Guest | Repository | Where it runs | Status |
| --- | --- | --- | --- |
| GrapheneOS-derived (CVD1) | [carbonhv-guest-grapheneos](https://github.com/itsrhotaudevvy/carbonhv-guest-grapheneos) | Needs ≈400 GB disk, ≥32 GB RAM, Linux x86_64 — a self-hosted or larger runner, **not** a standard hosted runner | Recipe verified here; image not yet built |
| Ubuntu Touch (Lomiri, CVD1) | [carbonhv-guest-ubuntu-touch](https://github.com/itsrhotaudevvy/carbonhv-guest-ubuntu-touch) | Standard hosted runner; kernel, initramfs and rootfs assembly | Kernel and initramfs build in CI; the image needs a pinned UBports rootfs (two repository variables) |
| AOSP Cuttlefish stand-in | [carbonhv-guest-android](https://github.com/itsrhotaudevvy/carbonhv-guest-android) | Standard hosted runner; fetch, verify, repack | Sanctioned stand-in guest. **Not GrapheneOS** and never labelled as such |

## The recipes stay here

Each repository checks out CarbonHV at a pinned commit and runs the recipe that already exists in
this tree:

- GrapheneOS-derived: `guest/android/build.sh`
- Ubuntu Touch: `guest/ubuntu-touch/build.sh`, with `ci/kernel.sh` merging
  `guest/common/kernel/cvd1-near-native.fragment`, the family fragment, and
  `guest/common/kernel/cvd1-emulated.fragment`, then checking the result with
  `carbonctl guest kernel-config`
- Stand-in: `ci/repack.sh` writes a `bundle.toml` in the shape `bundles/schema.md` requires, with
  real artifact hashes and `id = "android-standin"`

Duplicating a build script into a CI repository would create a second source of truth for the
guest ABI. The pin in each run's `provenance.txt` is what makes the build reproducible instead.

## Consuming a published bundle

`carbonctl bundle verify <bundle.toml>` checks the manifest; the artifact hashes in it must be the
published ones, never the development zeros in `bundles/*/bundle.toml`. Nothing in the app should
accept a bundle whose `release_status` is `development` as a shipped guest.

## Why a standard runner cannot build the GrapheneOS-derived image

GitHub's `ubuntu-24.04` runner provides 4 vCPU, 16 GB RAM and about 70 GB of free disk after
cleanup. AOSP/GrapheneOS needs roughly six times that disk and twice that memory just to link
`system.img`; the build also runs for many hours. `ci/preflight.sh` in that repository measures the
runner and refuses with the three options (self-hosted machine, paid larger runner, keep using the
stand-in) rather than starting a six-hour build that would die at 70 GB.
