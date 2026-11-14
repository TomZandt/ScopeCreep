import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# Q4 follow-up: several frame moves and a short playback with Shift-D bypass on, then off.
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
t = timeline(p, "sb-cache")
r.OpenPage("color")
p.SetSetting("perfRenderCacheMode", "none")


def burst(label):
    n0 = nlines()
    for f in (40, 41, 42, 43, 44):
        t.SetCurrentTimecode(tc(f)); time.sleep(0.8)
    viewer.send_key("L"); time.sleep(2.0); viewer.send_key("K"); time.sleep(1.0)
    rs = renders(lines_since(n0))
    print("%-22s renders=%d distinct frames=%d" % (label, len(rs), len(set(int(x["frame"]) for x in rs))))


burst("normal")
viewer.send_key("D", "SHIFT"); time.sleep(1.5)
burst("Shift-D bypass ON")
viewer.send_key("D", "SHIFT"); time.sleep(1.5)
burst("bypass OFF again")
