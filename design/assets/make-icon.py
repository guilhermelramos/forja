"""Gera o logo da Forja: SVG, PNG 16/32/48/256 e ICO multi-tamanho.

A geometria é a bigorna de lado com a centelha (losango) sobre um quadrado
arredondado. O gradiente vai de #F2762E a #8C2E12. A bigorna é #FFF6EE e a
centelha é #FFC14D.

Uso, na raiz do repositório ou daqui:

    python design/assets/make-icon.py
"""

from __future__ import annotations

import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent
VIEW = 32.0
RADIUS = 7.0
TOP = (0xF2, 0x76, 0x2E)
BOTTOM = (0x8C, 0x2E, 0x12)
METAL = (0xFF, 0xF6, 0xEE)
SPARK = (0xFF, 0xC1, 0x4D)
SPARK_CORE = (0xFF, 0xF6, 0xEE)

ANVIL = [
    (7, 13),
    (19, 13),
    (26, 17),
    (19, 17),
    (19, 20),
    (17, 20),
    (16, 23),
    (23, 23),
    (23, 27),
    (9, 27),
    (9, 23),
    (15, 23),
    (14, 20),
    (11, 20),
    (11, 17),
    (7, 17),
]
SPARK_SHAPE = [(16, 2.2), (19.6, 6.5), (16, 10.8), (12.4, 6.5)]
CORE_SHAPE = [(16, 4.4), (17.7, 6.5), (16, 8.6), (14.3, 6.5)]


def mix(a, b, t):
    return tuple(round(a[i] * (1 - t) + b[i] * t) for i in range(3))


def inside_round_rect(x, y, size, radius):
    half = size / 2
    dx = abs(x - half) - (half - radius)
    dy = abs(y - half) - (half - radius)
    outside = math.hypot(max(dx, 0.0), max(dy, 0.0))
    inside = min(max(dx, dy), 0.0)
    return outside + inside - radius <= 0


def inside_polygon(x, y, polygon):
    inside = False
    j = len(polygon) - 1
    for i, (xi, yi) in enumerate(polygon):
        xj, yj = polygon[j]
        if (yi > y) != (yj > y):
            cross = (xj - xi) * (y - yi) / (yj - yi) + xi
            if x < cross:
                inside = not inside
        j = i
    return inside


def shade(x, y):
    if not inside_round_rect(x, y, VIEW, RADIUS):
        return None
    if inside_polygon(x, y, CORE_SHAPE):
        return (*SPARK_CORE, 255)
    if inside_polygon(x, y, SPARK_SHAPE):
        return (*SPARK, 255)
    if inside_polygon(x, y, ANVIL):
        return (*METAL, 255)
    tone = mix(TOP, BOTTOM, y / VIEW)
    return (*tone, 255)


def render(master):
    buf = bytearray(master * master * 4)
    for y in range(master):
        py = (y + 0.5) * VIEW / master
        row = y * master
        for x in range(master):
            px = (x + 0.5) * VIEW / master
            color = shade(px, py)
            if color is None:
                continue
            i = (row + x) * 4
            buf[i : i + 4] = bytes(color)
    return buf


def downsample(master_buf, master, size):
    step = master // size
    area = step * step
    out = bytearray(size * size * 4)
    for y in range(size):
        for x in range(size):
            r = g = b = a = 0
            for sy in range(step):
                row = (y * step + sy) * master
                for sx in range(step):
                    i = (row + x * step + sx) * 4
                    r += master_buf[i]
                    g += master_buf[i + 1]
                    b += master_buf[i + 2]
                    a += master_buf[i + 3]
            o = (y * size + x) * 4
            out[o] = r // area
            out[o + 1] = g // area
            out[o + 2] = b // area
            out[o + 3] = a // area
    return bytes(out)


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def png(size, rgba):
    raw = b"".join(b"\x00" + rgba[y * size * 4 : (y + 1) * size * 4] for y in range(size))
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def ico(images):
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = b""
    blobs = b""
    for size, data in images:
        entries += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,
            0 if size >= 256 else size,
            0,
            0,
            1,
            32,
            len(data),
            offset,
        )
        offset += len(data)
        blobs += data
    return header + entries + blobs


def svg_path(points):
    head = f"M {points[0][0]} {points[0][1]}"
    rest = " ".join(f"L {x} {y}" for x, y in points[1:])
    return f"{head} {rest} Z"


def write_svg():
    body = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" role="img">
  <title>Forja</title>
  <defs>
    <linearGradient id="brasa" x1="16" y1="0" x2="16" y2="32" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#F2762E"/>
      <stop offset="1" stop-color="#8C2E12"/>
    </linearGradient>
  </defs>
  <rect width="32" height="32" rx="7" fill="url(#brasa)"/>
  <path fill="#FFC14D" d="{svg_path(SPARK_SHAPE)}"/>
  <path fill="#FFF6EE" d="{svg_path(CORE_SHAPE)}"/>
  <path fill="#FFF6EE" d="{svg_path(ANVIL)}"/>
</svg>
"""
    (ROOT / "logo.svg").write_text(body, encoding="utf-8")


def main():
    master_size = 1536
    master = render(master_size)
    encoded = []
    for size in (16, 32, 48, 256):
        rgba = downsample(master, master_size, size)
        data = png(size, rgba)
        (ROOT / f"logo-{size}.png").write_bytes(data)
        encoded.append((size, data))
    (ROOT / "logo.ico").write_bytes(ico(encoded))
    write_svg()
    print("logo.svg, logo-16/32/48/256.png e logo.ico")


if __name__ == "__main__":
    main()
