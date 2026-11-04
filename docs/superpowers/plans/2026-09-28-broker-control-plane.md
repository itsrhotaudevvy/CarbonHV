# CarbonHV Broker Control Plane Implementation Plan

**Goal:** Connect the ordinary Android app's authenticated host-service boundary to the Rust broker, and make broker-owned graphical start/stop state drive the existing privileged AVF runtime Binder.

**Design:** `docs/superpowers/specs/2026-09-28-broker-control-plane-design.md`

**Stack:** Rust, Android Binder Rust backend, AIDL Java/Rust backends, Kotlin platform service, Soong, SELinux.

## Task 1 — Transactional graphical runtime orchestration

- [ ] Add failing Rust tests for broker-selected generation, start publication, rollback/invalidation on publisher failure, validated stop ordering, and stale runtime-death reconciliation.
- [ ] Add broker APIs needed to select the next generation and reconcile exact runtime death.
- [ ] Add a platform-neutral graphical runtime coordinator around `Broker`, `AvfBackend`, and `HostRuntimePublisher`.
- [ ] Run focused tests, full `cargo test`, formatting, and `git diff --check`.

## Task 2 — Native broker AIDL service

- [ ] Add internal broker AIDL parcelables/interface with high-level bundle intent only; compile them with the local AIDL tool as a RED/GREEN contract.
- [ ] Add Soong Java/Rust backends and daemon dependencies.
- [ ] Implement Android Rust Binder server with UID-1000 enforcement, starter-bundle profile resolution, active-generation query, start/stop, and runtime-death reconciliation.
- [ ] Change `carbonhvd` main from parked client to registered Binder service/thread pool with lazy host-runtime connection.
- [ ] Keep host Cargo builds Android-dependency-free through cfg gating and run all host tests/contracts.

## Task 3 — Host integration forwarding boundary

- [ ] Extend the UI AIDL with a high-level graphical start request plus start/stop methods.
- [ ] Add a platform broker client in `CarbonHvHostIntegration` using `ServiceManager.waitForService("dev.carbonhv.broker")`.
- [ ] Enforce the existing UI caller certificate policy before every control/query call, then forward to the broker.
- [ ] Notify the broker when the host AVF runtime reports exact-generation VM death.
- [ ] Add host-contract/unit tests and AIDL source contracts.

## Task 4 — SELinux/service-manager directionality

- [ ] Add `dev.carbonhv.broker` service context/type.
- [ ] Permit only `carbonhvd` to add the broker service and only `carbonhv_host_app` to find/call it.
- [ ] Extend the platform-policy contract to assert both Binder directions and never mix their service types.
- [ ] Update security/build docs with the completed control path and remaining product-build gate.

## Task 5 — Ordinary app control client

- [ ] Add a small SDK-side control client over the existing bound `ICarbonHvHostService`.
- [ ] Make authoritative generation lookup come from the Rust broker path.
- [ ] Wire lifecycle calls into the session flow without storing host paths or privileged details in app state.
- [ ] Add reducer/client tests for success, unavailable host integration, stale generation, and runtime failure/recovery presentation.

## Task 6 — Verification and physical readiness

- [ ] Run all Rust, Kotlin, AIDL, shell-policy, and APK verification.
- [ ] Re-run read-only Pixel checks to confirm stock firmware still cannot execute the controlled path.
- [ ] Keep Mali `EXPERIMENTAL` until a controlled host image plus real guest artifacts produce physical gfxstream frames.
