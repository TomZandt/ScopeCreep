import sys
sys.path.insert(0, "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-ae523918ca419f48a/harness")
from scopecreep_harness import resolve_api, render, setups
r = resolve_api.connect()
pm = r.GetProjectManager()
name = "scopecreep-test-hb-scratch"
assert pm.GetCurrentProject().GetName() == name or pm.LoadProject(name)
p = pm.GetCurrentProject(); assert p.GetName() == name
s = [x for x in setups.SETUPS if x.key == "dwg-p3d65"][0]
disable = [int(x) for x in sys.argv[2].split(",") if x]
out = sys.argv[1]
for i in range(1, p.GetTimelineCount() + 1):
    t = p.GetTimelineByIndex(i)
    if t.GetName() == "hidden-branch": p.SetCurrentTimeline(t)
g = p.GetCurrentTimeline().GetNodeGraph()
print("enabled before", [g.GetNodeEnabled(n) for n in range(1, 5)])
for n in range(1, 5):
    g.SetNodeEnabled(n, n not in disable)
print("enabled now", [g.GetNodeEnabled(n) for n in range(1, 5)])
res = render.render_project(r, s, project_name=name, outdir=out)
print({k: v[0] for k, v in res.items() if k[1] == "exr"})
