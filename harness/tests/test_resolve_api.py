import ntpath
import os
import posixpath
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import resolve_api


class ScriptingPaths(unittest.TestCase):
    def test_linux_uses_resolve_under_opt(self):
        p = resolve_api.scripting_paths("linux", {})
        self.assertEqual(p.python, "/opt/resolve/bin/ResolvePython")
        self.assertEqual(p.modules, "/opt/resolve/Developer/Scripting/Modules")
        self.assertEqual(p.lib, "/opt/resolve/libs/Fusion/fusionscript.so")

    def test_windows_uses_program_data_and_program_files(self):
        p = resolve_api.scripting_paths("win32", {"PROGRAMDATA": "D:\\Data", "PROGRAMFILES": "D:\\Apps"})
        self.assertEqual(p.modules, "D:\\Data\\Blackmagic Design\\DaVinci Resolve\\Support\\Developer\\Scripting\\Modules")
        self.assertEqual(p.lib, "D:\\Apps\\Blackmagic Design\\DaVinci Resolve\\fusionscript.dll")
        self.assertEqual(p.python, "D:\\Apps\\Blackmagic Design\\DaVinci Resolve\\ResolvePython.exe")

    def test_windows_defaults_without_the_environment(self):
        p = resolve_api.scripting_paths("win32", {})
        self.assertEqual(p.lib, "C:\\Program Files\\Blackmagic Design\\DaVinci Resolve\\fusionscript.dll")
        self.assertTrue(p.modules.startswith("C:\\ProgramData\\Blackmagic Design\\"))

    def test_resolves_own_variables_override_the_install_paths(self):
        for platform in ("linux", "win32"):
            p = resolve_api.scripting_paths(platform, {"RESOLVE_SCRIPT_API": "/x/api", "RESOLVE_SCRIPT_LIB": "/x/lib"})
            self.assertEqual(p.lib, "/x/lib")
            self.assertIn(p.modules, ("/x/api/Modules", "/x/api\\Modules"))


class TheAppsHelperAgrees(unittest.TestCase):
    """The app's Resolve helper (app/resolve/colour_setup_helper.py) runs alone under Resolve's Python, so it keeps
    its own copy of these paths; this keeps the two copies the same."""

    def helper(self):
        import importlib.util
        path = os.path.join(os.path.dirname(__file__), "..", "..", "app", "resolve", "colour_setup_helper.py")
        spec = importlib.util.spec_from_file_location("colour_setup_helper", path)
        module = importlib.util.module_from_spec(spec)
        writes = sys.dont_write_bytecode
        sys.dont_write_bytecode = True  # no __pycache__ left in app/resolve
        try:
            spec.loader.exec_module(module)
        finally:
            sys.dont_write_bytecode = writes
        return module

    def test_the_helper_finds_the_scripting_module_and_library_where_the_harness_does(self):
        helper = self.helper()
        for platform in ("linux", "win32"):
            for environ in ({}, {"PROGRAMDATA": "D:\\Data", "PROGRAMFILES": "D:\\Apps"},
                            {"RESOLVE_SCRIPT_API": "/x/api", "RESOLVE_SCRIPT_LIB": "/x/lib"}):
                api, lib = helper.scripting_paths(platform, environ)
                harness = resolve_api.scripting_paths(platform, environ)
                join = ntpath.join if platform == "win32" else posixpath.join
                self.assertEqual((join(api, "Modules"), lib), (harness.modules, harness.lib), (platform, environ))


if __name__ == "__main__":
    unittest.main()
