"""The state of a test project that its test data depends on, recorded next to that data and checked against the
live project, so a project changed after its references were rendered fails loudly instead of skewing a
measurement (ticket #43).

The state is a short text file, `project-state.txt`, one `key = value` line each: the Colour Setup keys (as the
Stage 0 dump reads them), and the grade on the timeline node graph and on every clip (node tools, as Resolve
reports them). Any Sender build on the timeline counts as "Sender", so reinstalling it is not drift.

Written by `render` (into the renders directory) and `feed-frames` (into feed-converted). Checked by
`check-state` (live project vs the renders), by `feed-frames` before it captures, and by the accuracy crate's
tests (accuracy/tests/resolve_renders.rs), which also require every clip to carry no grade.
"""
import os
import re

from . import dump

FILE = "project-state.txt"
HEADER = "# scopecreep test project state (harness/scopecreep_harness/state.py); compared line by line"
ABSENT = "(absent)"


def clip_key(item_name):
    """'ramp.[0001-0024].exr' -> 'ramp'."""
    return re.split(r"[.\[]", item_name, maxsplit=1)[0]


def _is_sender(tool):
    return tool.startswith("OFX: scopecreep")


def grade_text(graph):
    """'none' for no node, or one node with no tools (a clip as Resolve creates it); else each node's tools."""
    if graph is None:
        return "unavailable"
    nodes = graph["nodes"]
    if len(nodes) <= 1 and not any(n["tools"] for n in nodes):
        return "none"
    parts = []
    for n in nodes:
        tools = ["Sender" if _is_sender(t) else t for t in (n["tools"] or [])]
        parts.append("node %d: %s" % (n["index"], ", ".join(tools) or "(no tools)"))
    return "; ".join(parts)


def describe(project_name, timeline_name, settings, timeline_graph, clip_graphs):
    """The state as text. clip_graphs: [(clip, graph)] with graphs in dump._graph form."""
    lines = [HEADER, "project = %s" % project_name, "timeline = %s" % timeline_name]
    for k in dump.COLOUR_KEYS:
        if k in settings:
            lines.append("colour %s = %s" % (k, settings[k]))
    lines.append("timeline grade = %s" % grade_text(timeline_graph))
    for clip, graph in clip_graphs:
        lines.append("clip %s grade = %s" % (clip, grade_text(graph)))
    return "\n".join(lines) + "\n"


def _parse(text):
    out = {}
    for line in text.splitlines():
        if line.startswith("#") or " = " not in line:
            continue
        k, v = line.split(" = ", 1)
        out[k] = v
    return out


def differences(recorded, live):
    """Every line that differs, as 'key: recorded X, now Y'. Empty when the states match."""
    a, b = _parse(recorded), _parse(live)
    out = []
    for k in list(a) + [k for k in b if k not in a]:
        if a.get(k) != b.get(k):
            out.append("%s: recorded %s, now %s" % (k, a.get(k, ABSENT), b.get(k, ABSENT)))
    return out


def graded_clips(text):
    """'clip: grade' for every clip that carries a grade."""
    out = []
    for k, v in _parse(text).items():
        if k.startswith("clip ") and k.endswith(" grade") and v != "none":
            out.append("%s: %s" % (k[len("clip "):-len(" grade")], v))
    return out


def problems(recorded, live):
    """Why the live project can't stand behind its references: no recorded state, any line that changed, or a
    clip grade (the Stage 0 Feed dumps were taken from ungraded clips, so every test project carries none)."""
    if recorded is None:
        out = ["no project state recorded with the references: record-state if the accuracy tests pass, else render"]
    else:
        out = differences(recorded, live)
    out += ["clip %s carries a grade (test projects carry none): %s" % tuple(g.split(": ", 1)) for g in graded_clips(live)]
    return out


def main_timeline(project):
    """The timeline the test data comes from: scopecreep-<key> (never 'hidden-branch'), else the current one."""
    for i in range(1, project.GetTimelineCount() + 1):
        tl = project.GetTimelineByIndex(i)
        if tl.GetName().startswith("scopecreep-"):
            return tl
    return project.GetCurrentTimeline() or project.GetTimelineByIndex(1)


def read_live(project):
    """Read the state of an open project by script. Read only."""
    tl = main_timeline(project)
    clips = [(clip_key(it.GetName()), dump._graph(it.GetNodeGraph())) for it in tl.GetItemListInTrack("video", 1)]
    return describe(project.GetName(), tl.GetName(), project.GetSetting(), dump._graph(tl.GetNodeGraph()), clips)


def write(directory, text):
    os.makedirs(directory, exist_ok=True)
    p = os.path.join(directory, FILE)
    with open(p, "w") as f:
        f.write(text)
    return p


def read(directory):
    """The recorded state, or None when there is none."""
    p = os.path.join(directory, FILE)
    if not os.path.exists(p):
        return None
    with open(p) as f:
        return f.read()


def check_all(resolve, selected=None):
    """check-state: every selected test project against the state recorded with its renders. Exits non-zero,
    naming each problem, when any project has drifted."""
    from . import projects, render, setups
    pm = resolve.GetProjectManager()
    original = projects.remember_original(pm)
    failed = 0
    for s in setups.ALL:
        if selected and s.key not in selected:
            continue
        project = projects.load(pm, s.project_name)
        if not project:
            print("check-state: %-34s MISSING (no such project)" % s.project_name)
            failed += 1
            continue
        found = problems(read(render.project_dir(s)), read_live(project))
        failed += 1 if found else 0
        print("check-state: %-34s %s" % (s.project_name, "ok" if not found else "DRIFTED"))
        for p in found:
            print("check-state:     %s" % p)
    if original:
        projects.load(pm, original)
    if failed:
        raise SystemExit("%d test project(s) no longer match their references" % failed)


def record_all(resolve, selected):
    """record-state: write the live state next to references rendered before states were recorded. Only for
    references known to match their project (the accuracy tests pass for them); refuses a graded project, or one
    whose references already carry a different state (re-render those instead)."""
    from . import projects, render, setups
    if not selected:
        raise SystemExit("record-state vouches for existing references: name them with --only")
    pm = resolve.GetProjectManager()
    original = projects.remember_original(pm)
    refused = []
    for s in setups.ALL:
        if s.key not in selected:
            continue
        project = projects.load(pm, s.project_name)
        if not project:
            raise SystemExit("project %s not found" % s.project_name)
        live, recorded = read_live(project), read(render.project_dir(s))
        found = problems(recorded, live) if recorded is not None else problems(live, live)
        if found:
            refused.append(s.project_name)
            print("record-state: %-34s REFUSED\n  %s" % (s.project_name, "\n  ".join(found)))
            continue
        print("record-state: %-34s wrote %s" % (s.project_name, write(render.project_dir(s), live)))
    if original:
        projects.load(pm, original)
    if refused:
        raise SystemExit("not recorded: " + ", ".join(refused))


def reset_clip_grades(resolve, selected):
    """reset-grades: remove every clip grade in the named test projects (Graph.ResetAllGrades on each clip), save,
    and verify by reading the clip graphs back. Refuses to run on every project at once."""
    from . import projects, setups
    if not selected:
        raise SystemExit("reset-grades changes projects: name them with --only")
    pm = resolve.GetProjectManager()
    original = projects.remember_original(pm)
    left = []
    for s in setups.ALL:
        if s.key not in selected:
            continue
        project = projects.load(pm, s.project_name)
        if not project:
            raise SystemExit("project %s not found" % s.project_name)
        for it in main_timeline(project).GetItemListInTrack("video", 1):
            ok = it.GetNodeGraph().ResetAllGrades()
            print("reset-grades: %-34s %-10s ResetAllGrades=%r" % (s.project_name, clip_key(it.GetName()), ok))
        pm.SaveProject()
        graded = graded_clips(read_live(project))
        left += ["%s %s" % (s.project_name, g) for g in graded]
        print("reset-grades: %-34s %s" % (s.project_name, "no clip grades" if not graded else "STILL GRADED"))
    if original:
        projects.load(pm, original)
    if left:
        raise SystemExit("grades remain: " + "; ".join(left))
