import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import exr, media


class Ramp(unittest.TestCase):
    def test_ramp_spans_zero_to_one_across_width(self):
        img = media.ramp(256, 64)
        top = [img.pixel(x, 0) for x in range(256)]
        self.assertEqual(top[0], (0.0, 0.0, 0.0))
        self.assertAlmostEqual(top[255][0], 1.0, places=6)
        vals = [p[0] for p in top]
        self.assertEqual(vals, sorted(vals))
        # neutral: R == G == B
        self.assertTrue(all(p[0] == p[1] == p[2] for p in top))

    def test_ramp_has_extended_range_band_below_zero_and_above_one(self):
        img = media.ramp(256, 64)
        band = [img.pixel(x, 63)[0] for x in range(256)]
        self.assertLess(min(band), 0.0)
        self.assertGreater(max(band), 1.0)


class Chart(unittest.TestCase):
    def test_patches_are_pure_values(self):
        img = media.chart(1920, 1080)
        for name, (rgb, (cx, cy)) in media.chart_patches(1920, 1080).items():
            for got, want in zip(img.pixel(cx, cy), rgb):
                self.assertAlmostEqual(got, want, places=6, msg=name)  # stored as float32

    def test_patch_set_has_primaries_secondaries_and_greys(self):
        names = set(media.chart_patches(1920, 1080))
        for n in ["red", "green", "blue", "cyan", "magenta", "yellow",
                  "black", "white", "grey18"]:
            self.assertIn(n, names)
        self.assertEqual(media.chart_patches(1920, 1080)["red"][0], (1.0, 0.0, 0.0))
        self.assertEqual(media.chart_patches(1920, 1080)["cyan"][0], (0.0, 1.0, 1.0))


class OutOfRange(unittest.TestCase):
    def test_single_pixels_at_known_positions(self):
        img = media.out_of_range(640, 360)
        pts = media.out_of_range_pixels(640, 360)
        self.assertGreaterEqual(len(pts), 4)
        for (x, y), rgb in pts.items():
            self.assertEqual(img.pixel(x, y), rgb)
        # only those pixels differ from the background
        bg = img.pixel(0, 0)
        diff = 0
        for y in range(img.height):
            for x in range(img.width):
                if img.pixel(x, y) != bg:
                    diff += 1
        self.assertEqual(diff, len(pts))

    def test_has_both_over_and_under_range(self):
        pts = media.out_of_range_pixels(640, 360)
        flat = [v for rgb in pts.values() for v in rgb]
        self.assertGreater(max(flat), 1.0)
        self.assertLess(min(flat), 0.0)


class Natural(unittest.TestCase):
    def test_deterministic_and_varied(self):
        a = media.natural(160, 90, frame=3)
        b = media.natural(160, 90, frame=3)
        self.assertEqual(a.pixel(10, 10), b.pixel(10, 10))
        vals = {round(a.pixel(x, y)[0], 3) for x in range(0, 160, 7) for y in range(0, 90, 7)}
        self.assertGreater(len(vals), 20)

    def test_frames_differ(self):
        a = media.natural(160, 90, frame=0)
        b = media.natural(160, 90, frame=5)
        self.assertNotEqual([a.pixel(x, 60) for x in range(160)],
                            [b.pixel(x, 60) for x in range(160)])

    def test_has_highlights_above_one_but_mostly_in_range(self):
        img = media.natural(160, 90, frame=0)
        flat = [v for y in range(90) for x in range(160) for v in img.pixel(x, y)]
        self.assertGreaterEqual(min(flat), 0.0)
        self.assertGreater(max(flat), 1.0)


class Sequences(unittest.TestCase):
    def test_writes_numbered_exr_sequence(self):
        with tempfile.TemporaryDirectory() as d:
            paths = media.write_sequence(d, "ramp", lambda f: media.ramp(64, 32), frames=3)
            self.assertEqual([os.path.basename(p) for p in paths],
                             ["ramp.0001.exr", "ramp.0002.exr", "ramp.0003.exr"])
            self.assertEqual(exr.read_exr(paths[0]).width, 64)

    def test_looped_sequence_uses_symlinks_beyond_unique_frames(self):
        with tempfile.TemporaryDirectory() as d:
            paths = media.write_sequence(d, "loop", lambda f: media.natural(32, 18, f),
                                         frames=6, unique=2, pixel_type="half")
            self.assertEqual(len(paths), 6)
            self.assertFalse(os.path.islink(paths[0]))
            self.assertFalse(os.path.islink(paths[1]))
            self.assertTrue(os.path.islink(paths[2]))
            self.assertEqual(os.path.realpath(paths[2]), os.path.realpath(paths[0]))
            self.assertEqual(os.path.realpath(paths[5]), os.path.realpath(paths[1]))


if __name__ == "__main__":
    unittest.main()
