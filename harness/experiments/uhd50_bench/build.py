"""Build the UHD 50 fps benchmark project in the scopecreep folder: one timeline with the Sender on its timeline
node (from the s19 .drp; needs the Sender on that node, see README "Test projects") and a matching
timeline "no-sender" without it. Then turn on loop playback in Resolve and run run.py."""
import os
import sys

sys.path.insert(0, "/home/tom/GitHub/scopecreep/harness")
from scopecreep_harness import paths, projects  # noqa: E402
from scopecreep_harness.resolve_api import connect  # noqa: E402

NAME = "scopecreep-test-bench50"
r = connect()
pm = r.GetProjectManager()
projects.enter_folder(pm)
for stale in ("scopecreep-test-bench-yrgb-50-plain", "scopecreep-test-scratch", NAME):
    if stale in pm.GetProjectListInCurrentFolder():
        cur = pm.GetCurrentProject()
        if cur and cur.GetName() == stale:
            pm.CloseProject(cur)
        projects.enter_folder(pm)
        print("deleted", stale, pm.DeleteProject(stale))
projects.enter_folder(pm)
assert pm.ImportProject(os.path.join(paths.ROOT, "drp", "s19-yrgb-rec709g24.drp"), NAME)
p = projects.load(pm, NAME)
tls = {p.GetTimelineByIndex(i).GetName(): p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1)}
print("timelines", list(tls))
tl = tls.get("sender") or p.GetTimelineByIndex(1)
p.SetCurrentTimeline(tl)
items = tl.GetItemListInTrack("video", 1)
if items:
    tl.DeleteClips(items, False)
for k, v in [("timelineFrameRate", "50"), ("timelinePlaybackFrameRate", "50"),
             ("timelineResolutionWidth", "3840"), ("timelineResolutionHeight", "2160"),
             ("timelineOutputResolutionWidth", "3840"), ("timelineOutputResolutionHeight", "2160"),
             ("perfRenderCacheMode", "none")]:
    p.SetSetting(k, v)
tl.SetSetting("useCustomSettings", "0")
if str(tl.GetSetting("timelineFrameRate")).split(".")[0] != "50":
    tl.SetSetting("useCustomSettings", "1")
    tl.SetSetting("timelineFrameRate", "50")
mp = p.GetMediaPool()
got = mp.ImportMedia([{"FilePath": os.path.join(paths.MEDIA, "uhd", "uhd.%04d.exr"), "StartIndex": 1, "EndIndex": 120}])
assert got, "UHD clip import failed"
added = mp.AppendToTimeline(got * 6)  # 6 x 120 frames = 14.4 s at 50 fps
g = tl.GetNodeGraph()
print("rate", tl.GetSetting("timelineFrameRate"), "size", tl.GetSetting("timelineResolutionWidth"),
      tl.GetSetting("timelineResolutionHeight"), "clips", len(added or []),
      "frames", tl.GetEndFrame() - tl.GetStartFrame(),
      "timeline nodes", g.GetNumNodes(), [g.GetNodeLabel(i) for i in range(1, g.GetNumNodes() + 1)],
      "cache", p.GetSetting("perfRenderCacheMode"))
plain = mp.CreateTimelineFromClips("no-sender", [got[0]])
p.SetCurrentTimeline(plain)
while plain.GetEndFrame() - plain.GetStartFrame() < tl.GetEndFrame() - tl.GetStartFrame():
    mp.AppendToTimeline([got[0]])  # same length as the Sender timeline, so both loop alike
print("no-sender frames", plain.GetEndFrame() - plain.GetStartFrame())
p.SetCurrentTimeline(tl)
pm.SaveProject()
r.OpenPage("color")
