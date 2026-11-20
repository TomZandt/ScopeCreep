import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import dump_feed, feed_frames


class CaptureArgs(unittest.TestCase):
    def test_one_frame_per_clip_at_its_first_frame(self):
        args = feed_frames.capture_args("/bin/cap", "/out", "scopecreep-test-dwg-rec709", dump_feed.clip_offsets(), 130)
        self.assertEqual(args, ["/bin/cap", "--out", "/out", "--project", "scopecreep-test-dwg-rec709", "--timeout", "130",
                                "--frame", "ramp=0", "--frame", "chart=24", "--frame", "outofrange=48",
                                "--frame", "natural=72"])

    def test_only_the_sender_counts(self):
        self.assertTrue(feed_frames.is_sender_tool("OFX: ScopeCreep"))
        self.assertFalse(feed_frames.is_sender_tool("OFX: scopecreep Probe Sender"))
        self.assertFalse(feed_frames.is_sender_tool("OFX: Gaussian Blur"))


if __name__ == "__main__":
    unittest.main()
