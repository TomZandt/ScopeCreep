"""Drive the Resolve viewer: playhead by script, playback by keystroke, screenshots.

Linux: keystrokes go through Hyprland (Resolve runs under XWayland). Hyprland 0.55+ uses Lua dispatchers
(`hl.dsp.send_shortcut`); older versions take `sendshortcut MODS,KEY,WINDOW`. Both are tried. Screenshots by grim.

Windows (`send_key` and `grab` only): Resolve's main window is brought to the front and sent the key as keyboard
input; screenshots by PowerShell's System.Drawing, with nothing to install.
"""
import json
import os
import subprocess
import sys
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
    if sys.platform == "win32":
        if mods:
            raise NotImplementedError("modifier keys are not sent on Windows")
        return _windows_send_key(key)
    win = window or find_window()
    addr = win["address"]
    lua = 'hl.dsp.send_shortcut({ mods = "%s", key = "%s", window = "address:%s" })' % (mods, key, addr)
    out = _hyprctl("dispatch", lua)
    if out != "ok":
        legacy = _hyprctl("dispatch", "sendshortcut", "%s,%s,address:%s" % (mods, key, addr))
        if legacy != "ok":
            raise RuntimeError("Hyprland rejected the keystroke: %r / %r" % (out, legacy))


def _windows_send_key(key):
    """Bring Resolve's main window to the front and press `key` (a letter or digit) in it."""
    import ctypes
    from ctypes import wintypes
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32")
    user32.GetForegroundWindow.restype = wintypes.HWND
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.c_void_p]
    user32.GetWindowThreadProcessId.restype = wintypes.DWORD
    user32.AttachThreadInput.argtypes = [wintypes.DWORD, wintypes.DWORD, wintypes.BOOL]
    user32.SetForegroundWindow.argtypes = [wintypes.HWND]
    user32.BringWindowToTop.argtypes = [wintypes.HWND]
    hwnd = _windows_find_resolve(user32)
    if user32.GetForegroundWindow() != hwnd:
        # Windows lets a process raise another's window only while its input is attached to the front window's
        # thread; attach, raise, detach.
        front = user32.GetWindowThreadProcessId(user32.GetForegroundWindow(), None)
        mine = kernel32.GetCurrentThreadId()
        attached = front != mine and user32.AttachThreadInput(mine, front, True)
        try:
            user32.BringWindowToTop(hwnd)
            user32.SetForegroundWindow(hwnd)
        finally:
            if attached:
                user32.AttachThreadInput(mine, front, False)
        time.sleep(0.2)
        if user32.GetForegroundWindow() != hwnd:
            raise RuntimeError("could not bring Resolve to the front; click its window and run again")
    vk = ord(key.upper())
    scan = user32.MapVirtualKeyW(vk, 0)
    KEYEVENTF_KEYUP = 0x0002
    user32.keybd_event(vk, scan, 0, 0)
    time.sleep(0.03)
    user32.keybd_event(vk, scan, KEYEVENTF_KEYUP, 0)


def _windows_find_resolve(user32):
    """The largest visible top-level window whose title starts with "DaVinci Resolve"."""
    import ctypes
    from ctypes import wintypes
    found = []
    proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def visit(hwnd, _):
        if user32.IsWindowVisible(hwnd):
            n = user32.GetWindowTextLengthW(hwnd)
            title = ctypes.create_unicode_buffer(n + 1)
            user32.GetWindowTextW(hwnd, title, n + 1)
            if title.value.startswith("DaVinci Resolve"):
                r = wintypes.RECT()
                user32.GetWindowRect(hwnd, ctypes.byref(r))
                found.append(((r.right - r.left) * (r.bottom - r.top), hwnd))
        return True

    user32.EnumWindows(proc(visit), 0)
    if not found:
        raise SystemExit("Resolve's window not found (no visible window titled \"DaVinci Resolve...\")")
    return max(found)[1]


def grab(path, region=None):
    """Save a screenshot of `region` ("X,Y WxH" in screen pixels, grim's form), or of every screen, to `path`."""
    if sys.platform != "win32":
        subprocess.run(["grim"] + (["-g", region] if region else []) + [path])
        return
    if region:
        xy, wh = region.split()
        x, y = (int(v) for v in xy.split(","))
        w, h = (int(v) for v in wh.split("x"))
        rect = "New-Object System.Drawing.Rectangle %d,%d,%d,%d" % (x, y, w, h)
    else:
        rect = "[System.Windows.Forms.SystemInformation]::VirtualScreen"
    script = (
        "Add-Type -AssemblyName System.Drawing,System.Windows.Forms;"
        # Physical pixels, as on a display at 100%, whatever the display scaling.
        "Add-Type -Namespace ScopeCreep -Name Dpi -MemberDefinition "
        "'[DllImport(\"user32.dll\")] public static extern bool SetProcessDPIAware();';"
        "[void][ScopeCreep.Dpi]::SetProcessDPIAware();"
        "$r = %s;"
        "$b = New-Object System.Drawing.Bitmap $r.Width,$r.Height;"
        "$g = [System.Drawing.Graphics]::FromImage($b);"
        "$g.CopyFromScreen($r.X,$r.Y,0,0,$b.Size);"
        "$b.Save('%s',[System.Drawing.Imaging.ImageFormat]::Png)" % (rect, path.replace("'", "''")))
    subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script])


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
