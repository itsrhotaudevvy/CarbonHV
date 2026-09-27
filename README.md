# CarbonHV

CarbonHV is an Android-native virtual-machine manager for ARM64 mobile operating systems that share Android's hardware and software ecosystem. The project targets local hardware virtualization, a native Kotlin/Jetpack Compose manager, and an immersive guest session presented through an Android `SurfaceView`.

The first two guest families are:

- a **CarbonHV-maintained GrapheneOS-derived ARM64 virtual-device build**; it is not an official GrapheneOS target or release;
- **Ubuntu Touch with Lomiri**, adapted as a real mobile Linux guest rather than a generic Ubuntu desktop.

## Current status

This repository is a development implementation milestone, not a supported CarbonHV v1 release.

Implemented and exercised in source:

- Rust host admission, lifecycle, storage, update, backup, and clone transaction logic;
- strict ARM64/CVD1 bundle and guest-contract validation;
- authenticated broker request model with generation/replay checks, resource caps, and managed object IDs;
- AVF launch-plan generation using a fixed program plus argv, with no shell interpolation;
- native Android Compose library/catalog/create/settings/recovery flows;
- an immersive `SurfaceView` session shell with host-owned escape, geometry generations, input mapping, and reconnect policy;
- GrapheneOS-derived and Ubuntu Touch CVD1 source/build recipes;
- vendor-neutral CVD1 GPU capability/renderer contracts, a Mali-first native presentation path, and deterministic GPU qualification fixtures;
- starter-guest graphics requirements for Android virtual gralloc/HWC/EGL/Vulkan and Ubuntu Touch Mesa/Mir on the same `virtio-gpu` ABI;
- a physical-host ADB probe plus fail-closed minimal and sustained GPU qualification entrypoints;
- one top-level verifier covering Rust, manifests, guest contracts, shell scripts, Android tests, and the debug APK.

Still hardware/integration gated:

- production Android platform/Binder registration for `carbonhvd` and final SELinux policy on a selected host branch;
- a minimal ARM64 guest actually booted through CarbonHV's AVF path;
- accelerated guest GPU rendering and guest-produced buffer presentation into the Android surface;
- real GrapheneOS-derived and Ubuntu Touch/Lomiri guest images built and booted;
- calibrated Pixel 7a GPU release thresholds derived from real 30-minute Android-derived and Ubuntu Touch runs;
- audio, networking, input, lifecycle, update, recovery, thermal, and sustained-performance qualification on real guests;
- production bundle/catalog signing, release signing, and supported-update infrastructure.

The repository deliberately does not fall back to software CPU emulation when hardware virtualization is unavailable.

## Repository map

| Path | Responsibility |
| --- | --- |
| `crates/carbon-core` | Admission, bundle/CVD contracts, lifecycle, storage, update, backup, clone semantics |
| `crates/carbon-protocol` | Typed broker-facing protocol data |
| `crates/carbon-broker` | Privileged broker policy shell and AVF backend launch planning |
| `crates/carbon-cli` | `carbonctl` validation/diagnostic CLI |
| `android/domain` | Android-facing immutable domain models and reducers |
| `android/app` | Native Compose manager and immersive session surface |
| `bundles` | Starter bundle manifests and schema notes |
| `guest/android` | GrapheneOS-derived CVD1 recipe and port contract |
| `guest/ubuntu-touch` | Ubuntu Touch/Lomiri CVD1 recipe and port contract |
| `host` | Android host-integration templates and boundary documentation |
| `qualification` | CI fixtures, physical-host capability probing, and GPU qualification policy/evidence |
| `scripts/verify.sh` | Whole-repository source/build verification |
| `docs/ARCHITECTURE.md` | Full product and system architecture |

## Quick verification

Prerequisites for the complete source verification are Rust 1.90+, JDK 17+, Android SDK 36, and a working Gradle/network setup for first dependency resolution.

On macOS with Homebrew OpenJDK 17:

```bash
export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
export GRADLE_USER_HOME="$PWD/.gradle-local"
scripts/verify.sh
```

The script checks formatting and Rust tests, validates both starter manifests and both CVD1 guest contracts, checks shell syntax and minimal/sustained GPU policy fixtures, runs Android JVM tests, and builds `android/app/build/outputs/apk/debug/app-debug.apk`.

## Build release-mode host tools

```bash
cargo build --release
```

The main executable outputs are:

- `target/release/carbonctl`
- `target/release/carbonhvd`

`carbonhvd` is currently a policy/runtime integration shell. Running a production privileged service still requires platform service registration, permissions, SELinux integration, fixed runtime ownership, and review against the selected Android host source branch.

## Physical-host probe

With an Android host connected through ADB:

```bash
adb devices -l
qualification/carbon-qualify.sh --adb <serial>
```

The ADB probe checks the host ABI, the AVF `vm` tool, and `/dev/kvm`. Even when all three are present, the result stays `EXPERIMENTAL` until physical GPU/display qualification is completed with a real guest workload.

The current Pixel 7a Mali profile is intentionally uncalibrated. `qualification/gpu/run-sustained.sh --adb <serial> --minutes 30` is the fail-closed bring-up preflight. Once real starter-guest frames and a measured baseline exist, the ADB+evidence form re-probes the live host and can qualify only when the calibrated exact tuple and every sustained threshold match. The recorded 2026-09-27 attempt is blocked before the sustained workload because the GrapheneOS-derived guest artifacts are absent and CarbonHV has not yet handed its live `SurfaceView` to AVF/crosvm's Android display service.

## Guest builds

The guest directories contain reproducible build entrypoints and CVD1 contracts, not prebuilt guest images. Full GrapheneOS-derived and Ubuntu Touch builds require dedicated Linux build infrastructure and pinned upstream inputs. See the guest READMEs and [docs/BUILDING.md](docs/BUILDING.md).

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Building and verification](docs/BUILDING.md)
- [Security model and current boundaries](docs/SECURITY.md)
- [Support matrix and release-gate coverage](docs/SUPPORT_MATRIX.md)

## License

The Rust workspace declares Apache-2.0. Upstream guest operating systems, Android components, Linux components, and future bundled artifacts retain their own licenses and attribution requirements; release packaging must include the applicable source notices and provenance.
