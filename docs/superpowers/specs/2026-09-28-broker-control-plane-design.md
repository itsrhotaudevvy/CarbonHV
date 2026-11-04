# CarbonHV Broker Control Plane Design

## Intent

Complete the Android control path so the ordinary CarbonHV app can request VM lifecycle operations without gaining AVF privileges or becoming the source of truth for runtime state. `carbonhvd` remains authoritative for VM generations, resource admission, managed storage references, the single graphical display slot, and launch/stop ordering. `CarbonHvHostIntegration` remains the only process that touches Android 17 internal AVF/display APIs.

This design continues the already-selected native architecture: Android Binder/AIDL between Android components, Rust for the broker, and the existing platform-signed `system_ext` host integration for the narrow privileged AVF bridge.

## Considered approaches

### Direct ordinary-app Binder access to `carbonhvd`

This gives the shortest logical path, but an SDK application cannot use the hidden ServiceManager path cleanly without JNI/native Binder glue. The daemon would also need to reproduce Android PackageManager signing-certificate verification to authenticate the dynamic app UID. That duplicates the caller policy already implemented in the host integration.

### Host integration as authenticated transport — selected

The ordinary app keeps binding to `CarbonHvHostService`. That service already obtains the real Binder calling UID and verifies the current package signing certificate against `ro.carbonhv.ui_signing_sha256`. After that check it forwards only a high-level lifecycle request to a new native Binder service published by `carbonhvd` as `dev.carbonhv.broker`.

The host app cannot turn a request into paths or launch parameters. `carbonhvd` resolves the bundle profile and managed object IDs, performs broker validation, owns generation/display state, builds the graphical AVF plan, and invokes `dev.carbonhv.host.runtime`. SELinux permits only `carbonhv_host_app` to find/call `dev.carbonhv.broker`, while the native service also checks that Binder's calling UID is Android system UID.

This reuses the existing authentication boundary, preserves an ordinary unprivileged UI APK, and keeps all launch authority in Rust.

### Unix-domain socket control service

This would be straightforward for Rust, but it adds a second IPC/lifecycle model beside Binder, complicates Android identity enforcement, and provides no advantage for the controlled platform build. It is rejected for the Android product path.

## Interfaces

### UI service

`ICarbonHvHostService` gains high-level methods for graphical start, stop, and authoritative live-generation lookup. A start request contains:

- client request ID for idempotency;
- instance ID;
- starter bundle ID (`grapheneos` or `ubuntu-touch` initially);
- requested memory and vCPU count;
- display width, height, density, and refresh rate.

It does not contain host file paths, kernel parameters, AVF service names, executables, or arbitrary managed object IDs.

Every method continues through `HostCallerPolicy.enforceUiCaller()` before forwarding.

### Broker service

An internal AIDL interface `dev.carbonhv.broker.ICarbonHvBroker` is built with Java and Rust backends and published as `dev.carbonhv.broker` by `carbonhvd`.

It exposes:

- `startGraphicalVm(BrokerStartRequest) -> long generation`;
- `stopVm(instanceId, generation, requestId)`;
- `activeDisplayGeneration(instanceId) -> long`, with `-1` meaning no live display session;
- `notifyRuntimeDied(instanceId, generation)`, callable by the host integration when its AVF VM callback reports death.

The service accepts calls only from UID 1000 and is additionally protected by SELinux service-manager/Binder rules. The original UI identity is authenticated by the host bridge; the native broker treats that bridge as the trusted caller boundary.

## Starter bundle resolution

The first control-plane implementation uses a broker-owned bootstrap profile table for the two requested starter families. The UI sends a bundle ID, and the daemon resolves only fixed managed-object names under `/data/misc/carbonhv/objects` plus an instance-specific writable-disk name derived from the already-validated instance ID.

The profile table is intentionally narrow and temporary infrastructure for the first real boot. It must later be replaced by the signed installed-bundle catalog, but that replacement does not change the Binder authority boundary.

The daemon never accepts an arbitrary path over UI or broker Binder.

## Start transaction

1. The ordinary app calls the bound host service.
2. The host service verifies the caller UID/package signing certificate.
3. The host service forwards the high-level request to `dev.carbonhv.broker`.
4. The Rust Binder service verifies Binder calling UID 1000.
5. `carbonhvd` resolves the bundle profile and chooses the next broker generation.
6. `Broker::start` validates identity, resource caps, managed object IDs, and request idempotency.
7. `Broker::claim_display_session` reserves the single Android crosvm display slot.
8. `AvfBackend::build_graphics_platform_plan` builds the exact gfxstream/display/input intent.
9. The plan is converted to `HostRuntimeGraphicalVmRequest` without lossy path conversion.
10. `AndroidHostRuntimePublisher` calls `dev.carbonhv.host.runtime`.
11. Only after the host runtime accepts the launch does the start return the generation to the UI.

If any step after broker admission fails, CarbonHV invalidates that attempted generation, releases the display slot, and removes the running entry. A failed generation is never silently reused.

## Stop and death transactions

For a UI stop, `carbonhvd` first validates that the requested generation is the live display owner. It asks the host runtime to stop that exact generation, then commits the broker stop. A runtime stop failure leaves broker state live so the caller cannot accidentally lose authority over an unknown-running VM.

When the host AVF callback reports VM death, `CarbonHvHostIntegration` notifies `dev.carbonhv.broker`. The daemon reconciles only an exact `(instanceId, generation)` match, releases the display slot, removes the running entry, and advances the minimum generation. Stale death notifications are ignored.

## Daemon restart behavior

The first implementation is fail-closed. `carbonhvd` does not reconstruct a running VM from UI/database state after losing its in-memory broker state. A later persistence/recovery slice will add durable runtime journals. Until then, a controlled host build must treat broker-process restart as a recovery event and ensure any orphaned host runtime is stopped before accepting a new start.

## Security boundaries

- Ordinary `dev.carbonhv` remains an SDK app with no direct AVF/internal-ServiceManager access.
- UI certificate authentication remains in the platform host integration, which can query PackageManager safely.
- `dev.carbonhv.broker` is a separate service-manager type from `dev.carbonhv.host.runtime`.
- Only `carbonhv_host_app` may find/call the broker service; only `carbonhvd` may add it.
- Only `carbonhvd` may find/call the host runtime service; only `carbonhv_host_app` may add it.
- No lifecycle AIDL accepts a host path, executable, shell command, or Binder service name.
- Generation checks are performed in the broker before display/runtime actions.

## Testing

Host-independent Rust tests cover successful start publication, failed-publication rollback, generation advancement, exact stop ordering, stale runtime-death notifications, and publisher failures. AIDL contract tests compile the new interface through the local Android AIDL compiler and pin both Java/Rust Soong backends. Kotlin host-contract tests verify UI caller checks are applied to start/stop/query forwarding. Shell policy tests pin service contexts and direction-specific Binder rules.

The normal repository verifier must remain green. None of these source tests upgrades the Pixel Mali profile: promotion still requires a controlled Android image, real guest artifacts, a guest-rendered frame through gfxstream, input proof, and sustained GPU qualification.
