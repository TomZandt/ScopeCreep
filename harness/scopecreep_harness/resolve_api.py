"""Connect to a running Resolve through its scripting API (external scripting = Local)."""
import collections
import ntpath
import os
import posixpath
import subprocess
import sys

ScriptingPaths = collections.namedtuple("ScriptingPaths", "python modules lib")


def scripting_paths(platform, environ):
    """Where Resolve installs its scripting on `platform` (a `sys.platform` value): its bundled Python (which may
    not exist), the folder holding the DaVinciResolveScript module, and the fusionscript library. Resolve's own
    RESOLVE_SCRIPT_API and RESOLVE_SCRIPT_LIB override the last two, as its scripting README says.

    The one list of Resolve's install paths. The app keeps copies it cannot import: the module and library in
    app/resolve/colour_setup_helper.py (tests/test_resolve_api.py checks they agree) and the bundled Python in
    `python_candidates` of app/src/platform/windows.rs and `resolve_python` of app/src/platform/linux.rs."""
    if platform == "win32":
        join = ntpath.join
        resolve = join(environ.get("PROGRAMFILES") or "C:\\Program Files", "Blackmagic Design", "DaVinci Resolve")
        python = join(resolve, "ResolvePython.exe")
        api = join(environ.get("PROGRAMDATA") or "C:\\ProgramData",
                   "Blackmagic Design", "DaVinci Resolve", "Support", "Developer", "Scripting")
        lib = join(resolve, "fusionscript.dll")
    else:
        join = posixpath.join
        python, api, lib = ("/opt/resolve/bin/ResolvePython", "/opt/resolve/Developer/Scripting",
                            "/opt/resolve/libs/Fusion/fusionscript.so")
    api = environ.get("RESOLVE_SCRIPT_API") or api
    return ScriptingPaths(python, join(api, "Modules"), environ.get("RESOLVE_SCRIPT_LIB") or lib)


def _import_module():
    try:
        import DaVinciResolveScript as dvr  # works out of the box under ResolvePython (21.1+)
        return dvr
    except ImportError:
        pass
    paths = scripting_paths(sys.platform, os.environ)
    if os.path.exists(paths.python) and os.path.realpath(sys.executable) != os.path.realpath(paths.python):
        # Re-run the same command under Resolve's bundled interpreter, which needs no env vars.
        if sys.platform == "win32":  # Windows' execv starts a new process and returns to the shell at once
            sys.exit(subprocess.call([paths.python] + sys.argv))
        os.execv(paths.python, [paths.python] + sys.argv)
    sys.path.append(paths.modules)
    os.environ.setdefault("RESOLVE_SCRIPT_LIB", paths.lib)
    import DaVinciResolveScript as dvr
    return dvr


def connect():
    dvr = _import_module()
    resolve = dvr.scriptapp("Resolve")
    if resolve is None:
        raise SystemExit(
            "Could not reach Resolve. Start Resolve Studio and set Preferences > System > General > "
            "External scripting using = Local."
        )
    return resolve


class SettingError(RuntimeError):
    pass


def set_checked(target, key, value):
    """SetSetting and verify by reading back. Resolve rejects unknown values with False."""
    ok = target.SetSetting(key, value)
    got = target.GetSetting(key)
    if not ok or str(got) != str(value):
        raise SettingError("Resolve refused %s=%r (SetSetting=%r, now %r)" % (key, value, ok, got))
