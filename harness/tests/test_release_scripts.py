"""The Linux Installer, its install script and the release notes, run on fake inputs (#241)."""
import hashlib
import os
import pathlib
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest

if sys.platform == "win32":
    raise unittest.SkipTest("the Linux Installer's scripts are bash, run on Linux")

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PACKAGE = os.path.join(ROOT, "scripts", "package-linux.sh")
NOTES = os.path.join(ROOT, "scripts", "release-notes.sh")
CHECK_TAG = os.path.join(ROOT, "scripts", "check-release-tag.sh")
VERSION = os.path.join(ROOT, "scripts", "workspace-version.sh")
PLATFORMS = os.path.join(ROOT, "scripts", "platforms.sh")

CHANGELOG = """# Changelog

## [Unreleased]

### Added
- Not yet released.

## [1.2.0] - 2026-01-02

### Added
- A thing.

### Fixed
- Another thing.

## [1.1.0] - 2026-01-01

- Older.

[Unreleased]: https://example.invalid/compare/v1.2.0...HEAD
[1.2.0]: https://example.invalid/compare/v1.1.0...v1.2.0
"""


def run(cmd, env=None, cwd=None):
    e = dict(os.environ)
    e.update(env or {})
    return subprocess.run(cmd, capture_output=True, text=True, env=e, cwd=cwd)


class Fakes(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = self._tmp.name
        self.addCleanup(self._tmp.cleanup)
        self.app = os.path.join(self.tmp, "scopecreep")
        with open(self.app, "w") as f:
            f.write("#!/bin/sh\necho fake app\n")
        os.chmod(self.app, 0o755)
        self.bundle = os.path.join(self.tmp, "scopecreep_sender.ofx.bundle")
        plugin_dir = os.path.join(self.bundle, "Contents", "Linux-x86-64")
        os.makedirs(plugin_dir)
        with open(os.path.join(plugin_dir, "scopecreep_sender.ofx"), "w") as f:
            f.write("fake plugin")
        self.out = os.path.join(self.tmp, "out")

    def package(self, version="1.2.3", app=None, bundle=None):
        return run([PACKAGE, version, app or self.app, bundle or self.bundle, self.out])


@unittest.skipUnless(sys.platform.startswith("linux"), "tests the Linux packaging and install scripts (GNU tar, Linux paths)")
class PackagingTest(Fakes):
    def test_makes_the_installer_with_its_files(self):
        r = self.package()
        self.assertEqual(r.returncode, 0, r.stderr)
        path = os.path.join(self.out, "scopecreep-1.2.3-linux-x86_64.tar.gz")
        self.assertTrue(os.path.isfile(path))
        with tarfile.open(path) as t:
            names = set(t.getnames())
            top = "scopecreep-1.2.3-linux-x86_64"
            for n in ("scopecreep", "install.sh", "INSTALL.txt",
                      "scopecreep_sender.ofx.bundle/Contents/Linux-x86-64/scopecreep_sender.ofx"):
                self.assertIn(f"{top}/{n}", names)
            for n in ("scopecreep", "install.sh"):
                self.assertTrue(t.getmember(f"{top}/{n}").mode & stat.S_IXUSR)

    def test_sha256sums_matches_the_installer(self):
        self.package()
        path = os.path.join(self.out, "scopecreep-1.2.3-linux-x86_64.tar.gz")
        digest = hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
        lines = pathlib.Path(self.out, "SHA256SUMS").read_text().splitlines()
        self.assertEqual(lines, [f"{digest}  scopecreep-1.2.3-linux-x86_64.tar.gz"])

    def test_sha256sums_covers_every_installer_in_the_folder(self):
        self.package("1.2.3")
        r = self.package("1.2.4")
        self.assertEqual(r.returncode, 0, r.stderr)
        lines = pathlib.Path(self.out, "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(lines), 2)

    def test_fails_when_the_app_is_missing(self):
        r = self.package(app=os.path.join(self.tmp, "nope"))
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(os.path.exists(os.path.join(self.out, "SHA256SUMS")))

    def test_fails_when_the_sender_bundle_is_missing_or_empty(self):
        self.assertNotEqual(self.package(bundle=os.path.join(self.tmp, "nope")).returncode, 0)
        empty = os.path.join(self.tmp, "empty.ofx.bundle")
        os.makedirs(empty)
        self.assertNotEqual(self.package(bundle=empty).returncode, 0)
        self.assertEqual(os.listdir(self.out) if os.path.isdir(self.out) else [], [])

    def test_fails_on_a_bad_version(self):
        self.assertNotEqual(self.package("latest").returncode, 0)

    def test_install_txt_in_the_installer_says_cuda_install_and_uninstall(self):
        self.package()
        with tarfile.open(os.path.join(self.out, "scopecreep-1.2.3-linux-x86_64.tar.gz")) as t:
            text = t.extractfile("scopecreep-1.2.3-linux-x86_64/INSTALL.txt").read().decode()
        self.assertIn("CUDA", text)
        self.assertIn("./install.sh", text)
        self.assertIn("./install.sh uninstall", text)
        self.assertIn("Restart", text)


@unittest.skipUnless(sys.platform.startswith("linux"), "tests the Linux packaging and install scripts (GNU tar, Linux paths)")
class InstallScriptTest(Fakes):
    def setUp(self):
        super().setUp()
        self.package()
        self.unpacked = os.path.join(self.tmp, "unpacked")
        with tarfile.open(os.path.join(self.out, "scopecreep-1.2.3-linux-x86_64.tar.gz")) as t:
            t.extractall(self.unpacked)
        self.dir = os.path.join(self.unpacked, "scopecreep-1.2.3-linux-x86_64")
        self.bin = os.path.join(self.tmp, "prefix", "bin")
        self.ofx = os.path.join(self.tmp, "ofx", "Plugins")
        # A fake sudo that records it was asked, then runs the command.
        self.sudo = os.path.join(self.tmp, "fakesudo")
        self.sudo_log = os.path.join(self.tmp, "sudo.log")
        with open(self.sudo, "w") as f:
            f.write(f'#!/bin/sh\necho "$@" >> "{self.sudo_log}"\nexec "$@"\n')
        os.chmod(self.sudo, 0o755)
        self.env = {"SCOPECREEP_BIN_DIR": self.bin, "SCOPECREEP_OFX_DIR": self.ofx, "SCOPECREEP_SUDO": self.sudo}

    def script(self, *args, env=None):
        return run([os.path.join(self.dir, "install.sh"), *args], env={**self.env, **(env or {})})

    def plugin(self):
        return os.path.join(self.ofx, "scopecreep_sender.ofx.bundle", "Contents", "Linux-x86-64", "scopecreep_sender.ofx")

    def test_install_puts_the_app_and_sender_in_place_and_says_restart(self):
        r = self.script()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertTrue(os.access(os.path.join(self.bin, "scopecreep"), os.X_OK))
        self.assertTrue(os.path.isfile(self.plugin()))
        self.assertIn("Restart DaVinci Resolve", r.stdout)

    def test_install_twice_replaces_the_sender(self):
        self.script()
        stale = os.path.join(self.ofx, "scopecreep_sender.ofx.bundle", "stale")
        open(stale, "w").close()
        self.assertEqual(self.script().returncode, 0)
        self.assertFalse(os.path.exists(stale))

    def test_uninstall_removes_both_and_says_restart(self):
        self.script()
        r = self.script("uninstall")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertFalse(os.path.exists(os.path.join(self.bin, "scopecreep")))
        self.assertFalse(os.path.exists(os.path.join(self.ofx, "scopecreep_sender.ofx.bundle")))
        self.assertIn("Restart DaVinci Resolve", r.stdout)

    def test_uninstall_with_nothing_installed_is_fine(self):
        self.assertEqual(self.script("uninstall").returncode, 0)

    def test_needing_admin_rights_is_said_before_sudo_is_used(self):
        if os.geteuid() == 0:
            self.skipTest("root never needs sudo")
        os.makedirs(self.ofx)
        os.chmod(self.ofx, 0o555)
        self.addCleanup(os.chmod, self.ofx, 0o755)
        # The fake sudo cannot write into a folder this user cannot, so only the order is checked.
        r = self.script()
        self.assertIn("Admin rights are needed", r.stdout)
        self.assertTrue(os.path.isfile(self.sudo_log))

    def test_no_admin_message_when_the_folders_are_yours(self):
        r = self.script()
        self.assertNotIn("Admin rights", r.stdout)
        self.assertFalse(os.path.exists(self.sudo_log))

    def test_unknown_command_fails(self):
        self.assertNotEqual(self.script("frobnicate").returncode, 0)


class ReleaseNotesTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.changelog = os.path.join(self._tmp.name, "CHANGELOG.md")
        with open(self.changelog, "w") as f:
            f.write(CHANGELOG)
        self.url = "https://example.invalid/repo"

    def notes(self, version):
        return run([NOTES, version, self.changelog], env={"SCOPECREEP_REPO_URL": self.url})

    def test_prints_exactly_the_section_then_the_link(self):
        r = self.notes("1.2.0")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(
            r.stdout,
            "### Added\n- A thing.\n\n### Fixed\n- Another thing.\n\n"
            "Full changelog: https://example.invalid/repo/blob/v1.2.0/CHANGELOG.md\n",
        )

    def test_the_last_section_stops_before_the_link_list(self):
        self.assertNotIn("example.invalid/compare", self.notes("1.1.0").stdout)
        self.assertIn("- Older.", self.notes("1.1.0").stdout)

    def test_accepts_a_leading_v(self):
        self.assertEqual(self.notes("v1.2.0").stdout, self.notes("1.2.0").stdout)

    def test_fails_for_a_missing_version(self):
        r = self.notes("9.9.9")
        self.assertNotEqual(r.returncode, 0)
        self.assertEqual(r.stdout, "")

    def test_a_version_is_not_matched_as_a_prefix(self):
        self.assertNotEqual(self.notes("1.2").returncode, 0)

    def test_works_on_the_real_changelog(self):
        r = run([NOTES, "0.3.1"])
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("Full changelog:", r.stdout)


class CheckReleaseTagTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.cargo = os.path.join(self._tmp.name, "Cargo.toml")
        with open(self.cargo, "w") as f:
            f.write('[workspace.package]\nversion = "1.2.0"\n')
        self.changelog = os.path.join(self._tmp.name, "CHANGELOG.md")
        with open(self.changelog, "w") as f:
            f.write(CHANGELOG)

    def check(self, tag):
        return run([CHECK_TAG, tag, self.cargo, self.changelog])

    def test_matching_tag_with_a_section_passes(self):
        r = self.check("v1.2.0")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.strip(), "1.2.0")

    def test_tag_not_matching_the_workspace_version_fails(self):
        r = self.check("v1.1.0")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("workspace version", r.stderr)

    def test_version_without_a_changelog_section_fails(self):
        with open(self.cargo, "w") as f:
            f.write('version = "1.3.0"\n')
        r = self.check("v1.3.0")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("no section", r.stderr)

    def test_a_tag_that_is_not_a_version_fails(self):
        r = self.check("release-1")
        self.assertNotEqual(r.returncode, 0)


class WorkspaceVersionTest(unittest.TestCase):
    def test_reads_the_first_version(self):
        with tempfile.NamedTemporaryFile("w", suffix=".toml") as f:
            f.write('[workspace.package]\nversion = "1.2.0"\n[x]\nversion = "9"\n')
            f.flush()
            r = run([VERSION, f.name])
        self.assertEqual(r.stdout.strip(), "1.2.0")

    def test_fails_without_a_version(self):
        with tempfile.NamedTemporaryFile("w", suffix=".toml") as f:
            f.write("[x]\n")
            f.flush()
            r = run([VERSION, f.name])
        self.assertNotEqual(r.returncode, 0)

    def test_matches_the_real_cargo_toml(self):
        r = run([VERSION])
        self.assertRegex(r.stdout.strip(), r"^\d+\.\d+\.\d+")


class PlatformsTest(unittest.TestCase):
    def read(self, text, *flags):
        with tempfile.NamedTemporaryFile("w", suffix=".txt") as f:
            f.write(text)
            f.flush()
            return run([PLATFORMS, *flags, f.name])

    def test_lists_names_without_comments_and_blanks(self):
        r = self.read("# c\nlinux\n\n  # d\nwindows\n")
        self.assertEqual(r.stdout.split(), ["linux", "windows"])

    def test_json_is_an_array(self):
        r = self.read("# c\nlinux\nwindows\n", "--json")
        self.assertEqual(r.stdout.strip(), '["linux","windows"]')

    def test_an_empty_list_fails(self):
        self.assertNotEqual(self.read("# nothing\n").returncode, 0)

    def test_real_list_has_linux(self):
        self.assertIn("linux", run([PLATFORMS]).stdout.split())


class ReleaseWorkflowTest(unittest.TestCase):
    def test_the_changelog_link_ends_the_notes(self):
        r = run([NOTES, "0.3.1"])
        self.assertTrue(r.stdout.rstrip().splitlines()[-1].startswith("Full changelog:"))
        wf = open(os.path.join(ROOT, ".github", "workflows", "release.yml")).read()
        self.assertLess(wf.index("Platforms included"), wf.index("scripts/release-notes.sh"))


if __name__ == "__main__":
    unittest.main()
