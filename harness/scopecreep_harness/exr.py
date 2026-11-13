"""Minimal OpenEXR scanline writer/reader: uncompressed RGB, 32-bit float or 16-bit half.

Resolve reads these as float EXR sequences. Written from the public file-format spec.
"""
import struct
import sys
from array import array

from .image import Image

PIXEL_TYPE = {"half": 1, "float": 2}  # EXR pixel type ids
_BYTES = {1: 2, 2: 4}


def _attr(name, typ, data):
    return name.encode() + b"\0" + typ.encode() + b"\0" + struct.pack("<i", len(data)) + data


def _row_bytes(row, pixel_type):
    if pixel_type == "float":
        a = array("f", row)
        if sys.byteorder == "big":
            a.byteswap()
        return a.tobytes()
    return struct.pack("<%de" % len(row), *row)


def write_exr(path, image, pixel_type="float"):
    ptype = PIXEL_TYPE[pixel_type]
    w, h = image.width, image.height
    chans = b"".join(
        n.encode() + b"\0" + struct.pack("<iBBBBii", ptype, 0, 0, 0, 0, 1, 1) for n in ("B", "G", "R")
    ) + b"\0"
    header = b"".join([
        _attr("channels", "chlist", chans),
        _attr("compression", "compression", b"\0"),
        _attr("dataWindow", "box2i", struct.pack("<4i", 0, 0, w - 1, h - 1)),
        _attr("displayWindow", "box2i", struct.pack("<4i", 0, 0, w - 1, h - 1)),
        _attr("lineOrder", "lineOrder", b"\0"),
        _attr("pixelAspectRatio", "float", struct.pack("<f", 1.0)),
        _attr("screenWindowCenter", "v2f", struct.pack("<2f", 0.0, 0.0)),
        _attr("screenWindowWidth", "float", struct.pack("<f", 1.0)),
    ]) + b"\0"
    line_payload = w * 3 * _BYTES[ptype]
    start = 8 + len(header) + 8 * h
    cache = {}
    with open(path, "wb") as f:
        f.write(b"\x76\x2f\x31\x01" + struct.pack("<i", 2))
        f.write(header)
        f.write(b"".join(struct.pack("<q", start + y * (8 + line_payload)) for y in range(h)))
        for y in range(h):
            r, g, b = image.rows[y]
            key = (id(r), id(g), id(b))
            body = cache.get(key)
            if body is None:
                body = _row_bytes(b, pixel_type) + _row_bytes(g, pixel_type) + _row_bytes(r, pixel_type)
                if len(cache) < 4096:
                    cache[key] = body
            f.write(struct.pack("<ii", y, line_payload))
            f.write(body)


def read_exr(path):
    with open(path, "rb") as f:
        data = f.read()
    assert data[:4] == b"\x76\x2f\x31\x01", "not an EXR"
    pos = 8
    ptypes = {}
    box = None
    while data[pos] != 0:
        end = data.index(b"\0", pos)
        name = data[pos:end].decode()
        end2 = data.index(b"\0", end + 1)
        size = struct.unpack_from("<i", data, end2 + 1)[0]
        val = data[end2 + 5:end2 + 5 + size]
        pos = end2 + 5 + size
        if name == "channels":
            p = 0
            while val[p] != 0:
                e = val.index(b"\0", p)
                ptypes[val[p:e].decode()] = struct.unpack_from("<i", val, e + 1)[0]
                p = e + 17
        elif name == "dataWindow":
            box = struct.unpack("<4i", val)
        elif name == "compression":
            assert val == b"\0", "only uncompressed EXR supported"
    pos += 1
    w, h = box[2] - box[0] + 1, box[3] - box[1] + 1
    order = sorted(ptypes)
    offsets = struct.unpack_from("<%dq" % h, data, pos)
    rows = [None] * h
    for off in offsets:
        y, _size = struct.unpack_from("<ii", data, off)
        p = off + 8
        planes = {}
        for ch in order:
            t = ptypes[ch]
            n = w * _BYTES[t]
            chunk = data[p:p + n]
            p += n
            if t == 2:
                a = array("f")
                a.frombytes(chunk)
                planes[ch] = a
            else:
                planes[ch] = array("f", struct.unpack("<%de" % w, chunk))
        rows[y - box[1]] = (planes["R"], planes["G"], planes["B"])
    return Image(w, h, rows)
