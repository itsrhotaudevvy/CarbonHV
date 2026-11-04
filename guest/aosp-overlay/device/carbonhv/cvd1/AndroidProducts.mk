# CarbonHV CVD1 products for AOSP-derived custom ROMs.
# Every product shares carbonhv_cvd1.mk (the AOSP cuttlefish arm64-only phone base, whose
# gfxstream guest GLES/Vulkan stack and ranchu composer match CVD1's single graphics transport)
# and adds only its ROM's vendor configuration.

PRODUCT_MAKEFILES := \
    lineage_carbonhv_cvd1:$(LOCAL_DIR)/lineage_carbonhv_cvd1.mk \
    aosp_carbonhv_cvd1:$(LOCAL_DIR)/aosp_carbonhv_cvd1.mk
