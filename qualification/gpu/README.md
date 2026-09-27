# GPU qualification harness

This directory records evidence for CarbonHV's stable CVD1 `virtio-gpu` ABI and host renderer/presentation path. A report is evidence for one exact qualification tuple; changing any tuple field requires a new run.

Deterministic fixture run:

```bash
qualification/gpu/run-minimal.sh \
  --fixture qualification/gpu/fixtures/mali-pass.env \
  --dry-run
```

Fixture reports always contain `evidence_source=fixture` and never qualify a physical device. The negative fixture exists to enforce the fail-closed gate: successful GLES/Vulkan-looking fields cannot grant `gpu.vulkan` or stronger unless `hardware_render_evidence=positive` is also present with a concrete proof source, virtio-gpu transport, presented frames, and no detected software renderer.

For a physical run, capture the guest/renderer fields described in `minimal-guest/README.md`, then combine them with read-only host facts:

```bash
qualification/gpu/run-minimal.sh \
  --adb <serial> \
  --evidence /path/to/guest-renderer.env \
  --output qualification/gpu/reports/<run>.txt
```

Before guest evidence exists, the bring-up preflight can be run with the selected renderer profile:

```bash
qualification/gpu/run-minimal.sh \
  --adb <serial> \
  --renderer gfxstream-vulkan-mali \
  --output qualification/gpu/reports/<run>.txt
```

If required guest artifacts, renderer deployment, or AVF renderer transport are missing, this form exits with the experimental status code `4`, emits `bringup_status=BLOCKED`, and keeps every GPU capability false. It is intended to make an incomplete physical attempt reproducible without turning host facts into acceleration claims.

The physical path does not accept host-only Android rendering as guest proof. `guest_gpu_transport=virtio-gpu`, a guest workload result, renderer identity, presentation counts, copy inventory, and a host-GPU proof source all come from the VM/renderer run.

The minimal harness can establish basic Vulkan/native-buffer evidence, but it always leaves `gpu.qualified=false`. Full qualification additionally requires the real Android-derived and Ubuntu Touch/Lomiri guests plus sustained testing.
