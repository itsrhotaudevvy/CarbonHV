# Building CarbonHV

This document separates source verification, Android application builds, host-integration work, physical-host qualification, and full guest-image builds. Passing one layer does not imply that later hardware-dependent layers are qualified.

## 1. Development requirements

The checked-in project currently expects:

| Component | Requirement |
| --- | --- |
| Rust | 1.90 or newer; workspace edition 2024 |
| JDK | 17 or newer; JDK 17 is the reference local setup |
| Gradle | Wrapper-provided Gradle 9.1.0 |
| Android Gradle Plugin | 9.0.1 |
| Kotlin | 2.3.20 |
| Android SDK | compile/target SDK 36; minimum SDK 30 |
| Android UI | Jetpack Compose + Material 3 |
| ADB | Optional for physical-host probing |
| Linux/x86_64 build host | Required for the GrapheneOS-derived source build; recommended for full guest-image pipelines |

The macOS development machine can build and verify the Rust workspace and native Android application. It is not treated as evidence that the Android target can run the VM stack.

## 2. Environment setup

Use the repository root as the working directory.

On macOS with Homebrew OpenJDK 17:

```bash
export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
export GRADLE_USER_HOME="$PWD/.gradle-local"
```

If your Android SDK is not already discoverable by Gradle, configure it through the normal Android tooling (`ANDROID_HOME`, `ANDROID_SDK_ROOT`, or a local SDK configuration appropriate to your environment). Do not commit machine-specific SDK paths.

Confirm the toolchains:

```bash
rustc --version
cargo --version
"$JAVA_HOME/bin/java" -version
./gradlew --version
```

## 3. Whole-repository verification

Run:

```bash
scripts/verify.sh
```

The verifier performs eight stages:

1. `cargo fmt --all -- --check`
2. `cargo test`
3. validation of `bundles/grapheneos/bundle.toml` and `bundles/ubuntu-touch/bundle.toml`
4. validation of both CVD1 guest contracts
5. shell syntax checks for guest and qualification scripts
6. deterministic qualification fixture checks
7. Android domain/app unit tests
8. Android debug APK assembly and artifact existence check

The final line must be:

```text
CarbonHV verification: PASS
```

This proves the repository's host-independent contracts and buildable Android shell. It does not prove that either full guest boots on a physical Android device.

## 4. Rust host tools

Debug/test builds:

```bash
cargo test
cargo build
```

Release-mode binaries:

```bash
cargo build --release
```

Expected executable outputs:

```text
target/release/carbonctl
target/release/carbonhvd
```

The Mali profile uses AVF/crosvm's integrated Rutabaga + gfxstream renderer. CarbonHV requests the
Android 17 gfxstream configuration (`gfxstream-vulkan` + `gfxstream-composer`, surfaceless Vulkan)
through the AVF custom-VM configuration and hands the interactive `SurfaceView` to crosvm's Android
display service. CarbonHV does not require a second `/system/bin/carbon-renderer` process for this
profile.

Useful validation commands:

```bash
cargo run -q -p carbon-cli -- bundle verify bundles/grapheneos/bundle.toml
cargo run -q -p carbon-cli -- bundle verify bundles/ubuntu-touch/bundle.toml
cargo run -q -p carbon-cli -- guest verify guest/android/device/cvd1.toml
cargo run -q -p carbon-cli -- guest verify guest/ubuntu-touch/device/cvd1.toml
```

`carbonhvd` is not a drop-in daemon for stock Android. The repository now supplies a `system_ext` Soong module, a native `dev.carbonhv.broker` Binder service, the generated Rust client for `dev.carbonhv.host.runtime`, managed-storage labels, and CarbonHV-owned SELinux/service/property contexts. The ordinary UI binds only to the platform host integration; that service authenticates the UI signing certificate and forwards high-level lifecycle intent to `carbonhvd`. The daemon resolves managed objects, owns generation/display state, and calls the privileged host runtime for the actual AVF operation. These sources must still be included and compiled in a controlled Android 17 platform build.

## 5. Android application

Run unit tests:

```bash
./gradlew --no-daemon :android:domain:test :android:host-contract:test :android:app:testDebugUnitTest
```

Build the debug APK:

```bash
./gradlew --no-daemon :android:app:assembleDebug
```

Artifact:

```text
android/app/build/outputs/apk/debug/app-debug.apk
```

The debug APK contains the native manager/session UI implementation. It cannot self-grant the platform privileges needed by the host broker. A normal APK build is therefore not equivalent to a working full VM product on a stock phone.

No production signing configuration is committed. A stable release must introduce deliberate key custody, release-channel separation, reproducibility/provenance records, and revocation/rotation procedures before a signed production APK is produced.

## 6. Qualification fixtures

Fixture mode tests the policy evaluator only:

```bash
qualification/carbon-qualify.sh \
  --profile qualification/profiles/reference-development.toml \
  --dry-run
```

A `READY` fixture is synthetic evidence that the evaluator accepts a complete recorded capability tuple. It is not hardware certification.

The unsupported fixture can also be exercised directly:

```bash
qualification/carbon-qualify.sh \
  --profile qualification/profiles/unsupported.toml \
  --dry-run
```

## 7. Physical Android host probe

Connect the target Android host over ADB:

```bash
adb devices -l
qualification/carbon-qualify.sh --adb <serial>
```

The current probe verifies only:

- ARM64 host ABI;
- executable `/apex/com.android.virt/bin/vm`;
- presence of `/dev/kvm`;
- on hosts without KVM, presence of `/dev/gunyah`, Qualcomm's `qvirt` service and the output of `vm info`, reported as `hypervisor=gunyah`. Such a host exits `UNSUPPORTED` (status 3): Gunyah is not a qualified backend;
- CPU `cpu_capacity` per core (the performance/efficiency split the broker uses to cap interactive vCPUs), the GPU identity (`Mali-*`, `Adreno (TM) ...`) and the Vulkan driver version.

If those checks succeed, the probe reports hardware-virtualized CPU capability but intentionally leaves graphics `unverified` and returns `EXPERIMENTAL`. The following require later physical qualification with real guests:

- actual minimal guest boot and repeated clean lifecycle;
- renderer selection, virtio-gpu compatibility, scanout/buffer/fence ownership;
- touch/input mapping against real guest composition;
- frame latency, missed frames, cold boot, pause/resume, thermals, and sustained performance;
- audio, network/VPN behavior, storage exhaustion, process/surface loss, host update rehearsal;
- GrapheneOS-derived and Ubuntu Touch/Lomiri guest workflows.

## 8. Host platform integration

The files under `host/` are build inputs for an integrated Android/AOSP image. They are not an installer for arbitrary stock Android devices. CarbonHV-owned platform policy is defined by `carbonhvd.te`, `carbonhv_host_app.te`, `service_contexts`, `file_contexts`, `seapp_contexts`, and `property_contexts`; include them in the platform/private or equivalent controlled product policy for the pinned Android 17 branch. The reusable `host/product/carbonhv.mk` product fragment packages `carbonhvd` and `CarbonHvHostIntegration`, imports this directory through `SYSTEM_EXT_PRIVATE_SEPOLICY_DIRS`, and sets the read-only UI-certificate property. It deliberately fails the product build unless `CARBONHV_UI_SIGNING_SHA256` is supplied by the controlled product configuration.

The host app is platform-signed, installed as a privileged `system_ext` app, uses `android.uid.system` for the private VM object-store DAC boundary, and remains separate from the ordinary `dev.carbonhv` UI through its own SELinux domain and AIDL surface. Set `ro.carbonhv.ui_signing_sha256` in the controlled product to the ordinary UI APK certificate SHA-256 digest; an empty value rejects UI calls.

For a development Android tree, use the staging helper rather than manually copying source and editing the product repeatedly:

```bash
host/product/prepare-controlled-host.sh \
  --android-root /path/to/android-17 \
  --product-makefile device/google/lynx/aosp_lynx.mk \
  --ui-apk android/app/build/outputs/apk/debug/app-debug.apk \
  --apksigner "$ANDROID_HOME/build-tools/<version>/apksigner"
```

The helper owns only `/path/to/android-17/external/carbonhv`; if that directory already contains unrelated files it fails closed. It stages a build-clean source snapshot, derives the UI certificate digest with `apksigner` (or accepts `--ui-signing-sha256`), and inserts one replaceable `CarbonHV controlled-host integration` block into the selected product makefile. Re-running the helper updates the staged source and digest without duplicating the product include. Because CarbonHV's narrow AVF bridge links `framework-virtualization.impl`, the helper also extends Android 17's existing implementation-library visibility by exactly `//external/carbonhv:__subpackages__`; unexpected upstream structure aborts preparation rather than widening private-framework visibility. CarbonHV's Android Rust modules use the same Rust 2024 edition as the Cargo workspace so host and platform builds do not compile the broker under different language editions.

The connected Pixel 7a used for bring-up has an unlocked bootloader, but the current macOS/arm64 development host is not a supported GrapheneOS/AOSP full-build host. GrapheneOS documents x86_64 Linux with at least 32 GiB RAM for OS builds, so source integration can be prepared here while the actual `lynx` platform build must run in a compatible Linux/x86_64 environment.

The Binder direction is explicit in policy: only `carbonhvd` publishes `dev.carbonhv.broker`, only `carbonhv_host_app` finds/calls it, only `carbonhv_host_app` publishes `dev.carbonhv.host.runtime`, and only `carbonhvd` finds/calls that runtime service.

Before a host tuple can move beyond development status, integrate and review:

- the existing broker/host control path and process-death/reconnect behavior in the final product image;
- the existing `dev.carbonhv.host.runtime` Binder registration and Rust client in the final product image;
- caller identity and signing/permission relationship;
- compilation of the supplied SELinux domains, labels, file ownership, property context, and runtime directories against the exact host branch;
- AVF/crosvm API and executable compatibility for the pinned host source revision;
- service lifetime and process-death recovery;
- renderer and GPU access policy;
- storage/network policy and host-update compatibility.

The selected device, Android build fingerprint, kernel, AVF/APEX revision, CarbonHV commit, renderer, and guest builds must be recorded together as one qualification tuple.

### Pinned GPU renderer sources

The first Mali renderer profile is tied to exact upstream inputs. Do not replace these with a branch name or a newer crate version without creating a new qualification tuple:

- crosvm: `88c1385ccb8cb31e8e4d683f580c37057cfc4b48` from `https://chromium.googlesource.com/crosvm/crosvm`;
- Rutabaga: `rutabaga_gfx = 0.1.80`, the version selected by that crosvm revision, crates.io checksum `2817d168222af3c49269f583dd8e67429912953ae608f715fd97683ed50320a5`;
- Rutabaga support crate: `magma-gpu = 0.1.80`, matching the crosvm lock, crates.io checksum `002536499cb45b3509c179f42f8c48729364931668f5a2c1bef8c407addaa629`;
- gfxstream: `ebe5b424e15c0816f64b97c9691060333bb43c7e` from `https://github.com/google/gfxstream`.

`rutabaga_gfx` does not vendor gfxstream. Its `gfxstream` feature links an external `gfxstream_backend`, so Android renderer builds must build that library from the pinned gfxstream revision and pass its library directory through `GFXSTREAM_PATH`. Normal host workspace tests keep the native gfxstream feature disabled and test the renderer profile/selection contract without linking a host gfxstream library.

## 9. GrapheneOS-derived guest recipe

`guest/android/build.sh` is a Linux-only source-build recipe. It is deliberately distinct from official GrapheneOS device support.

Validate the script without starting a multi-gigabyte build:

```bash
bash -n guest/android/build.sh
cargo run -q -p carbon-cli -- guest verify guest/android/device/cvd1.toml
```

For a real guest build, pin the upstream GrapheneOS source revision/manifest, maintain the CarbonHV CVD1 product overlay, capture the complete source manifest, and record every downstream patch. The initial virtual product foundation is `aosp_cf_arm64_phone`, subject to the pinned upstream tree.

A successful source build is only the start of qualification. The image must boot on CarbonHV's actual virtual hardware, reach the real Android shell/setup/lockscreen, preserve declared SELinux/security behavior, persist data, update/recover correctly, and pass the published feature matrix.

## 10. Ubuntu Touch guest recipe

The Ubuntu Touch path targets Lomiri/Mir and Ubuntu Touch services directly on CVD1 virtio devices.

Validate the source recipe:

```bash
bash -n guest/ubuntu-touch/build.sh
cargo run -q -p carbon-cli -- guest verify guest/ubuntu-touch/device/cvd1.toml
```

A real assembly requires pinned ARM64 kernel/initramfs inputs plus a verified Ubuntu Touch rootfs:

```text
UT_ROOTFS_TARBALL
UT_ROOTFS_SHA256
UT_KERNEL_IMAGE
UT_INITRD_IMAGE
```

The preferred design avoids physical-phone vendor blobs. If direct virtio adaptation cannot run the genuine mobile session, a Halium-compatible virtual adaptation is a separately reviewed fallback. It must not be silently substituted while retaining the same support claim.

## 11. Release packaging boundary

CarbonHV has four distinct release domains:

1. Android manager APK;
2. integrated host broker/runtime package;
3. guest images/bundles;
4. catalog/update metadata.

They should be built and signed independently, with provenance linking the exact compatible versions. Full guest images stay outside the source repository. Development keys/channels remain separate from production keys/channels.

The current repository can produce the Android debug APK and Rust release binaries. It does not yet contain a production signing setup or supported guest image artifacts, so those are not fabricated during development builds.

## Continuous integration and releases

- **CI:** `.github/workflows/ci.yml` runs `scripts/verify.sh` on every push to `main` and on every pull request, on Ubuntu 24.04. It installs Android SDK 36, build-tools 36.0.0, CMake 3.22.1 and NDK 28.2.13676358, and uploads the debug APK as an artifact.
- **Releases:** `.github/workflows/release.yml` runs when a `v*` tag is pushed. It re-runs the full verifier, builds `carbonctl` for Linux x86_64 and macOS arm64, and attaches those with the debug APK and `SHA256SUMS` to a GitHub release. The release notes come from the matching `CHANGELOG.md` section; the job fails if that section is missing. Tags containing `-` (for example `v0.1.0-alpha.1`) are published as pre-releases.
- **Cutting a release:**
  1. Move the `[Unreleased]` entries into a new version section and set `versionName` in `android/app/build.gradle.kts`.
  2. Commit, then `git tag -a vX.Y.Z -m "CarbonHV vX.Y.Z" && git push origin vX.Y.Z`.
- **Signing:** the APK is debug-signed by CI. Release signing, signed bundles and catalog trust are tracked as open release gates (`docs/SUPPORT_MATRIX.md`).
