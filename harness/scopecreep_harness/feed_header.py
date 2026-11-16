"""Read the Feed's header (feed/PROTOCOL.md) to see whether the Sender is rendering, without attaching as a reader.

The Sender counts every render call in the header, and every Sender instance records its renders in the registry,
whether or not an app is attached. So the counters move when Resolve renders a Sender node, and reading them
never makes the Sender copy a frame.
"""
import collections
import os
import struct

FEED_PATH = os.environ.get("SCOPECREEP_FEED") or "/dev/shm/scopecreep-feed"
MAGIC = b"SCFEED\0\0"
HEADER_BYTES = 4096
REGISTRY_OFFSET, REGISTRY_ENTRY, REGISTRY_ENTRIES = 1024, 128, 24
LIVE = 2

FeedHeader = collections.namedtuple("FeedHeader", "sender_name sender_pid renders")


def _text(b):
    return b.split(b"\0", 1)[0].decode("utf-8", "replace")


def parse(data):
    """FeedHeader from the first 4096 bytes of a Feed file, or None if it is not a Feed.

    `renders` is the Sender's non-thumbnail render calls: the owner's header counters, or the sum over live
    registry entries (which also holds guest Sender processes), whichever is larger."""
    if len(data) < HEADER_BYTES or data[:8] != MAGIC:
        return None
    pid, = struct.unpack_from("<q", data, 48)
    total, thumbs = struct.unpack_from("<QQ", data, 352)
    registry = 0
    for i in range(REGISTRY_ENTRIES):
        off = REGISTRY_OFFSET + REGISTRY_ENTRY * i
        state, = struct.unpack_from("<I", data, off)
        if state == LIVE:
            registry += struct.unpack_from("<Q", data, off + 48)[0]
    return FeedHeader(_text(data[64:128]), pid, max(total - thumbs, registry))


def read(path=FEED_PATH):
    """FeedHeader of the Feed file at path, or None if there is no Feed there."""
    try:
        with open(path, "rb") as f:
            return parse(f.read(HEADER_BYTES))
    except OSError:
        return None
