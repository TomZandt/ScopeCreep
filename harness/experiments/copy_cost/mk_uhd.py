"""usage: mk_uhd.py FPS  -> creates scratch project scopecreep-test-s12-uhd-<fps> (from the yrgb drp) with UHD media, probe on timeline node."""
import sys, time
HARNESS = "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-a9b6e642eccbe4530/harness"
sys.path.insert(0, HARNESS)
from scopecreep_harness import resolve_api, projects, testmedia, dump

fps = sys.argv[1]
name = "scopecreep-test-s12-uhd-" + fps.replace(".", "")
r = resolve_api.connect()
pm = r.GetProjectManager()
if name in (pm.GetProjectListInCurrentFolder() or []):
    projects.delete_project(pm, name)
drp = "/home/tom/scopecreep-testdata/drp/scopecreep-test-yrgb-rec709g24.drp"
print("import", pm.ImportProject(drp, name))
p = projects.load(pm, name)
print("loaded", p and p.GetName())
tl = p.GetCurrentTimeline() or p.GetTimelineByIndex(1)
print("timelines", p.GetTimelineCount(), tl.GetName(), "items", len(tl.GetItemListInTrack("video", 1)))
mp = p.GetMediaPool()
# empty the timeline
items = tl.GetItemListInTrack("video", 1)
print("delete clips", tl.DeleteClips(items, False))
print("items now", len(tl.GetItemListInTrack("video", 1) or []))
# try frame rate / resolution on project and timeline
for k, v in [("timelineFrameRate", fps), ("timelinePlaybackFrameRate", fps),
             ("timelineResolutionWidth", "3840"), ("timelineResolutionHeight", "2160"),
             ("timelineOutputResolutionWidth", "3840"), ("timelineOutputResolutionHeight", "2160")]:
    print("project set", k, v, p.SetSetting(k, v), "->", p.GetSetting(k))
print("timeline follows project:", tl.SetSetting("useCustomSettings", "0"), tl.GetSetting("timelineFrameRate"), tl.GetSetting("timelineResolutionWidth"))
if str(tl.GetSetting("timelineFrameRate")) not in (fps, fps + ".0", "%.3f" % float(fps)):
    for k, v in [("useCustomSettings", "1"), ("timelineFrameRate", fps)]:
        print("timeline set", k, v, tl.SetSetting(k, v), "->", tl.GetSetting(k))
got = mp.ImportMedia([{"FilePath": testmedia.clip_dir("uhd") + "/uhd.%04d.exr", "StartIndex": 1, "EndIndex": 120}])
print("import uhd", got)
print("append", len(mp.AppendToTimeline(got * 30) or []))
print("cache mode before", p.GetSetting("perfRenderCacheMode"), "set none:", p.SetSetting("perfRenderCacheMode", "none"), p.GetSetting("perfRenderCacheMode"))
tl2 = p.GetCurrentTimeline()
print("timeline", tl2.GetName(), "items", len(tl2.GetItemListInTrack("video", 1)), "fps", tl2.GetSetting("timelineFrameRate"),
      tl2.GetSetting("timelineResolutionWidth"), tl2.GetSetting("timelineResolutionHeight"))
print("tree", dump.dump_project(r, p)["timelineNodeTree"])
pm.SaveProject()
