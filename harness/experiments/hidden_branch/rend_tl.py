import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
from scopecreep_harness import resolve_api, render, setups
r = resolve_api.connect()
pm = r.GetProjectManager()
name = "scopecreep-test-hb-scratch"
s = [x for x in setups.SETUPS if x.key == "dwg-p3d65"][0]
tl = sys.argv[1]; out = sys.argv[2]
p = pm.LoadProject(name)
for i in range(1, p.GetTimelineCount() + 1):
    t = p.GetTimelineByIndex(i)
    if t.GetName() == tl: p.SetCurrentTimeline(t)
print("current", p.GetCurrentTimeline().GetName())
res = render.render_project(r, s, project_name=name, outdir=out)
for k, v in res.items(): print(k, v[0], len(v[1]))
