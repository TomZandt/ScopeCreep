import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import feed_header


def header(renders_total=0, renders_thumbnail=0, registry=(), name=b"scopecreep Sender"):
    """A Feed header laid out by hand from feed/PROTOCOL.md (version 1.2)."""
    b = bytearray(4096)
    b[0:8] = b"SCFEED\0\0"
    struct.pack_into("<II", b, 8, 1, 2)
    struct.pack_into("<q", b, 48, 4242)
    b[64:64 + len(name)] = name
    struct.pack_into("<QQ", b, 352, renders_total, renders_thumbnail)
    for i, (state, renders) in enumerate(registry):
        struct.pack_into("<I", b, 1024 + 128 * i, state)
        struct.pack_into("<Q", b, 1024 + 128 * i + 48, renders)
    return bytes(b)


class FeedHeader(unittest.TestCase):
    def test_reads_the_sender_name_and_pid(self):
        h = feed_header.parse(header())
        self.assertEqual(h.sender_name, "scopecreep Sender")
        self.assertEqual(h.sender_pid, 4242)

    def test_counts_non_thumbnail_renders_of_the_owner(self):
        self.assertEqual(feed_header.parse(header(renders_total=12, renders_thumbnail=2)).renders, 10)

    def test_counts_renders_recorded_in_live_registry_entries(self):
        # A guest Sender process records its renders only in the registry; free entries are ignored.
        h = feed_header.parse(header(renders_total=3, registry=[(2, 7), (0, 99), (2, 8)]))
        self.assertEqual(h.renders, 15)

    def test_not_a_feed(self):
        self.assertIsNone(feed_header.parse(b"\0" * 4096))
        self.assertIsNone(feed_header.parse(b"SCFEED"))

    def test_read_missing_file(self):
        self.assertIsNone(feed_header.read("/nonexistent/scopecreep-feed"))


if __name__ == "__main__":
    unittest.main()
