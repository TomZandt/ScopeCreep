import sys
sys.path.insert(0, "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-a9b6e642eccbe4530/harness")
from scopecreep_harness import resolve_api
r = resolve_api.connect()
p = r.GetProjectManager().GetCurrentProject()
fm = p.GetRenderFormats()
print(fm)
for f in ("mov", "mp4", "mxf", "avi"):
    if f in fm.values():
        print(f, p.GetRenderCodecs(f))
