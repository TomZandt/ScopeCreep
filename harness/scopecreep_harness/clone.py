"""clone-sender: copy the hand-built Sender node tree into every other test project.

Scripts cannot create nodes on a timeline node tree, so one project (SOURCE) was hand-built. This exports it as a
.drp, imports it once per other Colour Setup under a candidate name, applies that setup's Colour Setup by script,
and verifies the candidate before it replaces the harness-built project. Only scopecreep-test-* projects are
touched; SOURCE is read-only (export only).
"""
import json
import os
import shutil
import time

from . import dump, exr, feed_header, paths, projects, render, setups
from .resolve_api import set_checked

SOURCE_KEY = "dwg-p3d65"
SOURCE = paths.PROJECT_PREFIX + SOURCE_KEY
DRP_DIR = os.path.join(paths.ROOT, "drp")
CLONE_RENDERS = os.path.join(paths.ROOT, "renders-clone")
# The Sender's name in Timeline.GetNodeGraph().GetToolsInNode(): "OFX: " plus its label (#223; before that
# the label was "scopecreep Sender").
SENDER_TOOL = "OFX: ScopeCreep"
SENDER_LABEL = "sender"
CANDIDATE_SUFFIX = "-clone"
# Timelines in the source that are not the plain `sender` timeline.
EXTRA_TIMELINES = ("original", "hidden-branch")
EXR_TOLERANCE = 1e-6


# ---- pure helpers (unit tested) -------------------------------------------------------------------------------

def diff_settings(expected, actual, keys=None):
    """List 'key: expected != actual' for every key (default: all of expected) whose values differ."""
    out = []
    for k in (keys if keys is not None else expected):
        if str(expected.get(k)) != str(actual.get(k)):
            out.append("%s: expected %r, got %r" % (k, expected.get(k), actual.get(k)))
    return out


def is_sender_tool(tool):
    """True for the Sender's tool name."""
    return tool == SENDER_TOOL


def has_sender(tree, label=SENDER_LABEL):
    """True if a dump._graph() style tree has a node with this label holding the Sender OFX."""
    if not tree:
        return False
    return any(n["label"] == label and any(is_sender_tool(t) for t in (n["tools"] or [])) for n in tree["nodes"])


def max_abs_diff(a, b):
    """Largest per-channel absolute difference between two exr.read_exr images; inf if sizes differ."""
    if (a.width, a.height) != (b.width, b.height):
        return float("inf")
    worst = 0.0
    for ra, rb in zip(a.rows, b.rows):
        for ca, cb in zip(ra, rb):
            for x, y in zip(ca, cb):
                d = abs(x - y)
                if d != d:  # NaN
                    return float("inf")
                worst = max(worst, d)
    return worst


def clone_targets(only=None):
    """Setups to clone into: every colour setup except the source and the perf project."""
    return [s for s in setups.SETUPS if s.key != SOURCE_KEY and (not only or s.key in only)]


# ---- Resolve-facing steps -------------------------------------------------------------------------------------

def export_source(pm, path=None):
    path = path or os.path.join(DRP_DIR, SOURCE + ".drp")
    os.makedirs(DRP_DIR, exist_ok=True)
    projects.enter_folder(pm)
    if SOURCE not in (pm.GetProjectListInCurrentFolder() or []):
        raise SystemExit("source project %s not found" % SOURCE)
    if not pm.ExportProject(SOURCE, path, True):  # read-only on the source
        raise SystemExit("Resolve would not export %s" % SOURCE)
    return path


def _feed_renders():
    h = feed_header.read()
    return h.renders if h else 0


def sender_receives_frames(resolve, project, timeline):
    """Open the Color page, move the playhead across the timeline by script, count the Sender's new renders.

    The count comes from the Feed header's render counters, which move whether or not an app is attached."""
    resolve.OpenPage("color")
    project.SetCurrentTimeline(timeline)
    time.sleep(1.0)
    start = timeline.GetStartFrame()
    before = _feed_renders()
    from .testmedia import CLIPS
    off = 0
    for clip in ("ramp", "chart", "outofrange", "natural"):
        frames = CLIPS[clip][2]
        for f in (off + 3, off + 6):
            timeline.SetCurrentTimecode(_frame_to_tc(start + f, 24))
            time.sleep(0.8)
        off += frames
    time.sleep(1.0)
    return _feed_renders() - before


def _frame_to_tc(frame, fps):
    s, ff = divmod(frame, fps)
    m, ss = divmod(s, 60)
    h, mm = divmod(m, 60)
    return "%02d:%02d:%02d:%02d" % (h, mm, ss, ff)


def import_candidate(pm, drp, setup):
    cand = setup.project_name + CANDIDATE_SUFFIX
    projects.delete_project(pm, cand)  # leaves Resolve in the scopecreep folder
    if not pm.ImportProject(drp, cand):
        raise RuntimeError("Resolve would not import %s as %s" % (drp, cand))
    project = projects.load(pm, cand)
    if not project:
        raise RuntimeError("imported %s but could not open it" % cand)
    return cand, project


# A harness-built ACES project keeps Resolve's default (dormant) Timeline/Output Colour Space values; the clone
# would otherwise keep the source's DWG/P3-D65 ones, which ACES ignores but which would make the Colour Setup differ.
ACES_DORMANT = [("colorSpaceTimeline", "Rec.709 (Scene)"), ("colorSpaceOutput", "Rec.709 (Scene)")]


def apply_setup(project, setup):
    if dict(setup.settings).get("colorScienceMode") == "acescct":
        # The dormant values can only be set while a non-ACES mode is active, so do it first.
        project.SetSetting("colorScienceMode", "davinciYRGB")
        for k, v in ACES_DORMANT:
            project.SetSetting(k, v)  # best effort; verify_colour_setup reports it if Resolve refuses
    for k, v in setup.settings:
        set_checked(project, k, v)


def tidy_timelines(project, setup, keep_hidden=False):
    """Make the plain `sender` timeline first and current; drop the source's extra timelines unless kept."""
    mp = project.GetMediaPool()
    n = project.GetTimelineCount()
    tls = {project.GetTimelineByIndex(i).GetName(): project.GetTimelineByIndex(i) for i in range(1, n + 1)}
    main = project.GetTimelineByIndex(1)
    main_name = main.GetName()
    drop = [tls[name] for name in EXTRA_TIMELINES if name in tls and tls[name].GetName() != main_name
            and not (keep_hidden and name == "hidden-branch")]
    if drop and not mp.DeleteTimelines(drop):
        raise RuntimeError("could not delete extra timelines")
    main.SetName("scopecreep-" + setup.key)
    project.SetCurrentTimeline(main)
    return main


def verify_colour_setup(resolve, project, name_for_dump):
    """Compare project and timeline Colour Setup with the earlier dump of the harness-built project."""
    ref_path = os.path.join(paths.DUMPS, name_for_dump + ".json")
    ref = json.load(open(ref_path))["colourSetup"]
    got = dump.dump_project(resolve, project)["colourSetup"]
    problems = ["project " + d for d in diff_settings(ref["project"], got["project"])]
    problems += ["timeline " + d for d in diff_settings(ref["timeline"], got["timeline"])]
    return problems


def compare_renders(ref_dir, new_dir):
    """Compare every EXR under ref_dir with the same relative file (any frame number) under new_dir."""
    results = {}
    for sub in sorted(os.listdir(ref_dir)):
        if not sub.endswith("-exr"):
            continue
        rf = sorted(os.listdir(os.path.join(ref_dir, sub)))
        nf = sorted(os.listdir(os.path.join(new_dir, sub))) if os.path.isdir(os.path.join(new_dir, sub)) else []
        if not rf or not nf:
            results[sub] = float("inf")
            continue
        results[sub] = max_abs_diff(exr.read_exr(os.path.join(ref_dir, sub, rf[0])),
                                    exr.read_exr(os.path.join(new_dir, sub, nf[0])))
    return results


def swap_in(pm, setup, cand):
    """Replace the harness-built project with the verified candidate. Returns None or an error string.

    Old project is renamed aside first, so a failed rename can be rolled back and nothing is lost."""
    final = setup.project_name
    aside = final + "-old"
    projects.delete_project(pm, aside)
    old = projects.load(pm, final)
    if not old or not old.SetName(aside):
        return "could not rename existing %s aside; left alone" % final
    new = projects.load(pm, cand)
    if not new or not new.SetName(final):
        projects.load(pm, aside).SetName(final)  # roll back
        return "Resolve would not rename %s to %s; original restored" % (cand, final)
    pm.SaveProject()
    projects.delete_project(pm, aside)
    return None


def clone_one(resolve, setup, drp, keep_hidden=False, replace=True):
    """Clone, set up and verify one project. Returns a report dict; the candidate is deleted on failure."""
    pm = resolve.GetProjectManager()
    rep = {"project": setup.project_name, "problems": [], "replaced": False}
    cand, project = import_candidate(pm, drp, setup)
    try:
        apply_setup(project, setup)
        main = tidy_timelines(project, setup, keep_hidden)
        pm.SaveProject()
        rep["colour_setup"] = verify_colour_setup(resolve, project, setup.project_name)
        tree = dump.dump_project(resolve, project)["timelineNodeTree"]
        rep["node_graph"] = tree
        if not has_sender(tree):
            rep["problems"].append("timeline node tree has no `sender` node: %r" % (tree,))
        rep["sender_renders"] = sender_receives_frames(resolve, project, main)
        if rep["sender_renders"] <= 0:
            rep["problems"].append("the Feed counted no Sender render after moving the playhead")
        ref_dir = render.project_dir(setup)
        out_dir = os.path.join(CLONE_RENDERS, setup.project_name)
        render.render_project(resolve, setup, project_name=cand, outdir=out_dir)
        rep["render_max_abs_diff"] = compare_renders(ref_dir, out_dir)
        rep["problems"] += ["colour setup " + p for p in rep["colour_setup"]]
        rep["problems"] += ["render %s differs from reference by %g" % (k, v)
                            for k, v in rep["render_max_abs_diff"].items() if not v <= EXR_TOLERANCE]
        if not rep["problems"] and replace:
            err = swap_in(pm, setup, cand)
            if err:
                rep["problems"].append(err)
            else:
                rep["replaced"] = True
    except Exception as e:  # keep going with the other setups; the original project is untouched
        rep["problems"].append("%s: %s" % (type(e).__name__, e))
    finally:
        projects.enter_folder(pm)
        if not rep["replaced"] and cand in (pm.GetProjectListInCurrentFolder() or []):
            projects.delete_project(pm, cand)
    return rep


def clone_all(resolve, only=None, keep_hidden=False, replace=True):
    """Clone into every target; with replace=False only verify each clone, then delete it (projects untouched)."""
    pm = resolve.GetProjectManager()
    original = projects.remember_original(pm)
    drp = export_source(pm)
    print("clone-sender: exported %s -> %s" % (SOURCE, drp))
    reports = []
    for s in clone_targets(only):
        t = time.time()
        rep = clone_one(resolve, s, drp, keep_hidden, replace)
        reports.append(rep)
        if not replace:
            status = "FAILED" if rep["problems"] else "OK (verified only, original kept)"
        else:
            status = "OK, replaced" if rep["replaced"] else "FAILED, original kept"
        print("clone-sender: %-34s %s (%.0fs)" % (rep["project"], status, time.time() - t))
        print("  node graph: %s" % ([(n["label"], n["tools"]) for n in (rep.get("node_graph") or {}).get("nodes", [])],))
        print("  Sender renders in the Feed after moving playhead: %s" % rep.get("sender_renders"))
        print("  render max abs diff vs reference: %s" % rep.get("render_max_abs_diff"))
        for p in rep["problems"]:
            print("  PROBLEM:", p)
    if original:
        projects.load(pm, original)
    else:
        projects.load(pm, SOURCE)
    if any(r["problems"] or (replace and not r["replaced"]) for r in reports):
        raise SystemExit("some clones failed verification (see above)")
    return reports
