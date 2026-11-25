"""usage: freerun.py FPS CONFIG LABEL [FRAMES] [FORMAT CODEC]
Resolve free-run (uncapped) throughput: queue a Deliver render of FRAMES frames of the scratch UHD project (NOT realtime-capped),
with probe copy off/on (copy.off trigger). Records wall time, frames/s, and the probe's own COPYSTATS (renders/s, drops)."""
import json, os, re, subprocess, sys, time, shutil, glob
HARNESS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
sys.path.insert(0, HARNESS)
from scopecreep_harness import resolve_api, projects

fps_s, config, label = sys.argv[1], sys.argv[2], sys.argv[3]
FRAMES = int(sys.argv[4]) if len(sys.argv) > 4 else 600
fmt = sys.argv[5] if len(sys.argv) > 5 else "mp4"
codec = sys.argv[6] if len(sys.argv) > 6 else "H.265"
name = "scopecreep-test-s12-uhd-" + fps_s.replace(".", "")
D = "/tmp/scopecreep-probe"
OUT = "/tmp/s12/results"
STAGING = os.path.expanduser("~/Videos/scopecreep-test-render-tmp/s12")
r = resolve_api.connect()
pm = r.GetProjectManager()
p = projects.load(pm, name)
tl = p.GetCurrentTimeline()
off = D + "/copy.off"
reader=None
if config == "on_reader":
    reader = subprocess.Popen(["/tmp/s12/wt/prototype/probe-sender/build/probe_reader","--touch","--seconds","90"], stdout=open("%s/%s.reader"%(OUT,label),"w"), stderr=subprocess.STDOUT)
if config == "off":
    open(off, "w").close()
elif os.path.exists(off):
    os.unlink(off)
if os.path.isdir(STAGING):
    shutil.rmtree(STAGING)
os.makedirs(STAGING)
r.OpenPage("deliver")
time.sleep(2)
print("codecs for", fmt, p.GetRenderCodecs(fmt))
print("set format", p.SetCurrentRenderFormatAndCodec(fmt, codec))
start = tl.GetStartFrame()
ok = p.SetRenderSettings({"SelectAllFrames": False, "MarkIn": start, "MarkOut": start + FRAMES - 1, "TargetDir": STAGING,
                          "CustomName": label, "ExportVideo": True, "ExportAudio": False})
print("settings", ok)
p.DeleteAllRenderJobs()
job = p.AddRenderJob()
print("job", job)
log_off = os.path.getsize(D + "/probe.log")
t0 = time.time()
tm0 = time.monotonic()
p.StartRendering([job], False)
last = None
while p.IsRenderingInProgress():
    time.sleep(0.5)
    if time.time() - t0 > 600:
        p.StopRendering()
        break
wall = time.time() - t0
if reader: reader.terminate()
st = p.GetRenderJobStatus(job)
print("status", st, "wall %.1fs" % wall)
time.sleep(1.5)
with open(D + "/probe.log", "rb") as f:
    f.seek(log_off)
    log_new = f.read().decode(errors="replace")
cs = [l for l in log_new.splitlines() if "COPYSTATS" in l]
res = dict(label=label, config=config, frames=FRAMES, wall=wall, status=st, copystats=cs, fmt=fmt, codec=codec)
json.dump(res, open("%s/%s.json" % (OUT, label), "w"))
shutil.rmtree(STAGING, ignore_errors=True)
r.OpenPage("color")
print(label, "frames/s over whole job: %.1f" % (FRAMES / wall))
