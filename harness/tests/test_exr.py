import os
import struct
import sys
import tempfile
import unittest
from array import array

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import exr
from scopecreep_harness.image import Image


def small_image():
    r = array("f", [0.0, 0.5, 1.0, 2.5])
    g = array("f", [-0.25, 0.25, 0.75, 4.0])
    b = array("f", [1.0, 0.0, 0.125, 0.0])
    return Image(4, 2, [(r, g, b), (b, r, g)])


class ExrRoundTrip(unittest.TestCase):
    def roundtrip(self, pixel_type):
        img = small_image()
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "a.exr")
            exr.write_exr(p, img, pixel_type=pixel_type)
            with open(p, "rb") as f:
                self.assertEqual(f.read(4), b"\x76\x2f\x31\x01")
            return img, exr.read_exr(p)

    def test_float_roundtrip_is_exact(self):
        img, back = self.roundtrip("float")
        self.assertEqual((back.width, back.height), (4, 2))
        for y in range(2):
            for c in range(3):
                self.assertEqual(list(back.rows[y][c]), list(img.rows[y][c]))

    def test_half_roundtrip_exact_for_representable_values(self):
        img, back = self.roundtrip("half")
        for y in range(2):
            for c in range(3):
                self.assertEqual(list(back.rows[y][c]), list(img.rows[y][c]))

    def test_half_is_smaller_than_float(self):
        img = small_image()
        with tempfile.TemporaryDirectory() as d:
            pf, ph = os.path.join(d, "f.exr"), os.path.join(d, "h.exr")
            exr.write_exr(pf, img, pixel_type="float")
            exr.write_exr(ph, img, pixel_type="half")
            self.assertLess(os.path.getsize(ph), os.path.getsize(pf))

    def test_pixel_accessor(self):
        img = small_image()
        self.assertEqual(img.pixel(3, 0), (2.5, 4.0, 0.0))
        self.assertEqual(img.pixel(0, 1), (1.0, 0.0, -0.25))


if __name__ == "__main__":
    unittest.main()
