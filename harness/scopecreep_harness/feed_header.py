"""Read the Feed's header (feed/PROTOCOL.md) to see whether the Sender is rendering, without attaching as a reader.

The Sender counts every render call in the header, and every Sender instance records its renders in the registry,
whether or not an app is attached. So the counters move when Resolve renders a Sender node, and reading them
never makes the Sender copy a frame.

Where the Feed lives is per platform (ADR 0043): on Linux a file, on Windows a named file mapping and on macOS a
POSIX shared-memory object, each holding a locator that points at the Feed now published (ADRs 0049 and 0052,
feed/PROTOCOL.md "Locator"). `SCOPECREEP_FEED` overrides the location on all.
"""
import collections
import os
import struct
import sys


def default_location(platform, environ):
    """The Feed's location on `platform` (a `sys.platform` value): `SCOPECREEP_FEED`, else the platform's own."""
    if environ.get("SCOPECREEP_FEED"):
        return environ["SCOPECREEP_FEED"]
    return {"win32": "Local\\scopecreep-feed", "darwin": "/scopecreep-feed"}.get(platform, "/dev/shm/scopecreep-feed")


FEED_PATH = default_location(sys.platform, os.environ)
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
    """FeedHeader of the Feed at path (on Windows and macOS, the locator's name), or None if there is no Feed there."""
    if sys.platform == "win32":
        return read_located(path, _read_mapping)
    if sys.platform == "darwin":
        return read_located(path, _read_shm)
    try:
        with open(path, "rb") as f:
            return parse(f.read(HEADER_BYTES))
    except OSError:
        return None


def read_located(location, read_mapping):
    """FeedHeader of the Feed the (Windows or macOS) locator at `location` points at, or None if there is none.

    `read_mapping(name, size)` gives the first `size` bytes of the named mapping or object, or None if there is none."""
    page = read_mapping(location, 16)
    if page is None or len(page) < 16:
        return None
    current, = struct.unpack_from("<Q", page, 0)
    if current == 0:
        return None
    data = read_mapping("%s.%d" % (location, current), HEADER_BYTES)
    return parse(data) if data is not None else None


def _read_shm(name, size):
    """The first `size` bytes of the existing POSIX shared-memory object `name`, or None. Opens read-only, never
    creates: a Feed the harness made up would look to the Sender and the app like a real one."""
    import mmap
    import _posixshmem
    try:
        fd = _posixshmem.shm_open(name, os.O_RDONLY, 0o600)
    except OSError:
        return None
    try:
        if os.fstat(fd).st_size < size:
            return None
        with mmap.mmap(fd, size, mmap.MAP_SHARED, mmap.PROT_READ) as view:
            return view[:size]
    except (OSError, ValueError):
        return None
    finally:
        os.close(fd)


def _read_mapping(name, size):
    """The first `size` bytes of the existing named file mapping `name`, or None. Opens, never creates: a Feed
    the harness made up would look to the Sender and the app like a real one."""
    import ctypes
    from ctypes import wintypes
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    k32.OpenFileMappingW.restype = wintypes.HANDLE
    k32.OpenFileMappingW.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.LPCWSTR]
    k32.MapViewOfFile.restype = ctypes.c_void_p
    k32.MapViewOfFile.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.DWORD, wintypes.DWORD, ctypes.c_size_t]
    k32.UnmapViewOfFile.argtypes = [ctypes.c_void_p]
    k32.CloseHandle.argtypes = [wintypes.HANDLE]
    FILE_MAP_READ = 0x0004
    handle = k32.OpenFileMappingW(FILE_MAP_READ, False, name)
    if not handle:
        return None
    try:
        view = k32.MapViewOfFile(handle, FILE_MAP_READ, 0, 0, size)
        if not view:
            return None
        try:
            return ctypes.string_at(view, size)
        finally:
            k32.UnmapViewOfFile(view)
    finally:
        k32.CloseHandle(handle)
