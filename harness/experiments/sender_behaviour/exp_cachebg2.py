import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# usage: exp_cachebg2.py MODE SECONDS [name]   (timeline copy with an Adjustment Clip inserted on V1; park playhead; idle)
from lib import *
mode, secs = sys.argv[1], float(sys.argv[2])
name = sys.argv[3] if len(sys.argv) > 3 else "sb-adjcache"
PROJ, BASE = "scopecreep-test-dwg-rec709", "scopecreep-dwg-rec709"
r, pm = connect()
p = open_project(pm, PROJ)
mp = p.GetMediaPool()
base = timeline(p, BASE)
old = [p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1) if p.GetTimelineByIndex(i).GetName() == name]
if old:
    p.SetCurrentTimeline(base); mp.DeleteTimelines(old)
t = base.DuplicateTimeline(name)
p.SetCurrentTimeline(t)
p.SetSetting("perfRenderCacheMode", "none")
t.SetCurrentTimecode("01:00:00:00")
t.SetTrackEnable("video", 1, True)
t.AddTrack("video"); t.SetTrackEnable("video", 1, False)
adj = t.InsertGeneratorIntoTimeline("Adjustment Clip")
t.SetTrackEnable("video", 1, True)
print("adjustment inserted", bool(adj), [(i.GetName(), i.GetStart(), i.GetEnd()) for i in t.GetItemListInTrack("video", 1)])
r.OpenPage("color")
p.SetSetting("perfAutoRenderCacheEnable", "1"); p.SetSetting("perfAutoRenderCacheAfterTime", "1")
t.SetCurrentTimecode(tc(130))
time.sleep(4)
n0 = nlines()
s = Sampler(t); s.start()
t0 = time.time()
print("mode", mode, p.SetSetting("perfRenderCacheMode", mode), p.GetSetting("perfRenderCacheMode"))
time.sleep(secs)
s.stop()
rs = summarise("%s mode=%s parked at %s" % (name, mode, tc(130)), lines_since(n0), 130)
print("playhead samples", sorted(set(x[1] for x in s.samples)))
for x in rs[:5] + rs[-3:]:
    print("   +%.3fs inst=%d tid=%d frame=%d w=%d" % (x["t"] - t0, x["inst"], x["tid"], x["frame"], x["w"]))
if rs:
    print("render span %.2fs for %d renders" % (rs[-1]["t"] - rs[0]["t"], len(rs)))
