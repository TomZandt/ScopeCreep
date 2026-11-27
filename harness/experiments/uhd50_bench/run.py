"""UHD 50: Sender off (timeline without the Sender node) / on / on + app. Each: 15 s warm-up loop pass, then 60 s measured while looping.

Rate with the Sender in the chain: Sender renders per second from the Feed header (no scripting during play).
Resolve's own fps indicator is screenshotted every 5 s in every config, and the GPU sampled by nvidia-smi where
there is one (Linux and Windows with NVIDIA; on a Mac the frame rates Resolve reports are the measure).

Linux, Windows and macOS. On Windows and macOS the app's window is not moved off Resolve: place it before the run
(it opens where it was last closed), and keep Resolve's viewer visible. On a Mac the terminal needs Accessibility
(to send keys to Resolve) and Screen Recording (for the screenshots). SCOPECREEP_BENCH_APP runs another app build,
such as the installed one (/Applications/ScopeCreep.app/Contents/MacOS/scopecreep)."""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
from scopecreep_harness import feed_header, gpu, viewer  # noqa: E402
from scopecreep_harness.resolve_api import connect  # noqa: E402

WINDOWS = sys.platform == "win32"
LINUX = sys.platform.startswith("linux")
OUT = os.path.expanduser("~/scopecreep-testdata/bench/uhd50-" + time.strftime("%Y%m%d-%H%M%S"))
os.makedirs(OUT, exist_ok=True)
APP = os.environ.get("SCOPECREEP_BENCH_APP") or os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "target", "release",
    "scopecreep.exe" if WINDOWS else "scopecreep")
WARM, MEASURE, SHOT_EVERY = 15, 60, 5
# Resolve's fps readout, in global screen coordinates ("X,Y WxH"); set SCOPECREEP_BENCH_GRAB when Resolve sits
# elsewhere. Only Linux has a default: on Windows and macOS every screen is grabbed unless it is set.
INDICATOR = os.environ.get("SCOPECREEP_BENCH_GRAB") or ("1395,255 120x30" if LINUX else None)

r = connect()
project = r.GetProjectManager().GetCurrentProject()
timelines = {project.GetTimelineByIndex(i).GetName(): project.GetTimelineByIndex(i)
             for i in range(1, project.GetTimelineCount() + 1)}
SENDER_TL = [t for n, t in timelines.items() if n != "no-sender"][0]  # Sender on the timeline node
PLAIN_TL = timelines["no-sender"]  # same clips, no timeline node: the Sender is not in the chain
tl = SENDER_TL


def log(msg):
    line = time.strftime("%H:%M:%S ") + msg
    print(line, flush=True)
    with open(os.path.join(OUT, "run.log"), "a") as f:
        f.write(line + "\n")


def renders():
    h = feed_header.read()
    return h.renders if h else None


def run(label, sender_on, with_app):
    t = SENDER_TL if sender_on else PLAIN_TL
    assert project.SetCurrentTimeline(t), "could not switch timeline"
    time.sleep(1)
    log("%s: timeline %s (%d frames)" % (label, t.GetName(), t.GetEndFrame() - t.GetStartFrame()))
    app = None
    if with_app:
        env = dict(os.environ, SCOPECREEP_FRAME_LOG=os.path.join(OUT, label + ".frames.log"))
        # on the empty right-hand monitor, so it never covers Resolve
        app = subprocess.Popen([APP], env=env, stdout=open(os.path.join(OUT, label + ".app.log"), "w"),
                               stderr=subprocess.STDOUT)
        time.sleep(2)
        if LINUX:
            subprocess.run(["hyprctl", "dispatch", 'hl.dsp.window.move({workspace="11", window="class:scopecreep"})'])
        time.sleep(2)
    viewer.send_key("K")
    time.sleep(0.5)
    viewer.send_key("L")
    time.sleep(WARM)
    gpu_log = os.path.join(OUT, label + ".gpu.csv")
    monitor = gpu.start(gpu_log)
    r0, t0 = renders(), time.time()
    for i in range(MEASURE // SHOT_EVERY):
        time.sleep(SHOT_EVERY)
        viewer.grab(os.path.join(OUT, "%s-%02d.png" % (label, i)), INDICATOR)
    r1, t1 = renders(), time.time()
    gpu.stop(monitor)
    viewer.send_key("K")
    rate = (r1 - r0) / (t1 - t0) if (r0 is not None and r1 is not None) else None
    log("%s: Sender renders/s over %.0f s = %s" % (label, t1 - t0, "%.2f" % rate if rate is not None else "n/a"))
    log("%s: %s" % (label, gpu.summary(gpu_log) if monitor else "GPU: not sampled (no nvidia-smi)"))
    if app:
        viewer.grab(os.path.join(OUT, label + "-screen.png"))
        app.terminate()
        app.wait(10)
    time.sleep(2)


log("start; timeline %s at %s fps" % (tl.GetName(), tl.GetSetting("timelineFrameRate")))
try:
    for cfg in sys.argv[1:] or ["off", "on", "app"]:
        run(cfg, cfg != "off", cfg == "app")
finally:
    project.SetCurrentTimeline(SENDER_TL)
log("done: " + OUT)
