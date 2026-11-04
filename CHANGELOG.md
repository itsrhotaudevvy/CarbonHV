# Changelog

All notable changes to CarbonHV are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). Pre-release versions make **no support or
qualification claim**. See [`docs/SUPPORT_MATRIX.md`](docs/SUPPORT_MATRIX.md).

## [Unreleased]

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
- **Android app:** Compose library, catalog, create, settings and recovery flows, plus an immersive `SurfaceView` session with host-owned escape, geometry generations, input mapping and reconnect policy.
- **Qualification:**
  - a read-only ADB host probe covering hypervisor, AVF properties, vendor device label, core types, frequency domains, THP, GPU and renderer profile;
  - fail-closed minimal and sustained GPU qualification;
  - evidence for the Pixel 7a (Tensor G2) and the Redmi `kunzite` (Snapdragon 6 Gen 3).
- **Repository:** CI running `scripts/verify.sh`, a tag-driven release workflow, issue and PR templates, Dependabot, contributing and security policies, and screenshots.

### Fixed

- Host topology no longer mixes `cpu_capacity` and max-frequency units across cores.
- The GLES renderer parser handles vendor names that contain commas, such as `Samsung Electronics Co., Ltd.`.
- Duplicate ADR-011 numbering in the architecture document.

### Known limitations

- The controlled host build, guest boots, gfxstream presentation, sustained qualification and signed bundles are all still hardware or integration gated.
- `carbonhvd` does not yet spawn or supervise the direct crosvm tier.
- Per-bundle Android boot parameters (`androidboot.*`) are recorded by the ROM builds but not yet passed through the broker.
- No backend enables MTE in guests yet: an MTE guest cannot map the non-MTE gfxstream blobs used for zero-copy presentation. Gunyah, GenieZone and Halla guests also lack SVE and PMU.

[Unreleased]: https://github.com/itsrhotaudevvy/CarbonHV/compare/v0.1.0-alpha.1...HEAD
[0.1.0-alpha.1]: https://github.com/itsrhotaudevvy/CarbonHV/releases/tag/v0.1.0-alpha.1
