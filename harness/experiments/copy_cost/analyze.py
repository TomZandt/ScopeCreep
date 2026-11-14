"""usage: analyze.py [glob]  -> one row per run in results/"""
import glob, json, re, sys, time, statistics, datetime, os

def pct(xs, q):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(q * len(xs)))] if xs else float("nan")

def dmon_avg(path, w0, w1):
    off = time.time() - time.monotonic()
    rows = []
    for l in open(path):
        if l.startswith("#") or not l.strip():
            continue
        f = l.split()
        try:
            hh, mm, ss = [int(x) for x in f[0].split(":")]
        except ValueError:
            continue
        today = datetime.date.today()
        ts = datetime.datetime.combine(today, datetime.time(hh, mm, ss)).timestamp() - off  # -> monotonic
        if w0 <= ts <= w1:
            rows.append(dict(sm=float(f[2]), mem=float(f[3]), rx=float(f[8]), tx=float(f[9])))
    if not rows:
        return None
    return {k: sum(r[k] for r in rows) / len(rows) for k in rows[0]} | {"n": len(rows)}

def analyse(path, skip=2.0):
    d = json.load(open(path))
    tr = [x for x in d["trace"] if x[5] == 0.0]  # thumbnail==0
    t0, t1 = d["t_play"] + skip, d["t_stop"]
    win = [x for x in tr if t0 <= x[0] <= t1]
    first = {}
    for x in win:
        first.setdefault(x[3], x[0])
    frames = sorted(first)
    out = dict(label=d["label"], fps=d["fps"], cfg=d["config"], renders=len(win), distinct=len(frames))
    if len(frames) > 2:
        ft = [first[f] for f in frames]
        span = max(ft) - min(ft)
        out["play_fps"] = (frames[-1] - frames[0]) / span if span > 0 else float("nan")
        out["distinct_per_s"] = len(frames) / span
        out["dropped_frames"] = int(frames[-1] - frames[0] + 1 - len(frames))
        out["dropped_pct"] = 100.0 * out["dropped_frames"] / (frames[-1] - frames[0] + 1)
        out["dup_renders_per_frame"] = len(win) / len(frames)
    ms = [x[4] for x in win]
    if ms:
        out.update(render_ms_mean=statistics.mean(ms), render_ms_p99=pct(ms, 0.99), render_ms_max=max(ms))
    # playhead-derived rate over the whole key-to-key window
    out["playhead_fps"] = d["end_frame"] / (d["t_stop"] + 1.3 - d["t_play"])
    cs = []
    for l in d["copystats"]:
        m = dict(re.findall(r"(\w+)=([\w.\-]+)", l))
        cs.append(m)
    if cs:
        def tot(k):
            return float(cs[-1][k]) - float(cs[0][k])
        out["copy_started_per_s_mean"] = statistics.mean(float(c["started_per_s"]) for c in cs[2:-1] or cs)
        gm = [float(c["gpu_copy_ms_mean"]) for c in cs[2:-1] or cs if float(c["gpu_copy_ms_mean"]) > 0]
        lm = [float(c["wall_latency_ms_mean"]) for c in cs[2:-1] or cs if float(c["wall_latency_ms_mean"]) > 0]
        out["gpu_copy_ms"] = statistics.mean(gm) if gm else 0
        out["lat_ms"] = statistics.mean(lm) if lm else 0
        out["lat_ms_max"] = max(float(c.get("wall_latency_ms_max", c.get("wall_latency_ms_max_total", 0))) for c in cs)
        out["submit_ms_mean"] = statistics.mean(float(c["submit_ms_mean"]) for c in cs[2:-1] or cs)
        out["drop_overwrite"] = int(tot("drop_overwrite")); out["drop_noslot"] = int(tot("drop_noslot")); out["drop_nostaging"] = int(tot("drop_nostaging"))
    dm = dmon_avg(os.path.join(os.path.dirname(path), d["label"] + ".dmon"), t0, t1)
    if dm:
        out.update(gpu_sm=dm["sm"], pcie_rx=dm["rx"], pcie_tx=dm["tx"])
    return out

if __name__ == "__main__":
    pat = sys.argv[1] if len(sys.argv) > 1 else "/tmp/s12/results/*.json"
    rows = [analyse(p) for p in sorted(glob.glob(pat))]
    keys = ["label", "cfg", "renders", "distinct", "play_fps", "playhead_fps", "dropped_frames", "dropped_pct", "dup_renders_per_frame", "render_ms_mean", "render_ms_p99", "render_ms_max",
            "copy_started_per_s_mean", "gpu_copy_ms", "lat_ms", "lat_ms_max", "drop_overwrite", "drop_nostaging", "gpu_sm", "pcie_rx", "pcie_tx"]
    for r in rows:
        print("  ".join("%s=%s" % (k, ("%.3f" % r[k]) if isinstance(r.get(k), float) else r.get(k)) for k in keys if k in r))
