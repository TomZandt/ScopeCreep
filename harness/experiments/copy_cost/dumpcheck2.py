import sys, time, os
HARNESS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
sys.path.insert(0, HARNESS)
from scopecreep_harness import resolve_api
r = resolve_api.connect()
pm = r.GetProjectManager()
from scopecreep_harness import projects
p = projects.load(pm, "scopecreep-test-s12-uhd-25")
tl = p.GetCurrentTimeline()
r.OpenPage("color")
D = "/tmp/scopecreep-probe"
for f in (30, 31, 55, 77):
    open(D + "/dump.trigger", "w").close()
    tl.SetCurrentTimecode("01:00:%02d:%02d" % (f // 25, f % 25))
    time.sleep(4.0)
    print(f, "trigger consumed:", not os.path.exists(D + "/dump.trigger"))
