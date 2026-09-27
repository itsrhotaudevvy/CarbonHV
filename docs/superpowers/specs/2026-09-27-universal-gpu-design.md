# CarbonHV Universal GPU Architecture

**Date:** 2026-09-27  
**Status:** Approved design, implementation not started  
**Initial qualification target:** Pixel 7a / GS201 / Mali-G710 / Android 17  
**Future qualification target:** Snapdragon / Adreno devices without changing the guest ABI

## 1. Purpose

CarbonHV needs a graphics architecture that can deliver accelerated local guest rendering without coupling the guest contract to one Android GPU vendor. The first physical qualification target is the connected Pixel 7a, which exposes a Mali-G710 and Android Vulkan/EGL providers named `mali`.

The design therefore separates:

1. the stable guest-visible virtual GPU contract;
2. the renderer implementation used to decode guest graphics;
3. the Android host GPU provider;
4. native-buffer presentation into CarbonHV's `SurfaceView`;
5. qualification evidence for one exact device/build/driver/renderer tuple.

The first optimized path uses virtio-gpu plus Rutabaga/gfxstream on Vulkan for Mali. A future Adreno profile may use gfxstream or a Mesa/Venus route without changing the CVD1 guest-visible device contract.

## 2. Goals

- Accelerated ARM64 guest graphics on Android hosts with supported Vulkan-capable GPUs.
- A vendor-neutral CVD1 graphics ABI based on `virtio-gpu`.
- A Vulkan-first Mali path for Pixel 7a.
- Native Android presentation through `SurfaceView` and SurfaceFlinger.
- Explicit synchronization and bounded buffer ownership.
- Zero full-frame CPU readback in qualified accelerated mode.
- A renderer/provider boundary that allows later Adreno qualification.
- Independent qualification for Android-derived and Ubuntu Touch/Lomiri guests.
- Measured, truthful capability reporting instead of model-name assumptions.

## 3. Non-goals

- Physical GPU passthrough.
- Vendor-specific guest drivers for Mali or Adreno.
- Cross-ISA graphics emulation.
- Claiming zero-copy before it is measured.
- Treating a successful triangle as sufficient qualification.
- Supporting every Android GPU in the first implementation.
- Protected-VM graphics in this phase.

## 4. Architecture

The data path is:

```text
guest app
  -> guest GLES/Vulkan implementation
  -> guest compositor
  -> virtio-gpu protocol
  -> Rutabaga renderer boundary
  -> selected renderer backend
  -> host Vulkan/EGL provider
  -> native Android buffer
  -> CarbonHV presentation backend
  -> SurfaceView
  -> SurfaceFlinger
```

The guest never sees a Mali- or Adreno-specific device. Vendor knowledge exists only in host probing, renderer selection, quirks, and qualification records.

### 4.1 Stable virtual GPU contract

CVD1 continues to expose `virtio-gpu`. Renderer implementation details are not added to the guest contract. The host may negotiate optional capabilities, but a renderer change must not require a guest image rebuild unless the guest-side protocol requirements themselves change.

The stable contract includes:

- scanout/display identity;
- display geometry and refresh metadata;
- resource creation and destruction;
- guest graphics command transport;
- synchronization semantics;
- display resize/reconnect semantics;
- capability negotiation with an explicit protocol version.

### 4.2 Renderer boundary

CarbonHV introduces a renderer abstraction above the concrete graphics implementation.

`GpuRendererProfile` describes:

- renderer family: `gfxstream`, `venus`, or `virgl`;
- host API: Vulkan preferred, EGL/GLES only when required;
- required host Vulkan/EGL extensions;
- required virtio-gpu/Rutabaga features;
- supported guest API levels;
- buffer-sharing method;
- synchronization method;
- renderer sandbox requirements;
- known device/driver quirks.

The first profile is `gfxstream-vulkan-mali`.

Rutabaga is the preferred host abstraction because it keeps the VMM-facing virtual GPU device independent from the renderer backend. CarbonHV must not expose raw renderer-specific command surfaces through its privileged broker API.

### 4.3 Host provider probing

`GpuHostProbe` produces facts, not policy.

Required observations:

- SoC and device identity;
- GPU renderer/device name;
- EGL implementation;
- Vulkan implementation;
- Vulkan API version;
- Vulkan driver version;
- external-memory support;
- external semaphore/fence support;
- native-buffer import/export support;
- supported image formats and usage flags;
- protected-memory capability recorded separately;
- display refresh modes;
- Android graphics composition backend where observable.

Probe failures produce unknown/unavailable fields rather than invented defaults.

### 4.4 Presentation backend

`GpuPresentationBackend` owns the boundary between renderer output and the Android surface.

Responsibilities:

- attach/detach an Android `Surface`;
- import or receive renderer-owned native buffers;
- validate format, dimensions, stride, usage and generation;
- honor acquire fences before reading/presenting a buffer;
- return release fences or completion signals to the renderer;
- limit the number of in-flight frames;
- drop obsolete frames instead of accumulating latency;
- survive `SurfaceView` destruction/recreation without corrupting guest state;
- reject frames from stale session or geometry generations;
- record copy points for qualification.

Compose remains outside the high-frequency frame path.

## 5. Buffer and synchronization model

The preferred qualified path is:

```text
renderer GPU output
  -> Android-native shareable image/buffer
  -> explicit acquire synchronization
  -> Surface/BufferQueue-compatible presentation
  -> explicit release synchronization
```

The implementation must track:

- buffer identifier;
- owner;
- guest/session generation;
- width/height;
- pixel format;
- modifier or equivalent layout information when applicable;
- color space;
- acquire fence;
- release fence;
- lifetime state.

No buffer may be destroyed while the renderer, presentation backend, or SurfaceFlinger can still access it.

A bounded GPU-side copy is acceptable if required by the Pixel path and if measured latency, bandwidth, power and thermal behavior still meet the release target. A CPU readback/upload loop cannot qualify as accelerated presentation.

## 6. Mali-first implementation

### 6.1 Physical target

Initial device tuple:

- Google Pixel 7a;
- Tensor G2 / GS201;
- Mali-G710;
- Android 17 host build;
- host EGL provider `mali`;
- host Vulkan provider `mali`.

This tuple is evidence for the initial profile only. Other Mali devices require their own qualification because GPU generation, kernel driver, firmware and userspace driver can differ materially.

### 6.2 Renderer choice

The first renderer path is:

```text
virtio-gpu -> Rutabaga -> gfxstream -> host Vulkan -> Android presentation
```

Reasons:

- Android-derived guests are a primary workload;
- gfxstream aligns with Android virtual-device graphics work;
- Vulkan is the active high-performance host path on the Pixel;
- the Rutabaga boundary preserves the ability to add Mesa/Venus later.

GLES workloads may be translated inside the selected renderer stack. CarbonHV does not create a separate Mali-specific GLES backend unless measurement shows a clear need.

### 6.3 Mali optimization priorities

In order:

1. eliminate CPU framebuffer copies;
2. preserve explicit synchronization end-to-end;
3. minimize format conversions;
4. avoid unnecessary render-target reallocations on rotation/resize;
5. keep frame queues shallow;
6. reuse buffers where protocol-safe;
7. pace to the active Android display mode;
8. collect GPU/CPU time and thermal evidence before tuning heuristics;
9. apply device-specific workarounds only through a documented quirk table.

Mali-specific code must remain behind host/provider capability checks and must not leak into the guest protocol.

## 7. Future Adreno support

Adreno is a new host provider/profile, not a new guest ABI.

The future Snapdragon path reuses:

- CVD1 `virtio-gpu`;
- renderer selection APIs;
- buffer/fence model;
- Android presentation backend;
- qualification harness;
- guest images where protocol-compatible.

The Adreno renderer choice is made from evidence. Both gfxstream and Mesa/Venus remain candidates. CarbonHV must not assume that the best Mali renderer is also the best Adreno renderer.

Vendor-specific performance features may be enabled only through capability-driven profile data or small isolated quirks, never through broad `if vendor == ...` behavior in session or guest code.

## 8. Capability model

Graphics qualification is represented as progressive capabilities:

- `gpu.basic`: virtio-gpu display and scanout operate.
- `gpu.gles`: guest GLES workloads are host accelerated.
- `gpu.vulkan`: guest Vulkan workloads are host accelerated.
- `gpu.native-buffer`: renderer output reaches Android-native buffers without CPU framebuffer readback.
- `gpu.explicit-sync`: acquire/release synchronization is propagated without CPU wait loops in the steady-state path.
- `gpu.zero-copy`: measured presentation contains no full-frame copy between renderer output and display consumption.
- `gpu.sustained`: performance remains within the qualified thermal/power envelope for the sustained workload.
- `gpu.qualified`: every mandatory CarbonHV graphics test for that device/build/driver/renderer/guest tuple passes.

Capabilities are evidence-backed. They are never inferred solely from GPU model, Vulkan presence, or renderer library availability.

## 9. Renderer selection

Selection is deterministic:

1. probe host capabilities;
2. load CarbonHV renderer profiles compatible with CVD major 1;
3. reject profiles whose mandatory host capabilities are absent;
4. reject profiles not qualified for the requested release mode;
5. choose the explicitly preferred qualified profile for the exact host tuple;
6. if no qualified profile exists, expose graphics as experimental or unavailable according to product policy.

There is no silent fallback from a qualified accelerated profile to software rendering.

Developer builds may opt into experimental profiles explicitly.

## 10. Process and security boundaries

The renderer is treated as exposed to untrusted guest graphics input.

Requirements:

- renderer runs with the least privileges practical;
- no catalog credentials or signing keys are visible to it;
- no arbitrary host filesystem access;
- no generic network access unless a concrete renderer dependency requires and justifies it;
- broker requests remain typed and do not carry shell fragments;
- all shared handles are scoped to the active VM/session;
- stale handles are rejected after generation changes;
- renderer crash does not grant access to broader broker functionality;
- renderer and GPU driver vulnerabilities are part of the release patch cadence.

The GPU kernel driver remains in the host attack surface even with renderer sandboxing.

## 11. Failure handling

### 11.1 Surface loss

When Android destroys the session surface:

- stop presenting new frames to the old surface;
- retain only protocol-safe renderer resources;
- invalidate the presentation generation;
- attach the replacement surface;
- resume without rebooting the VM when supported.

### 11.2 Renderer crash

On renderer failure:

- stop accepting renderer output immediately;
- release presentation-owned resources safely;
- record the failure and renderer/profile identity;
- attempt one bounded graphics reconnection only when the selected protocol/profile supports safe reconstruction;
- otherwise transition the VM to a controlled restart-required state.

### 11.3 GPU device/context loss

Device/context loss is not hidden. The backend records the cause, tears down unsafe resources in dependency order, and either reconstructs the graphics path or marks the session failed according to renderer capability.

## 12. Qualification

### 12.1 Exact tuple

Every result records:

- CarbonHV commit;
- device model;
- SoC;
- GPU;
- host Android build fingerprint;
- kernel version;
- AVF/APEX revision;
- renderer and commit/version;
- Vulkan/EGL driver identity/version;
- guest bundle identity/version;
- CVD version;
- display mode;
- test workload version.

Changing a material tuple element invalidates automatic carry-over of `gpu.qualified`.

### 12.2 Bring-up stages

The first Mali qualification proceeds in this order:

1. host graphics capability probe;
2. minimal accelerated test guest;
3. hardware-rendered triangle with proof that host GPU work occurred;
4. compositor workload;
5. native-buffer presentation into CarbonHV `SurfaceView`;
6. GLES workload;
7. Vulkan workload;
8. rotation and dynamic resize;
9. surface destroy/recreate;
10. renderer/context-loss recovery;
11. Android-derived guest shell;
12. Ubuntu Touch/Lomiri shell;
13. sustained thermal/performance run.

A stage may provide useful engineering evidence without satisfying the next release gate.

### 12.3 Required workload classes

Qualification includes:

- static UI composition;
- continuous scrolling;
- browser/WebView rendering;
- video-like rapidly changing content;
- GLES app;
- Vulkan app;
- multi-touch during animation;
- rotation;
- repeated surface recreation;
- app background/foreground;
- renderer restart/failure injection where supported;
- at least 30 minutes of sustained animated workload.

### 12.4 Metrics

Record at minimum:

- presented FPS;
- frame-time distribution including p50/p95/p99;
- dropped/obsolete frames;
- queue depth;
- input-to-visible-response measurement for the defined interaction test;
- CPU utilization attributable to graphics path;
- renderer memory;
- graphics buffer memory;
- number and location of full-frame copies;
- device temperature/thermal status;
- power/energy data where accessible;
- recovery success/failure.

Initial acceptance budgets must be derived from measured Pixel 7a baseline data rather than invented in this design. A later qualification document locks release thresholds after the measurement harness exists.

## 13. Code boundaries

Planned units:

```text
crates/carbon-core/src/graphics/
  capabilities.rs     host-independent capability/profile model
  selection.rs        deterministic renderer selection
  qualification.rs    qualification tuple/result model

crates/carbon-broker/src/graphics/
  launch.rs           typed renderer launch contract
  handles.rs          scoped native-handle ownership model

native/graphics/
  include/            narrow native ABI
  src/probe/           Vulkan/EGL/native-buffer probing
  src/presentation/    Android surface/buffer/fence handling
  src/renderer/        Rutabaga integration and renderer adapters

android/app/.../session/
  GraphicsSession.kt   lifecycle bridge, no renderer internals

qualification/gpu/
  probe/
  workloads/
  reports/
```

Exact filenames may be adjusted during implementation planning to match existing crate conventions, but these responsibility boundaries are fixed by this design.

## 14. Testing strategy

### Host-independent tests

- capability/profile parsing;
- deterministic renderer selection;
- rejection of unsupported combinations;
- qualification invalidation when tuple fields change;
- stale session/generation rejection;
- buffer lifetime state machine;
- no silent software fallback.

### Android/native tests

- surface attach/detach;
- stale-surface rejection;
- resize generation handling;
- fence propagation;
- bounded in-flight buffers;
- frame dropping under backpressure;
- native-buffer format validation.

### Physical tests

Only physical-device tests may grant hardware capability states such as `gpu.vulkan`, `gpu.native-buffer`, `gpu.zero-copy`, `gpu.sustained`, or `gpu.qualified`.

## 15. Delivery phases

### GPU Phase A — Evidence and contracts

Implement host probing, capability/profile models, qualification tuple storage, and a renderer selection policy. Extend the current device qualifier so it reports the Mali/Vulkan facts required for later gates.

### GPU Phase B — Minimal Mali renderer path

Integrate Rutabaga/gfxstream behind the renderer boundary and prove a hardware-rendered minimal guest workload on the Pixel 7a.

### GPU Phase C — Android presentation

Implement native-buffer/fence presentation into the existing `SurfaceView`, instrument every copy and queue transition, and handle surface lifecycle correctly.

### GPU Phase D — Real guest qualification

Bring up Android-derived and Ubuntu Touch/Lomiri compositors, then exercise GLES/Vulkan, resize, recovery and sustained workloads.

### GPU Phase E — Qualcomm/Adreno

Connect the Snapdragon device, run the same host probe and measurement harness, select the best renderer profile from evidence, add only the provider/profile-specific work required, and preserve CVD1 compatibility.

## 16. Acceptance criteria

The Mali phase is complete only when:

- the Pixel 7a produces a reproducible qualification tuple;
- guest graphics are demonstrably executed on the Mali GPU;
- the guest compositor reaches CarbonHV's Android surface through the defined renderer/presentation boundary;
- there is no full-frame CPU readback in qualified accelerated mode;
- buffer and fence ownership survive resize and surface recreation;
- renderer failure has bounded, tested behavior;
- Android-derived and Ubuntu Touch/Lomiri guest graphics both use the same CVD1 virtual GPU contract;
- sustained testing produces recorded performance and thermal evidence;
- CarbonHV reports only the capability level actually demonstrated.

Qualcomm/Adreno support is not required to complete the Mali phase, but the implementation must preserve the abstraction boundaries that allow it to be added without changing the guest ABI.
