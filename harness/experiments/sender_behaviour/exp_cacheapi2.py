import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709"); t = timeline(p)
print("mode", p.GetSetting("perfRenderCacheMode"))
for page in ("edit", "color"):
    r.OpenPage(page); time.sleep(2)
    for it in t.GetItemListInTrack("video", 1):
        print(page, it.GetName(), "set True", it.SetColorOutputCache(True), "->", it.GetIsColorOutputCacheEnabled(),
              "| fusion auto", it.SetFusionOutputCache("auto"), it.GetIsFusionOutputCacheEnabled())
