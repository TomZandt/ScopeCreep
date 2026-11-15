import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import clone, setups
from scopecreep_harness.image import Image


class Helpers(unittest.TestCase):
    def test_targets_exclude_source_and_perf(self):
        keys = [s.key for s in clone.clone_targets()]
        self.assertNotIn("dwg-p3d65", keys)
        self.assertNotIn("perf-uhd", keys)
        self.assertEqual(len(keys), len(setups.SETUPS) - 1)
        self.assertEqual([s.key for s in clone.clone_targets({"acescct-pq"})], ["acescct-pq"])

    def test_diff_settings(self):
        self.assertEqual(clone.diff_settings({"a": "1", "b": 2}, {"a": "1", "b": "2"}), [])
        self.assertEqual(len(clone.diff_settings({"a": "1"}, {"a": "2"})), 1)
        self.assertEqual(len(clone.diff_settings({"a": "1"}, {})), 1)

    def test_count_probe_renders_ignores_detail_lines(self):
        log = ("[1.1 pid=1] RENDER inst=4 page=<null set> time=0.000 src_frame=1\n"
               "[1.1 pid=1] RENDER inst=4   src: depth=x\n"
               "[1.2 pid=1] unhandled action: OfxImageEffectActionEndSequenceRender\n"
               "[1.3 pid=1] RENDER inst=4 page=<null set> time=1.000 src_frame=2\n")
        self.assertEqual(clone.count_probe_renders(log), 2)

    def test_has_sender(self):
        tree = {"nodes": [{"label": "sender", "tools": ["OFX: scopecreep Probe Sender"]}]}
        self.assertTrue(clone.has_sender(tree))
        self.assertFalse(clone.has_sender({"nodes": [{"label": "", "tools": []}]}))
        self.assertFalse(clone.has_sender(None))

    def test_max_abs_diff(self):
        a = Image(1, 1, [([0.0], [0.5], [1.0])])
        b = Image(1, 1, [([0.0], [0.5], [1.25])])
        self.assertEqual(clone.max_abs_diff(a, a), 0.0)
        self.assertEqual(clone.max_abs_diff(a, b), 0.25)
        self.assertEqual(clone.max_abs_diff(a, Image(2, 1, [([0.0, 0.0], [0.0, 0.0], [0.0, 0.0])])), float("inf"))

    def test_frame_to_tc(self):
        self.assertEqual(clone._frame_to_tc(86400, 24), "01:00:00:00")
        self.assertEqual(clone._frame_to_tc(86400 + 25, 24), "01:00:01:01")


if __name__ == "__main__":
    unittest.main()
