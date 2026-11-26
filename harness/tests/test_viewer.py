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


if __name__ == "__main__":
    unittest.main()
