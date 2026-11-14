import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# Q4: Shift-D (bypass all grades). usage: exp_bypass.py TIMELINE
from exp_dumps import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
t = timeline(p, sys.argv[1])
r.OpenPage("color")
p.SetSetting("perfRenderCacheMode", "none")
for it in t.GetItemListInTrack("video", 1):
    it.GetNodeGraph().SetNodeCacheMode(1, r.CACHE_DISABLED)
FR = 5   # a frame in clip 1 (the strongly graded ramp)
res = {}
for state in ("normal", "bypass", "normal-again"):
    if state != "normal":
        viewer.send_key("D", "SHIFT"); time.sleep(1.5)
    files, rs = dump_frame(t, FR)
    print(state, "-> probe renders for frame %d: %d (frames %s); dump files %s" % (FR, len(rs), sorted(set(int(x["frame"]) for x in rs)), [os.path.basename(f) for f in files]))
    if files:
        mx, mid, a = stats(files[-1]); res[state] = a
        print("   dump max abs %.4f, centre pixel %s" % (mx, mid))
if "normal" in res and "bypass" in res:
    print("max abs diff normal vs bypass:", diff(res["normal"], res["bypass"]))
if "normal" in res and "normal-again" in res:
    print("max abs diff normal vs normal-again:", diff(res["normal"], res["normal-again"]))
