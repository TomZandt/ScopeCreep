import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709"); t = timeline(p)
it = t.GetItemListInTrack("video", 1)[0]
for name, g in (("timeline", t.GetNodeGraph()), ("clip", it.GetNodeGraph())):
    print(name, "nodes", g.GetNumNodes(), [g.GetNodeLabel(i) for i in range(1, g.GetNumNodes() + 1)])
    for i in range(1, g.GetNumNodes() + 1):
        print("  node", i, "get", g.GetNodeCacheMode(i))
        for v in (r.CACHE_ENABLED, r.CACHE_DISABLED, r.CACHE_AUTO_ENABLED):
            print("   set", v, g.SetNodeCacheMode(i, v), "->", g.GetNodeCacheMode(i))
print("clip colorcache set True", it.SetColorOutputCache(True), it.GetIsColorOutputCacheEnabled())
print("fusion", it.GetIsFusionOutputCacheEnabled())
