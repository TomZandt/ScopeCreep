import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# Remove the scratch timelines (sb-*) and restore cache settings on the harness project used for issue #11.
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
mp = p.GetMediaPool()
tls = [p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1)]
base = [t for t in tls if t.GetName() == "scopecreep-dwg-rec709"][0]
p.SetCurrentTimeline(base)
junk = [t for t in tls if t.GetName().startswith("sb-")]
print("deleting", [t.GetName() for t in junk], mp.DeleteTimelines(junk))
for it in base.GetItemListInTrack("video", 1):
    it.GetNodeGraph().SetNodeCacheMode(1, r.CACHE_AUTO_ENABLED)
for k, v in (("perfRenderCacheMode", "user"), ("perfAutoRenderCacheEnable", "1"), ("perfAutoRenderCacheAfterTime", "5")):
    p.SetSetting(k, v)
base.GetNodeGraph().SetNodeEnabled(1, True)
base.SetCurrentTimecode("01:00:00:00")
pm.SaveProject()
print("timelines now", [p.GetTimelineByIndex(i).GetName() for i in range(1, p.GetTimelineCount() + 1)], "page", r.GetCurrentPage())
