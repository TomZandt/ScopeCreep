"""feed-frames: the accuracy suite's capture (ticket #17). For every test project, frames from the real Feed after
the app's Output Conversion.

The Sender (installed in Resolve, on each test project's timeline node) feeds the Timeline Colour Space frame;
the app's capture tool (`app/examples/capture_converted.rs`) attaches to the Feed as the app does, reads the
project's Colour Setup by script as the app does (only while Resolve is quiet), converts on the GPU as the app
does, and saves frame 1 of each test clip with the app's label as
<test-data>/feed-converted/<project>/<clip>.{exr,label}. `cargo run -p scopecreep-accuracy -- feed` then compares
them with Resolve's renders.

Only scopecreep-test-* projects are opened, and only their playhead moves: nothing is edited or saved. Run one
project per call under the shared Resolve lock, so each locked step stays short:

  cargo build --release -p scopecreep --example capture_converted
  for k in yrgb-rec709g24 dwg-rec709 ...; do
    flock -o ~/scopecreep-testdata/resolve.lock python3 harness/harness.py feed-frames --only $k
  done
"""
import os
import shutil
import subprocess
import time

from . import paths, projects, render, setups, state
from . import dump
from .clone import SENDER_LABEL, _frame_to_tc
from .dump_feed import CLIP_ORDER, clip_offsets

CONVERTED = os.path.join(paths.ROOT, "feed-converted")
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CAPTURE_BIN = os.environ.get("SCOPECREEP_CAPTURE_BIN") or os.path.join(REPO, "target", "release", "examples", "capture_converted")
CLIP_TIMEOUT = 25.0


def is_sender_tool(tool):
    """Any scopecreep Sender build (the real Sender, or the probe it stands in for)."""
    return tool.startswith("OFX: scopecreep")


def _sender_timeline(project):
    """The timeline whose `sender` node holds a Sender: prefer scopecreep-<key>, never 'hidden-branch'."""
    best = None
    for i in range(1, project.GetTimelineCount() + 1):
        tl = project.GetTimelineByIndex(i)
        if tl.GetName() == "hidden-branch":
            continue
        g = dump._graph(tl.GetNodeGraph())
        if g and any(n["label"] == SENDER_LABEL and any(is_sender_tool(t) for t in (n["tools"] or [])) for n in g["nodes"]):
            if tl.GetName().startswith("scopecreep-") or best is None:
                best = tl
    return best


def capture_args(binary, out_dir, project_name, offsets, timeout):
    """The capture tool's command line: one --frame <clip>=<frame time> per clip (frame times from the timeline start)."""
    args = [binary, "--out", out_dir, "--project", project_name, "--timeout", "%g" % timeout]
    for clip, off in offsets.items():
        args += ["--frame", "%s=%d" % (clip, off)]
    return args


def _wait_for(path, timeout, proc):
    t0 = time.time()
    while time.time() - t0 < timeout:
        if os.path.exists(path):
            return True
        if proc.poll() is not None:
            return False
        time.sleep(0.1)
    return False


def capture_project(resolve, setup, clips=CLIP_ORDER):
    if not os.path.exists(CAPTURE_BIN):
        raise SystemExit("%s not built: cargo build --release -p scopecreep --example capture_converted" % CAPTURE_BIN)
    pm = resolve.GetProjectManager()
    project = projects.load(pm, setup.project_name)
    if not project:
        raise SystemExit("project %s not found" % setup.project_name)
    tl = _sender_timeline(project)
    if tl is None:
        raise SystemExit("%s: no timeline with the Sender node" % setup.project_name)
    live = state.read_live(project)
    recorded = state.read(render.project_dir(setup))
    if recorded is None:
        print("feed-frames: %-34s WARNING: no project state recorded with its references; only clip grades checked"
              % setup.project_name)
    drift = state.problems(live if recorded is None else recorded, live)
    if drift:
        raise SystemExit("%s does not match its references, so its captures would not either (run check-state):\n  %s"
                         % (setup.project_name, "\n  ".join(drift)))
    resolve.OpenPage("color")
    project.SetCurrentTimeline(tl)
    time.sleep(1.0)
    start = tl.GetStartFrame()
    out_dir = os.path.join(CONVERTED, setup.project_name)
    if os.path.isdir(out_dir):
        shutil.rmtree(out_dir)
    os.makedirs(out_dir)
    state.write(out_dir, live)
    offsets = clip_offsets(clips)
    proc = subprocess.Popen(capture_args(CAPTURE_BIN, out_dir, setup.project_name, offsets, CLIP_TIMEOUT * len(offsets) + 30),
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8",
                            errors="replace", close_fds=True)
    failed = []
    try:
        time.sleep(1.0)  # attach: the Sender copies frames only while an app is listening
        for clip, off in offsets.items():
            label = os.path.join(out_dir, clip + ".label")
            for _attempt in range(3):
                # Park elsewhere first so the target frame is a fresh render, then stay on it: the Colour Setup is
                # read once Resolve has been quiet for a second.
                tl.SetCurrentTimecode(_frame_to_tc(start + off + 10, 24))
                time.sleep(1.2)
                tl.SetCurrentTimecode(_frame_to_tc(start + off, 24))
                if _wait_for(label, CLIP_TIMEOUT, proc):
                    break
            else:
                failed.append(clip)
    finally:
        try:
            output, _ = proc.communicate(timeout=10 if not failed else 1)
        except subprocess.TimeoutExpired:
            proc.kill()
            output, _ = proc.communicate()
    for line in (output or "").splitlines():
        print("feed-frames: %-34s %s" % (setup.project_name, line))
    for clip in failed:
        print("feed-frames: %-34s %-10s FAILED (no converted frame)" % (setup.project_name, clip))
    return failed


def capture_all(resolve, selected=None):
    pm = resolve.GetProjectManager()
    original = projects.remember_original(pm)
    failures = 0
    for s in setups.SETUPS:
        if selected and s.key not in selected:
            continue
        failures += len(capture_project(resolve, s))
    if original:
        projects.load(pm, original)
    if failures:
        raise SystemExit("%d capture(s) failed" % failures)
