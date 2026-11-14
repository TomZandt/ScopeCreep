import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
for i in range(1, p.GetTimelineCount() + 1):
    t = p.GetTimelineByIndex(i)
    g = t.GetNodeGraph()
    print(i, t.GetName(), "nodes", g.GetNumNodes(), [(g.GetNodeLabel(n), g.GetToolsInNode(n)) for n in range(1, g.GetNumNodes() + 1)])
