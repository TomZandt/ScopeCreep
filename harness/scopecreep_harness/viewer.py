"""Drive the Resolve viewer: playhead by script, playback by keystroke, screenshots.

Keystrokes go through Hyprland (Resolve runs under XWayland). Hyprland 0.55+ uses Lua dispatchers
(`hl.dsp.send_shortcut`); older versions take `sendshortcut MODS,KEY,WINDOW`. Both are tried.
"""
import json
import os
import subprocess
import time

from . import paths


def _hyprctl(*args):
    r = subprocess.run(["hyprctl", *args], capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout + r.stderr).strip()


def find_window(title_prefix="DaVinci Resolve"):
    clients = json.loads(_hyprctl("-j", "clients"))
    for c in clients:
        if c.get("class") == "resolve" and c.get("title", "").startswith(title_prefix):
            return c
    raise SystemExit("Resolve window not found in `hyprctl clients`")


def send_key(key, mods="", window=None):
    """Send one key combination to the Resolve main window. Raises if Hyprland rejects the dispatch."""
    win = window or find_window()
    addr = win["address"]
    lua = 'hl.dsp.send_shortcut({ mods = "%s", key = "%s", window = "address:%s" })' % (mods, key, addr)
    out = _hyprctl("dispatch", lua)
    if out != "ok":
        legacy = _hyprctl("dispatch", "sendshortcut", "%s,%s,address:%s" % (mods, key, addr))
        if legacy != "ok":
            raise RuntimeError("Hyprland rejected the keystroke: %r / %r" % (out, legacy))


def timecode(resolve):
    project = resolve.GetProjectManager().GetCurrentProject()
    return project.GetCurrentTimeline().GetCurrentTimecode()


def status(resolve):
    pm = resolve.GetProjectManager()
    project = pm.GetCurrentProject()
    tl = project.GetCurrentTimeline() if project else None
    print("product:  %s %s (IsStudio=%s)" % (resolve.GetProductName(), resolve.GetVersionString(), resolve.IsStudio()))
    print("project:  %s" % (project.GetName() if project else None))
    print("timeline: %s" % (tl.GetName() if tl else None))
    print("page:     %s" % resolve.GetCurrentPage())
    print("timecode: %s" % (tl.GetCurrentTimecode() if tl else None))


def goto(resolve, tc):
    tl = resolve.GetProjectManager().GetCurrentProject().GetCurrentTimeline()
    if not tl.SetCurrentTimecode(tc):
        raise SystemExit("Resolve refused timecode %r" % tc)
    print("playhead:", tl.GetCurrentTimecode())


def play(resolve):
    """Start forward playback (J/K/L: L = play forward)."""
    before = timecode(resolve)
    send_key("L")
    time.sleep(1.0)
    after = timecode(resolve)
    print("play: timecode %s -> %s (%s)" % (before, after, "advancing" if after != before else "NOT advancing"))
    return after != before


def stop(resolve):
    """Stop playback (K = pause)."""
    send_key("K")
    time.sleep(0.5)
    a = timecode(resolve)
    time.sleep(0.7)
    b = timecode(resolve)
    print("stop: timecode %s then %s (%s)" % (a, b, "stopped" if a == b else "STILL MOVING"))
    return a == b


def screenshot(path=None):
    win = find_window()
    x, y = win["at"]
    w, h = win["size"]
    if path is None:
        os.makedirs(paths.SHOTS, exist_ok=True)
        path = os.path.join(paths.SHOTS, "resolve-%s.png" % time.strftime("%Y%m%d-%H%M%S"))
    r = subprocess.run(["grim", "-g", "%d,%d %dx%d" % (x, y, w, h), path], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError("grim failed: %s" % r.stderr)
    return path
