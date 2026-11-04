# CarbonHV on a controlled Qualcomm Gunyah host: include from the device's BoardConfig.mk.
#
# Declares non-protected VM support so AVF's virtualizationservice accepts VMs; crosvm then falls
# back from the absent /dev/kvm to /dev/gunyah (ADR-013). protected_vm.supported is deliberately
# left to the vendor: CarbonHV GUI sessions never request protected or QCOM trusted VMs.
BOARD_BOOTCONFIG += androidboot.hypervisor.vm.supported=1

# Vendor policy that lets AVF's crosvm use the vendor Gunyah node through AOSP's hook.
BOARD_VENDOR_SEPOLICY_DIRS += external/carbonhv/host/product/snapdragon/sepolicy
