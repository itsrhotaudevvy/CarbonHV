# Ubuntu Touch CVD1 guest

CarbonHV treats Ubuntu Touch as its own mobile Linux guest, not as an Android system image and not as generic Ubuntu with a desktop shell. The target session is Lomiri/Mir with Ubuntu Touch services and AppArmor confinement intact.

The preferred CVD1 adaptation uses direct virtio Linux devices. `build.sh` assembles a verified Ubuntu Touch rootfs tarball with a separately supplied ARM64 kernel/initramfs into CarbonHV boot/rootfs artifacts. This keeps physical-phone vendor blobs out of the VM. If the selected Ubuntu Touch release cannot bring up its real mobile session through this path, a Halium-compatible virtual adaptation is a separate reviewed fallback rather than an implicit dependency.

Release automation must supply `UT_ROOTFS_TARBALL`, `UT_ROOTFS_SHA256`, `UT_KERNEL_IMAGE`, and `UT_INITRD_IMAGE` from a pinned source/build pipeline. The assembler refuses an unverified rootfs.

