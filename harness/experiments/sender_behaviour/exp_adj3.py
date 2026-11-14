import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
mp = p.GetMediaPool()
base = timeline(p, "scopecreep-dwg-rec709")
old = [p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1) if p.GetTimelineByIndex(i).GetName().startswith("sb-")]
p.SetCurrentTimeline(base)
if old:
    print("deleted old", mp.DeleteTimelines(old))
for variant in ("plain", "v1disabled"):
    dup = base.DuplicateTimeline("sb-adj-" + variant)
    p.SetCurrentTimeline(dup)
    print(variant, "add track", dup.AddTrack("video"), dup.GetTrackCount("video"))
    if variant == "v1disabled":
        print("disable V1", dup.SetTrackEnable("video", 1, False))
    dup.SetCurrentTimecode("01:00:00:00")
    it = dup.InsertGeneratorIntoTimeline("Adjustment Clip")
    print("inserted", it)
    for tr in (1, 2):
        print("  V%d" % tr, [(i.GetName(), i.GetStart(), i.GetEnd()) for i in dup.GetItemListInTrack("video", tr)])
    dup.SetTrackEnable("video", 1, True)
