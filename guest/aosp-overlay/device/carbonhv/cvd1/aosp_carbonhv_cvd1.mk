# PixelOS on the CarbonHV CVD1 virtual phone. Unofficial build: it is not a PixelOS device and
# must not be distributed as an official PixelOS release. PixelOS's phone config already
# inherits vendor/lineage/config/common_full_phone.mk.
$(call inherit-product, device/carbonhv/cvd1/carbonhv_cvd1.mk)
$(call inherit-product, vendor/custom/config/common_full_phone.mk)

PRODUCT_NAME := aosp_carbonhv_cvd1
