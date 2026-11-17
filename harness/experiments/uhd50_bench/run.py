"""UHD 50: Sender off (timeline without the Sender node) / on / on + app. Each: 15 s warm-up loop pass, then 60 s measured while looping.

Rate with the Sender in the chain: Sender renders per second from the Feed header (no scripting during play).
Resolve's own fps indicator is screenshotted every 5 s in every config."""
import os
import subprocess
import sys
import time

sys.path.insert(0, "/home/tom/GitHub/scopecreep/harness")
from scopecreep_harness import feed_header, viewer  # noqa: E402
from scopecreep_harness.resolve_api import connect  # noqa: E402

OUT = os.path.expanduser("~/scopecreep-testdata/bench/uhd50-" + time.strftime("%Y%m%d-%H%M%S"))
os.makedirs(OUT, exist_ok=True)
APP = "/home/tom/GitHub/scopecreep/target/release/scopecreep"
WARM, MEASURE, SHOT_EVERY = 15, 60, 5
INDICATOR = "1395,255 120x30"

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
        subprocess.run(["hyprctl", "dispatch", 'hl.dsp.window.move({workspace="11", window="class:scopecreep"})'])
        time.sleep(2)
    viewer.send_key("K")
    time.sleep(0.5)
    viewer.send_key("L")
    time.sleep(WARM)
    r0, t0 = renders(), time.time()
    for i in range(MEASURE // SHOT_EVERY):
        time.sleep(SHOT_EVERY)
        subprocess.run(["grim", "-g", INDICATOR, os.path.join(OUT, "%s-%02d.png" % (label, i))])
    r1, t1 = renders(), time.time()
    viewer.send_key("K")
    rate = (r1 - r0) / (t1 - t0) if (r0 is not None and r1 is not None) else None
    log("%s: Sender renders/s over %.0f s = %s" % (label, t1 - t0, "%.2f" % rate if rate is not None else "n/a"))
    if app:
        subprocess.run(["grim", os.path.join(OUT, label + "-screen.png")])
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
