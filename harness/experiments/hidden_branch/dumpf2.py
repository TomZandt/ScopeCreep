import sys, os, time, threading
sys.path.insert(0, "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-ae523918ca419f48a/harness")
from scopecreep_harness import resolve_api
r = resolve_api.connect()
p = r.GetProjectManager().GetCurrentProject()
t = p.GetCurrentTimeline()
D = "/tmp/scopecreep-probe"
trig = D + "/dump.trigger"
stop = False
def rearm():
    while not stop:
        if not os.path.exists(trig):
            open(trig, "w").close()
        time.sleep(0.001)
th = threading.Thread(target=rearm)
for tc in sys.argv[1:]:
    t.SetCurrentTimecode("01:00:03:20")
    time.sleep(1.5)
    stop = False
    th = threading.Thread(target=rearm); th.start()
    t.SetCurrentTimecode(tc)
    time.sleep(1.5)
    stop = True; th.join()
    time.sleep(0.2)
    if os.path.exists(trig): os.unlink(trig)
