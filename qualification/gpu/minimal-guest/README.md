# Minimal guest GPU workload protocol

The minimal workload is an ARM64 guest workload, executed inside a CVD1 VM. It must use the guest `virtio-gpu` device and exercise both a GLES render and a Vulkan render. A host-only Android `SurfaceView`, host `vkcube`, or host GPU sample is not valid guest evidence.

For each run, collect a normalized `key=value` evidence file containing the non-host fields in `schema/report-v1.txt`. The renderer/presentation instrumentation must establish all of the following before setting `hardware_render_evidence=positive`:

1. The guest reports `guest_gpu_transport=virtio-gpu` and successfully completes its Vulkan workload.
2. `carbon-renderer` identifies the selected `gfxstream-vulkan-mali` backend and the host Vulkan physical device.
3. A host GPU activity source changes while the guest workload runs. Record the source in `hardware_render_source` and the observed delta in `host_gpu_busy_delta_pct`; a renderer trace that proves Vulkan submission may be combined with the counter source.
4. The frame reaches CarbonHV's Android presentation path, incrementing `native_buffer_presented_frames`.
5. Explicit synchronization is observed as `explicit_sync_evidence=positive`.
6. Every full-frame copy is counted. CPU readback/upload is recorded in `full_frame_cpu_copies`; GPU blits/copies are recorded separately in `gpu_side_full_frame_copies`.
7. Software renderers such as SwiftShader/llvmpipe set `software_renderer_detected=true` and therefore fail the hardware gate.

The workload version for this first protocol is `minimal-gpu-v1`. The initial run is intentionally short; the 30-minute sustained workload belongs to the later qualification stage.
