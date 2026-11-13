import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import dump_feed


class Helpers(unittest.TestCase):
    def test_parse_name(self):
        self.assertEqual(dump_feed.parse_dump_name("frame_inst2_t72.000_1920x1080.exr"), (2, 72.0, 1920, 1080))
        self.assertIsNone(dump_feed.parse_dump_name("probe.log"))

    def test_offsets(self):
        self.assertEqual(dump_feed.clip_offsets(), {"ramp": 0, "chart": 24, "outofrange": 48, "natural": 72})

    def test_pick_matching(self):
        files = ["/x/frame_inst2_t24.000_1920x1080.exr", "/x/frame_inst2_t0.000_288x162.exr",
                 "/x/frame_inst3_t0.000_1920x1080.exr"]
        self.assertEqual(dump_feed.pick_matching(files, 0.0, (1920, 1080)), "/x/frame_inst3_t0.000_1920x1080.exr")
        self.assertIsNone(dump_feed.pick_matching(files, 48.0, (1920, 1080)))


if __name__ == "__main__":
    unittest.main()
