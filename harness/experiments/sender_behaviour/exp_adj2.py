import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
p = open_project(pm, "scopecreep-test-dwg-rec709")
mp = p.GetMediaPool()
base = timeline(p, "scopecreep-dwg-rec709")
# remove old scratch
old = [p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1) if p.GetTimelineByIndex(i).GetName().startswith("sb-")]
p.SetCurrentTimeline(base)
print("deleted old", mp.DeleteTimelines(old) if old else None)
dup = base.DuplicateTimeline("sb-adj2")
p.SetCurrentTimeline(dup)
dup.SetCurrentTimecode("01:00:00:00")
it = dup.InsertGeneratorIntoTimeline("Adjustment Clip")
print("inserted", it, it.GetStart(), it.GetEnd(), it.GetDuration())
mpi = it.GetMediaPoolItem()
print("mpi", mpi, mpi.GetName() if mpi else None, mpi.GetClipProperty() if mpi else None)
print("items", [(i.GetName(), i.GetStart(), i.GetEnd()) for i in dup.GetItemListInTrack("video", 1)])
