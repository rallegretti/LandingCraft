#!/usr/bin/env python3
"""Make packaging/windows/landingcraft.ico from packaging/macos/AppIcon.icns.

The .icns is rendered from packaging/landingcraft.svg by packaging/macos/make-icon.swift, so rerun
that first if the SVG changes, then this. Uses only the standard library.

    python3 packaging/windows/make-icon.py

The .ico holds the 16 px image (unpacked from the icns's ARGB data) and the 32, 64, 128 and 256 px
PNGs as they are; Windows scales those for the sizes in between.
"""
import struct
from pathlib import Path

HERE = Path(__file__).resolve().parent
ICNS = HERE.parent / "macos" / "AppIcon.icns"
ICO = HERE / "landingcraft.ico"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def icns_entries(data):
    if data[:4] != b"icns":
        raise SystemExit(f"{ICNS} isn't an icns file")
    pos = 8
    while pos + 8 <= len(data):
        kind, length = data[pos:pos + 4], struct.unpack(">I", data[pos + 4:pos + 8])[0]
        yield kind, data[pos + 8:pos + length]
        pos += length


def unpack_argb(body, side):
    """icns ARGB data: 'ARGB', then the A, R, G and B planes, each run-length encoded."""
    if body[:4] != b"ARGB":
        raise ValueError("not ARGB data")
    out, pos, n = [], 4, side * side
    for _ in range(4):
        plane = bytearray()
        while len(plane) < n:
            b = body[pos]
            pos += 1
            if b & 0x80:
                plane += bytes([body[pos]]) * (b - 125)
                pos += 1
            else:
                plane += body[pos:pos + b + 1]
                pos += b + 1
        out.append(plane[:n])
    return out


def dib(planes, side):
    """A 32-bit bottom-up BMP as .ico entries store it: header, BGRA rows, empty AND mask."""
    a, r, g, b = planes
    header = struct.pack("<IiiHHIIiiII", 40, side, side * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    rows = b"".join(
        bytes(v for x in range(side) for v in (b[y * side + x], g[y * side + x], r[y * side + x], a[y * side + x]))
        for y in reversed(range(side))
    )
    mask = b"\0" * (((side + 31) // 32) * 4 * side)
    return header + rows + mask


def main():
    images = {}
    for kind, body in icns_entries(ICNS.read_bytes()):
        if body.startswith(PNG_MAGIC):
            side = struct.unpack(">I", body[16:20])[0]
            if side in (32, 64, 128, 256):
                images.setdefault(side, body)
        elif kind == b"ic04":
            images[16] = dib(unpack_argb(body, 16), 16)
    missing = {16, 32, 64, 128, 256} - images.keys()
    if missing:
        raise SystemExit(f"{ICNS} has no {sorted(missing)} px images")

    sizes = sorted(images)
    offset = 6 + 16 * len(sizes)
    directory, blobs = struct.pack("<HHH", 0, 1, len(sizes)), b""
    for side in sizes:
        blob = images[side]
        directory += struct.pack("<BBBBHHII", side % 256, side % 256, 0, 0, 1, 32, len(blob), offset)
        blobs += blob
        offset += len(blob)
    ICO.write_bytes(directory + blobs)
    print(f"Wrote {ICO} ({', '.join(f'{s}' for s in sizes)} px)")


if __name__ == "__main__":
    main()
