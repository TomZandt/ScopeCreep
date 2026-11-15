"""scopecreep's Resolve helper: reads the project's Colour Setup through Resolve's scripting API, on request only.

The scopecreep app starts this under Resolve's own Python (/opt/resolve/bin/ResolvePython) and owns it. It never
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


def scripting_module():
    try:
        import DaVinciResolveScript as dvr  # works as-is under ResolvePython (Resolve 21.1+)
        return dvr
    except ImportError:
        import os
        sys.path.append("/opt/resolve/Developer/Scripting/Modules")
        os.environ.setdefault("RESOLVE_SCRIPT_LIB", "/opt/resolve/libs/Fusion/fusionscript.so")
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
