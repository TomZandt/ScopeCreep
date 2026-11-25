import os
import sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
from scopecreep_harness import resolve_api, projects
r = resolve_api.connect()
pm = r.GetProjectManager()
projects.load(pm, "scopecreep-test-scratch")
for n in pm.GetProjectListInCurrentFolder():
    if n.startswith("scopecreep-test-s12-uhd-"):
        print(n, pm.DeleteProject(n))
print(pm.GetProjectListInCurrentFolder())
