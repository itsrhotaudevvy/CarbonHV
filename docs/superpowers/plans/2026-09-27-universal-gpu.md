# Universal GPU Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver CarbonHV's vendor-neutral accelerated graphics path, qualify the connected Pixel 7a / Mali-G710 first, and preserve the same CVD1 guest ABI for later Snapdragon/Adreno support.

**Architecture:** Keep `virtio-gpu` as the guest-visible contract, model renderer selection and qualification in Rust, isolate renderer launch/handle ownership in the broker, and use a narrow Android native bridge for Vulkan/AHardwareBuffer/fence presentation into the existing `SurfaceView`. The first physical renderer profile is Rutabaga + gfxstream + host Vulkan on Mali; Adreno remains a later profile selected through the same capability and qualification interfaces.

**Tech Stack:** Rust 1.90+ / edition 2024, Android Kotlin/Compose, Android NDK + CMake, Vulkan, AHardwareBuffer/ANativeWindow/sync fences, virtio-gpu, crosvm Rutabaga/gfxstream, Bash qualification tooling.

**Spec:** `docs/superpowers/specs/2026-09-27-universal-gpu-design.md`

## Global Constraints

- CVD major 1 continues to expose `virtio-gpu`; Mali/Adreno identities never enter the guest ABI.
- Initial physical tuple: Pixel 7a / GS201 / Mali-G710 / Android 17.
- First renderer profile: `gfxstream-vulkan-mali`.
- Qualified accelerated mode may not use a full-frame CPU readback/upload loop.
- Zero-copy is claimed only when measured; one bounded GPU-side copy may qualify if the measured release targets still pass.
- Renderer output is untrusted; renderer and shared handles stay isolated from unrelated storage, credentials, and broker capabilities.
- No silent fallback from a qualified accelerated renderer to software rendering.
- Only physical-device tests may grant `gpu.vulkan`, `gpu.native-buffer`, `gpu.zero-copy`, `gpu.sustained`, or `gpu.qualified`.
- Compose stays outside the high-frequency frame path.
- Protected-VM graphics and physical GPU passthrough remain out of scope.

## Review Focus

- A stale surface/session generation must reject a frame or native handle instead of presenting it to a new session; pin this in Tasks 4 and 6.
- Missing/unknown GPU probe fields must remain unknown and must not silently become supported capabilities; pin this in Tasks 1 and 3.
- Renderer/profile selection must fail closed when a required Vulkan/native-buffer/sync capability is absent; pin this in Task 2.
- A renderer crash or Android surface recreation must not leak buffers/fences or reboot a healthy VM unnecessarily; pin this in Tasks 4 and 6.
- Qualification evidence from one Pixel build/driver/renderer tuple must not carry over after a material tuple field changes; pin this in Task 2.

---

### Task 1: Graphics capability model

**Files:**
- Create: `crates/carbon-core/src/graphics/mod.rs`
- Create: `crates/carbon-core/src/graphics/capabilities.rs`
- Modify: `crates/carbon-core/src/lib.rs`
- Create: `crates/carbon-core/tests/graphics_capabilities.rs`

**Interfaces:**
- Produces: `GpuHostCapabilities`, `GpuIdentity`, `GraphicsCapability`, `RendererFamily`, `HostGraphicsApi`, and `CapabilityEvidence<T>`.
- Consumes: no new project interfaces.

- [ ] **Step 1: Write failing capability-model tests**

Add tests named `unknown_probe_values_do_not_grant_capabilities`, `mali_identity_is_host_only_metadata`, and `capability_evidence_distinguishes_observed_from_qualified`. Assert that unknown Vulkan/native-buffer/sync fields do not yield accelerated capability flags and that GPU vendor/model are metadata rather than guest-contract values.

- [ ] **Step 2: Run the tests and verify RED**

Run: `cargo test -p carbon-core --test graphics_capabilities`  
Expected: FAIL because the graphics types/module do not exist.

- [ ] **Step 3: Implement the capability types**

Define:

```rust
pub enum CapabilityEvidence<T> { Unknown, Observed(T), Qualified(T) }
pub enum RendererFamily { Gfxstream, Venus, Virgl }
pub enum HostGraphicsApi { Vulkan, Gles }
pub struct GpuIdentity { pub vendor: String, pub renderer: String, pub driver_version: Option<String> }
pub enum GraphicsCapability { Basic, Gles, Vulkan, NativeBuffer, ExplicitSync, ZeroCopy, Sustained, Qualified }
pub struct GpuHostCapabilities { /* identity + evidence-backed host facts */ }
```

Keep `carbon-core` free of platform/FFI code and preserve `#![forbid(unsafe_code)]`.

- [ ] **Step 4: Run the tests and verify GREEN**

Run: `cargo test -p carbon-core --test graphics_capabilities`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/carbon-core
git commit -m "feat: add graphics capability model"
```

### Task 2: Renderer profiles, selection, and qualification tuple

**Files:**
- Create: `crates/carbon-core/src/graphics/selection.rs`
- Create: `crates/carbon-core/src/graphics/qualification.rs`
- Modify: `crates/carbon-core/src/graphics/mod.rs`
- Modify: `crates/carbon-core/src/lib.rs`
- Create: `crates/carbon-core/tests/graphics_selection.rs`
- Create: `crates/carbon-core/tests/graphics_qualification.rs`

**Interfaces:**
- Consumes: `GpuHostCapabilities`, `RendererFamily`, `HostGraphicsApi`, `GraphicsCapability`.
- Produces: `GpuRendererProfile`, `RendererSelectionMode`, `RendererSelection`, `select_renderer(...)`, `GpuQualificationTuple`, `QualificationRecord`, `qualification_applies(...)`.

- [ ] **Step 1: Write failing renderer-selection tests**

Cover: selecting `gfxstream-vulkan-mali` only when mandatory Vulkan/native-buffer/sync facts exist; rejecting missing required capabilities; developer mode explicitly allowing an unqualified compatible profile; release mode refusing it; no automatic software fallback.

- [ ] **Step 2: Run selection tests and verify RED**

Run: `cargo test -p carbon-core --test graphics_selection`  
Expected: FAIL because selection interfaces do not exist.

- [ ] **Step 3: Implement profile and deterministic selection**

Define:

```rust
pub struct GpuRendererProfile {
    pub id: String,
    pub family: RendererFamily,
    pub host_api: HostGraphicsApi,
    pub required: Vec<GraphicsCapability>,
    pub required_host_extensions: Vec<String>,
    pub supported_guest_apis: Vec<String>,
    pub buffer_transport: String,
    pub synchronization: String,
    pub sandbox_profile: String,
    pub quirks: Vec<String>,
}
pub enum RendererSelectionMode { Release, DeveloperExperimental }
pub enum RendererSelection { Selected(GpuRendererProfile), Experimental(GpuRendererProfile), Unavailable }
pub fn select_renderer(host: &GpuHostCapabilities, profiles: &[GpuRendererProfile], mode: RendererSelectionMode) -> RendererSelection;
```

The built-in Mali profile ID is exactly `gfxstream-vulkan-mali`.

- [ ] **Step 4: Write qualification-tuple tests**

Add tests proving that changing host fingerprint, kernel, AVF/APEX revision, GPU driver version, renderer version, guest version, CVD version, display mode, or workload version makes a previous record inapplicable.

- [ ] **Step 5: Implement tuple matching and verify GREEN**

Define `GpuQualificationTuple`, `QualificationRecord`, and:

```rust
pub fn qualification_applies(record: &QualificationRecord, current: &GpuQualificationTuple) -> bool;
```

Run: `cargo test -p carbon-core --test graphics_selection --test graphics_qualification`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/carbon-core
git commit -m "feat: select and qualify GPU renderer profiles"
```

### Task 3: Extend physical GPU probing and evidence output

**Files:**
- Modify: `qualification/carbon-qualify.sh`
- Modify: `qualification/test-dry-run.sh`
- Create: `qualification/test-adb-probe.sh`
- Modify: `qualification/README.md`

**Interfaces:**
- Produces line-oriented probe fields consumed by qualification tooling: `soc`, `gpu_renderer`, `egl_provider`, `vulkan_provider`, `vulkan_driver_version`, `kernel`, `build_fingerprint`, `avf_revision`, and observable display modes, plus explicit `unknown` values when not observable.
- Consumes: ADB shell/getprop/dumpsys facts only; it does not grant hardware graphics qualification.

- [ ] **Step 1: Write a failing fake-ADB probe test**

Use a PATH-injected fake `adb` in `qualification/test-adb-probe.sh`. Assert Mali-G710/GS201/provider fields are emitted when present and that omitted driver data prints `vulkan_driver_version=unknown` rather than `gpu.vulkan=true`.

- [ ] **Step 2: Run the probe test and verify RED**

Run: `bash qualification/test-adb-probe.sh`  
Expected: FAIL because the new fields are absent.

- [ ] **Step 3: Extend `emit_adb_probe`**

Collect only read-only device facts. Preserve the current final state `graphics=unverified` / `qualification=EXPERIMENTAL`; probing Vulkan/Mali does not itself qualify acceleration.

- [ ] **Step 4: Run qualification tests and the real Pixel probe**

Run:

```bash
bash qualification/test-dry-run.sh
bash qualification/test-adb-probe.sh
qualification/carbon-qualify.sh --adb 38161JEHN03423
```

Expected: fixture tests PASS; the connected Pixel reports GS201/Mali-G710 and remains `EXPERIMENTAL` until renderer/display proof exists.

- [ ] **Step 5: Commit**

```bash
git add qualification
git commit -m "feat: probe Android GPU qualification facts"
```

### Task 4: Broker renderer launch and scoped handle ownership

**Files:**
- Create: `crates/carbon-broker/src/graphics/mod.rs`
- Create: `crates/carbon-broker/src/graphics/launch.rs`
- Create: `crates/carbon-broker/src/graphics/handles.rs`
- Modify: `crates/carbon-broker/src/lib.rs`
- Create: `crates/carbon-broker/tests/graphics.rs`

**Interfaces:**
- Consumes: selected `GpuRendererProfile`, existing `ProgramInvocation`, VM/session generation identity.
- Produces: `RendererLaunchPlan`, `RendererLaunchError`, `GraphicsHandleLeaseTable`, `GraphicsHandleToken`.

- [ ] **Step 1: Write failing broker graphics tests**

Cover fixed-program argv generation with no shell fragments; reject unsafe runtime paths; scope every handle to instance + VM generation + presentation generation; reject stale generations; successful renderer crash cleanup invalidates all outstanding leases.

- [ ] **Step 2: Run the tests and verify RED**

Run: `cargo test -p carbon-broker --test graphics`  
Expected: FAIL because graphics broker interfaces do not exist.

- [ ] **Step 3: Implement typed launch planning and handle leases**

Define:

```rust
pub struct RendererLaunchPlan { pub invocation: ProgramInvocation, pub runtime_dir: PathBuf, pub profile_id: String }
pub fn build_renderer_launch_plan(profile: &GpuRendererProfile, runtime_dir: PathBuf) -> Result<RendererLaunchPlan, RendererLaunchError>;
pub struct GraphicsHandleLeaseTable { /* generation-scoped leases */ }
```

Do not add a generic command or arbitrary environment-variable API.

- [ ] **Step 4: Verify GREEN and existing broker invariants**

Run: `cargo test -p carbon-broker --test graphics --test protocol --test avf`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/carbon-broker
git commit -m "feat: isolate renderer launch and GPU handles"
```

### Task 5: Add Android native graphics probe bridge

**Files:**
- Modify: `android/app/build.gradle.kts`
- Create: `android/app/src/main/cpp/CMakeLists.txt`
- Create: `android/app/src/main/cpp/carbon_graphics_jni.cpp`
- Create: `android/app/src/main/cpp/gpu_probe.h`
- Create: `android/app/src/main/cpp/gpu_probe.cpp`
- Create: `android/app/src/main/kotlin/dev/carbonhv/session/GraphicsNative.kt`
- Create: `android/app/src/test/kotlin/dev/carbonhv/session/GraphicsNativeContractTest.kt`

**Interfaces:**
- Produces Kotlin `GraphicsNative.probe(): NativeGpuProbe`.
- Native bridge exposes only structured probe fields; no arbitrary native call dispatcher.

- [ ] **Step 1: Write the failing Kotlin contract test**

Assert `NativeGpuProbe` can represent GPU name, Vulkan API/driver version, external-memory support, explicit-sync support, AHardwareBuffer support, protected-memory support, supported native-buffer formats/usages, display refresh modes, and unknown values without converting unknown to true.

- [ ] **Step 2: Run the unit test and verify RED**

Run:

```bash
export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
export GRADLE_USER_HOME="$PWD/.gradle-local"
./gradlew --no-daemon :android:app:testDebugUnitTest --tests dev.carbonhv.session.GraphicsNativeContractTest
```

Expected: FAIL because the contract does not exist.

- [ ] **Step 3: Add the NDK/CMake module and narrow JNI probe**

Implement Vulkan loader probing and Android hardware-buffer capability observation in native code. JNI returns a structured result to Kotlin; probe errors map to unknown/unavailable fields and never crash app startup.

- [ ] **Step 4: Build the native library and run tests**

Run: `./gradlew --no-daemon :android:app:testDebugUnitTest :android:app:assembleDebug`  
Expected: BUILD SUCCESSFUL and the APK contains the CarbonHV graphics native library for arm64-v8a.

- [ ] **Step 5: Commit**

```bash
git add android/app
git commit -m "feat: add Android native GPU probe"
```

### Task 6: Surface, buffer, fence, and generation state machine

**Files:**
- Create: `android/app/src/main/kotlin/dev/carbonhv/session/GraphicsSession.kt`
- Modify: `android/app/src/main/kotlin/dev/carbonhv/session/ImmersiveSessionScreen.kt`
- Create: `android/app/src/test/kotlin/dev/carbonhv/session/GraphicsSessionTest.kt`
- Create: `android/app/src/main/cpp/presentation.h`
- Create: `android/app/src/main/cpp/presentation.cpp`
- Modify: `android/app/src/main/cpp/carbon_graphics_jni.cpp`

**Interfaces:**
- Consumes: existing `GuestSurfaceCallbacks`, session identity/generation, native graphics bridge.
- Produces: `GraphicsSession.attachSurface(surface, width, height)`, `resizeSurface(...)`, `detachSurface()`, and generation-scoped `presentFrame(...)`.

- [ ] **Step 1: Write failing lifecycle/state tests**

Cover attach → resize → detach; stale presentation generation rejection; surface recreation increments presentation generation without changing VM generation; queue depth remains bounded; obsolete frames are dropped; crash cleanup releases all logical frame leases.

- [ ] **Step 2: Run and verify RED**

Run: `./gradlew --no-daemon :android:app:testDebugUnitTest --tests dev.carbonhv.session.GraphicsSessionTest`  
Expected: FAIL because `GraphicsSession` does not exist.

- [ ] **Step 3: Implement the Kotlin state machine**

Define:

```kotlin
class GraphicsSession {
    fun attachSurface(surface: Surface, width: Int, height: Int): Long
    fun resizeSurface(surface: Surface, width: Int, height: Int): Long
    fun detachSurface()
    fun presentFrame(frame: NativeFrame, presentationGeneration: Long): PresentResult
}
```

Wire `ImmersiveSessionScreen` callbacks through this class while keeping rendering out of Compose state.

- [ ] **Step 4: Implement native presentation ownership**

Use `ANativeWindow`/AHardwareBuffer-compatible ownership and explicit acquire/release synchronization primitives. Validate dimensions/format/usage and never consume a stale-generation frame. Do not add CPU framebuffer readback as a fallback.

- [ ] **Step 5: Verify lifecycle and APK build**

Run: `./gradlew --no-daemon :android:app:testDebugUnitTest :android:app:assembleDebug`  
Expected: BUILD SUCCESSFUL.

- [ ] **Step 6: Commit**

```bash
git add android/app
git commit -m "feat: add generation-safe GPU presentation"
```

### Task 7: Integrate Rutabaga/gfxstream as the Mali renderer profile

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/carbon-renderer/Cargo.toml`
- Create: `crates/carbon-renderer/src/lib.rs`
- Create: `crates/carbon-renderer/src/profile.rs`
- Create: `crates/carbon-renderer/src/rutabaga.rs`
- Create: `crates/carbon-renderer/tests/profile.rs`
- Modify: `crates/carbon-broker/src/graphics/launch.rs`
- Modify: `docs/BUILDING.md`

**Interfaces:**
- Consumes: `GpuRendererProfile`, broker `RendererLaunchPlan`.
- Produces: a renderer process/library entry point for profile `gfxstream-vulkan-mali`; no renderer-specific API is exposed to the Android UI.

- [ ] **Step 1: Pin the upstream renderer dependency before code changes**

Record the exact crosvm/Rutabaga/gfxstream source revision used by the implementation in `docs/BUILDING.md` and dependency metadata. Do not track a moving branch.

- [ ] **Step 2: Write failing profile tests**

Assert the Mali renderer requests Vulkan, refuses a host without mandatory native-buffer/explicit-sync capabilities, maps to one fixed renderer executable/entry point, and selects a sandbox profile with no generic network or unrelated host-filesystem access.

- [ ] **Step 3: Run and verify RED**

Run: `cargo test -p carbon-renderer --test profile`  
Expected: FAIL until the crate/profile exists.

- [ ] **Step 4: Implement the minimal Rutabaga/gfxstream adapter**

Configure virtio-gpu/Rutabaga/gfxstream for the minimal CVD1 display contract. Keep unsafe/upstream FFI or platform glue inside this renderer/native boundary, not `carbon-core`.

- [ ] **Step 5: Verify Rust workspace**

Run: `cargo test --workspace`  
Expected: PASS with the renderer crate included.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/carbon-renderer crates/carbon-broker docs/BUILDING.md
git commit -m "feat: add gfxstream Mali renderer backend"
```

### Task 8: Minimal accelerated guest workload and proof harness

**Files:**
- Create: `qualification/gpu/README.md`
- Create: `qualification/gpu/minimal-guest/README.md`
- Create: `qualification/gpu/run-minimal.sh`
- Create: `qualification/gpu/collect-report.sh`
- Create: `qualification/gpu/fixtures/mali-pass.env`
- Create: `qualification/gpu/schema/report-v1.txt`
- Modify: `scripts/verify.sh`

**Interfaces:**
- Consumes: selected renderer profile and existing AVF launch path.
- Produces: reproducible report fields for tuple identity, renderer identity, presented frames, GPU proof, copy inventory, frame timings, and final capability state.

- [ ] **Step 1: Write a failing dry-run harness test**

Add a fixture mode that asserts the report contains every exact tuple field from the spec and cannot emit `gpu.vulkan` or stronger without a positive hardware-render evidence field.

- [ ] **Step 2: Run and verify RED**

Run: `bash qualification/gpu/run-minimal.sh --fixture qualification/gpu/fixtures/mali-pass.env --dry-run`  
Expected: FAIL until the harness exists.

- [ ] **Step 3: Implement the minimal ARM64 graphics workload harness**

The workload must render through the guest virtual GPU rather than a host-only Android test surface. Collect enough evidence to distinguish host GPU execution from software rendering.

- [ ] **Step 4: Add harness checks to repository verification**

Extend `scripts/verify.sh` with deterministic fixture-only GPU harness tests; physical ADB execution stays opt-in.

- [ ] **Step 5: Verify**

Run: `scripts/verify.sh`  
Expected: `CarbonHV verification: PASS`.

- [ ] **Step 6: Commit**

```bash
git add qualification/gpu scripts/verify.sh
git commit -m "test: add GPU qualification harness"
```

### Task 9: Pixel 7a Mali hardware bring-up

**Files:**
- Modify: `qualification/gpu/run-minimal.sh`
- Create: `qualification/gpu/reports/.gitkeep`
- Modify: `docs/SUPPORT_MATRIX.md`

**Interfaces:**
- Consumes: Pixel probe, AVF backend, `gfxstream-vulkan-mali`, minimal accelerated workload, presentation backend.
- Produces: a recorded Pixel qualification report; capability level is whatever the evidence actually demonstrates.

- [ ] **Step 1: Reconfirm the exact physical tuple**

Run the extended ADB probe and record Android fingerprint, kernel, AVF/APEX revision, GS201, Mali-G710, Vulkan/EGL providers, and driver version where observable.

- [ ] **Step 2: Boot the minimal graphics guest through AVF**

Run `qualification/gpu/run-minimal.sh --adb 38161JEHN03423 --renderer gfxstream-vulkan-mali`. The run must use hardware virtualization and the CarbonHV renderer path rather than a host-only demo.

- [ ] **Step 3: Prove GPU execution and presentation**

Capture renderer/GPU evidence plus frame report, verify output reaches the CarbonHV `SurfaceView`, and record every full-frame copy. If proof is incomplete, keep the capability state experimental and record the blocker rather than promoting it.

- [ ] **Step 4: Exercise surface lifecycle**

On-device: rotate, background/foreground, destroy/recreate the activity surface, and repeat. Expected: no VM reboot solely because the Android surface changed, no stale frame presentation, and no leaked/ever-growing buffer queue.

- [ ] **Step 5: Commit only reproducible non-sensitive evidence**

Update `docs/SUPPORT_MATRIX.md` with the demonstrated tier and report format/location. Do not commit device-private logs or identifiers unnecessary for reproducing the result.

```bash
git add qualification/gpu docs/SUPPORT_MATRIX.md
git commit -m "test: qualify initial Mali graphics path"
```

### Task 10: Real guest graphics and sustained Mali qualification

**Files:**
- Modify: `guest/android/device/cvd1.toml`
- Modify: `guest/android/patches/README.md`
- Modify: `guest/ubuntu-touch/device/cvd1.toml`
- Modify: `guest/ubuntu-touch/patches/README.md`
- Create: `qualification/gpu/workloads/README.md`
- Create: `qualification/gpu/run-sustained.sh`
- Create: `qualification/gpu/profiles/pixel-7a-mali-g710.toml`
- Modify: `docs/SUPPORT_MATRIX.md`
- Modify: `README.md`

**Interfaces:**
- Consumes: the same CVD1 `virtio-gpu` contract, renderer/presentation stack, and qualification harness for both guests.
- Produces: guest-specific compatibility evidence and, only if all mandatory tests pass, `gpu.sustained` / `gpu.qualified` for the exact Pixel tuple.

- [ ] **Step 1: Lock Android-derived guest graphics requirements**

Document and configure the compatible virtual gralloc/HWC/EGL/Vulkan stack without inserting Mali-specific guest drivers. Build/boot evidence must identify the guest bundle revision.

- [ ] **Step 2: Lock Ubuntu Touch/Lomiri graphics requirements**

Document and configure the Mesa/Mir path against the same CVD1 `virtio-gpu` device. Do not substitute a desktop shell as proof of Lomiri support.

- [ ] **Step 3: Run mandatory interactive workload classes**

Exercise static composition, continuous scrolling, browser/WebView or equivalent browser rendering, rapid-content updates, GLES, Vulkan where the guest exposes it, multi-touch during animation, rotation, surface recreation, background/foreground, and renderer/context-loss failure injection where supported.

- [ ] **Step 4: Run sustained qualification**

Run `qualification/gpu/run-sustained.sh --adb 38161JEHN03423 --minutes 30`. Record FPS, p50/p95/p99 frame time, dropped frames, queue depth, the defined input-to-visible-response measurement, CPU utilization, renderer/buffer memory, copy inventory, thermal status, available power/energy evidence, and recovery results.

- [ ] **Step 5: Derive release thresholds from measured baseline**

Create `qualification/gpu/profiles/pixel-7a-mali-g710.toml` with explicit acceptance thresholds derived from the observed baseline. Re-run the sustained test against those thresholds; do not invent thresholds before measurement.

- [ ] **Step 6: Update product status truthfully**

Mark only demonstrated capabilities in `docs/SUPPORT_MATRIX.md` and `README.md`. If either real guest or sustained/recovery testing fails, retain `EXPERIMENTAL` and record the failed gate.

- [ ] **Step 7: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace
git diff --check
export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
export GRADLE_USER_HOME="$PWD/.gradle-local"
scripts/verify.sh
```

Expected: all host-independent tests pass and `CarbonHV verification: PASS`. Physical qualification status must match the recorded evidence rather than the repository test result alone.

- [ ] **Step 8: Commit**

```bash
git add guest qualification/gpu docs/SUPPORT_MATRIX.md README.md
git commit -m "feat: qualify Mali graphics for starter guests"
```

## Later Snapdragon / Adreno follow-up

The Snapdragon device is intentionally not required by this plan. When connected, create a separate bounded qualification/renderer-profile plan that reuses `GpuHostCapabilities`, `GpuRendererProfile`, `GpuQualificationTuple`, the broker handle model, the Android presentation backend, and CVD1 unchanged. Compare gfxstream and Mesa/Venus with the same workload/metrics harness; add Adreno-specific quirks only where measured evidence requires them.
