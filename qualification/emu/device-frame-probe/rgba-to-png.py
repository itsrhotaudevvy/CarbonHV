#!/usr/bin/env python3
"""Converts the frame probe's raw RGBA dumps into PNGs.

The probe writes `<w>x<h>` RGBA8888 frames with no header (the geometry is in the file name) so
that it needs no image library on the device. This turns them into PNGs with the standard library
only, which is all the review on the build host needs.

usage: rgba-to-png.py <frame-*.rgba> [more.rgba ...]
"""
import re
import struct
import sys
import zlib
from pathlib import Path

NAME = re.compile(r"(\d+)x(\d+)\.rgba$")


def png(width: int, height: int, rgba: bytes) -> bytes:
    stride = width * 4
    if len(rgba) != stride * height:
        raise SystemExit(f"pixel data is {len(rgba)} bytes, want {stride * height}")

    # One filter byte (0 = none) in front of every row, as PNG requires.
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        raw += rgba[y * stride : (y + 1) * stride]

    def chunk(kind: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", header),
            chunk(b"IDAT", zlib.compress(bytes(raw), 9)),
            chunk(b"IEND", b""),
        ]
    )


def main(paths: list[str]) -> int:
    if not paths:
        return 64
    for path in paths:
        source = Path(path)
        match = NAME.search(source.name)
        if not match:
            print(f"{source}: file name does not end in <w>x<h>.rgba", file=sys.stderr)
            return 65
        width, height = int(match.group(1)), int(match.group(2))
        target = source.with_suffix(".png")
        target.write_bytes(png(width, height, source.read_bytes()))
        print(f"{target} ({width}x{height}, {target.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
