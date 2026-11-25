"""usage: run_play.py FPS CONFIG LABEL [SECONDS]
CONFIG: off | on | on_reader | on_slowreader
Plays the scratch UHD project for FPS from frame 0 by keystroke, with probe copy off/on (copy.off trigger file),
reader process off/on, nvidia-smi dmon running. Saves everything needed to /tmp/s12/results/LABEL.json."""
import json, os, subprocess, sys, threading, time, re
HARNESS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
sys.path.insert(0, HARNESS)
from scopecreep_harness import resolve_api, projects, viewer

fps_s, config, label = sys.argv[1], sys.argv[2], sys.argv[3]
T = float(sys.argv[4]) if len(sys.argv) > 4 else 20.0
fps = float(fps_s)
name = "scopecreep-test-s12-uhd-" + fps_s.replace(".", "")
D = "/tmp/scopecreep-probe"
READER = "/tmp/s12/wt/prototype/probe-sender/build/probe_reader"
OUT = "/tmp/s12/results"
os.makedirs(OUT, exist_ok=True)

r = resolve_api.connect()
pm = r.GetProjectManager()
p = projects.load(pm, name)
tl = p.GetCurrentTimeline()
r.OpenPage("color")
time.sleep(2)
off = D + "/copy.off"
if config == "off":
    open(off, "w").close()
elif os.path.exists(off):
    os.unlink(off)
start_tc = "01:00:00:00"
tl.SetCurrentTimecode(start_tc)
time.sleep(2.5)

def tc_to_frames(tc):
    h, m, s, f = [int(x) for x in re.split("[:;]", tc)]
    nominal = round(fps)
    return ((h * 60 + m) * 60 + s) * nominal + f

base = tc_to_frames(start_tc)
samples = []
stop = False
def sampler():
    while not stop:
        try:
            samples.append((time.monotonic(), tc_to_frames(tl.GetCurrentTimecode()) - base))
        except Exception as e:
            pass
        time.sleep(0.2)

dmon = subprocess.Popen(["nvidia-smi", "dmon", "-s", "utmc", "-d", "1", "-o", "T"], stdout=open("%s/%s.dmon" % (OUT, label), "w"), stderr=subprocess.DEVNULL)
smi = subprocess.Popen(["nvidia-smi","--query-gpu=timestamp,utilization.gpu,pcie.link.gen.current,pcie.link.width.current,clocks.sm,power.draw,memory.used","--format=csv,noheader","-lms","500"], stdout=open("%s/%s.smi" % (OUT, label), "w"), stderr=subprocess.DEVNULL)
reader = None
if config.startswith("on_"):
    args = [READER, "--verify" if config == "on_verify" else "--touch", "--seconds", str(T + 6)]
    if config == "on_slowreader":
        args += ["--sleep-ms", "500"]
    reader = subprocess.Popen(args, stdout=open("%s/%s.reader" % (OUT, label), "w"), stderr=subprocess.STDOUT)
    time.sleep(0.5)
log_off = os.path.getsize(D + "/probe.log")
time.sleep(0.5)
t_play = time.monotonic()
viewer.send_key("L")
time.sleep(T)
t_stop = time.monotonic()
viewer.send_key("K")
time.sleep(1.3)
end_frame = tc_to_frames(tl.GetCurrentTimecode()) - base
tl.SetCurrentTimecode("01:00:00:07")  # one render after the run so the probe flushes its last second of trace
time.sleep(1.8)
dmon.terminate()
smi.terminate()
gpu_pcie = subprocess.run(["nvidia-smi", "--query-gpu=pcie.link.gen.current,pcie.link.width.current", "--format=csv,noheader"],
                          capture_output=True, text=True).stdout.strip()
if reader:
    try:
        reader.wait(timeout=T + 20)
    except subprocess.TimeoutExpired:
        reader.terminate()
with open(D + "/probe.log", "rb") as f:
    f.seek(log_off)
    log_new = f.read().decode(errors="replace")
copystats = [l for l in log_new.splitlines() if "COPYSTATS" in l]
trace = []
tp = D + "/render_trace.csv"
for l in open(tp).read().splitlines()[1:]:
    parts = l.split(",")
    t = float(parts[0]) / 1000.0
    if t_play - 0.2 <= t <= t_stop + 2.0:
        trace.append([t] + [float(x) for x in parts[1:]])
res = dict(label=label, fps=fps, config=config, T=T, t_play=t_play, t_stop=t_stop, end_frame=end_frame,
           samples=samples, trace=trace, copystats=copystats, pcie_after=gpu_pcie, project=name)
json.dump(res, open("%s/%s.json" % (OUT, label), "w"))
print(label, "renders in window:", len(trace), "end_frame", end_frame, "playhead fps ~ %.2f" % (end_frame / (t_stop + 1.3 - t_play)))
