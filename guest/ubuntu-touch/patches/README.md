# Ubuntu Touch virtual-device patch inventory

Track downstream changes required to run Lomiri/Mir and Ubuntu Touch services against CVD1 here. Typical areas are DRM/virtio-gpu session configuration, input naming, virtual power/battery properties, audio device discovery, networking expectations, AppArmor rules, and the CarbonHV guest-agent service.

Do not add Android vendor blobs solely to make a desktop session boot. A Halium-compatible path is acceptable only when the actual Ubuntu Touch mobile stack requires it and its maintenance/security cost is documented.

