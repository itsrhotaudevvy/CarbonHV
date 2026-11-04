# Ubuntu Touch CVD1 guest

CarbonHV treats Ubuntu Touch as its own mobile Linux guest, not as an Android system image and not as generic Ubuntu with a desktop shell. The target session is Lomiri/Mir with Ubuntu Touch services and AppArmor confinement intact.

The preferred CVD1 adaptation uses direct virtio Linux devices. `build.sh` assembles a verified Ubuntu Touch rootfs tarball with a separately supplied ARM64 kernel/initramfs into CarbonHV boot/rootfs artifacts. This keeps physical-phone vendor blobs out of the VM. If the selected Ubuntu Touch release cannot bring up its real mobile session through this path, a Halium-compatible virtual adaptation is a separate reviewed fallback rather than an implicit dependency.

Release automation must supply `UT_ROOTFS_TARBALL`, `UT_ROOTFS_SHA256`, `UT_KERNEL_IMAGE`, and `UT_INITRD_IMAGE` from a pinned source/build pipeline. The assembler refuses an unverified rootfs.

## Kernel configuration

The guest kernel must satisfy `carbonctl guest kernel-config`. Merge `guest/common/kernel/cvd1-near-native.fragment` and `guest/ubuntu-touch/device/kernel.fragment` into the kernel config. Missing virtio-gpu or sync_file breaks the CVD1 contract (exit 3). Missing `CONFIG_CPUFREQ_VIRT`, THP, or balloon page reporting means the guest boots but cannot use the host's near-native plan (exit 5). On Snapdragon/Gunyah hosts the guest sees no SVE, MTE, or PMU; record that in the guest's security-difference matrix.
`build.sh` requires `UT_KERNEL_CONFIG`, the exact `.config` `UT_KERNEL_IMAGE` was built from, and refuses one that fails verification.
