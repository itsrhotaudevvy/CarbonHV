# LineageOS on the CarbonHV CVD1 virtual phone. Unofficial build: it is not a LineageOS device
# and must not be distributed as an official LineageOS release.
$(call inherit-product, device/carbonhv/cvd1/carbonhv_cvd1.mk)
$(call inherit-product, vendor/lineage/config/common_full_phone.mk)

PRODUCT_NAME := lineage_carbonhv_cvd1
