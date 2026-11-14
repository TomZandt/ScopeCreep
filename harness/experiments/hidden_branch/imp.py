import sys, os
sys.path.insert(0, "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-ae523918ca419f48a/harness")
from scopecreep_harness import resolve_api
r = resolve_api.connect()
pm = r.GetProjectManager()
path = "/tmp/claude-1000/sp/hb.drp"
print("export", pm.ExportProject("scopecreep-test-dwg-p3d65", path, True))
name = "scopecreep-test-hb-scratch"
print("import", pm.ImportProject(path, name))
p = pm.LoadProject(name)
print(p and p.GetName())
for i in range(1, p.GetTimelineCount() + 1):
    t = p.GetTimelineByIndex(i)
    g = t.GetNodeGraph()
    print(t.GetName(), [(g.GetNodeLabel(n), g.GetToolsInNode(n)) for n in range(1, g.GetNumNodes() + 1)])
print({k: p.GetSetting(k) for k in ("colorScienceMode", "colorSpaceTimeline", "colorSpaceOutput")})
