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

    def test_has_sender_finds_the_real_sender(self):
        self.assertTrue(clone.has_sender({"nodes": [{"label": "sender", "tools": ["OFX: ScopeCreep"]}]}))
        self.assertFalse(clone.has_sender({"nodes": [{"label": "sender", "tools": ["OFX: scopecreep Probe Sender"]}]}))
        self.assertFalse(clone.has_sender({"nodes": [{"label": "other", "tools": ["OFX: ScopeCreep"]}]}))
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
