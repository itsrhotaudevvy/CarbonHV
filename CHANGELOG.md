# Changelog

All notable changes to CarbonHV are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). Pre-release versions make **no support or
qualification claim**. See [`docs/SUPPORT_MATRIX.md`](docs/SUPPORT_MATRIX.md).

## [Unreleased]

### Added

- **Emulated engine planning (QEMU TCG, ADR-017).** For stock phones where Android denies apps every hypervisor device, an instance can now be created as *Emulated*: guest CPU instructions through QEMU's dynamic binary translation, graphics through gfxstream on the host GPU, over the unchanged CVD1 guest ABI.
  - `crates/carbon-emu`: exact QEMU argv, TCG vCPU placement on performance cores, memory admission with a 768 MiB host reserve, per-profile CPU model (`pauth-impdef`, SVE/SME off, MTE opt-in), boot/init_boot/vendor_boot v3/v4 parsing with bootconfig assembly, a typed QMP client, and the versioned frame/input wire format. Golden-tested.
  - `ExecutionEngine`/`EmulationReadiness` in the domain, and broker engine selection that never turns a hardware-VM instance into an emulated one.
  - `carbonctl emu boot-assets` runs the engine's boot-image code on the build host and writes `kernel`, `initrd` (vendor ramdisk ‖ generic ramdisk ‖ bootconfig) and `cmdline`, refusing output the kernel would not accept.
  - `qualification/emu/android-standin/`: boots the AOSP Cuttlefish stand-in guest (not GrapheneOS) on a phone. Cuttlefish's `os_composite` GPT layout rebuilt from the CI bundle (`make-disk.py`, checked by `verify-disk.py`), the U-Boot/assemble_cvd bootconfig written out key by key with provenance (`bootconfig.txt`), and a Cuttlefish-shaped QEMU command line.
  - **An Android guest boots on the phone.** The AOSP Cuttlefish stand-in (not GrapheneOS) reaches `system_server` and its boot animation on the stock Redmi `kunzite`, drawn on the phone's screen. See `qualification/emu/reports/2026-11-09-android-standin-boot-on-phone.md`.
  - `qualification/emu/viewer-app` (`dev.carbonhv.emuviewer`, debug-only, never shipped) runs QEMU as a child of an `untrusted_app` foreground service and draws its frames on a `SurfaceView`. It keeps the latest frame for surfaces that attach later and logs every frame it cannot draw. `run-on-phone.sh --viewer` stages the guest into its private storage with `run-as`.
  - Stand-in bootconfig fixes found on the phone: `openthread_node_id` and `vsock_lights_port`. Without them the Thread and lights HALs crash-loop and `system_server` waits for `ILights` forever.
  - The device frame probe takes `--interval S` to spread its frames over a long boot, and no longer caps QEMU at 64 arguments.
  - `carbonctl emu plan --facts <json>` prints the exact argv, the memory budget, placement and every warning. Exit 65 with the numbers when admission refuses.
  - `emu/qemu/` cross-build recipe with pinned versions and `--check`, and `qualification/emu/` fixtures and reports.
- Snapdragon controlled-host fragment (`host/product/snapdragon/`). It declares AVF VM support in the boot config, adds the one-line `vm_manager_device_type` vendor policy for the observed `vendor_gunyah_dev` label, and sets THP to `madvise`. A contract test checks it against the physical probe evidence.
- `host/product/snapdragon/patch_vendor_boot.py`, with tests, plus the `KUNZITE.md` procedure:
  - it adds only `androidboot.hypervisor.vm.supported=1` to a stock `vendor_boot` v3/v4 and leaves every other byte and the image size intact;
  - it is idempotent and refuses foreign images or images without padding;
  - the procedure covers patching, flashing, verifying, a Microdroid first-boot test and rollback.
- Physical re-probe of the Redmi `kunzite` (Snapdragon 6 Gen 3) on 2026-11-04. Its stock vendor policy already grants AVF's crosvm `/dev/gunyah` through `vm_manager_device_type`, so on this phone only the boot-config declaration is missing.
- Leaner, icon-led manager UI:
  - symbolic OS tiles (shield, Android, sparkle, touch) instead of letters;
  - icon + value facts, shorter copy and icon actions;
  - vendored Material vector icons (Apache-2.0), so the APK stays at about 31 MB.

### Changed

- **Redesigned manager screens.** Library and Systems are now adaptive grids of large OS tiles under a single-line app bar, replacing thin rows under a collapsing bar that took a third of the screen. Each OS has a new glyph drawn for CarbonHV (`res/drawable/ic_os_*.xml`). These are CarbonHV's own symbols, not the projects' logos.
- **Memory per VM: 3, 4 or 6 GB**, chosen on the New VM screen, saved with the instance (state codec v2) and passed to the host start request.
- **LineageOS and PixelOS are planned**: listed under "Planned" in Systems and cannot be created, because no image is built.
- Screenshots are now from the phone (`docs/screenshots/*.jpg`), including the emulated guest on screen.
- The emulated engine is admitted only from an instance's stored engine choice. Admission now distinguishes `EmulationNotReady` (memory, Vulkan, ABI) from missing hardware virtualization.

### Fixed

- **Ubuntu Touch rootfs assembly ran as an ordinary user.** `guest/ubuntu-touch/build.sh` unpacked the UBports tarball without root, so every file would have been the build user's and root-only directories could not be imaged (`mkfs.ext4: Permission denied while scanning directory "void"`, CI run 42652745740). It now unpacks with `--numeric-owner -p` and images as root.
- **Emulated engine bootconfig was invisible to the kernel.** `carbon_emu::bootimg` wrote `#BOOTCONFIG\n` *before* `size`/`checksum` and used a word-sum-to-zero checksum; Linux (`get_boot_config_from_initrd`, init/main.c) looks for the magic as the initrd's last 12 bytes and checks a plain byte sum. Every `androidboot.*` key would have been silently dropped. The block now matches the kernel byte for byte (golden test written from the v6.12 source, plus `kernel_view_of_bootconfig`, a reader that does what the kernel does). `assemble` also now carries vendor_boot's own bootconfig section (e.g. `androidboot.hardware=cutf_cvm`), which it had parsed and then discarded.
- Landscape phones: on short windows the manager now uses a compact app bar with "New VM" as a bar action, and keeps content at a centered, readable width. On the Snapdragon phone, the large collapsing bar had taken half the screen and the New VM button covered the card's Open button.

### Changed

- The performance plan drops huge-page advice on hosts whose runtime THP mode is `never` (`ThpMode`), as observed on stock Snapdragon firmware.

## [0.1.0-alpha.1] - 2026-11-04

First public development milestone. The source, contracts and tooling are verified in CI. **No guest has booted on a physical phone through CarbonHV yet**; every physical host stays `EXPERIMENTAL` or `UNSUPPORTED`.

### Downloads

- `carbonhv-v0.1.0-alpha.1-debug.apk`: the Compose manager, debug-signed by CI. On a stock phone it runs, but sessions fail closed ("Host integration unavailable") because no CarbonHV host integration is installed.
- `carbonctl-*`: the bundle, guest-contract, guest kernel-config and backup verifier for Linux x86_64 and macOS arm64.
- `SHA256SUMS`.

`carbonhvd` and the platform host integration are built from source inside a controlled Android 17 product (`host/product/README.md`) and are not shipped as binaries.

### Added

- **Core model:**
  - ARM64-only host admission with no CPU-emulation fallback;
  - strict bundle manifests;
  - CVD1 guest contracts;
  - lifecycle generations;
  - storage, update, backup, restore and clone transactions.
- **Broker (`carbonhvd`):**
  - an authenticated request model with generation and replay checks, resource caps and managed object IDs;
  - a single graphical display slot;
  - the Binder control plane (`dev.carbonhv.broker`) and runtime publication to the platform host integration (`dev.carbonhv.host.runtime`);
  - Soong packaging plus service-manager, file, property, seapp and SELinux contracts.
- **AVF path:** Android 17 raw-config launches with gfxstream Vulkan/composer, the AVF display service, and multi-touch, keyboard and mouse input.
- **Near-native performance plan:**
  - vCPUs pinned 1:1 to performance cores, with in-order efficiency cores excluded by `MIDR_EL1`;
  - mirrored capacities and cpufreq domains;
  - guest DVFS through `--virt-cpufreq-upstream`;
  - THP advice, free-page reporting, and SVE in auto mode;
  - AVF `hugePages`, `boostUclamp` and `balloon` carried end to end.
- **Hypervisors:**
  - KVM/pKVM (Google Tensor), GenieZone (MediaTek), Halla (Samsung Exynos) and Gunyah (Qualcomm), probed in crosvm's order;
  - AVF-readiness classification from `ro.boot.hypervisor.*`;
  - `DirectCrosvmBackend` launch plans with an explicit `--hypervisor` and fd-only inputs, never a protected VM.
- **GPU:** gfxstream renderer profiles for Mali, Immortalis, Adreno, PowerVR and Samsung Xclipse on one shared CVD1 transport contract.
- **Starter guests:**
  - GrapheneOS-derived (Android 17 line) and Ubuntu Touch (24.04 series) build recipes;
  - `carbonctl guest kernel-config` with shared and per-family fragments, which the Ubuntu Touch assembly requires.
- **Custom ROM ports (unofficial, recipe only):**
  - LineageOS (`lineage-24.0`, Android 17; `lineage-23.2` selectable) and PixelOS (`seventeen`, Android 17; `sixteen-qpr2` selectable);
  - one shared CVD1 product overlay on the AOSP cuttlefish arm64-only phone (ADR-016), with direct-boot kernel, initrd and super artifacts;
  - bundles, broker starter profiles and app catalog entries.
- **Android app:**
  - a Material 3 manager with CarbonHV's own light and dark palette, a DayNight window theme and an adaptive launcher icon;
  - a library with OS badges and status pills, a Systems catalog with "Unofficial" chips, and a guided New VM flow;
  - grouped settings, plus centered status and recovery screens;
  - an immersive `SurfaceView` session with host-owned escape, geometry generations, input mapping and reconnect policy.
- **Qualification:**
  - a read-only ADB host probe covering hypervisor, AVF properties, vendor device label, core types, frequency domains, THP, GPU and renderer profile;
  - fail-closed minimal and sustained GPU qualification;
  - evidence for the Pixel 7a (Tensor G2) and the Redmi `kunzite` (Snapdragon 6 Gen 3).
- **Repository:** CI running `scripts/verify.sh`, a tag-driven release workflow, issue and PR templates, Dependabot, contributing and security policies, and screenshots.

### Changed

- The emulated engine is admitted only from an instance's stored engine choice. Admission now distinguishes `EmulationNotReady` (memory, Vulkan, ABI) from missing hardware virtualization.

### Fixed

- Host topology no longer mixes `cpu_capacity` and max-frequency units across cores.
- The GLES renderer parser handles vendor names that contain commas, such as `Samsung Electronics Co., Ltd.`.
- The app no longer leaks its host-integration `ServiceConnection` when the activity is destroyed on a build without the host integration, and it unbinds before rebinding after binding death.
- The library and any in-progress New VM form now survive configuration changes (rotation, dark mode) and process recreation.
- Duplicate ADR-011 numbering in the architecture document.

### Known limitations

- The controlled host build, guest boots, gfxstream presentation, sustained qualification and signed bundles are all still hardware or integration gated.
- `carbonhvd` does not yet spawn or supervise the direct crosvm tier.
- Per-bundle Android boot parameters (`androidboot.*`) are recorded by the ROM builds but not yet passed through the broker.
- No backend enables MTE in guests yet: an MTE guest cannot map the non-MTE gfxstream blobs used for zero-copy presentation. Gunyah, GenieZone and Halla guests also lack SVE and PMU.

[Unreleased]: https://github.com/itsrhotaudevvy/CarbonHV/compare/v0.1.0-alpha.1...HEAD
[0.1.0-alpha.1]: https://github.com/itsrhotaudevvy/CarbonHV/releases/tag/v0.1.0-alpha.1
