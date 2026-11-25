import sys, os, time, threading, glob
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
from scopecreep_harness import resolve_api, exr
r = resolve_api.connect()
pm = r.GetProjectManager()
p = pm.GetCurrentProject(); assert p.GetName() == "scopecreep-test-hb-scratch", p.GetName()
for i in range(1, p.GetTimelineCount() + 1):
    t = p.GetTimelineByIndex(i)
    if t.GetName() == "hidden-branch": p.SetCurrentTimeline(t)
t = p.GetCurrentTimeline(); g = t.GetNodeGraph()
disable = [int(x) for x in sys.argv[1].split(",") if x]
for n in range(1, 5): g.SetNodeEnabled(n, n not in disable)
print("enabled", [g.GetNodeEnabled(n) for n in range(1, 5)])
r.OpenPage("color")
D = "/tmp/scopecreep-probe"; trig = D + "/dump.trigger"
for f in glob.glob(D + "/frame_*.exr"): os.unlink(f)
stop = False
def rearm():
    while not stop:
        if not os.path.exists(trig): open(trig, "w").close()
        time.sleep(0.001)
for tc in ("01:00:03:20", ):
    t.SetCurrentTimecode("01:00:00:05"); time.sleep(1.5)
    th = threading.Thread(target=rearm); th.start()
    t.SetCurrentTimecode("01:00:03:00"); time.sleep(1.5)
    stop = True; th.join(); time.sleep(0.3)
    if os.path.exists(trig): os.unlink(trig)
for f in sorted(glob.glob(D + "/frame_*.exr")):
    a = exr.read_exr(f)
    mx = max(max(abs(v) for v in ch) for rw in a.rows for ch in rw)
    print(os.path.basename(f), "max abs", mx, "px(480,304)", [round(a.rows[304][c][480], 4) for c in range(3)])
