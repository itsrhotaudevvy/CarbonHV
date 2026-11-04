"""Tests for patch_vendor_boot.py on synthetic vendor_boot v3/v4 images (bootimg.h layout)."""

import os
import struct
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import patch_vendor_boot as pvb  # noqa: E402

PAGE = 4096


def pad(data, page=PAGE):
    return data + b"\0" * (pvb.align(len(data), page) - len(data))


def make_image(version, bootconfig=b"", cmdline=b"console=hvc0", trailer=b""):
    ramdisk, dtb, table = b"RAMDISK" * 50, b"DTB" * 40, b"TABLE" * 20
    header = bytearray(pvb.V3_HEADER + (16 if version == 4 else 0))
    struct.pack_into("<8sIIIII", header, 0, pvb.MAGIC, version, PAGE, 0x8000, 0x1000000, len(ramdisk))
    header[pvb.CMDLINE_OFFSET:pvb.CMDLINE_OFFSET + len(cmdline)] = cmdline
    struct.pack_into("<I16sIIQ", header, pvb.CMDLINE_OFFSET + pvb.CMDLINE_SIZE, 0x100, b"kunzite",
                     len(header), len(dtb), 0x1f00000)
    body = pad(bytes(header)) + pad(ramdisk) + pad(dtb)
    if version == 4:
        struct.pack_into("<IIII", header, pvb.V3_HEADER, len(table), 1, 108, len(bootconfig))
        body = pad(bytes(header)) + pad(ramdisk) + pad(dtb) + pad(table) + pad(bootconfig)
    return body + b"\0" * PAGE + trailer


class PatchVendorBootTest(unittest.TestCase):
    def test_v4_adds_the_line_to_bootconfig_and_changes_nothing_else(self):
        stock = make_image(4, b"androidboot.hardware=qcom\nandroidboot.selinux=enforcing\n",
                           trailer=b"AVBf" + b"\x01" * 60)
        patched, changed = pvb.patch(stock)
        self.assertTrue(changed)
        self.assertEqual(len(patched), len(stock))
        self.assertEqual(patched[-64:], stock[-64:])  # AVB footer bytes untouched
        info = pvb.parse(patched)
        text = patched[info["bootconfig_offset"]:info["bootconfig_offset"] + info["bootconfig_size"]].decode()
        self.assertEqual(text.splitlines(), ["androidboot.hardware=qcom", "androidboot.selinux=enforcing", pvb.LINE])
        diff = [i for i, (a, b) in enumerate(zip(stock, patched)) if a != b]
        allowed = set(range(pvb.V3_HEADER + 12, pvb.V3_HEADER + 16)) | set(
            range(info["bootconfig_offset"], info["bootconfig_offset"] + info["bootconfig_size"]))
        self.assertTrue(diff and set(diff) <= allowed)

    def test_patching_is_idempotent(self):
        once, _ = pvb.patch(make_image(4, b"androidboot.hardware=qcom\n"))
        twice, changed = pvb.patch(once)
        self.assertFalse(changed)
        self.assertEqual(once, twice)

    def test_a_conflicting_value_is_replaced_not_duplicated(self):
        patched, _ = pvb.patch(make_image(4, b"androidboot.hypervisor.vm.supported=0\nandroidboot.x=1\n"))
        info = pvb.parse(patched)
        text = patched[info["bootconfig_offset"]:info["bootconfig_offset"] + info["bootconfig_size"]].decode()
        self.assertEqual(text.count("androidboot.hypervisor.vm.supported"), 1)
        self.assertIn(pvb.LINE, text.splitlines())

    def test_protected_vm_settings_are_never_added(self):
        patched, _ = pvb.patch(make_image(4, b"androidboot.hardware=qcom\n"))
        self.assertNotIn(b"protected_vm", patched)

    def test_v3_uses_the_vendor_cmdline(self):
        stock = make_image(3, cmdline=b"console=hvc0 androidboot.hardware=qcom")
        patched, changed = pvb.patch(stock)
        self.assertTrue(changed)
        field = patched[pvb.CMDLINE_OFFSET:pvb.CMDLINE_OFFSET + pvb.CMDLINE_SIZE].split(b"\0", 1)[0]
        self.assertEqual(field.decode().split(), ["console=hvc0", "androidboot.hardware=qcom", pvb.LINE])
        self.assertEqual(stock[pvb.CMDLINE_OFFSET + pvb.CMDLINE_SIZE:], patched[pvb.CMDLINE_OFFSET + pvb.CMDLINE_SIZE:])

    def test_non_padding_bytes_are_never_overwritten(self):
        stock = bytearray(make_image(4, b"a" * (PAGE - 4)))
        info = pvb.parse(bytes(stock))
        end = info["bootconfig_offset"] + info["bootconfig_size"]
        stock[end:end + PAGE] = b"\xff" * PAGE  # no zero padding left after bootconfig
        with self.assertRaises(pvb.PatchError):
            pvb.patch(bytes(stock))

    def test_other_images_are_refused(self):
        with self.assertRaises(pvb.PatchError):
            pvb.patch(b"ANDROID!" + b"\0" * 8192)
        bad = bytearray(make_image(4))
        struct.pack_into("<I", bad, 8, 2)
        with self.assertRaises(pvb.PatchError):
            pvb.patch(bytes(bad))

    def test_command_line_writes_output_and_reports(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = os.path.join(tmp, "in.img"), os.path.join(tmp, "out.img")
            with open(src, "wb") as handle:
                handle.write(make_image(4, b"androidboot.hardware=qcom\n"))
            script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "patch_vendor_boot.py")
            result = subprocess.run([sys.executable, "-I", script, src, dst], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("patched: header v4", result.stdout)
            with open(dst, "rb") as handle:
                self.assertIn(pvb.LINE.encode(), handle.read())


if __name__ == "__main__":
    unittest.main()
