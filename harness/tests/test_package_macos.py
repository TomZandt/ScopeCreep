"""The macOS Installer's packaging seam (#257).

`scripts/package-macos.sh` runs here on Linux with stand-ins for `pkgbuild` and `productbuild` (they exist only on a
Mac): pkgbuild's stand-in writes the files of the root it was given, and its options, into the "component package";
productbuild's writes the distribution, the resources and the component it was given into the "pkg". Those tests,
and the checks of the text the .pkg carries (INSTALL.txt, the distribution's Apple Silicon check, the workflow and
platforms.txt wiring), run anywhere. The real .pkg is inspected with `pkgutil` only on a Mac.
"""
import hashlib
import os
import pathlib
import plistlib
import re
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

if sys.platform == "win32":
    raise unittest.SkipTest("the packaging script is bash, run directly on Linux and macOS")

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PACKAGE = os.path.join(ROOT, "scripts", "package-macos.sh")
PACKAGE_LINUX = os.path.join(ROOT, "scripts", "package-linux.sh")
PLATFORMS = os.path.join(ROOT, "scripts", "platforms.sh")
SRC = os.path.join(ROOT, "packaging", "macos")
WORKFLOW = os.path.join(ROOT, ".github", "workflows", "release.yml")
UPDATES_RS = os.path.join(ROOT, "app", "src", "updates.rs")
ON_MAC = sys.platform == "darwin"
APP_DIR = "Applications/ScopeCreep.app"
SENDER_DIR = "Library/OFX/Plugins/scopecreep_sender.ofx.bundle"
INSTALL_TXT_PATH = "Library/Application Support/ScopeCreep/INSTALL.txt"

# pkgbuild's stand-in keeps a copy of what it wrote at $FAKE_KEEP, since productbuild's output hides it.
FAKE_PKGBUILD = """#!/usr/bin/env bash
set -euo pipefail
root=
for ((i = 1; i <= $#; i++)); do
  if [[ ${!i} == --root ]]; then j=$((i + 1)); root=${!j}; fi
done
out=${!#}
{ echo "ARGS $*"; (cd "$root" && find . -mindepth 1 | sed 's|^\\./||' | sort); } > "$out"
cp "$out" "$FAKE_KEEP"
"""
FAKE_PRODUCTBUILD = """#!/usr/bin/env bash
set -euo pipefail
dist= path= res=
for ((i = 1; i <= $#; i++)); do
  if [[ ${!i} == --distribution ]]; then j=$((i + 1)); dist=${!j}; fi
  if [[ ${!i} == --package-path ]]; then j=$((i + 1)); path=${!j}; fi
  if [[ ${!i} == --resources ]]; then j=$((i + 1)); res=${!j}; fi
done
out=${!#}
{ echo "ARGS $*"; echo "DIST"; cat "$dist"; echo "RESOURCES"; (cd "$res" && ls); echo "COMPONENTS"; (cd "$path" && ls); } > "$out"
"""


def run(cmd, env=None):
    e = dict(os.environ)
    e.update(env or {})
    return subprocess.run(cmd, capture_output=True, text=True, env=e)


class Fixture(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = self._tmp.name
        self.addCleanup(self._tmp.cleanup)
        self.app = os.path.join(self.tmp, "scopecreep")
        with open(self.app, "w") as f:
            f.write("fake app")
        os.chmod(self.app, 0o755)
        self.bundle = os.path.join(self.tmp, "scopecreep_sender.ofx.bundle")
        plugin_dir = os.path.join(self.bundle, "Contents", "MacOS")
        os.makedirs(plugin_dir)
        with open(os.path.join(plugin_dir, "scopecreep_sender.ofx"), "w") as f:
            f.write("fake plugin")
        self.out = os.path.join(self.tmp, "out")

    def pkg(self, version="1.2.3"):
        return os.path.join(self.out, f"scopecreep-{version}-macos-arm64.pkg")


class PackageMacosFakeToolsTest(Fixture):
    def setUp(self):
        super().setUp()
        self.bin = os.path.join(self.tmp, "bin")
        os.makedirs(self.bin)
        for name, body in (("pkgbuild", FAKE_PKGBUILD), ("productbuild", FAKE_PRODUCTBUILD)):
            p = os.path.join(self.bin, name)
            pathlib.Path(p).write_text(body)
            os.chmod(p, 0o755)
        self.keep = os.path.join(self.tmp, "kept.txt")

    def package(self, version="1.2.3", app=None, bundle=None):
        return run([PACKAGE, version, app or self.app, bundle or self.bundle, self.out],
                   env={"PATH": self.bin + os.pathsep + os.environ["PATH"], "FAKE_KEEP": self.keep})

    def staged(self):
        return pathlib.Path(self.keep).read_text().splitlines()

    def test_the_payload_holds_the_app_the_sender_bundle_and_install_txt(self):
        r = self.package()
        self.assertEqual(r.returncode, 0, r.stderr)
        listing = self.staged()
        for path in (f"{APP_DIR}/Contents/MacOS/scopecreep", f"{APP_DIR}/Contents/Info.plist",
                     f"{SENDER_DIR}/Contents/MacOS/scopecreep_sender.ofx", INSTALL_TXT_PATH):
            self.assertIn(path, listing)

    def test_the_package_is_named_as_the_update_chooser_expects_and_is_the_only_output_besides_sha256sums(self):
        pattern = re.search(r'Platform::MacOs, "([^"]+)"', pathlib.Path(UPDATES_RS).read_text()).group(1)
        self.assertEqual(pattern, "scopecreep-{v}-macos-arm64.pkg")
        r = self.package("0.9.1")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(sorted(os.listdir(self.out)), sorted([pattern.replace("{v}", "0.9.1"), "SHA256SUMS"]))
        self.assertEqual(r.stdout.strip(), self.pkg("0.9.1"))

    def test_the_component_installs_from_the_root_without_relocating_its_bundles(self):
        self.package()
        args = self.staged()[0]
        self.assertIn("--version 1.2.3", args)
        self.assertIn("--install-location /", args)
        self.assertIn("--component-plist", args)
        self.assertIn("--identifier", args)

    def test_the_distribution_is_given_the_component_a_readme_and_the_version(self):
        self.package()
        text = pathlib.Path(self.pkg()).read_text()
        self.assertIn("--distribution", text)
        self.assertIn("scopecreep-component.pkg", text.split("COMPONENTS")[1])
        self.assertIn("INSTALL.txt", text.split("RESOURCES")[1].split("COMPONENTS")[0])
        self.assertIn('version="1.2.3"', text)
        self.assertNotIn("@VERSION@", text)

    def test_sha256sums_matches_the_package(self):
        self.package()
        digest = hashlib.sha256(pathlib.Path(self.pkg()).read_bytes()).hexdigest()
        lines = pathlib.Path(self.out, "SHA256SUMS").read_text().splitlines()
        self.assertEqual(lines, [f"{digest}  scopecreep-1.2.3-macos-arm64.pkg"])

    def test_sha256sums_covers_every_installer_in_the_folder(self):
        os.makedirs(self.out)
        pathlib.Path(self.out, "scopecreep-1.2.3-linux-x86_64.tar.gz").write_text("linux")
        self.package()
        names = [line.split()[1] for line in pathlib.Path(self.out, "SHA256SUMS").read_text().splitlines()]
        self.assertEqual(names, ["scopecreep-1.2.3-linux-x86_64.tar.gz", "scopecreep-1.2.3-macos-arm64.pkg"])

    def test_the_linux_script_keeps_a_pkg_in_sha256sums_too(self):
        os.makedirs(self.out)
        pathlib.Path(self.out, "scopecreep-1.2.3-macos-arm64.pkg").write_text("mac")
        r = run([PACKAGE_LINUX, "1.2.3", self.app, self.bundle, self.out])
        self.assertEqual(r.returncode, 0, r.stderr)
        names = [line.split()[1] for line in pathlib.Path(self.out, "SHA256SUMS").read_text().splitlines()]
        self.assertIn("scopecreep-1.2.3-macos-arm64.pkg", names)

    def test_fails_when_the_app_is_missing(self):
        self.assertNotEqual(self.package(app=os.path.join(self.tmp, "nope")).returncode, 0)

    def test_fails_when_the_sender_bundle_is_missing_or_empty(self):
        self.assertNotEqual(self.package(bundle=os.path.join(self.tmp, "nope")).returncode, 0)
        empty = os.path.join(self.tmp, "empty.ofx.bundle")
        os.makedirs(empty)
        self.assertNotEqual(self.package(bundle=empty).returncode, 0)

    def test_fails_on_a_bad_version_and_with_the_wrong_arguments(self):
        self.assertNotEqual(self.package("latest").returncode, 0)
        self.assertEqual(run([PACKAGE]).returncode, 2)

    def test_fails_when_the_packaging_tools_make_no_package(self):
        for name in ("pkgbuild", "productbuild"):
            pathlib.Path(self.bin, name).write_text("#!/bin/sh\nexit 0\n")
        r = self.package()
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(os.path.exists(self.pkg()))


class InstallTxtTest(unittest.TestCase):
    def setUp(self):
        self.text = pathlib.Path(SRC, "INSTALL.txt").read_text()

    def test_explains_opening_an_unsigned_pkg(self):
        for words in ("right-click", "Open", "Privacy & Security", "Open Anyway"):
            self.assertIn(words, self.text)

    def test_says_to_close_resolve_and_that_it_is_apple_silicon_only(self):
        for words in ("Close DaVinci Resolve", "Apple Silicon", "Intel"):
            self.assertIn(words, self.text)

    def test_explains_how_to_uninstall_both_places(self):
        self.assertIn("/Applications/ScopeCreep.app", self.text)
        self.assertIn("/Library/OFX/Plugins/scopecreep_sender.ofx.bundle", self.text)


class DistributionTest(unittest.TestCase):
    def setUp(self):
        self.root = ET.parse(os.path.join(SRC, "distribution.xml")).getroot()

    def test_requires_apple_silicon_and_refuses_intel_with_a_message(self):
        self.assertEqual(self.root.find("options").get("hostArchitectures"), "arm64")
        self.assertEqual(self.root.find("installation-check").get("script"), "installCheck();")
        script = self.root.find("script").text
        self.assertIn("hw.optional.arm64", script)
        self.assertIn("Apple Silicon", script)
        self.assertIn("Intel", script)
        self.assertIn("Fatal", script)

    def test_shows_a_welcome_with_the_close_resolve_note_and_the_readme(self):
        self.assertEqual(self.root.find("readme").get("file"), "INSTALL.txt")
        self.assertEqual(self.root.find("welcome").get("file"), "welcome.txt")
        self.assertIn("Close DaVinci Resolve", pathlib.Path(SRC, "welcome.txt").read_text())

    def test_installs_the_one_component_package_with_the_version_from_the_script(self):
        refs = [e.text for e in self.root.iter("pkg-ref") if e.text]
        self.assertEqual(refs, ["scopecreep-component.pkg"])
        self.assertIn('version="@VERSION@"', pathlib.Path(SRC, "distribution.xml").read_text())

    def test_the_component_plist_does_not_relocate_the_bundles(self):
        with open(os.path.join(SRC, "components.plist"), "rb") as f:
            comps = plistlib.load(f)
        self.assertEqual({c["RootRelativeBundlePath"] for c in comps}, {APP_DIR, SENDER_DIR})
        for c in comps:
            self.assertIs(c["BundleIsRelocatable"], False)


class WiringTest(unittest.TestCase):
    def setUp(self):
        self.wf = pathlib.Path(WORKFLOW).read_text()

    def test_macos_is_in_test_builds_but_not_in_releases_until_its_bench_passes(self):
        # #258 sets macos to yes in packaging/platforms.txt once uhd50_bench passes on the Mac, and changes this test.
        self.assertNotIn("macos", run([PLATFORMS]).stdout.split())
        self.assertIn("macos", run([PLATFORMS, "--test-builds"]).stdout.split())

    def test_the_workflow_has_an_apple_silicon_runner_and_accepts_the_platform(self):
        self.assertIn("macos-14", self.wf)
        self.assertIn("matrix.platform == 'macos'", self.wf)
        self.assertRegex(self.wf, r"\$p == linux \|\| \$p == windows \|\| \$p == macos")

    def test_the_macos_job_builds_the_sender_with_the_pinned_headers_and_the_standin_then_the_pkg(self):
        self.assertIn("make -C sender OFX_SDK=", self.wf)
        self.assertIn("scripts/package-macos.sh", self.wf)
        self.assertIn("target/release/scopecreep", self.wf)
        self.assertIn("sender/build/scopecreep_sender.ofx.bundle", self.wf)


@unittest.skipUnless(ON_MAC, "pkgbuild, productbuild and pkgutil are macOS tools")
class RealPkgOnMacTest(Fixture):
    def test_the_pkg_payload_holds_the_app_the_sender_bundle_and_install_txt(self):
        r = run([PACKAGE, "1.2.3", self.app, self.bundle, self.out])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertTrue(os.path.isfile(self.pkg()))
        expanded = os.path.join(self.tmp, "expanded")
        r = run(["pkgutil", "--expand-full", self.pkg(), expanded])
        self.assertEqual(r.returncode, 0, r.stderr)
        files = []
        for dirpath, _dirs, names in os.walk(expanded):
            files += [os.path.relpath(os.path.join(dirpath, n), expanded) for n in names]
        joined = "\n".join(files)
        for tail in (f"{APP_DIR}/Contents/MacOS/scopecreep", f"{APP_DIR}/Contents/Info.plist",
                     f"{SENDER_DIR}/Contents/MacOS/scopecreep_sender.ofx", INSTALL_TXT_PATH):
            self.assertIn(tail, joined)
        self.assertIn('hostArchitectures="arm64"', pathlib.Path(expanded, "Distribution").read_text())


if __name__ == "__main__":
    unittest.main()
