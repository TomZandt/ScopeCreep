import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# Helpers: grab the probe's input frame at a frame number (dump trigger re-armed while Resolve renders the frame).
from lib import *
from scopecreep_harness import exr


def dump_frame(t, frame, other=0):
    """Move away, arm the dump trigger, jump to `frame`; return (list of EXR paths written, probe render lines)."""
    for f in glob.glob(D + "/frame_*.exr"):
        os.unlink(f)
    t.SetCurrentTimecode(tc(other)); time.sleep(1.2)
    n0 = nlines()
    stop = [False]
    trig = D + "/dump.trigger"

    def rearm():
        while not stop[0]:
            if not os.path.exists(trig):
                open(trig, "w").close()
            time.sleep(0.001)
    th = threading.Thread(target=rearm); th.start()
    t.SetCurrentTimecode(tc(frame)); time.sleep(1.5)
    stop[0] = True; th.join(); time.sleep(0.3)
    if os.path.exists(trig):
        os.unlink(trig)
    return sorted(glob.glob(D + "/frame_*.exr")), renders(lines_since(n0))


def stats(path):
    a = exr.read_exr(path)
    rows = a.rows
    mx = max(max(abs(v) for v in ch) for rw in rows for ch in rw)
    mid = [round(rows[len(rows) // 2][c][len(rows[0][0]) // 2], 5) for c in range(3)]
    return mx, mid, a


def diff(a, b):
    m = 0.0
    for ra, rb in zip(a.rows, b.rows):
        for ca, cb in zip(ra, rb):
            for x, y in zip(ca, cb):
                m = max(m, abs(x - y))
    return m
