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

The sustained policy is documented in `workloads/README.md`. Its entrypoint is:

```bash
qualification/gpu/run-sustained.sh --adb <serial> --minutes 30
```

That form is the bring-up preflight and remains fail-closed while guest/renderer transport is blocked. After the real workload runner captures normalized evidence, qualification uses:

```bash
qualification/gpu/run-sustained.sh \
  --adb <serial> \
  --evidence <physical-sustained.env> \
  --profile qualification/gpu/profiles/pixel-7a-mali-g710.toml \
  --minutes 30
```

The ADB+evidence path first re-validates the minimal hardware-render/native-buffer proof against the currently connected host, then requires every material tuple field in the calibrated profile to match: CarbonHV commit, device/SoC/GPU, host build, kernel, AVF revision, renderer/version, graphics driver, both guest bundle versions, CVD major, display mode, and workload version. Only that physical path may emit `gpu.sustained=true`, `gpu.qualified=true`, and `qualification=QUALIFIED`.

The Pixel 7a profile remains `uncalibrated` until both real starter guests complete the mandatory workload set and 30-minute physical baseline. Numeric thresholds and exact tuple locks are deliberately absent before that measurement. A blocked run records `unknown`/`not-run` metrics and keeps `gpu.sustained=false`, `gpu.qualified=false`, and `qualification=EXPERIMENTAL`.

`test-sustained.sh` exercises duration, workload, threshold, and fixture-evidence gates without granting physical capability state.
