# Shared CVD1 overlay for AOSP-derived custom ROMs

`device/carbonhv/cvd1` turns an AOSP-derived ROM tree into a CarbonHV CVD1 guest. It inherits the AOSP cuttlefish arm64-only phone, which provides:

- virtio devices;
- the gfxstream guest GLES/Vulkan drivers and the ranchu composer;
- the Android 17 virtualization APEX packages.

It then adds a single ROM vendor configuration:

| Product | ROM | Adds |
| --- | --- | --- |
| `lineage_carbonhv_cvd1` | LineageOS | `vendor/lineage/config/common_full_phone.mk` |
| `aosp_carbonhv_cvd1` | PixelOS | `vendor/custom/config/common_full_phone.mk` |

The virtual identity is always "CarbonHV Virtual Device (CVD1)". The validated `guest/<rom>/device/cvd1.toml` is installed at `/vendor/etc/carbonhv/device.toml`. `build-cvd1.sh --rom <rom>` syncs the ROM, stages this overlay, lunches with the tree's own `aosp_target_release`, and builds the boot, init_boot, vendor_boot and super images. It then emits direct-boot artifacts (`kernel`, `initrd.img`, `super.img`) with provenance.

Adding another AOSP-derived ROM takes four steps:

1. Add a product makefile and an `AndroidProducts.mk` entry.
2. Add a `--rom` case to `build-cvd1.sh`.
3. Add `guest/<rom>/device/cvd1.toml` and `kernel.fragment`.
4. Add `bundles/<rom>/bundle.toml`, a broker starter profile, and a catalog entry.

ROM-specific virtual device trees that change the graphics transport or boot flow are out of scope (ADR-016).
