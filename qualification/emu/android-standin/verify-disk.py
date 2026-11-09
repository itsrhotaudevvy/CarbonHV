#!/usr/bin/env python3
"""Checks a disk.img built by assemble.sh against layout.txt, independently of make-disk.py.

Parses the GPT the way a kernel would (protective MBR, primary + backup headers, both CRC32s),
checks every label/LBA against layout.txt, and hashes each partition's leading bytes against the
source image's sha256 (blank partitions must be all zero).

usage: verify-disk.py <disk.img> <layout.txt> [--replay <stage-dir>]
  --replay rebuilds disk.img on this host by executing the dd lines of <stage-dir>/assemble.sh
  against <stage-dir>/chunks.bin, so the exact script the phone runs is what gets verified.
Standard library only.
"""
import hashlib
import os
import re
import struct
import sys
import zlib

SECTOR = 512


def replay(stage, disk):
    lines = open(os.path.join(stage, 'assemble.sh')).read().splitlines()
    size = int(next(l for l in lines if l.startswith('truncate')).split()[2])
    with open(disk, 'wb') as out:
        out.truncate(size)
    blob = open(os.path.join(stage, 'chunks.bin'), 'rb')
    with open(disk, 'r+b') as out:
        for line in lines:
            match = re.match(r'dd if=chunks.bin of=disk.img bs=(\d+) skip=(\d+) seek=(\d+) '
                             r'count=(\d+) conv=notrunc', line)
            if not match:
                continue
            bs, skip, seek, count = map(int, match.groups())
            blob.seek(skip * bs)
            out.seek(seek * bs)
            out.write(blob.read(count * bs))


def read_header(disk, lba):
    disk.seek(lba * SECTOR)
    raw = disk.read(SECTOR)
    fields = struct.unpack_from('<8sIIIIQQQQ16sQIII', raw)
    sig, rev, hsize, crc = fields[0], fields[1], fields[2], fields[3]
    assert sig == b'EFI PART', f'no GPT signature at LBA {lba}'
    check = raw[:16] + b'\0\0\0\0' + raw[20:hsize]
    assert zlib.crc32(check) == crc, f'header CRC mismatch at LBA {lba}'
    current, backup, first_usable, last_usable = fields[5:9]
    entries_lba, count, esize, ecrc = fields[10:14]
    disk.seek(entries_lba * SECTOR)
    entries = disk.read(count * esize)
    assert zlib.crc32(entries) == ecrc, f'entries CRC mismatch for header at LBA {lba}'
    assert current == lba
    return backup, entries, count, esize


def main():
    disk_path, layout_path = sys.argv[1], sys.argv[2]
    if '--replay' in sys.argv:
        replay(sys.argv[sys.argv.index('--replay') + 1], disk_path)
    layout = [l.split() for l in open(layout_path) if not l.startswith('#')]
    size = os.path.getsize(disk_path)
    with open(disk_path, 'rb') as disk:
        mbr = disk.read(SECTOR)
        assert mbr[510:512] == b'\x55\xaa' and mbr[450] == 0xEE, 'no protective MBR'
        backup_lba, entries, count, esize = read_header(disk, 1)
        assert backup_lba == size // SECTOR - 1, 'backup header is not at the last LBA'
        _, backup_entries, _, _ = read_header(disk, backup_lba)
        assert backup_entries == entries, 'primary and backup tables differ'
        parts = []
        for index in range(count):
            entry = entries[index * esize:(index + 1) * esize]
            if entry[:16] == bytes(16):
                continue
            first, last = struct.unpack_from('<QQ', entry, 32)
            label = entry[56:128].decode('utf-16-le').rstrip('\0')
            parts.append((label, first, last))
        assert len(parts) == len(layout), f'{len(parts)} GPT entries vs {len(layout)} in layout'
        art = None
        for (label, first, last), row in zip(parts, layout):
            want_label, want_first, want_last, nbytes, src_len, source, digest = row
            assert (label, first, last) == (want_label, int(want_first), int(want_last)), row
            disk.seek(first * SECTOR)
            if digest == 'zero-filled':
                data = disk.read((last - first + 1) * SECTOR)
                assert data.count(0) == len(data), f'{label} is not blank'
                print(f'ok {label:22s} blank')
                continue
            # Hash exactly the source's length (the partition is padded to 1 MiB past it); the
            # padding itself must be zero.
            hasher = hashlib.sha256()
            left = int(src_len)
            while left:
                chunk = disk.read(min(left, 8 << 20))
                hasher.update(chunk)
                left -= len(chunk)
            got = hasher.hexdigest()
            assert got == digest, f'{label}: {got} != {digest}'
            pad = disk.read((last - first + 1) * SECTOR - int(src_len))
            assert pad.count(0) == len(pad), f'{label}: padding past the image is not zero'
            print(f'ok {label:22s} sha256 {digest[:16]}…')
    print(f'GPT valid (primary + backup), {len(parts)} partitions match {layout_path}')


main()
