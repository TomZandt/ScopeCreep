"""Connect to a running Resolve through its scripting API (external scripting = Local)."""
import os
import sys

RESOLVE_PYTHON = "/opt/resolve/bin/ResolvePython"
MODULES = "/opt/resolve/Developer/Scripting/Modules"


def _import_module():
    try:
        import DaVinciResolveScript as dvr  # works out of the box under ResolvePython (21.1+)
        return dvr
    except ImportError:
        pass
    if os.path.exists(RESOLVE_PYTHON) and os.path.realpath(sys.executable) != os.path.realpath(RESOLVE_PYTHON):
        # Re-run the same command under Resolve's bundled interpreter, which needs no env vars.
        os.execv(RESOLVE_PYTHON, [RESOLVE_PYTHON] + sys.argv)
    sys.path.append(MODULES)
    os.environ.setdefault("RESOLVE_SCRIPT_LIB", "/opt/resolve/libs/Fusion/fusionscript.so")
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
