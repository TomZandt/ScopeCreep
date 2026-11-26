"""The Windows Installer's packaging seam, run on Linux with a stand-in for the Inno Setup compiler (#251).

Inno Setup only runs on Windows, so `scripts/package-windows.sh` is run here with SCOPECREEP_ISCC pointing at a
fake compiler that writes, as the "setup .exe", the list of files it was given. The tests check that list, the name
(the one the app's Update chooser looks for) and the Inno Setup script's own instructions.
"""
import hashlib
import os
import pathlib
import re
import subprocess
import tempfile
import unittest

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PACKAGE = os.path.join(ROOT, "scripts", "package-windows.sh")
PLATFORMS = os.path.join(ROOT, "scripts", "platforms.sh")
ISS = os.path.join(ROOT, "packaging", "windows", "scopecreep.iss")
INSTALL_TXT = os.path.join(ROOT, "packaging", "windows", "INSTALL.txt")
UPDATES_RS = os.path.join(ROOT, "app", "src", "updates.rs")

# Stands in for ISCC.exe: run in the staging folder with the script's name, it reads the output name from the
# generated version.iss and writes Output/<name>.exe holding every staged file, one per line.
FAKE_ISCC = """#!/usr/bin/env bash
set -euo pipefail
echo "$@" > "$FAKE_ISCC_LOG"
[[ -f $1 ]] || { echo "fake iscc: no script $1" >&2; exit 1; }
name=$(sed -n 's/^#define OutputName "\\(.*\\)"$/\\1/p' version.iss)
version=$(sed -n 's/^#define AppVersion "\\(.*\\)"$/\\1/p' version.iss)
[[ -n $name && -n $version ]] || { echo "fake iscc: version.iss lacks a define" >&2; exit 1; }
mkdir -p Output
{ echo "version=$version"; find . -path ./Output -prune -o -type f -print | sed 's|^\\./||' | sort; } > "Output/$name.exe"
"""


def run(cmd, env=None):
    e = dict(os.environ)
    e.update(env or {})
    return subprocess.run(cmd, capture_output=True, text=True, env=e)


class PackageWindowsTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = self._tmp.name
        self.addCleanup(self._tmp.cleanup)
        self.app = os.path.join(self.tmp, "scopecreep.exe")
        with open(self.app, "w") as f:
            f.write("fake app")
        self.bundle = os.path.join(self.tmp, "scopecreep_sender.ofx.bundle")
        plugin_dir = os.path.join(self.bundle, "Contents", "Win64")
        os.makedirs(plugin_dir)
        with open(os.path.join(plugin_dir, "scopecreep_sender.ofx"), "w") as f:
            f.write("fake plugin")
        self.iscc = os.path.join(self.tmp, "iscc")
        with open(self.iscc, "w") as f:
            f.write(FAKE_ISCC)
        os.chmod(self.iscc, 0o755)
        self.log = os.path.join(self.tmp, "iscc.log")
        self.out = os.path.join(self.tmp, "out")

    def package(self, version="1.2.3", app=None, bundle=None):
        return run([PACKAGE, version, app or self.app, bundle or self.bundle, self.out],
                   env={"SCOPECREEP_ISCC": self.iscc, "FAKE_ISCC_LOG": self.log})

    def setup_exe(self, version="1.2.3"):
        return os.path.join(self.out, f"scopecreep-{version}-windows-x86_64-setup.exe")

    def test_the_installer_holds_the_app_the_sender_bundle_and_install_txt(self):
        r = self.package()
        self.assertEqual(r.returncode, 0, r.stderr)
        listing = pathlib.Path(self.setup_exe()).read_text().splitlines()
        self.assertEqual(listing[0], "version=1.2.3")
        for name in ("scopecreep.exe", "INSTALL.txt", "scopecreep.iss",
                     "scopecreep_sender.ofx.bundle/Contents/Win64/scopecreep_sender.ofx"):
            self.assertIn(name, listing)
        self.assertEqual(pathlib.Path(self.out, "INSTALL.txt").exists(), False)

    def test_the_name_is_the_one_the_update_chooser_looks_for(self):
        self.package("0.9.1")
        pattern = re.search(r'Platform::Windows, "([^"]+)"', pathlib.Path(UPDATES_RS).read_text()).group(1)
        self.assertEqual(pattern, "scopecreep-{v}-windows-x86_64-setup.exe")
        self.assertEqual(os.listdir(self.out), sorted([pattern.replace("{v}", "0.9.1"), "SHA256SUMS"]))
        self.assertEqual(self.package("0.9.1").stdout.strip(), self.setup_exe("0.9.1"))

    def test_sha256sums_matches_the_installer(self):
        self.package()
        digest = hashlib.sha256(pathlib.Path(self.setup_exe()).read_bytes()).hexdigest()
        lines = pathlib.Path(self.out, "SHA256SUMS").read_text().splitlines()
        self.assertEqual(lines, [f"{digest}  scopecreep-1.2.3-windows-x86_64-setup.exe"])

    def test_sha256sums_covers_every_installer_in_the_folder(self):
        os.makedirs(self.out)
        pathlib.Path(self.out, "scopecreep-1.2.3-linux-x86_64.tar.gz").write_text("linux")
        self.package()
        names = [line.split()[1] for line in pathlib.Path(self.out, "SHA256SUMS").read_text().splitlines()]
        self.assertEqual(names, ["scopecreep-1.2.3-linux-x86_64.tar.gz", "scopecreep-1.2.3-windows-x86_64-setup.exe"])

    def test_fails_when_the_app_is_missing(self):
        r = self.package(app=os.path.join(self.tmp, "nope.exe"))
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(os.path.exists(self.log))

    def test_fails_when_the_sender_bundle_is_missing_or_empty(self):
        self.assertNotEqual(self.package(bundle=os.path.join(self.tmp, "nope")).returncode, 0)
        empty = os.path.join(self.tmp, "empty.ofx.bundle")
        os.makedirs(empty)
        self.assertNotEqual(self.package(bundle=empty).returncode, 0)
        self.assertFalse(os.path.exists(self.log))

    def test_fails_on_a_bad_version(self):
        self.assertNotEqual(self.package("latest").returncode, 0)

    def test_fails_when_the_compiler_makes_no_installer(self):
        with open(self.iscc, "w") as f:
            f.write("#!/bin/sh\nexit 0\n")
        r = self.package()
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("no Installer", r.stderr)


class InnoSetupScriptTest(unittest.TestCase):
    """What the Inno Setup script tells the compiler. ISCC itself runs only in the Windows CI job."""

    def setUp(self):
        self.iss = pathlib.Path(ISS).read_text()

    def section(self, name):
        m = re.search(rf"^\[{name}\]\n(.*?)(?=^\[|\Z)", self.iss, re.S | re.M)
        self.assertIsNotNone(m, f"no [{name}] section")
        return m.group(1)

    def setting(self, key):
        m = re.search(rf"^{key}=(.*)$", self.section("Setup"), re.M)
        self.assertIsNotNone(m, f"no {key} in [Setup]")
        return m.group(1).strip()

    def test_installs_per_machine_with_admin_rights_on_x64(self):
        self.assertEqual(self.setting("PrivilegesRequired"), "admin")
        self.assertEqual(self.setting("ArchitecturesInstallIn64BitMode"), "x64os")
        self.assertEqual(self.setting("ArchitecturesAllowed"), "x64os")

    def test_app_goes_to_program_files_and_the_sender_to_the_common_files_ofx_folder(self):
        self.assertEqual(self.setting("DefaultDirName"), r"{autopf}\ScopeCreep")
        files = self.section("Files")
        self.assertRegex(files, r'Source: "scopecreep\.exe"; DestDir: "\{app\}"')
        self.assertRegex(files, r'Source: "INSTALL\.txt"; DestDir: "\{app\}"')
        self.assertRegex(
            files,
            r'Source: "scopecreep_sender\.ofx\.bundle\\\*"; '
            r'DestDir: "\{commoncf64\}\\OFX\\Plugins\\scopecreep_sender\.ofx\.bundle";.*recursesubdirs',
        )

    def test_a_reinstall_replaces_the_whole_sender_bundle_and_uninstall_removes_it(self):
        bundle = r"{commoncf64}\OFX\Plugins\scopecreep_sender.ofx.bundle"
        for section in ("InstallDelete", "UninstallDelete"):
            self.assertIn(f'Type: filesandordirs; Name: "{bundle}"', self.section(section))
        self.assertNotIn(r'Name: "{commoncf64}\OFX\Plugins"', self.iss)

    def test_start_menu_entry_and_uninstaller(self):
        self.assertRegex(self.section("Icons"), r'Name: "\{autoprograms\}\\ScopeCreep"; Filename: "\{app\}\\scopecreep\.exe"')
        self.assertEqual(self.setting("Uninstallable"), "yes")
        self.assertEqual(self.setting("UninstallDisplayIcon"), r"{app}\scopecreep.exe")

    def test_asks_to_close_resolve_before_installing_and_uninstalling(self):
        code = self.section("Code")
        self.assertIn("'Resolve.exe'", code)
        for hook in ("function InitializeSetup(): Boolean;", "function InitializeUninstall(): Boolean;"):
            self.assertIn(hook, code)
        self.assertIn("Close DaVinci Resolve", code)

    def test_the_wizard_says_cuda_is_needed_and_amd_and_intel_are_not_supported(self):
        self.assertEqual(self.setting("DisableWelcomePage"), "no")
        welcome = re.search(r"^WelcomeLabel2=(.*)$", self.section("Messages"), re.M).group(1)
        for words in ("NVIDIA", "CUDA", "AMD and Intel", "not supported"):
            self.assertIn(words, welcome)
        self.assertEqual(self.setting("InfoBeforeFile"), "INSTALL.txt")

    def test_name_and_version_come_from_the_packaging_script(self):
        self.assertIn('#include "version.iss"', self.iss)
        self.assertEqual(self.setting("OutputBaseFilename"), "{#OutputName}")
        self.assertEqual(self.setting("AppVersion"), "{#AppVersion}")


class InstallTxtTest(unittest.TestCase):
    def test_explains_smartscreen_and_that_nvidia_and_cuda_are_needed(self):
        text = pathlib.Path(INSTALL_TXT).read_text()
        for words in ("Windows protected your PC", "More info", "Run anyway", "NVIDIA", "CUDA",
                      "AMD and Intel", "Add or Remove Programs", "Restart"):
            self.assertIn(words, text)


class TestBuildPlatformsTest(unittest.TestCase):
    def read(self, text, *flags):
        with tempfile.NamedTemporaryFile("w", suffix=".txt") as f:
            f.write(text)
            f.flush()
            return run([PLATFORMS, *flags, f.name])

    def test_releases_take_only_the_platforms_marked_yes(self):
        r = self.read("# c\nlinux    yes\nwindows  no\nmacos\n")
        self.assertEqual(r.stdout.split(), ["linux", "macos"])

    def test_test_builds_take_every_platform(self):
        r = self.read("linux yes\nwindows no\n", "--test-builds", "--json")
        self.assertEqual(r.stdout.strip(), '["linux","windows"]')

    def test_a_last_line_without_a_newline_counts(self):
        self.assertEqual(self.read("linux\nwindows no", "--test-builds").stdout.split(), ["linux", "windows"])

    def test_a_value_other_than_yes_or_no_fails(self):
        self.assertNotEqual(self.read("windows maybe\n").returncode, 0)

    def test_windows_is_in_test_builds_but_not_in_releases_until_its_bench_passes(self):
        # #252 sets windows to yes in packaging/platforms.txt once uhd50_bench passes there, and changes this test.
        self.assertNotIn("windows", run([PLATFORMS]).stdout.split())
        self.assertIn("windows", run([PLATFORMS, "--test-builds"]).stdout.split())


if __name__ == "__main__":
    unittest.main()
