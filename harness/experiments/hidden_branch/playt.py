import sys, time, re, collections
sys.path.insert(0, "/home/tom/GitHub/scopecreep/.claude/worktrees/agent-ae523918ca419f48a/harness")
from scopecreep_harness import resolve_api, viewer
r = resolve_api.connect()
p = r.GetProjectManager().GetCurrentProject()
name = sys.argv[1]; runs = int(sys.argv[2])
for i in range(1, p.GetTimelineCount() + 1):
    t = p.GetTimelineByIndex(i)
    if t.GetName() == name: p.SetCurrentTimeline(t)
t = p.GetCurrentTimeline()
LOG = "/tmp/scopecreep-probe/probe.log"
for run in range(runs):
    t.SetCurrentTimecode("01:00:00:00"); time.sleep(2)
    n0 = sum(1 for _ in open(LOG))
    t0 = time.time()
    viewer.send_key("L")
    time.sleep(5.5)
    viewer.send_key("K")
    time.sleep(1)
    lines = open(LOG).read().splitlines()[n0:]
    d = collections.defaultdict(list); gaps = collections.defaultdict(list); cp = collections.defaultdict(list)
    for l in lines:
        m = re.match(r"\[([\d.]+)\b.*RENDER inst=(\d+) page=.*? time=([\d.]+)", l)
        if m: d[int(m[2])].append((float(m[1]), float(m[3])))
        m = re.search(r"RENDER inst=(\d+) .*copy_ms=([\d.]+)", l)
        if m: cp[int(m[1])].append(float(m[2]))
    print("run", run, name, "t_end", t.GetCurrentTimecode())
    for i, v in sorted(d.items()):
        times = sorted(set(x[1] for x in v))
        span = v[-1][0] - v[0][0]
        ts = [x[0] for x in v]
        first = {}
        for ts_, fr in v: first.setdefault(fr, ts_)
        fs = sorted(first.items(), key=lambda x: x[1])
        dts = [b[1] - a[1] for a, b in zip(fs, fs[1:])]
        mean = sum(dts) / len(dts) if dts else 0
        c = cp[i]
        print("  inst", i, "renders", len(v), "distinct frames", len(times), "of 96; first-render span %.2fs; mean interval between new frames %.1f ms (24fps=41.7); copy_ms mean %.2f max %.2f" % (fs[-1][1] - fs[0][1], mean * 1000, sum(c) / len(c), max(c)))
