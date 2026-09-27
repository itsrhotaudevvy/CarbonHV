# GrapheneOS virtual-device patch inventory

Keep every downstream change required for the CarbonHV ARM64 virtual device here as an auditable patch series or repo-project overlay. The release manifest must record patch IDs and upstream bases.

The graphics contract is CVD1 `virtio-gpu`; the guest must never contain Mali host drivers or depend on a Pixel GPU device node. The Android 17 virtual product must select the pinned tree's AOSP/Cuttlefish-compatible virtual gralloc and hardware-composer implementations, expose GLES through the virtual graphics stack, and expose Vulkan through the guest virtualization ICD when that product is built. Buffer allocation must remain compatible with Android native-buffer export/import and explicit fences so the host renderer can present through `AHardwareBuffer` without a CPU framebuffer readback loop.

For every pinned upstream revision, the patch inventory must record the exact virtual gralloc/HWC/EGL/Vulkan components selected by the product overlay and the command/output used to prove them in the built guest. A release candidate fails the graphics gate if it loads `libGLES_mali`, expects `/dev/mali*`, embeds Pixel GPU firmware solely for graphics, silently falls back to SwiftShader, or replaces the CVD1 `virtio-gpu` contract with a host-vendor ABI.

Other expected patch areas are virtual audio/input/power integration, guest-agent service policy, updater endpoint ownership, and security-feature accounting. Do not copy Pixel vendor firmware or present virtual software keys as hardware-backed StrongBox keys.
