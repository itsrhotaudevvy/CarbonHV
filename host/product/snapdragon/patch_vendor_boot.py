#!/usr/bin/env python3
"""Declare non-protected AVF VM support in a stock vendor_boot image, changing nothing else.

    patch_vendor_boot.py <stock vendor_boot.img> <output.img>

The clean Snapdragon route (ADR-013) needs exactly one boot-time fact that stock firmware lacks:
`androidboot.hypervisor.vm.supported=1`. With it, AVF's virtualizationservice accepts VMs and crosvm
falls back from the absent /dev/kvm to /dev/gunyah.

Format (AOSP system/tools/mkbootimg/include/bootimg/bootimg.h, vendor_boot_img_hdr_v3/v4):
header, vendor ramdisk, dtb, [v4: vendor ramdisk table, bootconfig], each page aligned.

* v4: the line is added to the bootconfig section (the last section), and bootconfig_size is
  updated. Only bytes that were zero padding may be overwritten.
* v3: the line is added to the NUL-terminated 2048-byte vendor cmdline (init also imports
  androidboot.* from the kernel command line).

The output keeps the input's exact length and every other byte, including any AVB footer. That
footer is now stale, so the image only boots with vbmeta verification disabled on an unlocked
bootloader. Never request protected or trusted VMs here.
"""

import struct
import sys

MAGIC = b"VNDRBOOT"
LINE = "androidboot.hypervisor.vm.supported=1"
KEY = "androidboot.hypervisor.vm.supported"
CMDLINE_OFFSET = 8 + 4 * 5  # magic, header_version, page_size, kernel/ramdisk addr, ramdisk size
CMDLINE_SIZE = 2048
V3_HEADER = 2112
V4_FIELDS = struct.Struct("<IIII")  # table size, entry num, entry size, bootconfig size


class PatchError(Exception):
    pass


def align(value, page):
    return (value + page - 1) // page * page


def parse(image):
    if image[:8] != MAGIC:
        raise PatchError("not a vendor_boot image (missing VNDRBOOT magic)")
    version, page_size = struct.unpack_from("<II", image, 8)
    if version not in (3, 4):
        raise PatchError(f"unsupported vendor_boot header version {version}")
    if page_size == 0 or page_size & (page_size - 1):
        raise PatchError(f"invalid page size {page_size}")
    (ramdisk_size,) = struct.unpack_from("<I", image, 8 + 16)
    header_size, dtb_size = struct.unpack_from("<II", image, CMDLINE_OFFSET + CMDLINE_SIZE + 4 + 16)
    info = {
        "version": version,
        "page": page_size,
        "ramdisk_size": ramdisk_size,
        "header_size": header_size,
        "dtb_size": dtb_size,
    }
    if version == 4:
        table_size, _, _, bootconfig_size = V4_FIELDS.unpack_from(image, V3_HEADER)
        offset = align(header_size, page_size) + align(ramdisk_size, page_size) + align(dtb_size, page_size)
        offset += align(table_size, page_size)
        info.update(bootconfig_offset=offset, bootconfig_size=bootconfig_size)
        if offset + bootconfig_size > len(image):
            raise PatchError("bootconfig section extends past the end of the image")
    return info


def updated_lines(text):
    lines = [line for line in text.split("\n") if line.strip()]
    kept = [line for line in lines if line.split("=", 1)[0].strip() != KEY]
    if any(line.strip() == LINE for line in lines) and len(kept) == len(lines) - 1:
        return None  # already declared exactly once
    return kept + [LINE]


def patch(image):
    image = bytearray(image)
    info = parse(image)
    if info["version"] == 4:
        start, size = info["bootconfig_offset"], info["bootconfig_size"]
        text = bytes(image[start:start + size]).decode("ascii")
        lines = updated_lines(text)
        if lines is None:
            return bytes(image), False
        new = ("\n".join(lines) + "\n").encode("ascii")
        end = start + max(size, len(new))
        if end > len(image) or any(image[start + size:end]):
            raise PatchError("no zero padding after bootconfig to hold the new line")
        image[start:start + size] = b"\0" * size
        image[start:start + len(new)] = new
        struct.pack_into("<I", image, V3_HEADER + 12, len(new))
        return bytes(image), True

    field = bytes(image[CMDLINE_OFFSET:CMDLINE_OFFSET + CMDLINE_SIZE])
    cmdline = field.split(b"\0", 1)[0].decode("ascii")
    words = cmdline.split()
    if LINE in words and sum(w.split("=", 1)[0] == KEY for w in words) == 1:
        return bytes(image), False
    words = [w for w in words if w.split("=", 1)[0] != KEY] + [LINE]
    new = " ".join(words).encode("ascii")
    if len(new) >= CMDLINE_SIZE:
        raise PatchError("vendor cmdline has no room for the new parameter")
    image[CMDLINE_OFFSET:CMDLINE_OFFSET + CMDLINE_SIZE] = new.ljust(CMDLINE_SIZE, b"\0")
    return bytes(image), True


def main(argv):
    if len(argv) != 3:
        print(__doc__.strip().splitlines()[2].strip(), file=sys.stderr)
        return 64
    with open(argv[1], "rb") as handle:
        stock = handle.read()
    try:
        patched, changed = patch(stock)
    except PatchError as error:
        print(f"refusing to patch: {error}", file=sys.stderr)
        return 3
    with open(argv[2], "wb") as handle:
        handle.write(patched)
    changed_bytes = sum(a != b for a, b in zip(stock, patched))
    state = "patched" if changed else "already declared; copied unchanged"
    print(f"{state}: header v{parse(stock)['version']}, {changed_bytes} bytes differ, size {len(patched)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
