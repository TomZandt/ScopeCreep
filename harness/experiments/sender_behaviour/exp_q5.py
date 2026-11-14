import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# Q5 proxy: probe on a clip-level node (stands in for an Edit-page adjustment clip, which scripts cannot place above
# the footage) versus the probe on the timeline-level node, with a strong LUT applied on the timeline-level node.
# usage: exp_q5.py [lut-relative-path | none]
from exp_dumps import *
LUT = sys.argv[1] if len(sys.argv) > 1 else "Blackmagic Design/Blackmagic 4K Film to Rec709 v3.cube"
PROJ, BASE, NAME = "scopecreep-test-dwg-rec709", "scopecreep-dwg-rec709", "sb-q5"
r, pm = connect()
p = open_project(pm, PROJ)
mp = p.GetMediaPool()
base = timeline(p, BASE)
old = [p.GetTimelineByIndex(i) for i in range(1, p.GetTimelineCount() + 1) if p.GetTimelineByIndex(i).GetName() == NAME]
if old:
    p.SetCurrentTimeline(base); mp.DeleteTimelines(old)
t = base.DuplicateTimeline(NAME)
p.SetCurrentTimeline(t)
r.OpenPage("color")
p.SetSetting("perfRenderCacheMode", "none")
clip1 = t.GetItemListInTrack("video", 1)[0]
g_t = t.GetNodeGraph()
res = {}


def grab(label):
    files, rs = dump_frame(t, 5)
    by_inst = collections.defaultdict(list)
    for f in files:
        by_inst[int(os.path.basename(f).split("_inst")[1].split("_")[0])].append(f)
    print(label, "-> renders per instance:", dict(collections.Counter(x["inst"] for x in rs)), "dump files:", [os.path.basename(f) for f in files])
    return files


f0 = grab("A: timeline probe only, no timeline grade")
print("add probe to clip 1 (clip-level node):", clip1.AddEffect("ofx", [e["id"] for e in r.GetEffectsList("video") if "scopecreep" in e["name"].lower()][0]))
print("clip-level effects:", clip1.GetEffects("ofx"), "clip nodes:", clip1.GetNodeGraph().GetNumNodes())
f1 = grab("B: + clip-level probe")
if LUT != "none":
    print("SetLUT on timeline-level node 1:", g_t.SetLUT(1, LUT), "->", g_t.GetLUT(1))
    f2 = grab("C: + strong LUT on the timeline-level node")
    for f in f2:
        mx, mid, a = stats(f)
        print("   %s max %.4f centre %s" % (os.path.basename(f), mx, mid))
        res[os.path.basename(f).split("_t")[0]] = a
    names = sorted(res)
    if len(names) >= 2:
        print("max abs diff between the two probes' frames:", diff(res[names[0]], res[names[1]]), "(%s vs %s)" % tuple(names[:2]))
    g_t.SetLUT(1, "")
