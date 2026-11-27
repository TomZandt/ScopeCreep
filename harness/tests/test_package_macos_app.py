"""ScopeCreep.app's packaging seam (#254), run on Linux: `scripts/package-macos-app.sh` makes the bundle from a
stand-in executable (it only signs the bundle on a Mac). The checks are the bundle's layout, the version in its
Info.plist, and that the plist asks for Retina and names the executable that is really there.
"""
import os
import plistlib
import subprocess
import sys
import tempfile
import unittest

if sys.platform == "win32":
    raise unittest.SkipTest("the packaging script is bash, run directly on Linux and macOS")

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PACKAGE = os.path.join(ROOT, "scripts", "package-macos-app.sh")


class PackageMacosAppTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = self._tmp.name
        self.addCleanup(self._tmp.cleanup)
        self.app = os.path.join(self.tmp, "scopecreep")
        with open(self.app, "w") as f:
            f.write("fake app")
        os.chmod(self.app, 0o755)
        self.out = os.path.join(self.tmp, "out")

    def package(self, version="1.2.3", app=None):
        return subprocess.run([PACKAGE, version, app or self.app, self.out], capture_output=True, text=True)

    def test_the_bundle_holds_the_executable_the_plist_and_the_icon(self):
        r = self.package()
        self.assertEqual(r.returncode, 0, r.stderr)
        bundle = os.path.join(self.out, "ScopeCreep.app")
        self.assertEqual(r.stdout.strip(), bundle)
        with open(os.path.join(bundle, "Contents", "Info.plist"), "rb") as f:
            plist = plistlib.load(f)
        exe = os.path.join(bundle, "Contents", "MacOS", plist["CFBundleExecutable"])
        self.assertTrue(os.access(exe, os.X_OK), "the executable the plist names is there and runs")
        icon = os.path.join(bundle, "Contents", "Resources", plist["CFBundleIconFile"] + ".icns")
        with open(icon, "rb") as f:
            self.assertEqual(f.read(4), b"icns")

    def test_the_plist_carries_the_version_and_asks_for_retina(self):
        self.package("0.5.0")
        with open(os.path.join(self.out, "ScopeCreep.app", "Contents", "Info.plist"), "rb") as f:
            plist = plistlib.load(f)
        self.assertEqual(plist["CFBundleShortVersionString"], "0.5.0")
        self.assertEqual(plist["CFBundleVersion"], "0.5.0")
        self.assertIs(plist["NSHighResolutionCapable"], True)
        self.assertEqual(plist["CFBundlePackageType"], "APPL")

    def test_a_missing_app_or_a_bad_version_fails(self):
        self.assertNotEqual(self.package(app=os.path.join(self.tmp, "nowhere")).returncode, 0)
        self.assertNotEqual(self.package("not-a-version").returncode, 0)
        self.assertFalse(os.path.exists(os.path.join(self.out, "ScopeCreep.app")))


if __name__ == "__main__":
    unittest.main()
