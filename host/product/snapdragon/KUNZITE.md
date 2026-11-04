# Clean Snapdragon route: Redmi `kunzite` (Snapdragon 6 Gen 3)

This route does not use root or any third-party code. It patches the stock `vendor_boot` image with exactly one line, `androidboot.hypervisor.vm.supported=1`, and makes no other change.

Read-only probes on 2026-11-04 (`qualification/gpu/reports/redmi-kunzite-sm6475-host-probe-2026-11-04.txt`) showed everything else is already in place:

- **SELinux:** the stock vendor policy already contains `(typeattributeset vm_manager_device_type (vendor_gunyah_dev))`, so AVF's crosvm may open `/dev/gunyah`.
- **crosvm:** the APEX crosvm is built with the `gunyah` feature.
- **Shell permissions:** the ADB shell holds `MANAGE_`, `USE_CUSTOM_` and `DEBUG_VIRTUAL_MACHINE`.
- **Guest images:** the AVF APEX ships Microdroid (kernel, initrd, super), which is enough for a first boot test.

What is still unknown: whether Qualcomm's Gunyah resource manager accepts a **non-protected** VM on a retail phone. Step 5 is the experiment that answers it.

## 0. Prerequisites and risk

- The bootloader is unlocked (verified boot state `orange`), as already observed.
- Back up your data. Flashing `vbmeta` with verification disabled can make some devices demand a factory reset on the next boot.
- Use the **exact** stock fastboot ROM for the running build: `kunzite_global`, build `OS3.0.304.0.WPQCNXM`, fingerprint `Redmi/kunzite/kunzite:16/BP2A.250605.031.A3/OS3.0.304.0.WPQCNXM:user/release-keys`. Images from any other build can soft-brick the phone. Download only from Xiaomi, and check the archive's published MD5 or SHA.
- Rollback is always possible while the bootloader stays unlocked (step 6).

## 1. Patch `vendor_boot`

```bash
python3 host/product/snapdragon/patch_vendor_boot.py images/vendor_boot.img vendor_boot.carbonhv.img
```

The patcher:

- refuses anything that isn't a v3/v4 `vendor_boot`;
- writes the line into bootconfig (v4) or the vendor cmdline (v3);
- keeps the image size and every other byte, including the AVB footer;
- reports how many bytes changed, which for v4 is only the bootconfig text and its size field.

## 2. Flash

Both `vbmeta.img` and `vendor_boot.carbonhv.img` must come from the same stock ROM.

```bash
adb reboot bootloader
fastboot getvar current-slot                       # observed: a
fastboot --disable-verity --disable-verification flash vbmeta images/vbmeta.img
fastboot flash vendor_boot_a vendor_boot.carbonhv.img   # use the current slot
fastboot reboot
```

## 3. Confirm the declaration

```bash
adb shell getprop ro.boot.hypervisor.vm.supported        # expect: 1
adb shell /apex/com.android.virt/bin/vm info             # must no longer say "VMs are not supported"
qualification/carbon-qualify.sh --adb <serial>           # expect: avf_vendor_path=declared, EXPERIMENTAL
```

## 4. Optional: THP

Stock firmware runs transparent huge pages as `never`. The broker then simply omits huge-page advice. On this route THP stays as it is, since changing it needs a vendor-image or root change.

## 5. First boot test (Gate 1 smoke)

```bash
adb shell /apex/com.android.virt/bin/vm run-microdroid --debug full --mem 1024 --cpus 2
adb shell /apex/com.android.virt/bin/vm list
```

Record the outcome in a new report under `qualification/gpu/reports/`, whatever it is:

- **Success:** Microdroid reaches its console or payload, and `vm list` shows it. Repeat 20 times for Gate 1.
- **The resource manager refuses a non-protected VM:** record the exact error. This route then ends here, and a full custom host build is needed instead.

Only an actually running CarbonHV guest goes into the README screenshots taken on this phone.

## 6. Roll back to stock

```bash
adb reboot bootloader
fastboot flash vendor_boot_a images/vendor_boot.img
fastboot flash vbmeta images/vbmeta.img     # stock flags: verification enabled again
fastboot reboot
```

## Limits of this route

The platform-signed CarbonHV host integration (`system_ext`, `android.uid.system`) cannot be installed on Xiaomi firmware without Xiaomi's platform key. So this route proves Gunyah VM execution through AVF, but not the full CarbonHV app session. That still needs a controlled host build (`host/product/README.md`).
