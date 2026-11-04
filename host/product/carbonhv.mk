# CarbonHV controlled-host product integration.
# Include this from an Android 17 product makefile after defining
# CARBONHV_UI_SIGNING_SHA256 to the SHA-256 digest of the ordinary UI APK cert.

LOCAL_PATH := $(call my-dir)

ifndef CARBONHV_UI_SIGNING_SHA256
$(error CARBONHV_UI_SIGNING_SHA256 must be set before including CarbonHV host integration)
endif

PRODUCT_PACKAGES += \
    carbonhvd \
    CarbonHvHostIntegration

# CarbonHV owns coredomain/system_ext service-manager, file, app and property labels.
# Keep these declarations private to the controlled system_ext product policy.
SYSTEM_EXT_PRIVATE_SEPOLICY_DIRS += $(LOCAL_PATH)/..

PRODUCT_SYSTEM_EXT_PROPERTIES += \
    ro.carbonhv.ui_signing_sha256=$(CARBONHV_UI_SIGNING_SHA256)
