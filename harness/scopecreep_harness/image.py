"""A tiny float RGB image: rows of (r, g, b) channel arrays. Rows may be shared."""
from array import array


class Image:
    def __init__(self, width, height, rows):
        assert len(rows) == height
        self.width = width
        self.height = height
        self.rows = rows  # list of (r_row, g_row, b_row), each array('f') of len width

    def pixel(self, x, y):
        r, g, b = self.rows[y]
        return (r[x], g[x], b[x])


def solid_row(width, rgb):
    return tuple(array("f", [float(v)]) * width for v in rgb)
