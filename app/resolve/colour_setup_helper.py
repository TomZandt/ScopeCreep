"""scopecreep's Resolve helper: reads the project's Colour Setup through Resolve's scripting API, on request only.

The scopecreep app starts this under Resolve's own Python (Linux: /opt/resolve/bin/ResolvePython; Windows: a
Python the user installed, see app/src/platform/windows.rs; macOS: the system's python3, see
app/src/platform/macos.rs) and owns it. It never
calls Resolve by itself: it waits on stdin, and each line `read` makes it do one short read and answer on stdout.
The app sends `read` only while playback is stopped, because Resolve scripting calls made during playback block
and starve Resolve's main thread (Stage 0, ADR 0005). See docs/adr/0006-colour-setup-helper.md.

Answer: lines of `key=value`, ended by an empty line.
  result=ok | unreachable | no-project | error
  message=...                  (when result is not ok)
  product=, version=, studio=  (Resolve product name, version, 1 = Studio)
  project=, timeline=          (names of the current project and timeline)
  setting.<key>=<value>        (Resolve's own Colour Setup setting keys and values, from the current timeline,
                                else the project; the app turns these into a Colour Setup)
  read_ms=                     (how long the scripting calls took)
"""
import sys
import time

# The Colour Setup keys (glossary: colour science mode, Timeline/Output Colour Space, tone and gamut mapping, ACES
# version and output transform, Levels). Keys and values are in #6's closing comment. acesVersion is asked for in
# case a later Resolve exposes it; 21.2 does not.
KEYS = [
    "colorScienceMode", "separateColorSpaceAndGamma",
    "colorSpaceTimeline", "colorSpaceTimelineGamma",
    "colorSpaceOutput", "colorSpaceOutputGamma",
    "colorSpaceOutputToneMapping", "colorSpaceOutputGamutMapping",
    "inputDRT", "outputDRT",
    "acesVersion", "colorAcesIDT", "colorAcesODT", "colorAcesGamutCompressType",
    "videoDataLevels",
]


def clean(value):
    return " ".join(str(value).split())  # one line, whatever Resolve returns


def answer(fields):
    out = sys.stdout
    for key, value in fields:
        out.write("%s=%s\n" % (key, clean(value)))
    out.write("\n")
    out.flush()


def scripting_paths(platform, environ):
    """(scripting API folder, fusionscript library) where Resolve installs them on `platform` (a `sys.platform`
    value), unless Resolve's own RESOLVE_SCRIPT_API and RESOLVE_SCRIPT_LIB say otherwise. The module is in
    <API folder>/Modules. A copy of harness/scopecreep_harness/resolve_api.py's `scripting_paths`, since this
    script runs alone; harness/tests/test_resolve_api.py checks the two agree."""
    import ntpath
    if platform == "win32":
        api = ntpath.join(environ.get("PROGRAMDATA") or "C:\\ProgramData",
                          "Blackmagic Design", "DaVinci Resolve", "Support", "Developer", "Scripting")
        lib = ntpath.join(environ.get("PROGRAMFILES") or "C:\\Program Files",
                          "Blackmagic Design", "DaVinci Resolve", "fusionscript.dll")
    elif platform == "darwin":
        api = "/Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/Scripting"
        lib = "/Applications/DaVinci Resolve/DaVinci Resolve.app/Contents/Libraries/Fusion/fusionscript.so"
    else:
        api, lib = "/opt/resolve/Developer/Scripting", "/opt/resolve/libs/Fusion/fusionscript.so"
    return environ.get("RESOLVE_SCRIPT_API") or api, environ.get("RESOLVE_SCRIPT_LIB") or lib


WATCH_PARENT_ENV = "SCOPECREEP_HELPER_WATCH_PARENT"


def watch_parent(app_pid, helper_pid, poll=0.5):
    """Start a watcher process that kills the helper (`helper_pid`) once the app (`app_pid`) is gone.

    macOS has no parent-death signal, and a helper stuck inside a Resolve call cannot be trusted to notice by
    itself (a thread of the helper needs the interpreter lock that call may hold), so the watcher is a separate,
    tiny process, started before Resolve's module is loaded. It also ends when the helper does. Returns the
    watcher's pid in the helper, None in the watcher (which never returns to the caller's code)."""
    import os
    import signal
    child = os.fork()
    if child:
        return child
    try:
        # Hold none of the helper's pipes: the app sees the helper's end when the helper ends.
        null = os.open(os.devnull, os.O_RDWR)
        for fd in (0, 1, 2):
            os.dup2(null, fd)
        while os.getppid() == helper_pid:  # reparented: the helper has gone
            try:
                os.kill(app_pid, 0)
            except ProcessLookupError:
                os.kill(helper_pid, signal.SIGKILL)
                break
            except PermissionError:
                pass
            time.sleep(poll)
    finally:
        os._exit(0)


def scripting_module():
    try:
        import DaVinciResolveScript as dvr  # works as-is under ResolvePython (Resolve 21.1+)
        return dvr
    except ImportError:
        import os
        api, lib = scripting_paths(sys.platform, os.environ)
        sys.path.append(os.path.join(api, "Modules"))
        os.environ["RESOLVE_SCRIPT_LIB"] = lib
        import DaVinciResolveScript as dvr
        return dvr


class Helper:
    def __init__(self):
        self.dvr = None
        self.resolve = None
        self.product = []

    def connect(self):
        if self.dvr is None:
            self.dvr = scripting_module()
        resolve = self.dvr.scriptapp("Resolve")
        if resolve is None:
            return False
        self.resolve = resolve
        self.product = [("product", resolve.GetProductName()), ("version", resolve.GetVersionString()),
                        ("studio", 1 if resolve.IsStudio() else 0)]
        return True

    def read(self):
        t0 = time.monotonic()
        try:
            if self.resolve is None and not self.connect():
                return [("result", "unreachable"),
                        ("message", "Resolve is not running, or Preferences > System > General > External scripting "
                                    "using is not set to Local (Resolve Studio only)")]
            project = self.resolve.GetProjectManager().GetCurrentProject()
            if project is None:
                # Either no project is open, or Resolve went away: connect again next time.
                self.resolve = None
                return [("result", "no-project"), ("message", "no project is open in Resolve")]
            timeline = project.GetCurrentTimeline()
            settings = (timeline.GetSetting() if timeline else None) or project.GetSetting() or {}
            fields = [("result", "ok")] + self.product + [("project", project.GetName())]
            if timeline:
                fields.append(("timeline", timeline.GetName()))
            fields += [("setting." + k, settings[k]) for k in KEYS if settings.get(k) is not None]
            fields.append(("read_ms", "%.1f" % ((time.monotonic() - t0) * 1000)))
            return fields
        except Exception as e:  # Resolve quit mid-call, or the API changed: report it, try again next time
            self.resolve = None
            return [("result", "error"), ("message", "%s: %s" % (type(e).__name__, e))]


def main():
    import os
    if os.environ.get(WATCH_PARENT_ENV) == "1" or sys.platform == "darwin":
        watch_parent(os.getppid(), os.getpid())
    helper = Helper()
    answer([("result", "started"), ("python", sys.version.split()[0])])
    for line in sys.stdin:
        command = line.strip()
        if command == "read":
            answer(helper.read())
        elif command == "quit":
            break


if __name__ == "__main__":
    main()
