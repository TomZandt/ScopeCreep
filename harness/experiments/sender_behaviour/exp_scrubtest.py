import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# usage: exp_scrubtest.py TIMELINE [clipnode: on|auto|off|keep] [project cache mode|keep]
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
t = timeline(p, sys.argv[1])
cn = sys.argv[2] if len(sys.argv) > 2 else "keep"
if len(sys.argv) > 3 and sys.argv[3] != "keep":
    p.SetSetting("perfRenderCacheMode", sys.argv[3])
print("page", r.GetCurrentPage(), "project cache mode", p.GetSetting("perfRenderCacheMode"))
if cn != "keep":
    v = {"auto": r.CACHE_AUTO_ENABLED, "on": r.CACHE_ENABLED, "off": r.CACHE_DISABLED}[cn]
    for it in t.GetItemListInTrack("video", 1):
        it.GetNodeGraph().SetNodeCacheMode(1, v)
print("clip node cache modes", [it.GetNodeGraph().GetNodeCacheMode(1) for it in t.GetItemListInTrack("video", 1)])
time.sleep(3)
for f in (30, 31, 32, 10, 10):
    n0 = nlines()
    t.SetCurrentTimecode(tc(f)); time.sleep(1.5)
    rs = renders(lines_since(n0))
    print("goto frame", f, "-> renders", [(x["inst"], int(x["frame"]), x["w"]) for x in rs])
