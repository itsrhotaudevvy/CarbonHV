# Sustained GPU qualification workloads

CarbonHV qualifies graphics against the same CVD1 `virtio-gpu` guest ABI for every host GPU family. The first target is the Pixel 7a / GS201 / Mali-G710 with `gfxstream-vulkan-mali`; later Adreno work must reuse these workload classes and evidence definitions.

## Mandatory guest workload classes

Run the full set on both starter guests. The Android-derived guest must expose GLES and Vulkan. Ubuntu Touch must expose GLES; Vulkan is run when the selected Mesa/virtio-gpu stack exposes it and is otherwise recorded as `not-exposed`.

1. **Static composition** — a stable multi-layer UI with text, translucency, images, and clipped regions.
2. **Continuous scrolling** — sustained vertical and horizontal scrolling through real UI content.
3. **Browser rendering** — WebView on the Android-derived guest and the native Ubuntu Touch browser/WebEngine path (or its release-equivalent browser engine), with animated CSS/canvas content.
4. **Rapid content** — video-like full-surface updates with no host video-decoder shortcut counted as renderer proof.
5. **GLES** — a guest GLES workload that continuously submits rendered frames through `virtio-gpu`.
6. **Vulkan** — a guest Vulkan workload where the guest advertises Vulkan.
7. **Multi-touch during animation** — two or more concurrent pointers while animation continues.
8. **Rotation/dynamic resize** — portrait/landscape transitions with generation-safe buffer replacement.
9. **Repeated surface recreation** — destroy/recreate the Android host surface while the VM remains healthy.
10. **Background/foreground** — leave and re-enter the immersive session without rebooting a healthy VM.
11. **Renderer/context-loss recovery** — inject the supported renderer restart or device/context-loss path and record whether graphics reconstructs safely.
12. **Sustained animation** — at least 30 continuous minutes with the workload mix above and no CPU framebuffer readback/upload loop.

Do not substitute a host-only Vulkan sample, Android `SurfaceView` animation, generic Ubuntu desktop, or software renderer for guest evidence.

## Measurement definitions

Every run records presented FPS; p50/p95/p99 frame time; dropped or obsolete frames; maximum queue depth; graphics-path CPU utilization; renderer RSS; graphics-buffer memory; all full-frame CPU and GPU copies; peak Android thermal status; available power/energy evidence; and recovery result. Unknown or inaccessible measurements are written as `unknown`, never converted to a passing value.

The input-to-visible-response measurement uses the instrumented interaction workload. CarbonHV timestamps the injected input event on the host monotonic clock and tags it with a sequence number. The guest workload reports the first rendered frame containing the matching sequence marker. The presentation path records the signal/presentation timestamp for that frame. `input_to_visible_ms` is the elapsed time from host input dispatch to that first matching presented frame. Runs without this correlation are incomplete.

Copy inventory is measured across the guest renderer, native-buffer import/export, and Android presentation path. `full_frame_cpu_copies` counts readback/upload or CPU framebuffer copies. `gpu_side_full_frame_copies` counts full-surface blits/copies on the GPU. Zero-copy is only granted when the measured count is zero; a bounded GPU-side copy may still qualify the general accelerated path when the calibrated release budgets pass.

## Release threshold calibration

`qualification/gpu/profiles/pixel-7a-mali-g710.toml` intentionally begins `uncalibrated`. Numeric release thresholds are added only after both real starter guests complete the mandatory 30-minute physical baseline on the exact recorded host/build/driver/renderer tuple. The locked thresholds must cite that baseline report. Any material tuple change invalidates carry-over.

`run-sustained.sh` refuses an uncalibrated profile. Its deterministic fixture mode only tests policy math and always leaves `gpu.sustained=false` and `gpu.qualified=false`. Physical qualification is the only path allowed to promote those capabilities.
