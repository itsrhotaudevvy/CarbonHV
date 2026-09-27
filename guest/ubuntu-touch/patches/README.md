# Ubuntu Touch virtual-device patch inventory

Track downstream changes required to run the real Lomiri/Mir mobile session against CVD1 here. The graphics path is the Linux `virtio_gpu` DRM device plus Mesa and Mir; it must not depend on host Mali libraries, `/dev/mali*`, or Android GPU vendor blobs. The guest kernel must provide the virtio-gpu DRM, dma-buf, and explicit-sync primitives required by the selected Mesa/Mir revisions, and Mir must consume that virtual DRM device for the Lomiri session.

Mesa is built/configured for the virtual GPU capsets exposed by CarbonHV's CVD1 renderer. GLES is mandatory. Vulkan is exercised when the selected Mesa/virtio-gpu path exposes it; absence of Vulkan is recorded as `not-exposed` rather than replaced with a software ICD. Native-buffer sharing and fence behavior must remain compatible with the same host renderer/presentation contract used by the Android-derived guest.

The patch inventory must record the exact Mesa, Mir, kernel, and Lomiri revisions plus the runtime renderer/DRM evidence from the built guest. A generic Wayland/X11 desktop session is not proof of Ubuntu Touch graphics support.

Other typical areas are input naming, virtual power/battery properties, audio device discovery, networking expectations, AppArmor rules, and the CarbonHV guest-agent service. Do not add Android vendor blobs solely to make a desktop session boot. A Halium-compatible path is acceptable only when the actual Ubuntu Touch mobile stack requires it and its maintenance/security cost is documented.
