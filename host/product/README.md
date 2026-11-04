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

Qualcomm phones run Gunyah, not KVM. Stock firmware does not declare AVF VM support, so `vm info` reports `VMs are not supported`. [`snapdragon/`](snapdragon) holds the complete delta a controlled Snapdragon build needs. Each item was checked against a physical Redmi `kunzite` (Snapdragon 6 Gen 3) on 2026-11-04:

| File | Effect | Evidence |
| --- | --- | --- |
| `BoardConfigCarbonHvGunyah.mk` | `BOARD_BOOTCONFIG += androidboot.hypervisor.vm.supported=1`, and adds the vendor policy dir | Stock: `ro.boot.hypervisor.*` unset |
| `sepolicy/carbonhv_gunyah.te` | `typeattribute vendor_gunyah_dev vm_manager_device_type;`, AOSP's hook that lets AVF's crosvm (and only crosvm) open VM-manager devices | `/dev/gunyah` is `u:object_r:vendor_gunyah_dev:s0`; the shell is denied |
| `carbonhv_gunyah.mk` + `init.carbonhv.gunyah.rc` | Sets THP to `madvise` at early boot, so the broker's huge-page advice can take effect | Stock runtime THP is `never` (compiled default `madvise`) |

On the Redmi `kunzite`, the stock vendor policy **already** contains `(typeattributeset vm_manager_device_type (vendor_gunyah_dev))`, so `sepolicy/carbonhv_gunyah.te` is a no-op there. That leaves this phone needing only the bootconfig line (and, optionally, the THP setting) in a patched `vendor_boot`. Other Snapdragon devices still need the policy line. The step-by-step clean route for that phone is in [`snapdragon/KUNZITE.md`](snapdragon/KUNZITE.md), and uses `snapdragon/patch_vendor_boot.py`.

Include the board fragment from the device `BoardConfig.mk`, and inherit `carbonhv_gunyah.mk` next to `carbonhv.mk`. Leave `protected_vm.supported` to the vendor: CarbonHV GUI sessions never request protected or QCOM trusted VMs, and the contract test (`snapdragon/test-snapdragon-contract.sh`) rejects any fragment that does. The node is not relabeled and no other domain gains access. The kernel already ships Gunyah (`CONFIG_GUNYAH=y`, resource-manager modules loaded), and AOSP's APEX crosvm is built with the `gunyah` feature.

After flashing, the probe must report `hypervisor=gunyah`, `hypervisor_vm_supported=1`, `thp_mode=madvise` and `avf_vendor_path=declared`, and `carbonhvd` must log `class=VendorHypervisorAvfReady`. The result stays `EXPERIMENTAL` until a starter guest boots, renders through gfxstream into the CarbonHV surface, and passes sustained qualification.

The second tier, where CarbonHV launches crosvm itself to pin vCPUs (`DirectCrosvmBackend`), additionally needs a dedicated crosvm SELinux domain that `carbonhvd` may transition into. That spawn and supervision path is a later slice and is not part of this fragment yet.
