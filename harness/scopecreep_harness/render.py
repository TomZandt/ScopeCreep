"""Render reference frames through the Deliver queue in each project's Output Colour Space.

For each test project and each clip, the first frame is rendered twice: 16-bit TIFF and float EXR,
with Color Space Tag / Gamma Tag = 'Same as Project' (that is, the project's Output Colour Space).
"""
import glob
import os
import shutil
import time

from . import exr, paths, setups, testmedia

# Resolve refuses render paths outside its Media Storage locations ("Render Path Inaccessible"), and on this
# machine the only one is ~/Videos. Render there, then move the frames into the test-data directory.
STAGING = os.environ.get("SCOPECREEP_RENDER_STAGING") or os.path.expanduser("~/Videos/scopecreep-test-render-tmp")

FORMATS = [("exr", "RGBFloat"), ("tif", "RGB16")]
DONE = {"Complete"}
FAILED = {"Failed", "Cancelled"}


def clip_start_offsets(setup):
    off, out = 0, {}
    for c in setup.clips:
        out[c] = off
        off += testmedia.CLIPS[c][2]
    return out


def project_dir(setup):
    return os.path.join(paths.RENDERS, setup.project_name)


def render_project(resolve, setup, timeout=600, project_name=None, outdir=None):
    """Render frame 1 of every clip. project_name/outdir override where to render from/to (used by clone-sender
    to render a candidate project without touching the reference renders)."""
    from . import projects
    pm = resolve.GetProjectManager()
    project_name = project_name or setup.project_name
    project = projects.load(pm, project_name)
    if not project:
        raise SystemExit("project %s not found (run `harness projects`)" % project_name)
    timeline = project.GetCurrentTimeline() or project.GetTimelineByIndex(1)
    project.SetCurrentTimeline(timeline)
    start = timeline.GetStartFrame()

    outdir = outdir or project_dir(setup)
    if os.path.isdir(outdir):
        shutil.rmtree(outdir)  # idempotent: always re-render from scratch
    os.makedirs(outdir)
    staging = os.path.join(STAGING, project_name)
    if os.path.isdir(staging):
        shutil.rmtree(staging)
    project.DeleteAllRenderJobs()

    jobs = {}
    for clip, off in clip_start_offsets(setup).items():
        frame = start + off
        for ext, codec in FORMATS:
            if not project.SetCurrentRenderFormatAndCodec(ext, codec):
                raise RuntimeError("Resolve refused render format %s/%s" % (ext, codec))
            target = os.path.join(staging, "%s-%s" % (clip, ext))
            os.makedirs(target)
            ok = project.SetRenderSettings({
                "SelectAllFrames": False, "MarkIn": frame, "MarkOut": frame,
                "TargetDir": target, "CustomName": "%s_%s" % (clip, ext),
                "ExportVideo": True, "ExportAudio": False,
                "ColorSpaceTag": "Same as Project", "GammaTag": "Same as Project",
            })
            if not ok:
                raise RuntimeError("Resolve refused render settings for %s %s" % (clip, ext))
            job = project.AddRenderJob()
            if not job:
                raise RuntimeError("could not queue render for %s %s" % (clip, ext))
            jobs[job] = (clip, ext)

    project.StartRendering(list(jobs), False)
    t0 = time.time()
    while project.IsRenderingInProgress():
        if time.time() - t0 > timeout:
            project.StopRendering()
            raise RuntimeError("render timed out for %s" % setup.project_name)
        time.sleep(0.5)
    results = {}
    for job, (clip, ext) in jobs.items():
        st = project.GetRenderJobStatus(job)
        status = st.get("JobStatus")
        staged = sorted(glob.glob(os.path.join(staging, "%s-%s" % (clip, ext), "*." + ext)))
        files = []
        for f in staged:
            dest_dir = os.path.join(outdir, "%s-%s" % (clip, ext))
            os.makedirs(dest_dir, exist_ok=True)
            files.append(shutil.move(f, os.path.join(dest_dir, os.path.basename(f))))
        results[(clip, ext)] = (status, files)
        if status in FAILED or not files:
            print("render: FAILED %s %s %s %r" % (setup.project_name, clip, ext, st))
    project.DeleteAllRenderJobs()
    shutil.rmtree(staging, ignore_errors=True)
    try:
        os.rmdir(STAGING)  # only succeeds when empty
    except OSError:
        pass
    return results


def describe_exr(path):
    img = exr.read_exr(path)
    flat_min, flat_max = float("inf"), float("-inf")
    for r, g, b in img.rows:
        for ch in (r, g, b):
            lo, hi = min(ch), max(ch)
            flat_min, flat_max = min(flat_min, lo), max(flat_max, hi)
    return "%dx%d min=%.4f max=%.4f" % (img.width, img.height, flat_min, flat_max)


def render_all(resolve, selected=None):
    pm = resolve.GetProjectManager()
    from . import projects
    original = projects.remember_original(pm)
    failures = 0
    for s in setups.ALL:
        if selected and s.key not in selected:
            continue
        t = time.time()
        results = render_project(resolve, s)
        for (clip, ext), (status, files) in sorted(results.items()):
            ok = status in DONE and files
            failures += 0 if ok else 1
            info = ""
            if ok and ext == "exr":
                try:
                    info = describe_exr(files[0])
                except Exception as e:  # reference may use a compression the stdlib reader lacks
                    info = "(not stats-readable: %s)" % e
            print("render: %-34s %-11s %s %s %s" % (s.project_name, clip, ext, status, info))
        print("render: %s done (%.1fs)" % (s.project_name, time.time() - t))
    if original:
        projects.load(pm, original)
    if failures:
        raise SystemExit("%d render job(s) failed" % failures)
