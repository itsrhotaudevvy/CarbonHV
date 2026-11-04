# PixelOS CVD1 guest (unofficial)

An unofficial CarbonHV virtual-device build of [PixelOS](https://pixelos.net). It is **not** an official PixelOS device or release, and inherits none of PixelOS's device-support claims. Google Pixel–exclusive apps and features that need Google-signed device certification are not provided.

| | |
| --- | --- |
| Default source | `https://github.com/PixelOS-AOSP/android_manifest.git`, branch **`seventeen`** (Android 17, `android-17.0.0_r1` with LineageOS `lineage-24.0` components, release config `cp2a`) |
| Latest stable line | `sixteen-qpr2` (PixelOS 16.2, Android 16 QPR2); build it with `CARBON_ROM_REF=sixteen-qpr2` |
| Product | `aosp_carbonhv_cvd1` (shared overlay in [`guest/aosp-overlay`](../aosp-overlay)) |
| Base | AOSP cuttlefish arm64-only phone: gfxstream guest GLES/Vulkan, ranchu composer, virtio devices |
| Integration profile | Balanced |
| Status | **Recipe only**: not built or booted by CarbonHV yet |

PixelOS's `vendor/custom/config/common_full_phone.mk` already inherits LineageOS's phone config, so this port shares the LineageOS port's overlay, build flow and known gaps.

## Build

The build needs a Linux x86_64 Android build host with `repo`, `rsync`, and about 400 GB of disk.

```bash
guest/pixelos/build.sh                        # seventeen
CARBON_ROM_REF=sixteen-qpr2 guest/pixelos/build.sh
```

The outputs match the LineageOS port and land in `out/carbonhv/pixelos/`. See [`guest/lineageos/README.md`](../lineageos/README.md) for the known gaps before the first boot.
