#!/usr/bin/env python3
"""scopecreep Resolve test harness. Drives a running DaVinci Resolve Studio end to end.

Commands (Resolve must be running, Preferences > System > General > External scripting using = Local):

  media      generate float EXR test media into the test-data directory (no Resolve needed)
  projects   rebuild every scopecreep-test-* project from scratch (idempotent)
  dump       read the Colour Setup of every test project by script and save it
  render     render 16-bit TIFF and float EXR reference frames in the Output Colour Space
  clone-sender  export the hand-built scopecreep-test-dwg-p3d65 (Sender on its timeline) to a .drp and clone it
             into every other colour-setup project, applying that setup's Colour Setup; each clone is verified
             (settings vs dump, node graph, Sender renders counted in the Feed, EXR render vs reference) before replacing the original
  dump-timeline-frames  collect the probe Sender's Timeline Colour Space frame 1 of every clip, per project, into
             <test-data>/dumps-feed/<project>/<clip>.exr (the input for accuracy/)
  feed-frames  the accuracy suite's capture: frame 1 of every clip from the real Feed after the app's Output
             Conversion, with the app's label, into <test-data>/feed-converted/<project>/ (input for
             `cargo run -p scopecreep-accuracy -- feed`; needs the capture_converted example built)
  check-state  compare each test project with the state recorded next to its renders (Colour Setup, timeline and
             clip grades) and fail, naming every difference, when they no longer match or a clip carries a grade
  record-state  write the --only projects' live state next to references rendered before states were recorded
             (only for references the accuracy tests show match; refuses a graded project)
  reset-grades  remove every clip grade in the --only projects (Graph.ResetAllGrades) and verify the clip graphs
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
                                        "goto", "play", "stop", "shot", "clone-sender", "dump-timeline-frames",
                                        "feed-frames", "check-state", "record-state", "reset-grades"])
    ap.add_argument("arg", nargs="?")
    ap.add_argument("--only", help="comma-separated setup keys: " + ", ".join(s.key for s in setups.ALL))
    ap.add_argument("--keep-hidden", action="store_true", help="clone-sender: keep the hidden-branch timeline (its CST is P3-D65 specific)")
    ap.add_argument("--verify-only", action="store_true", help="clone-sender: verify each clone, then delete it; replace nothing")
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
    if cmd == "clone-sender":
        from scopecreep_harness import clone
        clone.clone_all(resolve, only, a.keep_hidden, replace=not a.verify_only)
    if cmd == "dump-timeline-frames":
        from scopecreep_harness import dump_feed
        dump_feed.dump_all(resolve, only)
    if cmd == "feed-frames":
        from scopecreep_harness import feed_frames
        feed_frames.capture_all(resolve, only)
    if cmd == "check-state":
        from scopecreep_harness import state
        state.check_all(resolve, only)
    if cmd == "record-state":
        from scopecreep_harness import state
        state.record_all(resolve, only)
    if cmd == "reset-grades":
        from scopecreep_harness import state
        state.reset_clip_grades(resolve, only)
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
