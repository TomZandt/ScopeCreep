"""dump-timeline-frames: collect the probe Sender's Timeline Colour Space frames for every test project.

The probe (prototype/probe-sender) writes a float EXR of the next full-size frame it renders once
$PROBE_DIR/dump.trigger exists, named frame_inst<N>_t<time>_<W>x<H>.exr, where <time> is the render time in
frames from the timeline start. For each project and each test clip this arms the trigger, moves the playhead
onto frame 1 of that clip, waits for a dump whose <time> and size match, and stores it as
~/scopecreep-testdata/dumps-feed/<project>/<clip>.exr. Dumps that belong to a different frame (a background
cache render won the race) are discarded and the attempt is repeated.

Only scopecreep-test-* projects are opened. The hand-built dwg-p3d65 project is used read-only: only its
playhead moves; nothing is saved or edited.
"""
import glob
import os
import re
import shutil
import time

from . import paths, projects, setups, testmedia
from .clone import SENDER_LABEL, SENDER_TOOL, _frame_to_tc, has_sender
from . import dump

PROBE_DIR = os.environ.get("SCOPECREEP_PROBE_DIR") or "/tmp/scopecreep-probe"
FEED = os.path.join(paths.ROOT, "dumps-feed")
NAME_RE = re.compile(r"^frame_inst(\d+)_t(-?[0-9.]+)_(\d+)x(\d+)\.exr$")
CLIP_ORDER = ("ramp", "chart", "outofrange", "natural")


def parse_dump_name(name):
    """frame_inst2_t72.000_1920x1080.exr -> (inst, time, w, h), or None."""
    m = NAME_RE.match(name)
    if not m:
        return None
    return int(m.group(1)), float(m.group(2)), int(m.group(3)), int(m.group(4))


def clip_offsets(clips=CLIP_ORDER):
    """clip -> timeline frame offset of its frame 1 (clips are laid back to back)."""
    off, out = 0, {}
    for c in clips:
        out[c] = off
        off += testmedia.CLIPS[c][2]
    return out


def feed_path(project_name, clip):
    return os.path.join(FEED, project_name, clip + ".exr")


def pick_matching(candidates, want_time, want_size):
    """Pick the dump file (by name) whose render time and size match; None if there is none."""
    for p in sorted(candidates):
        info = parse_dump_name(os.path.basename(p))
        if info and abs(info[1] - want_time) < 1e-6 and (info[2], info[3]) == tuple(want_size):
            return p
    return None


def _main_timeline(project):
    """The timeline carrying the `sender` node: prefer scopecreep-<key>, else any with a Sender, never 'hidden-branch'."""
    best = None
    for i in range(1, project.GetTimelineCount() + 1):
        tl = project.GetTimelineByIndex(i)
        if tl.GetName() == "hidden-branch":
            continue
        g = dump._graph(tl.GetNodeGraph())
        if has_sender(g):
            if tl.GetName().startswith("scopecreep-") or best is None:
                best = tl
    return best


def _wait_for(pattern_dir, want_time, size, timeout):
    t0 = time.time()
    while time.time() - t0 < timeout:
        files = [f for f in glob.glob(os.path.join(pattern_dir, "frame_inst*.exr"))]
        got = pick_matching(files, want_time, size)
        if got:
            time.sleep(0.4)  # let the writer finish (size stable)
            a = os.path.getsize(got)
            time.sleep(0.3)
            if os.path.getsize(got) == a:
                return got
        time.sleep(0.2)
    return None


def collect_one(timeline, start, offset, size, attempts=6):
    trigger = os.path.join(PROBE_DIR, "dump.trigger")
    for attempt in range(attempts):
        for f in glob.glob(os.path.join(PROBE_DIR, "frame_inst*.exr")):
            os.unlink(f)
        if os.path.exists(trigger):
            os.unlink(trigger)
        # Park elsewhere first so the target frame is a fresh render, not a repeat of the one on screen.
        timeline.SetCurrentTimecode(_frame_to_tc(start + offset + 10, 24))
        time.sleep(1.2)
        open(trigger, "w").close()
        timeline.SetCurrentTimecode(_frame_to_tc(start + offset, 24))
        got = _wait_for(PROBE_DIR, float(offset), size, 6.0)
        if got:
            return got
    return None


def dump_project_frames(resolve, setup, clips=CLIP_ORDER):
    pm = resolve.GetProjectManager()
    project = projects.load(pm, setup.project_name)
    if not project:
        raise SystemExit("project %s not found" % setup.project_name)
    tl = _main_timeline(project)
    if tl is None:
        raise SystemExit("%s: no timeline with the sender node" % setup.project_name)
    resolve.OpenPage("color")
    project.SetCurrentTimeline(tl)
    time.sleep(1.0)
    start = tl.GetStartFrame()
    out_dir = os.path.join(FEED, setup.project_name)
    if os.path.isdir(out_dir):
        shutil.rmtree(out_dir)
    os.makedirs(out_dir)
    res = {}
    for clip, off in clip_offsets(clips).items():
        w, h = testmedia.CLIPS[clip][0], testmedia.CLIPS[clip][1]
        got = collect_one(tl, start, off, (w, h))
        if got is None:
            res[clip] = None
            print("dump-timeline-frames: %-34s %-10s FAILED (no matching dump)" % (setup.project_name, clip))
            continue
        shutil.move(got, feed_path(setup.project_name, clip))
        res[clip] = feed_path(setup.project_name, clip)
        print("dump-timeline-frames: %-34s %-10s ok (timeline '%s' t=%d)" % (setup.project_name, clip, tl.GetName(), off))
    return res


def dump_all(resolve, selected=None):
    pm = resolve.GetProjectManager()
    original = projects.remember_original(pm)
    os.makedirs(PROBE_DIR, exist_ok=True)
    failures = 0
    for s in setups.SETUPS:
        if selected and s.key not in selected:
            continue
        res = dump_project_frames(resolve, s)
        failures += sum(1 for v in res.values() if v is None)
    if original:
        projects.load(pm, original)
    if failures:
        raise SystemExit("%d dump(s) failed" % failures)
