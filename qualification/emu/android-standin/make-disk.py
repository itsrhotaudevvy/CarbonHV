#!/usr/bin/env python3
"""Lays out the stand-in guest's OS disk the way Cuttlefish's assemble_cvd does, for a phone.

Cuttlefish hands its VMM one GPT disk (`os_composite`) whose partition *labels* are what
first-stage init resolves `/dev/block/by-name/<label>` against (under androidboot.boot_devices).
This rebuilds that layout from the CI bundle, in the order AndroidCompositeDiskConfig emits it
(android/cuttlefish base/cvd/.../assemble_cvd/disk/android_composite_disk_config.cc: partitions
sorted by name, A/B partitions as `_a`/`_b` pairs), plus the `frp` partition Cuttlefish keeps on
its persistent disk.

The disk is mostly holes (super is 8 GiB with 1.6 GiB of data, userdata is 8 GiB with ~70 MiB),
so it is not copied whole. Output, in --out:

  chunks.bin   every non-zero 1 MiB run of the disk, concatenated
  assemble.sh  run on the phone: truncate a sparse disk.img, dd each run into place
  layout.txt   the partition table (label, first/last LBA, bytes, source, sha256 of source)

Sources must be raw images (unsparsed with simg2img). Standard library only.
usage: make-disk.py --bundle <bundle-dir> --raw <dir with super.raw, userdata.raw> --out <dir>
"""
import argparse
import hashlib
import os
import struct
import uuid
import zlib

MIB = 1 << 20
SECTOR = 512
ENTRIES = 128
ENTRY_SIZE = 128
LINUX_FS = uuid.UUID('0fc63daf-8483-4772-8e79-3d69d8477de4')
# Deterministic GUIDs, so the same bundle always yields the same disk bytes.
NAMESPACE = uuid.UUID('6f1f6d4c-6e8f-4a59-9d3a-2b3c0a1d5e01')
AB = ('boot', 'init_boot', 'vbmeta', 'vbmeta_system', 'vbmeta_system_dlkm',
      'vbmeta_vendor_dlkm', 'vendor_boot')
BLANK_MIB = {'metadata': 64, 'misc': 1, 'frp': 1}


def round_up(value, unit):
    return (value + unit - 1) // unit * unit


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for block in iter(lambda: handle.read(4 * MIB), b''):
            digest.update(block)
    return digest.hexdigest()


def partitions(bundle, raw):
    art = os.path.join(bundle, 'artifacts')
    sources = {
        'boot': os.path.join(art, 'boot.img'),
        'init_boot': os.path.join(art, 'init_boot.img'),
        'metadata': None,
        'misc': None,
        'super': os.path.join(raw, 'super.raw'),
        'userdata': os.path.join(raw, 'userdata.raw'),
        'vbmeta': os.path.join(art, 'vbmeta.img'),
        'vbmeta_system': os.path.join(art, 'vbmeta_system.img'),
        'vbmeta_system_dlkm': os.path.join(art, 'vbmeta_system_dlkm.img'),
        'vbmeta_vendor_dlkm': os.path.join(art, 'vbmeta_vendor_dlkm.img'),
        'vendor_boot': os.path.join(art, 'vendor_boot.img'),
    }
    out = []
    for name in sorted(sources):
        labels = [name + '_a', name + '_b'] if name in AB else [name]
        out.extend((label, sources[name], name) for label in labels)
    out.append(('frp', None, 'frp'))
    return out


def nonzero_runs(path, size):
    """Yields (offset, length) of non-zero 1 MiB blocks, coalesced, skipping sparse holes."""
    with open(path, 'rb') as handle:
        fd = handle.fileno()
        run_start = None
        offset = 0
        while offset < size:
            try:
                data_at = os.lseek(fd, offset, os.SEEK_DATA)
            except OSError:
                data_at = size
            if data_at >= size:
                break
            if data_at > offset:
                if run_start is not None:
                    yield run_start, offset - run_start
                    run_start = None
                offset = data_at - data_at % MIB
            handle.seek(offset)
            block = handle.read(min(MIB, size - offset))
            if block.count(0) != len(block):
                if run_start is None:
                    run_start = offset
            elif run_start is not None:
                yield run_start, offset - run_start
                run_start = None
            offset += MIB
        if run_start is not None:
            yield run_start, min(offset, size) - run_start


def gpt(table, total):
    last_lba = total // SECTOR - 1
    entries = bytearray(ENTRIES * ENTRY_SIZE)
    for index, (label, first, last) in enumerate(table):
        entry = LINUX_FS.bytes_le + uuid.uuid5(NAMESPACE, label).bytes_le
        entry += struct.pack('<QQQ', first, last, 0)
        entry += label.encode('utf-16-le').ljust(72, b'\0')
        entries[index * ENTRY_SIZE:(index + 1) * ENTRY_SIZE] = entry
    entries_crc = zlib.crc32(entries)
    disk_guid = uuid.uuid5(NAMESPACE, 'disk').bytes_le

    def header(current, backup, entries_lba):
        fields = struct.pack('<8sIIIIQQQQ16sQIII', b'EFI PART', 0x10000, 92, 0, 0, current,
                             backup, 34, last_lba - 33, disk_guid, entries_lba, ENTRIES,
                             ENTRY_SIZE, entries_crc)
        crc = zlib.crc32(fields)
        return (fields[:16] + struct.pack('<I', crc) + fields[20:]).ljust(SECTOR, b'\0')

    mbr = bytearray(SECTOR)
    mbr[446:462] = struct.pack('<BBBBBBBBII', 0, 0, 2, 0, 0xEE, 0xFF, 0xFF, 0xFF, 1,
                               min(last_lba, 0xFFFFFFFF))
    mbr[510:512] = b'\x55\xaa'
    head = bytes(mbr) + header(1, last_lba, 2) + bytes(entries)
    tail = bytes(entries) + header(last_lba, 1, last_lba - 32)
    return head, tail


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bundle', required=True)
    parser.add_argument('--raw', required=True)
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)

    plan = []  # (label, source, size, offset)
    offset = MIB  # first MiB: protective MBR + primary GPT
    for label, source, base in partitions(args.bundle, args.raw):
        size = os.path.getsize(source) if source else BLANK_MIB[base] * MIB
        size = round_up(size, MIB)
        plan.append((label, source, size, offset))
        offset += size
    total = offset + MIB  # last MiB: backup GPT
    table = [(label, off // SECTOR, (off + size) // SECTOR - 1)
             for label, _, size, off in plan]
    head, tail = gpt(table, total)
    assert len(table) <= ENTRIES

    runs = [(0, head, None)]  # (disk offset, inline bytes or None, (source, src offset, len))
    for label, source, size, off in plan:
        if not source:
            continue
        for start, length in nonzero_runs(source, os.path.getsize(source)):
            runs.append((off + start, None, (source, start, length)))
    runs.append((total - len(tail), tail, None))

    blob_path = os.path.join(args.out, 'chunks.bin')
    script = ['#!/system/bin/sh',
              '# Generated by qualification/emu/android-standin/make-disk.py; rebuilds disk.img.',
              'set -e', 'cd "$(dirname "$0")"', 'rm -f disk.img', f'truncate -s {total} disk.img']
    blob_mib = 0
    with open(blob_path, 'wb') as blob:
        for disk_off, inline, src in runs:
            if inline is not None:
                data = inline.ljust(MIB, b'\0') if disk_off == 0 else inline
                if disk_off != 0:
                    # Backup GPT: write the whole last MiB so the dd stays MiB-granular.
                    data = bytes(MIB - len(inline)) + inline
                    disk_off = total - MIB
                blob.write(data)
                length = MIB
            else:
                source, start, length = src
                with open(source, 'rb') as handle:
                    handle.seek(start)
                    left = length
                    while left:
                        chunk = handle.read(min(left, 8 * MIB))
                        blob.write(chunk)
                        left -= len(chunk)
                pad = round_up(length, MIB) - length
                blob.write(bytes(pad))
                length = round_up(length, MIB)
            count = length // MIB
            script.append(f'dd if=chunks.bin of=disk.img bs=1048576 skip={blob_mib} '
                          f'seek={disk_off // MIB} count={count} conv=notrunc 2>/dev/null')
            blob_mib += count
    script.append('echo "disk.img assembled: $(ls -l disk.img)"')
    with open(os.path.join(args.out, 'assemble.sh'), 'w') as handle:
        handle.write('\n'.join(script) + '\n')

    with open(os.path.join(args.out, 'layout.txt'), 'w') as handle:
        handle.write(f'# GPT disk, {total} bytes ({total / 2**30:.2f} GiB), {len(plan)} '
                     f'partitions, sector {SECTOR}\n')
        handle.write('# label first_lba last_lba partition_bytes source_bytes source sha256\n')
        for (label, source, size, off), (_, first, last) in zip(plan, table):
            digest = sha256(source) if source else 'zero-filled'
            name = os.path.basename(source) if source else 'blank'
            src_len = os.path.getsize(source) if source else 0
            handle.write(f'{label} {first} {last} {size} {src_len} {name} {digest}\n')
    print(f'disk {total} bytes, {len(plan)} partitions, {len(runs)} runs, '
          f'chunks.bin {blob_mib} MiB -> {args.out}')


if __name__ == '__main__':
    main()
