#!/usr/bin/env python3
"""scopecreep Resolve test harness. Drives a running DaVinci Resolve Studio end to end.

Commands (Resolve must be running, Preferences > System > General > External scripting using = Local):

  media      generate float EXR test media into the test-data directory (no Resolve needed)
  projects   rebuild every scopecreep-test-* project from scratch (idempotent)
  dump       read the Colour Setup of every test project by script and save it
  render     render 16-bit TIFF and float EXR reference frames in the Output Colour Space
  all        media + projects + dump + render
  clean      delete every scopecreep-test-* project
  status     product, current project/page/timecode
  goto TC    move the playhead (e.g. 01:00:00:12)
  play|stop  start/stop playback by sending keystrokes to the Resolve window (Hyprland)
  shot [P]   screenshot the Resolve window to P (default: test-data/screenshots)

Test data root: $SCOPECREEP_TESTDATA, default ~/scopecreep-testdata (never inside git).
Options: --only KEY[,KEY]  limit to some setups (e.g. dwg-rec709); --force  regenerate media.
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from scopecreep_harness import paths, setups, testmedia  # noqa: E402


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["media", "projects", "dump", "render", "all", "clean", "status",
                                        "goto", "play", "stop", "shot"])
    ap.add_argument("arg", nargs="?")
    ap.add_argument("--only", help="comma-separated setup keys: " + ", ".join(s.key for s in setups.ALL))
    ap.add_argument("--force", action="store_true", help="regenerate media even if present")
    a = ap.parse_args(argv)
    only = set(a.only.split(",")) if a.only else None
    cmd = a.command

    if cmd == "media":
        testmedia.generate(force=a.force)
        return 0

    from scopecreep_harness import resolve_api  # may re-exec under Resolve's Python
    resolve = resolve_api.connect()

    from scopecreep_harness import projects, dump, render, viewer
    if cmd in ("projects", "all"):
        if cmd == "all":
            testmedia.generate(force=a.force)
        projects.build_all(resolve, only)
    if cmd in ("dump", "all"):
        dump.dump_all(resolve, only)
    if cmd in ("render", "all"):
        render.render_all(resolve, only)
    if cmd == "clean":
        pm = resolve.GetProjectManager()
        gone = projects.clean(pm, projects.remember_original(pm))
        print("deleted:", ", ".join(gone) or "(none)")
    if cmd == "status":
        viewer.status(resolve)
    if cmd == "goto":
        viewer.goto(resolve, a.arg)
    if cmd == "play":
        viewer.play(resolve)
    if cmd == "stop":
        viewer.stop(resolve)
    if cmd == "shot":
        print(viewer.screenshot(a.arg))
    return 0


if __name__ == "__main__":
    sys.exit(main())
