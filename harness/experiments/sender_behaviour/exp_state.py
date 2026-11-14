import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
p = open_project(pm, sys.argv[1] if len(sys.argv) > 1 else "scopecreep-test-dwg-rec709")
t = timeline(p)
print("page", r.GetCurrentPage(), "tl", t.GetName(), "tc", t.GetCurrentTimecode(), "start", t.GetStartFrame(), "end", t.GetEndFrame())
for k in ("perfRenderCacheMode", "perfAutoRenderCacheEnable", "perfAutoRenderCacheAfterTime", "perfAutoRenderCacheComposite",
          "perfAutoRenderCacheFuEffect", "perfCacheClipsLocation", "perfRenderCacheCodec", "timelineFrameRate"):
    print(k, repr(p.GetSetting(k)))
print("CACHE consts", r.CACHE_AUTO_ENABLED, r.CACHE_DISABLED, r.CACHE_ENABLED)
g = t.GetNodeGraph()
print("node1 label", g.GetNodeLabel(1), "enabled", g.GetNodeEnabled(1), "cache", g.GetNodeCacheMode(1), "tools", g.GetToolsInNode(1))
for tr in range(1, t.GetTrackCount("video") + 1):
    for it in t.GetItemListInTrack("video", tr):
        print("clip", it.GetName(), it.GetStart(), it.GetEnd(), "colorcache", it.GetIsColorOutputCacheEnabled())
