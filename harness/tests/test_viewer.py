import os
import sys
import unittest
from unittest import mock

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import viewer


class SendKeyRaisesWhenTheKeyIsNotSent(unittest.TestCase):
    def test_hyprland_rejecting_both_dispatchers_raises(self):
        with mock.patch.object(viewer.sys, "platform", "linux"), \
                mock.patch.object(viewer, "_hyprctl", return_value="error"):
            with self.assertRaises(RuntimeError):
                viewer.send_key("space", window={"address": "0x1"})

    def test_windows_sends_letters_and_digits_as_their_virtual_keys(self):
        self.assertEqual(viewer._windows_virtual_key("a"), 0x41)
        self.assertEqual(viewer._windows_virtual_key("L"), 0x4C)
        self.assertEqual(viewer._windows_virtual_key("7"), 0x37)

    def test_windows_refuses_a_key_it_cannot_send_before_touching_resolve(self):
        for key in ("", "space", "F5", "-"):
            with self.assertRaises(ValueError):
                viewer._windows_virtual_key(key)

    def test_windows_refuses_modifiers(self):
        with mock.patch.object(viewer.sys, "platform", "win32"):
            with self.assertRaises(NotImplementedError):
                viewer.send_key("l", mods="CTRL")

    def test_macos_types_the_key_into_resolve_through_system_events(self):
        script = viewer._macos_key_script("L")
        self.assertIn('tell application "DaVinci Resolve" to activate', script)
        self.assertTrue(script.endswith('keystroke "l"'), script)

    def test_macos_refuses_a_key_it_cannot_send_and_modifiers(self):
        for key in ("", "space", "-"):
            with self.assertRaises(ValueError):
                viewer._macos_key_script(key)
        with mock.patch.object(viewer.sys, "platform", "darwin"):
            with self.assertRaises(NotImplementedError):
                viewer.send_key("l", mods="CTRL")

    def test_macos_says_so_when_osascript_is_refused(self):
        failed = mock.Mock(returncode=1, stderr="not allowed assistive access")
        with mock.patch.object(viewer.sys, "platform", "darwin"), \
                mock.patch.object(viewer.subprocess, "run", return_value=failed):
            with self.assertRaisesRegex(RuntimeError, "Accessibility"):
                viewer.send_key("k")

    def test_macos_screenshots_a_region_or_every_screen(self):
        self.assertEqual(viewer.screencapture_args("/t/a.png", "1395,255 120x30"),
                         ["screencapture", "-x", "-R", "1395,255,120,30", "/t/a.png"])
        self.assertEqual(viewer.screencapture_args("/t/a.png"), ["screencapture", "-x", "/t/a.png"])


if __name__ == "__main__":
    unittest.main()
