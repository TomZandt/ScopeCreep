import sys, time
sys.path.insert(0, "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-ae523918ca419f48a/harness")
from scopecreep_harness import resolve_api
r = resolve_api.connect()
p = r.GetProjectManager().GetCurrentProject()
t = p.GetCurrentTimeline()
order = list(range(96))
if sys.argv[1] == "rev": order = order[::-1]
if sys.argv[1] == "rand":
    import random; random.seed(1); order = random.sample(order, 96)
gap = float(sys.argv[2])
print("start", time.time())
for f in order:
    tc = "01:00:%02d:%02d" % (f // 24, f % 24)
    t.SetCurrentTimecode(tc)
    time.sleep(gap)
print("end", time.time())
