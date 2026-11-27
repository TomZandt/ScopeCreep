import mmap
import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import feed_header


def header(renders_total=0, renders_thumbnail=0, registry=(), name=b"ScopeCreep"):
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
        self.assertEqual(h.sender_name, "ScopeCreep")
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

    @unittest.skipIf(sys.platform == "win32", "a Feed on Windows is a named mapping, not a file")
    def test_read_missing_file(self):
        self.assertIsNone(feed_header.read("/nonexistent/scopecreep-feed"))


def locator(current, last=None):
    """A Windows Feed locator page (feed/PROTOCOL.md, "Windows")."""
    b = bytearray(4096)
    struct.pack_into("<QQ", b, 0, current, current if last is None else last)
    return bytes(b)


class Location(unittest.TestCase):
    def test_each_platform_has_its_own_default(self):
        self.assertEqual(feed_header.default_location("linux", {}), "/dev/shm/scopecreep-feed")
        self.assertEqual(feed_header.default_location("win32", {}), "Local\\scopecreep-feed")
        self.assertEqual(feed_header.default_location("darwin", {}), "/scopecreep-feed")

    def test_scopecreep_feed_overrides_it_everywhere(self):
        for platform in ("linux", "win32", "darwin"):
            self.assertEqual(feed_header.default_location(platform, {"SCOPECREEP_FEED": "Local\\mine"}),
                             "Local\\mine")


class WindowsLocator(unittest.TestCase):
    """On Windows the location names a locator holding the number N of the Feed now published (ADR 0049)."""

    def read(self, mappings):
        return feed_header.read_located("Local\\scopecreep-feed", lambda name, size: mappings.get(name))

    def test_reads_the_feed_the_locator_points_at(self):
        h = self.read({"Local\\scopecreep-feed": locator(3, last=4),
                       "Local\\scopecreep-feed.3": header(renders_total=9),
                       "Local\\scopecreep-feed.4": header(renders_total=1)})
        self.assertEqual(h.renders, 9)

    def test_no_locator_means_no_sender(self):
        self.assertIsNone(self.read({"Local\\scopecreep-feed.1": header()}))

    def test_nothing_published_yet_means_no_sender(self):
        self.assertIsNone(self.read({"Local\\scopecreep-feed": locator(0),
                                     "Local\\scopecreep-feed.0": header()}))

    def test_a_feed_gone_since_the_locator_was_read_is_no_feed(self):
        self.assertIsNone(self.read({"Local\\scopecreep-feed": locator(2)}))


@unittest.skipIf(sys.platform == "win32", "POSIX shared memory does not exist on Windows")
class PosixLocator(unittest.TestCase):
    """On macOS the location names a POSIX shared-memory object holding the locator (ADR 0052): /scopecreep-feed
    holds N and the Feed is /scopecreep-feed.N. The same objects work on Linux, so this runs there too."""

    def setUp(self):
        import _posixshmem
        self.shm = _posixshmem
        # macOS names hold 31 bytes, so keep it short enough for ".N"
        self.location = "/scfeedtest%d" % os.getpid()
        self.made = []

    def tearDown(self):
        for name in self.made:
            try:
                self.shm.shm_unlink(name)
            except OSError:
                pass

    def publish(self, name, data):
        fd = self.shm.shm_open(name, os.O_CREAT | os.O_RDWR, 0o600)
        self.made.append(name)
        try:
            os.ftruncate(fd, 16384)  # macOS rounds an object up to a page
            # macOS shared-memory fds refuse pwrite (Illegal seek), so write through a mapping
            with mmap.mmap(fd, 16384) as view:
                view[: len(data)] = data
        finally:
            os.close(fd)

    def test_reads_a_feed_published_in_shared_memory_objects(self):
        self.publish(self.location, locator(7))
        self.publish(self.location + ".7", header(renders_total=5))
        self.assertEqual(feed_header.read_located(self.location, feed_header._read_shm).renders, 5)

    def test_no_object_means_no_feed_and_none_is_made(self):
        self.assertIsNone(feed_header.read_located(self.location, feed_header._read_shm))
        with self.assertRaises(OSError):
            self.shm.shm_open(self.location, os.O_RDONLY, 0o600)

    def test_a_locator_pointing_at_an_unlinked_feed_means_no_feed(self):
        self.publish(self.location, locator(2))
        self.assertIsNone(feed_header.read_located(self.location, feed_header._read_shm))

    @unittest.skipUnless(sys.platform == "darwin", "read() follows the locator on macOS only")
    def test_read_follows_the_locator_on_macos(self):
        self.publish(self.location, locator(1))
        self.publish(self.location + ".1", header(renders_total=3))
        self.assertEqual(feed_header.read(self.location).renders, 3)


@unittest.skipUnless(sys.platform == "win32", "named file mappings are Windows'")
class WindowsMapping(unittest.TestCase):
    def test_reads_a_feed_published_in_named_mappings(self):
        import ctypes
        from ctypes import wintypes
        k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        k32.CreateFileMappingW.restype = wintypes.HANDLE
        k32.CreateFileMappingW.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
                                           wintypes.DWORD, wintypes.LPCWSTR]
        k32.MapViewOfFile.restype = ctypes.c_void_p
        k32.MapViewOfFile.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.DWORD, wintypes.DWORD,
                                      ctypes.c_size_t]
        k32.UnmapViewOfFile.argtypes = [ctypes.c_void_p]
        k32.CloseHandle.argtypes = [wintypes.HANDLE]
        location = "Local\\scopecreep-feed-test-%d" % os.getpid()
        handles = []
        for name, data in ((location, locator(7)), (location + ".7", header(renders_total=5))):
            h = k32.CreateFileMappingW(wintypes.HANDLE(-1), None, 0x04, 0, len(data), name)  # PAGE_READWRITE
            self.assertTrue(h, ctypes.get_last_error())
            handles.append(h)
            view = k32.MapViewOfFile(h, 0x0002, 0, 0, len(data))  # FILE_MAP_WRITE
            ctypes.memmove(view, data, len(data))
            k32.UnmapViewOfFile(view)
        try:
            self.assertEqual(feed_header.read(location).renders, 5)
            self.assertIsNone(feed_header.read(location + "-absent"))
        finally:
            for h in handles:
                k32.CloseHandle(h)


if __name__ == "__main__":
    unittest.main()
