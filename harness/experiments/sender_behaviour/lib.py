"""Experiment helpers: settings, playhead sampling, log summarising."""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa: F401,F403
import collections


class Sampler(threading.Thread):
    """Samples the timeline playhead (script API) so probe render times can be compared with it."""

    def __init__(self, timeline, interval=0.05):
        super().__init__(daemon=True)
        self.t, self.interval, self.samples, self.stop_flag = timeline, interval, [], False

    def run(self):
        while not self.stop_flag:
            try:
                self.samples.append((time.time(), self.t.GetCurrentTimecode()))
            except Exception:
                pass
            time.sleep(self.interval)

    def stop(self):
        self.stop_flag = True
        self.join()

    def playhead_at(self, ts):
        best = None
        for s in self.samples:
            if s[0] <= ts:
                best = s
        return best[1] if best else None


def tc_to_frame(tcs):
    h, m, s, f = [int(x) for x in tcs.split(":")]
    return ((h * 60 + m) * 60 + s) * 24 + f - 3600 * 24  # frame index within the timeline (starts at 01:00:00:00)


def summarise(label, lines, playhead_frame=None, sampler=None):
    rs = renders(lines)
    frames = collections.Counter(int(r["frame"]) for r in rs)
    insts = collections.Counter(r["inst"] for r in rs)
    tids = collections.Counter(r["tid"] for r in rs)
    thumbs = sum(1 for r in rs if r["thumb"] == "1")
    widths = collections.Counter(r["w"] for r in rs)
    print("--- %s: %d RENDER lines; inst=%s tids=%s thumbnail=1: %d; render widths=%s" % (
        label, len(rs), dict(insts), dict(tids), thumbs, dict(widths)))
    if rs:
        print("    distinct frames %d: %s" % (len(frames), sorted(frames)[:40]))
        if playhead_frame is not None:
            off = sum(c for f, c in frames.items() if f != playhead_frame)
            print("    renders at playhead frame %d: %d; at other frames: %d" % (playhead_frame, frames.get(playhead_frame, 0), off))
    return rs
