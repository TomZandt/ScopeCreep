import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# usage: exp_cachebg.py MODE(smart|user|none) CLIPNODE(on|auto|off) SECONDS [name]
# Fresh duplicate of the base timeline, caching configured first, then every clip's grade is reset (an edit that
# invalidates any cache and makes Resolve cache again). The playhead stays parked at frame 10; every probe render is
# logged with its frame number so background-cache renders (frame != playhead) can be seen.
from lib import *
mode, clipnode, secs = sys.argv[1], sys.argv[2], float(sys.argv[3])
name = sys.argv[4] if len(sys.argv) > 4 else "sb-cache"
PROJ, BASE = "scopecreep-test-dwg-rec709", "scopecreep-dwg-rec709"
r, pm = connect()
T0 = time.time(); N0 = nlines()
marks = []
def mark(m): marks.append((time.time(), m))
p = open_project(pm, PROJ)
mp = p.GetMediaPool()
base = timeline(p, BASE)
old = [p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1) if p.GetTimelineByIndex(i).GetName() == name]
if old:
    p.SetCurrentTimeline(base)
    mp.DeleteTimelines(old)
mark("dup"); t = base.DuplicateTimeline(name)
p.SetCurrentTimeline(t); mark("set current")
r.OpenPage("color")
p.SetSetting("perfAutoRenderCacheEnable", "1"); p.SetSetting("perfAutoRenderCacheAfterTime", "1")
v = {"auto": r.CACHE_AUTO_ENABLED, "on": r.CACHE_ENABLED, "off": r.CACHE_DISABLED}[clipnode]
for it in t.GetItemListInTrack("video", 1):
    it.GetNodeGraph().SetNodeCacheMode(1, v)
p.SetSetting("perfRenderCacheMode", mode); mark("mode set")
t.SetCurrentTimecode(tc(10))
time.sleep(6)  # let anything triggered by the setup finish
print("config: project mode=%s, clip node cache=%s" % (p.GetSetting("perfRenderCacheMode"), clipnode),
      [it.GetNodeGraph().GetNodeCacheMode(1) for it in t.GetItemListInTrack("video", 1)])
n0 = nlines()
s = Sampler(t); s.start()
t0 = time.time()
for it in t.GetItemListInTrack("video", 1):
    print("Pan", it.SetProperty("Pan", (time.time() % 1000) / 10.0))   # the edit: a unique clip transform, so nothing for it is cached yet
mark("edit done"); time.sleep(secs)
s.stop()
rs = summarise("%s mode=%s clipnode=%s, playhead parked at frame 10 (%s)" % (name, mode, clipnode, tc(10)), lines_since(n0), 10)
print("playhead samples", sorted(set(x[1] for x in s.samples)))
for x in rs[:4] + rs[-2:]:
    print("   +%.3fs inst=%d tid=%d frame=%d inter=%s draft=%s thumb=%s w=%d" % (x["t"] - t0, x["inst"], x["tid"], x["frame"], x["inter"], x["draft"], x["thumb"], x["w"]))
if rs:
    print("render span %.2fs for %d renders (%.1f ms/render)" % (rs[-1]["t"] - rs[0]["t"], len(rs), 1000 * (rs[-1]["t"] - rs[0]["t"]) / max(1, len(rs) - 1)))

allr = renders(lines_since(N0))
print("whole-script timeline (all renders since script start):", [(round(t - T0, 1), m) for t, m in marks])
import itertools
bursts = []
for x in allr:
    if bursts and x["t"] - bursts[-1][-1]["t"] < 1.0: bursts[-1].append(x)
    else: bursts.append([x])
for b in bursts:
    fr = sorted(set(int(x["frame"]) for x in b))
    print("  burst at +%.1fs: %d renders, inst=%s, frames %s..%s (%d distinct), span %.2fs, widths=%s" % (
        b[0]["t"] - T0, len(b), sorted(set(x["inst"] for x in b)), fr[0], fr[-1], len(fr), b[-1]["t"] - b[0]["t"], sorted(set(x["w"] for x in b))))
