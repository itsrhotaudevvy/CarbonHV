# CarbonHV Platform Runtime Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the broker-authoritative graphical VM identity and AVF launch plan cross the Android Binder boundary into `CarbonHvHostIntegration`, and package the Rust broker plus required service/SELinux contracts for a controlled Android 17 `system_ext` build.

**Architecture:** Keep validation and `(instanceId, generation)` ownership in `carbon-broker`. Convert the already-validated `AvfPlatformVmPlan` into a small platform-neutral runtime publication DTO, then use an Android-only Rust Binder adapter to call `dev.carbonhv.host.ICarbonHvHostRuntime`. Build the same tested Rust broker sources with Soong; keep the ordinary UI APK outside this privileged runtime interface.

**Tech Stack:** Rust 1.98, Android Binder Rust backend, AIDL, Soong, Android 17 `system_ext`, SELinux policy.

**Spec:** `docs/ARCHITECTURE.md`

## Global Constraints

- ARM64 hardware virtualization remains mandatory; no CPU-emulation fallback.
- `carbonhvd` remains the source of truth for live VM generation and display ownership.
- The UI AIDL and privileged runtime AIDL remain separate.
- Graphical execution uses Android 17 AVF/crosvm gfxstream; no separate CarbonHV renderer process is introduced.
- Managed guest artifacts remain under `/data/misc/carbonhv/objects`.
- Pixel 7a/Mali qualification remains EXPERIMENTAL until a real guest renders through the physical path.

## Review Focus

- A generation larger than Android AIDL `long` must be rejected before Binder publication.
- A managed path that cannot be represented as UTF-8 must never be lossy-converted into a different host path.
- Binder start publication must preserve every broker-controlled disk/resource/display field exactly.
- A stale stop must never be translated into a stop for another generation.
- SELinux service-manager ownership must allow only the dedicated host integration domain to add the runtime service and `carbonhvd` to find/call it.

---

### Task 1: Broker-owned runtime publication contract

**Files:**
- Create: `crates/carbon-broker/src/host_runtime.rs`
- Modify: `crates/carbon-broker/src/lib.rs`
- Test: `crates/carbon-broker/tests/host_runtime.rs`

**Interfaces:**
- Consumes: `DisplaySessionIdentity`, `AvfPlatformVmPlan`.
- Produces: `HostRuntimeGraphicalVmRequest::try_from_plan(identity, plan) -> Result<Self, HostRuntimeRequestError>` and `HostRuntimePublisher` start/stop methods.

- [ ] Write tests that assert exact identity, kernel/initrd, disks, memory, vCPU, kernel params, display values, and generation preservation; add overflow/non-UTF-8 rejection cases.
- [ ] Run `cargo test -p carbon-broker --test host_runtime` and verify RED because the runtime publication API does not exist.
- [ ] Implement the DTO, fallible conversion, and publisher trait with no Android dependencies.
- [ ] Run the task test and full `cargo test`; verify GREEN.

### Task 2: Android Rust Binder publisher and Soong packaging

**Files:**
- Create: `crates/carbon-broker/src/android_host_runtime.rs`
- Modify: `crates/carbon-broker/src/lib.rs`
- Modify: `crates/carbon-broker/src/main.rs`
- Create: `Android.bp`
- Create: `host/android/test-rust-runtime-binder-contract.sh`
- Modify: `scripts/verify.sh`

**Interfaces:**
- Consumes: Task 1 `HostRuntimePublisher` and generated `dev.carbonhv.host.runtime-rust` AIDL bindings.
- Produces: `AndroidHostRuntimePublisher::connect()` and Binder-backed `start_graphical_vm` / `stop_graphical_vm`; Soong modules for `carbon_core`, `carbon_protocol`, `carbon_broker`, and `carbonhvd`.

- [ ] Write a shell contract test that fails until the Android-only source uses the generated Rust AIDL interface, service name `dev.carbonhv.host.runtime`, fallible integer conversion, and Soong links `libbinder_rs` plus `dev.carbonhv.host.runtime-rust`.
- [ ] Run the contract test and verify RED.
- [ ] Implement the Binder adapter and Soong modules while keeping normal host Cargo builds free of Android Binder dependencies.
- [ ] Make `carbonhvd` fail explicitly on non-Android hosts and initialize the Android Binder runtime publisher on Android; do not add a second launch authority.
- [ ] Run the contract test, `cargo test`, `cargo fmt --all -- --check`, and `git diff --check`; verify GREEN.

### Task 3: Service-manager and SELinux ownership contract

**Files:**
- Create: `host/service_contexts`
- Create: `host/file_contexts`
- Create: `host/seapp_contexts`
- Create: `host/carbonhv_host_app.te`
- Modify: `host/carbonhvd.te`
- Create: `host/test-platform-policy-contract.sh`
- Modify: `scripts/verify.sh`
- Modify: `host/README.md`
- Modify: `docs/BUILDING.md`
- Modify: `docs/SECURITY.md`

**Interfaces:**
- Consumes: runtime service name and `system_ext` package identity.
- Produces: dedicated service-manager type, daemon executable/data labels, dedicated host-app SELinux domain, and explicit add/find/Binder-call rules.

- [ ] Write a shell contract test for the service-context mapping, daemon executable/data labels, dedicated app-domain mapping, and least-scope add/find/call permissions; verify RED.
- [ ] Implement the policy files and replace the current template comments with concrete policy for the CarbonHV-owned service/data boundary; leave AVF-internal device/service permissions tied to the selected Android 17 product policy.
- [ ] Update integration docs to distinguish the now-wired CarbonHV Binder/service-manager path from still-unqualified physical AVF/GPU permissions.
- [ ] Run both shell contract tests and `scripts/verify.sh`; verify GREEN.

### Task 4: Physical Pixel readiness check without qualification promotion

**Files:**
- Modify only if evidence requires it: `docs/SUPPORT_MATRIX.md`, `qualification/gpu/reports/pixel-7a-mali-g710-2026-09-27.txt`.

**Interfaces:**
- Consumes: Tasks 1-3 source/build contracts and the connected Pixel 7a.
- Produces: fresh evidence on whether the stock Pixel can exercise the controlled-host integration; no qualification promotion without a real guest-rendered frame.

- [ ] Re-run `adb` host facts and confirm the device/build tuple is unchanged or record the new tuple.
- [ ] Verify that stock firmware still lacks the newly built `CarbonHvHostIntegration`/`carbonhvd` product integration; do not attempt to mutate `/system_ext` on the stock user build.
- [ ] Run full repository verification and `git diff --check`.
- [ ] Keep Mali status EXPERIMENTAL unless an actual GrapheneOS-derived or Ubuntu Touch guest renders through gfxstream into the CarbonHV Surface and input path.
