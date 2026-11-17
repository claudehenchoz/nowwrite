#!/usr/bin/env python3
"""Create and extract FAT16 disk images for testing NowWrite in QEMU.

Standard library only. The image has an MBR with one FAT16 partition, long
file names, and subdirectories, which is all the UEFI FAT driver needs.

    fatimg.py create IMAGE SRC_DIR [--efi FILE] [--size-mb N]
    fatimg.py extract IMAGE DEST_DIR [--skip NAME ...]

`create` copies SRC_DIR into a fresh image; --efi adds FILE as
\\EFI\\BOOT\\BOOTX64.EFI. `extract` copies every file in the image back into
DEST_DIR (overwriting, never deleting), skipping top-level entries named
with --skip.
"""

import argparse
import datetime
import os
import struct
import sys

SECTOR = 512
SECTORS_PER_CLUSTER = 4
CLUSTER = SECTOR * SECTORS_PER_CLUSTER
RESERVED = 4
NUM_FATS = 2
ROOT_ENTRIES = 512
PART_START = 2048  # partition offset in sectors (1 MiB)
EOC = 0xFFFF

ATTR_DIR = 0x10
ATTR_ARCHIVE = 0x20
ATTR_LFN = 0x0F
ATTR_VOLUME = 0x08


# ---------------------------------------------------------------- create


def fat_datetime(ts):
    t = datetime.datetime.fromtimestamp(ts)
    year = max(t.year, 1980)
    date = ((year - 1980) << 9) | (t.month << 5) | t.day
    time = (t.hour << 11) | (t.minute << 5) | (t.second // 2)
    return date, time


def lfn_checksum(short):
    s = 0
    for b in short:
        s = (((s & 1) << 7) + (s >> 1) + b) & 0xFF
    return s


def is_plain_83(name):
    """Name that is already a valid upper-case 8.3 name (no LFN needed)."""
    if name in (".", ".."):
        return True
    base, dot, ext = name.partition(".")
    allowed = set("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-~!#$%&'(){}^@`")
    return (
        0 < len(base) <= 8
        and len(ext) <= 3
        and "." not in ext
        and all(c in allowed for c in base + ext)
    )


def short_name(name, used):
    if is_plain_83(name):
        base, _, ext = name.partition(".")
        short = base.ljust(8).encode() + ext.ljust(3).encode()
    else:
        ext = name.rsplit(".", 1)[1] if "." in name.lstrip(".") else ""
        ext = "".join(c for c in ext.upper() if c.isalnum())[:3]
        n = len(used) + 1
        while True:
            short = f"NW~{n:05d}".ljust(8).encode() + ext.ljust(3).encode()
            if short not in used:
                break
            n += 1
    used.add(short)
    return short


def dir_entries(name, short, attr, cluster, size, mtime):
    """Directory entries (LFN entries first if needed) for one item."""
    out = b""
    if not is_plain_83(name):
        units = list(name.encode("utf-16-le"))
        chars = [units[i] | (units[i + 1] << 8) for i in range(0, len(units), 2)]
        chars.append(0)
        while len(chars) % 13:
            chars.append(0xFFFF)
        chunks = [chars[i : i + 13] for i in range(0, len(chars), 13)]
        csum = lfn_checksum(short)
        for seq in range(len(chunks), 0, -1):
            c = chunks[seq - 1]
            order = seq | (0x40 if seq == len(chunks) else 0)
            out += struct.pack(
                "<B10sBBB12sH4s",
                order,
                struct.pack("<5H", *c[0:5]),
                ATTR_LFN,
                0,
                csum,
                struct.pack("<6H", *c[5:11]),
                0,
                struct.pack("<2H", *c[11:13]),
            )
    date, time = fat_datetime(mtime)
    out += struct.pack(
        "<11sBBBHHHHHHHI", short, attr, 0, 0, time, date, date, 0, time, date, cluster, size
    )
    return out


class Builder:
    def __init__(self, total_sectors):
        self.total_sectors = total_sectors
        root_sectors = ROOT_ENTRIES * 32 // SECTOR
        # Solve for FAT size: each cluster needs 2 bytes of FAT.
        data_guess = total_sectors - RESERVED - root_sectors
        clusters = data_guess // SECTORS_PER_CLUSTER
        self.fat_sectors = (2 * (clusters + 2) + SECTOR - 1) // SECTOR
        self.data_start = RESERVED + NUM_FATS * self.fat_sectors + root_sectors
        self.clusters = (total_sectors - self.data_start) // SECTORS_PER_CLUSTER
        if not 4085 <= self.clusters < 65525:
            sys.exit("image size out of FAT16 range; use --size-mb between 16 and 1000")
        self.fat = [0xFFF8, 0xFFFF] + [0] * self.clusters
        self.next_free = 2
        self.data = {}  # cluster -> bytes

    def alloc(self, content):
        """Store content in a new cluster chain; return first cluster (0 if empty)."""
        if not content:
            return 0
        count = (len(content) + CLUSTER - 1) // CLUSTER
        first = self.next_free
        if first + count > self.clusters + 2:
            sys.exit("image is full; use a larger --size-mb")
        for i in range(count):
            c = first + i
            self.fat[c] = c + 1 if i + 1 < count else EOC
            self.data[c] = content[i * CLUSTER : (i + 1) * CLUSTER]
        self.next_free += count
        return first

    def add_dir(self, items, cluster, parent_cluster, now):
        """Build a subdirectory's entries (items: list of (name, kind, payload))."""
        entries = dir_entries(".", b".          ", ATTR_DIR, cluster, 0, now)
        entries += dir_entries("..", b"..         ", ATTR_DIR, parent_cluster, 0, now)
        return entries + self.entries_for(items, cluster, now)

    def entries_for(self, items, this_cluster, now):
        used = set()
        out = b""
        for name, kind, payload, mtime in items:
            short = short_name(name, used)
            if kind == "file":
                cluster = self.alloc(payload)
                out += dir_entries(name, short, ATTR_ARCHIVE, cluster, len(payload), mtime)
            else:
                # Reserve the directory's cluster(s) first; content needs its own number.
                children = payload
                estimate = 64 * (len(children) + 2) * 32  # generous: LFNs need several entries
                count = max(1, (estimate + CLUSTER - 1) // CLUSTER)
                first = self.next_free
                self.next_free += count
                for i in range(count):
                    self.fat[first + i] = first + i + 1 if i + 1 < count else EOC
                content = self.add_dir(children, first, this_cluster, mtime)
                for i in range(count):
                    self.data[first + i] = content[i * CLUSTER : (i + 1) * CLUSTER]
                out += dir_entries(name, short, ATTR_DIR, first, 0, mtime)
        return out

    def image(self, root_items):
        now = datetime.datetime.now().timestamp()
        root = dir_entries("NOWWRITE", b"NOWWRITE   ", ATTR_VOLUME, 0, 0, now)
        root += self.entries_for(root_items, 0, now)
        if len(root) > ROOT_ENTRIES * 32:
            sys.exit("too many files in the root directory")

        boot = bytearray(SECTOR)
        boot[0:3] = b"\xEB\x3C\x90"
        boot[3:11] = b"NOWWRITE"
        struct.pack_into(
            "<HBHBHHBHHHII", boot, 11,
            SECTOR, SECTORS_PER_CLUSTER, RESERVED, NUM_FATS, ROOT_ENTRIES,
            0, 0xF8, self.fat_sectors, 63, 255, PART_START, self.total_sectors,
        )
        struct.pack_into("<BBBI11s8s", boot, 36, 0x80, 0, 0x29, 0x4E57_5752, b"NOWWRITE   ", b"FAT16   ")
        boot[510:512] = b"\x55\xAA"

        fat_bytes = struct.pack(f"<{len(self.fat)}H", *self.fat)
        fat_bytes += bytes(self.fat_sectors * SECTOR - len(fat_bytes))

        part = bytearray(self.total_sectors * SECTOR)
        part[0:SECTOR] = boot
        off = RESERVED * SECTOR
        for _ in range(NUM_FATS):
            part[off : off + len(fat_bytes)] = fat_bytes
            off += len(fat_bytes)
        part[off : off + len(root)] = root
        base = self.data_start * SECTOR
        for cluster, content in self.data.items():
            pos = base + (cluster - 2) * CLUSTER
            part[pos : pos + len(content)] = content

        mbr = bytearray(SECTOR)
        # One partition: bootable=0, type 0x0E (FAT16 LBA), CHS fields unused.
        struct.pack_into("<B3sB3sII", mbr, 446, 0, b"\xFE\xFF\xFF", 0x0E, b"\xFE\xFF\xFF", PART_START, self.total_sectors)
        mbr[510:512] = b"\x55\xAA"
        return bytes(mbr) + bytes((PART_START - 1) * SECTOR) + bytes(part)


def scan(path):
    items = []
    for name in sorted(os.listdir(path)):
        full = os.path.join(path, name)
        mtime = os.path.getmtime(full)
        if os.path.isdir(full):
            items.append((name, "dir", scan(full), mtime))
        else:
            with open(full, "rb") as f:
                items.append((name, "file", f.read(), mtime))
    return items


def create(args):
    items = scan(args.src) if os.path.isdir(args.src) else []
    if args.efi:
        with open(args.efi, "rb") as f:
            efi = f.read()
        now = datetime.datetime.now().timestamp()
        items = [i for i in items if i[0].upper() != "EFI"]
        items.insert(0, ("EFI", "dir", [("BOOT", "dir", [("BOOTX64.EFI", "file", efi, now)], now)], now))
    total = args.size_mb * 1024 * 1024 // SECTOR
    data = Builder(total).image(items)
    with open(args.image, "wb") as f:
        f.write(data)


# ---------------------------------------------------------------- extract


class Reader:
    def __init__(self, data):
        self.data = data
        start = 0
        if data[510:512] == b"\x55\xAA" and data[0:3] not in (b"\xEB\x3C\x90",) and data[450] != 0:
            start = struct.unpack_from("<I", data, 454)[0] * SECTOR
        self.base = start
        (bps, spc, reserved, nfats, root_entries, _, _, fat_size) = struct.unpack_from("<HBHBHHBH", data, start + 11)
        if bps != SECTOR or fat_size == 0:
            sys.exit("not a FAT16 image created by this tool")
        self.spc = spc
        self.fat_off = start + reserved * bps
        self.root_off = self.fat_off + nfats * fat_size * bps
        self.root_len = root_entries * 32
        self.data_off = self.root_off + self.root_len
        self.cluster_size = spc * bps

    def chain(self, cluster):
        out = bytearray()
        seen = set()
        while 2 <= cluster < 0xFFF8 and cluster not in seen:
            seen.add(cluster)
            pos = self.data_off + (cluster - 2) * self.cluster_size
            out += self.data[pos : pos + self.cluster_size]
            cluster = struct.unpack_from("<H", self.data, self.fat_off + 2 * cluster)[0]
        return bytes(out)

    def entries(self, raw):
        lfn = {}
        for i in range(0, len(raw) - 31, 32):
            e = raw[i : i + 32]
            if e[0] == 0x00:
                break
            if e[0] == 0xE5:
                lfn = {}
                continue
            attr = e[11]
            if attr == ATTR_LFN:
                seq = e[0] & 0x1F
                units = e[1:11] + e[14:26] + e[28:32]
                lfn[seq] = units
                continue
            if attr & ATTR_VOLUME:
                lfn = {}
                continue
            if lfn:
                units = b"".join(lfn[k] for k in sorted(lfn))
                name = units.decode("utf-16-le", "replace").split("\0", 1)[0]
            else:
                base = e[0:8].decode("ascii", "replace").rstrip()
                ext = e[8:11].decode("ascii", "replace").rstrip()
                name = base + ("." + ext if ext else "")
                # NT case flags (byte 12): lower-case base / extension.
                if e[12] & 0x08:
                    name = base.lower() + ("." + ext if ext else "")
                if e[12] & 0x10 and ext:
                    name = name.rsplit(".", 1)[0] + "." + ext.lower()
            lfn = {}
            if name in (".", ".."):
                continue
            cluster = struct.unpack_from("<H", e, 26)[0]
            size = struct.unpack_from("<I", e, 28)[0]
            yield name, bool(attr & ATTR_DIR), cluster, size

    def walk(self, raw, dest, skip, count):
        for name, is_dir, cluster, size in self.entries(raw):
            if name in skip:
                continue
            target = os.path.join(dest, name)
            if is_dir:
                os.makedirs(target, exist_ok=True)
                count = self.walk(self.chain(cluster), target, (), count)
            else:
                with open(target, "wb") as f:
                    f.write(self.chain(cluster)[:size])
                count += 1
        return count


def extract(args):
    with open(args.image, "rb") as f:
        reader = Reader(f.read())
    os.makedirs(args.dest, exist_ok=True)
    root = reader.data[reader.root_off : reader.root_off + reader.root_len]
    count = reader.walk(root, args.dest, set(args.skip or ()), 0)
    print(f"extracted {count} file(s) to {args.dest}")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("create")
    c.add_argument("image")
    c.add_argument("src")
    c.add_argument("--efi")
    c.add_argument("--size-mb", type=int, default=64)
    c.set_defaults(func=create)
    x = sub.add_parser("extract")
    x.add_argument("image")
    x.add_argument("dest")
    x.add_argument("--skip", action="append")
    x.set_defaults(func=extract)
    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
