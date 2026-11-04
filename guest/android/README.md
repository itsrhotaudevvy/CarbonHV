# GrapheneOS-derived CVD1 guest

This is a CarbonHV-maintained ARM64 virtual-device port derived from GrapheneOS sources. It is not an official GrapheneOS device target and must not inherit GrapheneOS hardware-security or supported-device claims.

The port targets CVD major 1: ARM64, virtio-gpu, virtio-input, virtio-net, virtio-blk, virtio-snd, and vsock. Release qualification requires an enforcing SELinux build, working setup/lockscreen, persistent encrypted guest data where supported, honest virtual-device identity, and a published security-difference matrix.

`build.sh` is intentionally Linux-only because the upstream GrapheneOS build environment is Linux/x86_64. It syncs the configured GrapheneOS manifest, enters the Android build environment, selects the CarbonHV ARM64 virtual target, builds boot/system artifacts, and copies release inputs into `out/carbonhv/grapheneos`.

The default source ref tracks the GrapheneOS Android 17 development line used by this architecture. Production release automation must replace it with an exact reviewed tag/manifest revision and record the resulting source manifest in provenance.

The virtual product name is configurable with `CARBON_GRAPHENE_TARGET`. The initial bring-up target is `aosp_cf_arm64_phone`; if the pinned GrapheneOS tree renames or removes it, the port overlay under this directory must supply the equivalent CVD1 product before a release is built.

## Kernel configuration

The guest kernel must satisfy `carbonctl guest kernel-config`. Merge `guest/common/kernel/cvd1-near-native.fragment` and `guest/android/device/kernel.fragment` into the kernel config. Missing virtio-gpu or sync_file breaks the CVD1 contract (exit 3). Missing `CONFIG_CPUFREQ_VIRT`, THP, or balloon page reporting means the guest boots but cannot use the host's near-native plan (exit 5). On Snapdragon/Gunyah hosts the guest sees no SVE, MTE, or PMU; record that in the guest's security-difference matrix.
Set `CARBON_GRAPHENE_KERNEL_CONFIG` to the prebuilt GKI kernel's config so `build.sh` verifies it and records it as `kernel.config`.
