"""Shared helpers for the issue #11 experiments (Sender behaviour in Resolve).
Run any script here with `python3 <script>` (resolve_api re-execs under Resolve's interpreter)."""
import os, re, sys, time, glob, threading
HARNESS = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
sys.path.insert(0, HARNESS)
from scopecreep_harness import resolve_api, viewer  # noqa: E402

D = "/tmp/scopecreep-probe"
LOG = D + "/probe.log"


def connect():
    r = resolve_api.connect()
    return r, r.GetProjectManager()


def open_project(pm, name):
    cur = pm.GetCurrentProject()
    p = cur if (cur and cur.GetName() == name) else pm.LoadProject(name)
    assert p, name
    return p


def timeline(p, name=None):
    if name:
        for i in range(1, p.GetTimelineCount() + 1):
            t = p.GetTimelineByIndex(i)
            if t.GetName() == name:
                p.SetCurrentTimeline(t)
    return p.GetCurrentTimeline()


def tc(f):
    return "01:00:%02d:%02d" % (f // 24, f % 24)


def nlines():
    try:
        return sum(1 for _ in open(LOG))
    except OSError:
        return 0


def lines_since(n):
    return open(LOG).read().splitlines()[n:]


RE = re.compile(r"\[(\d+\.\d+) pid=\d+ tid=(\d+)\] RENDER inst=(\d+) page=(.*?) time=([\d.]+) .*?interactive=(\S+) draft=(\S+) thumbnail=(\S+)")


def renders(lines):
    out = []
    for l in lines:
        m = RE.match(l)
        if m:
            out.append(dict(t=float(m[1]), tid=int(m[2]), inst=int(m[3]), frame=float(m[5]), inter=m[6], draft=m[7], thumb=m[8],
                            w=int(re.search(r"render_window=\d+,\d+,(\d+),(\d+)", l)[1])))
    return out
