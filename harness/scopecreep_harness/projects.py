"""Build the test projects in the running Resolve. Only ever touches projects named scopecreep-test-*."""
import time

from . import paths, setups, testmedia
from .resolve_api import set_checked

SCRATCH = paths.PROJECT_PREFIX + "scratch"
FIRST_TC_FRAME_OFFSET = 0  # clips are laid back to back from the timeline start


def _is_ours(name):
    return name.startswith(paths.PROJECT_PREFIX)


def remember_original(pm):
    """Name of the project the user had open, or None if it is one of ours."""
    cur = pm.GetCurrentProject()
    name = cur.GetName() if cur else None
    return None if (name is None or _is_ours(name)) else name


def _ensure_scratch(pm):
    if SCRATCH in (pm.GetProjectListInCurrentFolder() or []):
        return load(pm, SCRATCH)
    scratch = pm.CreateProject(SCRATCH)
    if not scratch:
        raise RuntimeError("Resolve would not create the scratch project %s" % SCRATCH)
    pm.SaveProject()  # an unsaved empty project is discarded when another is opened
    return scratch


def delete_project(pm, name):
    assert _is_ours(name), "refusing to delete %r: not a %s project" % (name, paths.PROJECT_PREFIX)
    if name not in (pm.GetProjectListInCurrentFolder() or []):
        return
    cur = pm.GetCurrentProject()
    if cur and cur.GetName() == name:
        if name == SCRATCH:
            raise RuntimeError("cannot delete the open scratch project")
        _ensure_scratch(pm)
    if not pm.DeleteProject(name):
        raise RuntimeError("Resolve would not delete project %s (open: %s)" % (
            name, pm.GetCurrentProject().GetName()))


def clean(pm, original=None):
    """Delete every scopecreep-test-* project (including scratch), then reopen the user's project."""
    names = [n for n in (pm.GetProjectListInCurrentFolder() or []) if _is_ours(n)]
    if original:
        load(pm, original)
    else:
        _ensure_scratch(pm)
    for n in names:
        if n != SCRATCH or original:
            delete_project_unchecked_open(pm, n)
    return names


def delete_project_unchecked_open(pm, name):
    assert _is_ours(name)
    if not pm.DeleteProject(name):
        raise RuntimeError("Resolve would not delete project %s" % name)


def build(resolve, setup):
    """Create (replacing any existing project of that name) one project, import media, build a timeline."""
    pm = resolve.GetProjectManager()
    delete_project(pm, setup.project_name)
    project = pm.CreateProject(setup.project_name)
    if not project:
        raise RuntimeError("Resolve would not create project %s" % setup.project_name)
    w, h = setup.resolution
    for k, v in [("timelineResolutionWidth", str(w)), ("timelineResolutionHeight", str(h)),
                 ("timelineOutputResolutionWidth", str(w)), ("timelineOutputResolutionHeight", str(h)),
                 ("timelineFrameRate", setup.fps), ("timelinePlaybackFrameRate", setup.fps)]:
        project.SetSetting(k, v)  # frame rate/resolution echo can differ in type; verified in the dump
    for k, v in setup.settings:
        set_checked(project, k, v)

    mp = project.GetMediaPool()
    items = []
    for clip in setup.clips:
        _w, _h, frames, *_ = testmedia.CLIPS[clip]
        got = mp.ImportMedia([{
            "FilePath": testmedia.clip_dir(clip) + "/%s.%%04d.exr" % clip,
            "StartIndex": 1, "EndIndex": frames,
        }])
        if not got:
            raise RuntimeError("could not import %s (run `harness media` first)" % clip)
        if setup.input_space:
            if not got[0].SetClipProperty("Input Color Space", setup.input_space):
                raise RuntimeError("Resolve refused Input Color Space %r for %s" % (setup.input_space, clip))
        items.append(got[0])
    timeline = mp.CreateTimelineFromClips("scopecreep-" + setup.key, items)
    if not timeline:
        raise RuntimeError("could not create timeline for %s" % setup.project_name)
    project.SetCurrentTimeline(timeline)
    pm.SaveProject()
    return project, timeline


def build_all(resolve, selected=None):
    pm = resolve.GetProjectManager()
    original = remember_original(pm)
    _ensure_scratch(pm)
    for s in setups.ALL:
        if selected and s.key not in selected:
            continue
        t = time.time()
        build(resolve, s)
        print("project: built %s (%.1fs)" % (s.project_name, time.time() - t))
    if original:
        load(pm, original)
        print("project: reopened your project %r" % original)
    return original


def load(pm, name):
    """Open a project by name. Resolve's LoadProject returns None when it is already the open one."""
    cur = pm.GetCurrentProject()
    if cur and cur.GetName() == name:
        return cur
    return pm.LoadProject(name) or None
