# CarbonHV Android product fragment

`carbonhv.mk` is the integration entrypoint for a controlled Android 17 host product. It installs the Rust broker daemon and platform host-integration app into `system_ext`, imports CarbonHV's private SELinux policy, and writes the ordinary UI APK signing-certificate SHA-256 digest into the read-only CarbonHV property.

From the target product makefile, set the digest and inherit this fragment using the path where CarbonHV is checked out in that Android tree:

```make
CARBONHV_UI_SIGNING_SHA256 := <64-lowercase-hex-digest>
$(call inherit-product, external/carbonhv/host/product/carbonhv.mk)
```

The digest must correspond to the certificate that signs the ordinary `dev.carbonhv` APK installed on that same product. The fragment intentionally fails the product build when the value is absent. Release products should provide the value through their controlled product configuration rather than committing a production key or digest to this repository.

For a development source tree, `prepare-controlled-host.sh` stages a clean CarbonHV snapshot at `external/carbonhv` and injects the fragment into one explicit product makefile. It is idempotent, replaces a previously injected digest in place, and refuses to delete an unrelated pre-existing `external/carbonhv` directory. Android 17 restricts `framework-virtualization.impl` to the Virtualization module subtree, so the helper also adds exactly `//external/carbonhv:__subpackages__` to that implementation library's visibility list. It fails closed if the expected Android 17 Soong structure has drifted instead of applying a broad visibility change.

```bash
host/product/prepare-controlled-host.sh \
  --android-root /path/to/android-17 \
  --product-makefile device/google/lynx/aosp_lynx.mk \
  --ui-apk android/app/build/outputs/apk/debug/app-debug.apk \
  --apksigner "$ANDROID_HOME/build-tools/<version>/apksigner"
```

If the APK is signed elsewhere, pass `--ui-signing-sha256 <64-hex-digest>` instead. The generated product block contains only the certificate digest; it never copies or records the signing key.

After building the product, verify on-device that `dev.carbonhv.broker` is owned by `carbonhvd`, `dev.carbonhv.host.runtime` is owned by `dev.carbonhv.host`, both processes have their dedicated SELinux domains, and `/data/misc/carbonhv` has the `carbonhv_data_file` label before attempting a guest boot.

## Snapdragon (Gunyah) controlled hosts

Qualcomm phones run Gunyah, not KVM. Stock firmware does not declare AVF VM support, so `vm info` reports `VMs are not supported`. A controlled Snapdragon host build needs three changes beyond `carbonhv.mk`. None of them can be made from this repository alone, and none may be made on a stock user build.

1. **Declare non-protected VM support** in the boot configuration that produces `ro.boot.*` (for example `BOARD_BOOTCONFIG += androidboot.hypervisor.vm.supported=1` in the rebuilt `vendor_boot`). Leave `protected_vm.supported` untouched. CarbonHV GUI sessions never request protected or QCOM trusted VMs.
2. **Let the AVF crosvm domain open `/dev/gunyah`** under the label the vendor policy assigns. Read the label with `qualification/carbon-qualify.sh --adb <serial>` (field `gunyah_selinux_label`). Add only that one device rule to the product's vendor policy; do not relabel the node.
3. **Use a `com.android.virt` APEX whose crosvm is built with the `gunyah` feature.** AOSP virtmgr already passes `--hypervisor gunyah[...]` for trusted VMs, so arm64 builds normally carry it. With no `/dev/kvm`, crosvm's default-hypervisor probe then selects `/dev/gunyah` for the unprotected VM.

After flashing, the probe must report `hypervisor=gunyah`, `hypervisor_vm_supported=1`, and `avf_gunyah_path=declared`, and `carbonhvd` must log `class=GunyahAvfReady`. The result is still `EXPERIMENTAL` until a starter guest boots, renders through gfxstream into the CarbonHV surface, and passes sustained qualification.

The second tier, where CarbonHV launches crosvm itself to pin vCPUs (`GunyahCrosvmBackend`), additionally needs a dedicated crosvm SELinux domain that `carbonhvd` may transition into. That spawn and supervision path is a later slice and is not part of this fragment yet.
