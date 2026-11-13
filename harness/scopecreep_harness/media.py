"""Deterministic test media: ramp, chart, out-of-range pixels, natural-looking scene.

All values are scene-referred floats written to EXR. Pure standard library.
"""
import math
import os
from array import array

from .image import Image, solid_row
from . import exr


def _row(values):
    a = array("f", values)
    return (a, a, a)  # neutral: the same array object serves R, G and B


def ramp(width, height):
    """Top 60%: 0..1 ramp. Next 20%: 21-step grey wedge. Bottom 20%: -0.25..1.25 ramp."""
    n = width - 1
    plain = _row([x / n for x in range(width)])
    steps = _row([min(x * 21 // width, 20) / 20.0 for x in range(width)])
    ext = _row([-0.25 + 1.5 * x / n for x in range(width)])
    rows = []
    for y in range(height):
        f = y / height
        rows.append(plain if f < 0.6 else steps if f < 0.8 else ext)
    return Image(width, height, rows)


_CHART_GRID = [
    [("red", (1.0, 0.0, 0.0)), ("green", (0.0, 1.0, 0.0)), ("blue", (0.0, 0.0, 1.0)),
     ("cyan", (0.0, 1.0, 1.0)), ("magenta", (1.0, 0.0, 1.0)), ("yellow", (1.0, 1.0, 0.0))],
    [("black", (0.0, 0.0, 0.0)), ("grey05", (0.05,) * 3), ("grey18", (0.18,) * 3),
     ("grey50", (0.5,) * 3), ("grey75", (0.75,) * 3), ("white", (1.0, 1.0, 1.0))],
]


def chart_patches(width, height):
    """name -> (rgb, (centre_x, centre_y)). Pure primaries/secondaries and greys."""
    out = {}
    rows, cols = len(_CHART_GRID), len(_CHART_GRID[0])
    for ri, row in enumerate(_CHART_GRID):
        for ci, (name, rgb) in enumerate(row):
            cx = (ci * width // cols + (ci + 1) * width // cols) // 2
            cy = (ri * height // rows + (ri + 1) * height // rows) // 2
            out[name] = (rgb, (cx, cy))
    return out


def chart(width, height):
    rows_n, cols = len(_CHART_GRID), len(_CHART_GRID[0])
    rows = []
    for y in range(height):
        grid_row = _CHART_GRID[y * rows_n // height]
        chans = ([], [], [])
        for ci, (_n, rgb) in enumerate(grid_row):
            span = (ci + 1) * width // cols - ci * width // cols
            for c in range(3):
                chans[c].extend([rgb[c]] * span)
        rows.append(tuple(array("f", c) for c in chans))
    # dedupe identical rows so the EXR writer can cache them
    cache = {}
    for i, r in enumerate(rows):
        key = (r[0].tobytes(), r[1].tobytes(), r[2].tobytes())
        rows[i] = cache.setdefault(key, r)
    return Image(width, height, rows)


def out_of_range_pixels(width, height):
    """(x, y) -> rgb for every pixel that differs from the 0.5 grey background."""
    return {
        (width // 4, height // 4): (2.0, 2.0, 2.0),
        (width // 2, height // 4): (-0.5, -0.5, -0.5),
        (3 * width // 4, height // 4): (1.5, 0.5, 0.5),
        (width // 4, height // 2): (0.5, -0.25, 0.5),
        (width // 2, height // 2): (10.0, 10.0, 10.0),
        (3 * width // 4, height // 2): (0.0, 0.0, 0.0),
        (width // 4, 3 * height // 4): (1.0, 1.0, 1.0),
        (3 * width // 4, 3 * height // 4): (0.5, 0.5, -1.0),
    }


def out_of_range(width, height):
    bg = solid_row(width, (0.5, 0.5, 0.5))
    pts = out_of_range_pixels(width, height)
    by_row = {}
    for (x, y), rgb in pts.items():
        by_row.setdefault(y, []).append((x, rgb))
    rows = []
    for y in range(height):
        if y in by_row:
            r, g, b = (array("f", c) for c in bg)
            for x, rgb in by_row[y]:
                r[x], g[x], b[x] = rgb
            rows.append((r, g, b))
        else:
            rows.append(bg)
    return Image(width, height, rows)


def _noise_table(n, seed):
    s = seed
    out = []
    for _ in range(n):
        s = (s * 1103515245 + 12345) & 0x7FFFFFFF
        out.append(s / 0x7FFFFFFF)
    return out


def natural(width, height, frame=0):
    """Sky with a sun (highlights above 1.0), hills, textured ground, mild grain. Deterministic."""
    sx = width * (0.25 + 0.01 * frame)
    sy = height * 0.28
    sigma = width * 0.06
    ex = [math.exp(-((x - sx) ** 2) / (2 * sigma * sigma)) for x in range(width)]
    hill = [height * (0.62 + 0.06 * math.sin(x / width * 9.0 + 0.1 * frame)
                      + 0.03 * math.sin(x / width * 31.0)) for x in range(width)]
    noise = _noise_table(width + 64, 1234)
    tex = [0.85 + 0.3 * noise[(x + 3 * frame) % width] for x in range(width)]
    rows = []
    for y in range(height):
        t = y / height
        ey = math.exp(-((y - sy) ** 2) / (2 * sigma * sigma))
        sky = (0.10 + 0.45 * t, 0.22 + 0.45 * t, 0.50 + 0.30 * t)
        g = 0.55 + 0.9 * (t - 0.6)
        ground = (0.10 * g, 0.18 * g, 0.05 * g)
        r_row, g_row, b_row = array("f"), array("f"), array("f")
        for x in range(width):
            if y < hill[x]:
                glow = 4.0 * ex[x] * ey
                r_row.append(sky[0] + glow)
                g_row.append(sky[1] + glow * 0.92)
                b_row.append(sky[2] + glow * 0.75)
            else:
                k = tex[x] * (0.9 + 0.2 * noise[(y * 7 + x) % len(noise)])
                r_row.append(ground[0] * k)
                g_row.append(ground[1] * k)
                b_row.append(ground[2] * k)
        rows.append((r_row, g_row, b_row))
    return Image(width, height, rows)


def write_sequence(directory, name, make_frame, frames, unique=None, pixel_type="float"):
    """Write name.0001.exr ... Frames beyond `unique` are symlinks that loop the unique ones."""
    os.makedirs(directory, exist_ok=True)
    unique = unique or frames
    paths = []
    for i in range(frames):
        p = os.path.join(directory, "%s.%04d.exr" % (name, i + 1))
        if os.path.lexists(p):
            os.unlink(p)
        if i < unique:
            exr.write_exr(p, make_frame(i), pixel_type=pixel_type)
        else:
            os.symlink(paths[i % unique], p)
        paths.append(p)
    return paths
