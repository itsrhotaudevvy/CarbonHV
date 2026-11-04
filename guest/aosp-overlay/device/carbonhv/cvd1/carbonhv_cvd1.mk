#
# CarbonHV CVD1 virtual phone, shared by every AOSP-derived ROM port.
#
# Base: AOSP cuttlefish arm64-only phone (device/google/cuttlefish/vsoc_arm64_only/phone/aosp_cf.mk,
# PRODUCT_NAME aosp_cf_arm64_only_phone at android-17.0.0_r1). It provides virtio-blk/net/input/
# snd/vsock, the gfxstream guest drivers (libEGL_emulation, vulkan.ranchu) and the ranchu
# composer, so the guest speaks exactly the gfxstream-vulkan/gfxstream-composer context types the
# CarbonHV broker asks AVF/crosvm for. ROM-specific device trees (for example LineageOS
# device/virt/virtio_*, which use Mesa virgl/venus, GRUB/EFI boot and a SwiftShader fallback)
# are deliberately not used: CVD1 has one graphics transport and no software rendering fallback.
#

$(call inherit-product, device/google/cuttlefish/vsoc_arm64_only/phone/aosp_cf.mk)

# Honest virtual identity: never impersonate a physical phone or a ROM's official device.
PRODUCT_BRAND := CarbonHV
PRODUCT_MANUFACTURER := CarbonHV
PRODUCT_MODEL := CarbonHV Virtual Device (CVD1)

PRODUCT_VENDOR_PROPERTIES += \
    ro.carbonhv.cvd_major=1 \
    ro.carbonhv.graphics.transport=gfxstream

# The guest contract the broker validated for this ROM; staged by guest/aosp-overlay/build-cvd1.sh.
PRODUCT_COPY_FILES += \
    device/carbonhv/cvd1/device.toml:$(TARGET_COPY_OUT_VENDOR)/etc/carbonhv/device.toml
