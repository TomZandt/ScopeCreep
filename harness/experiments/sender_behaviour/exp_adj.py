import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
mp = p.GetMediaPool()
base = timeline(p, "scopecreep-dwg-rec709")
print("timelines", [p.GetTimelineByIndex(i).GetName() for i in range(1, p.GetTimelineCount() + 1)])
dup = base.DuplicateTimeline("sb-adj")
print("dup", dup)
p.SetCurrentTimeline(dup)
for name in ("Adjustment Clip", "Adjustment", "Adjustment Layer"):
    for fn in ("InsertGeneratorIntoTimeline", "InsertFusionGeneratorIntoTimeline", "InsertOFXGeneratorIntoTimeline"):
        try:
            res = getattr(dup, fn)(name)
        except Exception as e:
            res = "EXC %s" % e
        print(fn, name, "->", res)
print("tracks", dup.GetTrackCount("video"), [it.GetName() for tr in range(1, dup.GetTrackCount("video") + 1) for it in dup.GetItemListInTrack("video", tr)])
print("nodes timeline graph", dup.GetNodeGraph().GetNumNodes())
