import sys
sys.path.insert(0, "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-a9b6e642eccbe4530/harness")
from scopecreep_harness import resolve_api, projects
r = resolve_api.connect()
pm = r.GetProjectManager()
projects.load(pm, "scopecreep-test-scratch")
for n in pm.GetProjectListInCurrentFolder():
    if n.startswith("scopecreep-test-s12-uhd-"):
        print(n, pm.DeleteProject(n))
print(pm.GetProjectListInCurrentFolder())
