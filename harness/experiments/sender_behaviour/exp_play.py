import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# usage: exp_play.py TIMELINE MODE CLIPNODE(on|auto|off|keep) [warm_seconds] [play_seconds]
# Plays the timeline from frame 0 by keystroke (L), stops with K, counts probe renders per frame.
# Does NOT edit anything first, so cached frames stay cached; warm_seconds lets background caching finish.
from lib import *
tlname, mode, cn = sys.argv[1], sys.argv[2], sys.argv[3]
warm = float(sys.argv[4]) if len(sys.argv) > 4 else 6
play = float(sys.argv[5]) if len(sys.argv) > 5 else 3.0
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
t = timeline(p, tlname)
r.OpenPage("color")
p.SetSetting("perfRenderCacheMode", mode)
if cn != "keep":
    v = {"auto": r.CACHE_AUTO_ENABLED, "on": r.CACHE_ENABLED, "off": r.CACHE_DISABLED}[cn]
    for it in t.GetItemListInTrack("video", 1):
        it.GetNodeGraph().SetNodeCacheMode(1, v)
t.SetCurrentTimecode(tc(0))
time.sleep(warm)
n0 = nlines()

t0 = time.time()
viewer.send_key("L")
time.sleep(play)
viewer.send_key("K")
time.sleep(1.0)

rs = renders(lines_since(n0))
end = tc_to_frame(t.GetCurrentTimecode())
by = collections.Counter(int(x["frame"]) for x in rs)
print("config mode=%s clipnode=%s timeline=%s: played %.1fs, playhead ended at frame %d" % (mode, cn, tlname, play, end))
print("  probe renders: %d, distinct frames %d, frames 0..%d with no render: %s" % (
    len(rs), len(by), end, [f for f in range(0, end) if f not in by][:30]))
if rs:
    fs = sorted(set(int(x["frame"]) for x in rs))
    print("  first render +%.2fs, last +%.2fs; widths %s" % (rs[0]["t"] - t0, rs[-1]["t"] - t0, sorted(set(x["w"] for x in rs))))
    firsts = {}
    for x in rs: firsts.setdefault(int(x["frame"]), x["t"])
    ts = [firsts[f] for f in sorted(firsts)]
    if len(ts) > 2:
        d = [b - a for a, b in zip(ts, ts[1:])]
        print("  mean gap between first render of consecutive frames %.1f ms (24 fps = 41.7)" % (1000 * sum(d) / len(d)))
