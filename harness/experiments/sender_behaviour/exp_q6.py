import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# Q6: what does the probe see when the node is disabled/enabled, the page changes, or the timeline changes?
# Reports, per action, probe lifecycle lines (CREATE/DESTROY) and renders (count, frames, instances, widths, thumbnail flag).
from lib import *
PROJ, BASE, NAME = "scopecreep-test-dwg-rec709", "scopecreep-dwg-rec709", "sb-cache"
r, pm = connect()
p = open_project(pm, PROJ)
t = timeline(p, NAME)
r.OpenPage("color")
p.SetSetting("perfRenderCacheMode", "none")
t.SetCurrentTimecode(tc(20)); time.sleep(2)
g = t.GetNodeGraph()


def step(label, fn, wait=2.5):
    n0 = nlines()
    out = fn()
    time.sleep(wait)
    ls = lines_since(n0)
    rs = renders(ls)
    life = [l.split("] ", 1)[1][:60] for l in ls if "=== CREATE" in l or "=== DESTROY" in l]
    thumbs = sum(1 for x in rs if x["thumb"] == "1")
    print("%-44s result=%-6s renders=%-3d frames=%s inst=%s widths=%s thumb=1:%d | lifecycle: %s | page now %s" % (
        label, out, len(rs), sorted(set(int(x["frame"]) for x in rs))[:6], sorted(set(x["inst"] for x in rs)),
        sorted(set(x["w"] for x in rs)), thumbs, life, r.GetCurrentPage()))


step("baseline: nudge playhead 20->21", lambda: t.SetCurrentTimecode(tc(21)))
step("timeline node DISABLED (SetNodeEnabled)", lambda: g.SetNodeEnabled(1, False))
print("   GetNodeEnabled ->", g.GetNodeEnabled(1))
step("  ...playhead 21->22 while disabled", lambda: t.SetCurrentTimecode(tc(22)))
step("timeline node RE-ENABLED", lambda: g.SetNodeEnabled(1, True))
print("   GetNodeEnabled ->", g.GetNodeEnabled(1))
step("  ...playhead 22->23", lambda: t.SetCurrentTimecode(tc(23)))
for page in ("edit", "cut", "fusion", "fairlight", "deliver", "media", "color"):
    step("OpenPage(%s) by script" % page, lambda page=page: r.OpenPage(page), wait=3)
    step("  ...playhead move on %s" % page, lambda: t.SetCurrentTimecode(tc(30 + len(page))), wait=2)
step("keystroke Shift+3 (Edit page)", lambda: viewer.send_key("3", "SHIFT"), wait=3)
step("keystroke Shift+5 (Color page)", lambda: viewer.send_key("5", "SHIFT"), wait=3)
other = [p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1) if p.GetTimelineByIndex(i).GetName() == BASE][0]
step("switch timeline -> %s" % BASE, lambda: p.SetCurrentTimeline(other), wait=3)
step("switch timeline back -> %s" % NAME, lambda: p.SetCurrentTimeline(t), wait=3)
