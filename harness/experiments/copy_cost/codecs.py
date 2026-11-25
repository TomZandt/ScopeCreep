import os
import sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
from scopecreep_harness import resolve_api
r = resolve_api.connect()
p = r.GetProjectManager().GetCurrentProject()
fm = p.GetRenderFormats()
print(fm)
for f in ("mov", "mp4", "mxf", "avi"):
    if f in fm.values():
        print(f, p.GetRenderCodecs(f))
