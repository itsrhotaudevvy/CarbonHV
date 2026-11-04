# LineageOS CVD1 guest (unofficial)

An unofficial CarbonHV virtual-device build of [LineageOS](https://lineageos.org). It is **not** an official LineageOS device or release, does not use LineageOS branding as an official build, and inherits none of LineageOS's device-support claims.

| | |
| --- | --- |
| Default source | `https://github.com/LineageOS/android.git`, branch **`lineage-24.0`** (Android 17, release config `cp2a`) |
| Latest stable line | `lineage-23.2` (Android 16 QPR2); build it with `CARBON_ROM_REF=lineage-23.2` |
| Product | `lineage_carbonhv_cvd1` (shared overlay in [`guest/aosp-overlay`](../aosp-overlay)) |
| Base | AOSP cuttlefish arm64-only phone: gfxstream guest GLES/Vulkan, ranchu composer, virtio devices |
| Integration profile | Balanced |
| Status | **Recipe only**: not built or booted by CarbonHV yet |

## Why not LineageOS's own `virtio_arm64` target?

LineageOS ships virtual-machine targets (`device/virt/virtio_arm64`, `virtio_arm64only`), but they do not fit CVD1:

- they boot through GRUB/EFI instead of a direct kernel + initrd launch;
- they render with Mesa virgl/venus instead of gfxstream;
- they silently fall back to SwiftShader when the GPU lacks 3D.

CVD1 has one graphics transport and forbids software rendering fallback, so CarbonHV puts the LineageOS vendor configuration on the same cuttlefish base the other Android-derived guests use.

## Build

The build needs a Linux x86_64 Android build host with `repo`, `rsync`, and about 400 GB of disk.

```bash
guest/lineageos/build.sh                      # lineage-24.0
CARBON_ROM_REF=lineage-23.2 guest/lineageos/build.sh
```

The build writes these files to `out/carbonhv/lineageos/`:

- `kernel`;
- `initrd.img` (the vendor ramdisk followed by the generic ramdisk);
- `super.img`;
- `source-manifest.xml`, `product.txt`, `boot-parameters.txt` and `SHA256SUMS`.

Release automation must also set `CARBON_ROM_KERNEL_CONFIG` so the guest kernel passes `carbonctl guest kernel-config android`.

## Known gaps before the first boot

- **Boot parameters:** `boot-parameters.txt` records the `androidboot.*` parameters the product declares. The broker's starter profile currently passes only `console=hvc0 gfxstream_enabled`, so carrying the bundle's boot parameters through the broker is the next integration step.
- **Lineage vendor config on a cuttlefish board** is unverified. The first build may need overlay fixes for Lineage's board hooks.
- **Security differences** from an official device build must be recorded per host hypervisor, for example no MTE in CVD1 guests (`docs/SUPPORT_MATRIX.md`).
