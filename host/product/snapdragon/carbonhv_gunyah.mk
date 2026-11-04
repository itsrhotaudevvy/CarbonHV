# CarbonHV on a controlled Qualcomm Gunyah host: inherit from the device product makefile,
# alongside host/product/carbonhv.mk.
#
# Stock Snapdragon firmware can run transparent huge pages as "never" (observed on the Redmi
# kunzite, 2026-11-04), which turns the broker's huge-page advice into a no-op. madvise only
# affects memory that asks for it, so Android app memory behaviour is unchanged.
PRODUCT_COPY_FILES += \
    external/carbonhv/host/product/snapdragon/init.carbonhv.gunyah.rc:$(TARGET_COPY_OUT_SYSTEM_EXT)/etc/init/init.carbonhv.gunyah.rc
