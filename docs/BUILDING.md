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

Useful validation commands:

```bash
cargo run -q -p carbon-cli -- bundle verify bundles/grapheneos/bundle.toml
cargo run -q -p carbon-cli -- bundle verify bundles/ubuntu-touch/bundle.toml
cargo run -q -p carbon-cli -- guest verify guest/android/device/cvd1.toml
cargo run -q -p carbon-cli -- guest verify guest/ubuntu-touch/device/cvd1.toml
```

`carbonhvd` is not yet a drop-in daemon for stock Android. The Rust crate supplies the broker policy model and AVF launch-plan boundary. Production integration must wire it into the selected Android platform release with authenticated platform IPC, fixed service identity, SELinux domains/labels, managed storage, and direct process spawning without a generic privileged shell.

## 5. Android application

Run unit tests:

```bash
./gradlew --no-daemon :android:domain:test :android:app:testDebugUnitTest
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
- presence of `/dev/kvm`.

If those checks succeed, the probe reports hardware-virtualized CPU capability but intentionally leaves graphics `unverified` and returns `EXPERIMENTAL`. The following require later physical qualification with real guests:

- actual minimal guest boot and repeated clean lifecycle;
- renderer selection, virtio-gpu compatibility, scanout/buffer/fence ownership;
- touch/input mapping against real guest composition;
- frame latency, missed frames, cold boot, pause/resume, thermals, and sustained performance;
- audio, network/VPN behavior, storage exhaustion, process/surface loss, host update rehearsal;
- GrapheneOS-derived and Ubuntu Touch/Lomiri guest workflows.

## 8. Host platform integration

The files under `host/` are templates for an integrated Android/AOSP build. They are not an installer for arbitrary stock Android devices.

Before a host tuple can move beyond development status, integrate and review:

- the broker service process and platform IPC/Binder registration;
- caller identity and signing/permission relationship;
- SELinux domains, labels, file ownership, and runtime directories;
- AVF/crosvm API and executable compatibility for the pinned host source revision;
- service lifetime and process-death recovery;
- renderer and GPU access policy;
- storage/network policy and host-update compatibility.

The selected device, Android build fingerprint, kernel, AVF/APEX revision, CarbonHV commit, renderer, and guest builds must be recorded together as one qualification tuple.

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
